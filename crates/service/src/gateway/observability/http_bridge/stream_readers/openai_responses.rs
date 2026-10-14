use super::{
    client_stream_read_error_message, mark_first_response_ms, stream_idle_timed_out,
    stream_idle_timeout_message, stream_reader_disconnected_message, stream_wait_timeout,
    upstream_hint_or_stream_incomplete_message, Arc, Cursor, Mutex, OpenAIResponsesEvent,
    OpenAIResponsesOutputTextState, PassthroughSseCollector, Read, SseKeepAliveFrame, SseTerminal,
};
use crate::gateway::upstream::{GatewayByteStream, GatewayByteStreamItem, GatewayStreamResponse};
use eventsource_stream::{Event, Eventsource};
use futures_util::pin_mut;
use futures_util::stream::unfold;
use futures_util::task::noop_waker_ref;
use futures_util::Stream;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::task::{Context, Poll};
use std::thread;
use std::time::{Duration, Instant};

const OPENAI_RESPONSES_SSE_CHANNEL_CAPACITY: usize = 128;
const OPENAI_RESPONSES_SIDECAR_POLL_INTERVAL: Duration = Duration::from_millis(100);

// Track dispatch boundaries without buffering or rewriting upstream payloads.
// SSE permits LF, CRLF and bare CR; a network chunk is not a frame boundary.
#[derive(Default)]
struct SseFrameBoundary {
    line_has_bytes: bool,
    previous_cr: bool,
    pending_frame: bool,
}

impl SseFrameBoundary {
    fn observe(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' && self.previous_cr {
                self.previous_cr = false;
                continue;
            }
            self.previous_cr = byte == b'\r';
            if matches!(byte, b'\r' | b'\n') {
                self.pending_frame = self.line_has_bytes;
                self.line_has_bytes = false;
            } else {
                self.line_has_bytes = true;
                self.pending_frame = true;
            }
        }
    }

    fn at_boundary(&self) -> bool {
        !self.pending_frame
    }
}

#[derive(Debug)]
enum OpenAIResponsesSidecarItem {
    Event(OpenAIResponsesEvent, usize),
    Progress(usize),
    Eof,
    Error(String),
}

struct OpenAIResponsesSidecarObserver {
    rx: Receiver<OpenAIResponsesSidecarItem>,
    shutdown: Arc<AtomicBool>,
}

impl OpenAIResponsesSidecarObserver {
    fn new(byte_stream: GatewayByteStream) -> Self {
        let (tx, rx) =
            mpsc::sync_channel::<OpenAIResponsesSidecarItem>(OPENAI_RESPONSES_SSE_CHANNEL_CAPACITY);
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
        thread::spawn(move || {
            let read_bytes = Arc::new(AtomicUsize::new(0));
            let input_read_bytes = Arc::clone(&read_bytes);
            let progress_tx = tx.clone();
            let byte_stream = unfold(Some((byte_stream, worker_shutdown)), move |state| {
                let read_bytes = Arc::clone(&input_read_bytes);
                let progress_tx = progress_tx.clone();
                async move {
                    let (mut byte_stream, shutdown) = state?;
                    // Asking for more input means all events in the preceding
                    // input have been dispatched: an ordered parser barrier.
                    if progress_tx
                        .send(OpenAIResponsesSidecarItem::Progress(
                            read_bytes.load(Ordering::Acquire),
                        ))
                        .is_err()
                    {
                        return None;
                    }
                    loop {
                        if shutdown.load(Ordering::Acquire) {
                            return None;
                        }
                        match byte_stream.recv_timeout(OPENAI_RESPONSES_SIDECAR_POLL_INTERVAL) {
                            Ok(GatewayByteStreamItem::Chunk(bytes)) => {
                                read_bytes.fetch_add(bytes.len(), Ordering::Release);
                                return Some((Ok(bytes), Some((byte_stream, shutdown))));
                            }
                            Ok(GatewayByteStreamItem::Eof) => return None,
                            Ok(GatewayByteStreamItem::Error(err)) => return Some((Err(err), None)),
                            Err(RecvTimeoutError::Timeout) => continue,
                            Err(RecvTimeoutError::Disconnected) => return None,
                        }
                    }
                }
            });

            let stream = byte_stream.eventsource();
            pin_mut!(stream);
            let waker = noop_waker_ref();
            let mut cx = Context::from_waker(waker);

            loop {
                match stream.as_mut().poll_next(&mut cx) {
                    Poll::Ready(Some(Ok(event))) => {
                        let lines = event_to_sse_lines(&event);
                        if let Some(parsed) = OpenAIResponsesEvent::parse(&lines) {
                            if tx
                                .send(OpenAIResponsesSidecarItem::Event(
                                    parsed,
                                    read_bytes.load(Ordering::Acquire),
                                ))
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                    Poll::Ready(Some(Err(err))) => {
                        let _ = tx.send(OpenAIResponsesSidecarItem::Error(err.to_string()));
                        return;
                    }
                    Poll::Ready(None) => {
                        let _ = tx.send(OpenAIResponsesSidecarItem::Eof);
                        return;
                    }
                    Poll::Pending => thread::yield_now(),
                }
            }
        });
        Self { rx, shutdown }
    }

    fn try_recv(&self) -> Result<OpenAIResponsesSidecarItem, mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}

impl Drop for OpenAIResponsesSidecarObserver {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
    }
}

fn event_to_sse_lines(event: &Event) -> Vec<String> {
    let mut lines = Vec::new();
    if !event.id.is_empty() {
        lines.push(format!("id: {}\n", event.id));
    }
    if let Some(retry) = event.retry {
        lines.push(format!("retry: {}\n", retry.as_millis()));
    }
    if !event.event.is_empty() && !event.event.eq_ignore_ascii_case("message") {
        lines.push(format!("event: {}\n", event.event));
    }
    for data_line in event.data.split('\n') {
        lines.push(format!("data: {data_line}\n"));
    }
    lines.push("\n".to_string());
    lines
}

pub(crate) struct OpenAIResponsesPassthroughSseReader {
    raw_upstream: GatewayByteStream,
    observer: OpenAIResponsesSidecarObserver,
    observer_finished: bool,
    forwarded_bytes: usize,
    observed_bytes: usize,
    terminal_offset: Option<usize>,
    frame_boundary: SseFrameBoundary,
    out_cursor: Cursor<Vec<u8>>,
    usage_collector: Arc<Mutex<PassthroughSseCollector>>,
    usage_text_state: OpenAIResponsesOutputTextState,
    keepalive_frame: SseKeepAliveFrame,
    request_started_at: Instant,
    last_upstream_activity: Instant,
    finished: bool,
}

impl OpenAIResponsesPassthroughSseReader {
    pub(crate) fn new(
        upstream: reqwest::blocking::Response,
        usage_collector: Arc<Mutex<PassthroughSseCollector>>,
        keepalive_frame: SseKeepAliveFrame,
        request_started_at: Instant,
    ) -> Self {
        Self::from_stream_response(
            GatewayStreamResponse::from_blocking_response(upstream),
            usage_collector,
            keepalive_frame,
            request_started_at,
        )
    }

    pub(crate) fn from_stream_response(
        upstream: GatewayStreamResponse,
        usage_collector: Arc<Mutex<PassthroughSseCollector>>,
        keepalive_frame: SseKeepAliveFrame,
        request_started_at: Instant,
    ) -> Self {
        let (raw_upstream, sidecar_upstream) = upstream.into_body().tee();
        Self {
            raw_upstream,
            observer: OpenAIResponsesSidecarObserver::new(sidecar_upstream),
            observer_finished: false,
            forwarded_bytes: 0,
            observed_bytes: 0,
            terminal_offset: None,
            frame_boundary: SseFrameBoundary::default(),
            out_cursor: Cursor::new(Vec::new()),
            usage_collector,
            usage_text_state: OpenAIResponsesOutputTextState::default(),
            keepalive_frame,
            request_started_at,
            last_upstream_activity: Instant::now(),
            finished: false,
        }
    }

    fn update_usage_from_event(&mut self, event: OpenAIResponsesEvent, offset: usize) {
        // The first dispatched response terminal is final, just as in the
        // Responses -> Chat adapter. Ignore trailing events/keepalive data.
        if self.terminal_offset.is_some() {
            return;
        }
        if event.terminal.is_some() {
            self.terminal_offset = Some(offset);
        }
        if let Ok(mut collector) = self.usage_collector.lock() {
            if let Some(event_type) = event.event_type.as_ref() {
                collector.last_event_type = Some(event_type.clone());
            }
            event.merge_usage_into(&mut collector.usage, &mut self.usage_text_state);
            if let Some(upstream_error_hint) = event.upstream_error_hint.as_ref() {
                collector.upstream_error_hint = Some(upstream_error_hint.clone());
            }
            if let Some(terminal) = event.terminal.as_ref() {
                collector.saw_terminal = true;
                if let SseTerminal::Err(message) = terminal {
                    collector.terminal_error = Some(message.clone());
                }
            }
        }
    }

    fn drain_sidecar_events(&mut self) {
        loop {
            match self.observer.try_recv() {
                Ok(OpenAIResponsesSidecarItem::Event(event, offset)) => {
                    self.update_usage_from_event(event, offset);
                }
                Ok(OpenAIResponsesSidecarItem::Progress(offset)) => self.observed_bytes = offset,
                Ok(OpenAIResponsesSidecarItem::Eof) => {
                    self.observer_finished = true;
                    return;
                }
                Ok(OpenAIResponsesSidecarItem::Error(err)) => {
                    self.record_observer_error(&err);
                    self.observer_finished = true;
                    return;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.observer_finished = true;
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => return,
            }
        }
    }

    fn finish_sidecar(&mut self) {
        // Only called after the raw stream ends. tee() forwards the same end to
        // the observer, so consume its ordered events through that end instead
        // of treating a >50ms parsing/scheduling delay as a truncated response.
        while !self.observer_finished {
            match self.observer.rx.recv() {
                Ok(OpenAIResponsesSidecarItem::Event(event, offset)) => {
                    self.update_usage_from_event(event, offset);
                }
                Ok(OpenAIResponsesSidecarItem::Progress(offset)) => self.observed_bytes = offset,
                Ok(OpenAIResponsesSidecarItem::Eof) => self.observer_finished = true,
                Ok(OpenAIResponsesSidecarItem::Error(err)) => {
                    self.record_observer_error(&err);
                    self.observer_finished = true;
                }
                Err(_) => self.observer_finished = true,
            }
        }
    }

    fn record_observer_error(&self, error: &str) {
        if let Ok(mut collector) = self.usage_collector.lock() {
            if !collector.saw_terminal {
                collector
                    .terminal_error
                    .get_or_insert_with(|| client_stream_read_error_message(error));
            }
        }
    }

    fn catch_up_sidecar(&mut self) {
        // At an idle timeout, wait only until the parser has consumed bytes
        // already returned by this reader, never for more network input.
        while !self.observer_finished && self.observed_bytes < self.forwarded_bytes {
            match self
                .observer
                .rx
                .recv_timeout(OPENAI_RESPONSES_SIDECAR_POLL_INTERVAL)
            {
                Ok(OpenAIResponsesSidecarItem::Event(event, offset)) => {
                    self.update_usage_from_event(event, offset);
                }
                Ok(OpenAIResponsesSidecarItem::Progress(offset)) => self.observed_bytes = offset,
                Ok(OpenAIResponsesSidecarItem::Eof) => self.observer_finished = true,
                Ok(OpenAIResponsesSidecarItem::Error(err)) => {
                    self.record_observer_error(&err);
                    self.observer_finished = true;
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => self.observer_finished = true,
            }
        }
    }

    fn terminal_was_forwarded(&self) -> bool {
        // Eventsource dispatches only complete frames. This offset is an input
        // chunk upper bound, so it can delay recognition but never recognizes
        // a terminal before the raw bytes containing it have been forwarded.
        self.terminal_offset
            .is_some_and(|offset| offset <= self.forwarded_bytes)
    }

    fn stream_error_chunk(&mut self, code: &str, message: String) -> std::io::Result<Vec<u8>> {
        self.finished = true;
        // A dispatched terminal is sticky even if a later comment is cut off.
        // A truncated terminal itself was never dispatched by Eventsource.
        if self.terminal_was_forwarded() {
            return Ok(Vec::new());
        }
        if let Ok(mut collector) = self.usage_collector.lock() {
            collector
                .terminal_error
                .get_or_insert_with(|| message.clone());
        }
        if !self.frame_boundary.at_boundary() {
            // These bytes have already been forwarded. Adding a separator
            // could dispatch an unfinished success event or malformed JSON.
            // Fail the transport instead of manufacturing a completed frame.
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, message));
        }
        let error = serde_json::json!({
            "type": "error", "code": code, "message": message, "param": null
        });
        Ok(format!("event: error\ndata: {error}\n\n").into_bytes())
    }

    fn next_chunk(&mut self) -> std::io::Result<Vec<u8>> {
        self.drain_sidecar_events();
        if self.terminal_was_forwarded() {
            self.finished = true;
            return Ok(Vec::new());
        }
        match self
            .raw_upstream
            .recv_timeout(stream_wait_timeout(self.last_upstream_activity))
        {
            Ok(GatewayByteStreamItem::Chunk(bytes)) => {
                self.forwarded_bytes = self.forwarded_bytes.saturating_add(bytes.len());
                self.frame_boundary.observe(bytes.as_ref());
                self.last_upstream_activity = Instant::now();
                mark_first_response_ms(&self.usage_collector, self.request_started_at);
                self.drain_sidecar_events();
                Ok(bytes.to_vec())
            }
            Ok(GatewayByteStreamItem::Eof) => {
                self.finish_sidecar();
                self.stream_error_chunk(
                    "upstream_stream_incomplete",
                    upstream_hint_or_stream_incomplete_message(None),
                )
            }
            Ok(GatewayByteStreamItem::Error(err)) => {
                self.last_upstream_activity = Instant::now();
                self.finish_sidecar();
                self.stream_error_chunk(
                    "upstream_stream_read_error",
                    client_stream_read_error_message(&err),
                )
            }
            Err(RecvTimeoutError::Timeout) => {
                self.drain_sidecar_events();
                if self.terminal_was_forwarded() {
                    self.finished = true;
                    return Ok(Vec::new());
                }
                if stream_idle_timed_out(self.last_upstream_activity) {
                    self.catch_up_sidecar();
                    return self.stream_error_chunk(
                        "upstream_stream_idle_timeout",
                        stream_idle_timeout_message(),
                    );
                }
                if crate::gateway::current_sse_keepalive_enabled()
                    && self.frame_boundary.at_boundary()
                {
                    Ok(self.keepalive_frame.bytes().to_vec())
                } else {
                    Ok(Vec::new())
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.finish_sidecar();
                self.stream_error_chunk(
                    "upstream_stream_disconnected",
                    stream_reader_disconnected_message(),
                )
            }
        }
    }
}

impl Read for OpenAIResponsesPassthroughSseReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            let read = self.out_cursor.read(buf)?;
            if read > 0 {
                return Ok(read);
            }
            if self.finished {
                return Ok(0);
            }
            self.out_cursor = Cursor::new(self.next_chunk()?);
        }
    }
}
