use super::Storage;

/// Private authentication state. Never expose this value through settings RPCs.
#[derive(Clone)]
pub struct WebTwoFactorState {
    pub version: i64,
    pub state_json: String,
}

impl Storage {
    pub fn web_two_factor_state(&self) -> rusqlite::Result<WebTwoFactorState> {
        self.conn.query_row(
            "SELECT version, state_json FROM web_two_factor_state WHERE id = 1",
            [],
            |row| {
                Ok(WebTwoFactorState {
                    version: row.get(0)?,
                    state_json: row.get(1)?,
                })
            },
        )
    }

    /// A single compare-and-swap consumes a code/challenge even across workers.
    pub fn compare_exchange_web_two_factor_state(
        &self,
        expected_version: i64,
        state_json: &str,
    ) -> rusqlite::Result<bool> {
        self.conn.execute(
            "UPDATE web_two_factor_state SET version = version + 1, state_json = ?1 WHERE id = 1 AND version = ?2",
            rusqlite::params![state_json, expected_version],
        ).map(|changed| changed == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_security_update_cannot_reuse_a_consumed_code() {
        let storage = Storage::open_in_memory().unwrap();
        storage.init().unwrap();
        let original = storage.web_two_factor_state().unwrap();
        assert_eq!(original.state_json, "{}");
        assert!(storage
            .compare_exchange_web_two_factor_state(original.version, "{\"step\":1}")
            .unwrap());
        assert!(!storage
            .compare_exchange_web_two_factor_state(original.version, "{\"step\":0}")
            .unwrap());
        assert_eq!(
            storage.web_two_factor_state().unwrap().state_json,
            "{\"step\":1}"
        );
    }
}
