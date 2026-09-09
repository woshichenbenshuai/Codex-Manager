/// 函数 `main`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 无
#[cfg(windows)]
fn main() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon_path = manifest_dir.join("../../apps/src-tauri/icons/icon.ico");

    println!("cargo:rerun-if-changed={}", icon_path.display());

    if !icon_path.is_file() {
        panic!("Windows icon not found: {}", icon_path.display());
    }

    let mut res = winres::WindowsResource::new();
    println!("cargo:rerun-if-env-changed=WindowsSdkVerBinPath");
    if let Ok(toolkit_path) = std::env::var("WindowsSdkVerBinPath") {
        let toolkit_path = toolkit_path.trim_end_matches(['\\', '/']);
        if !toolkit_path.is_empty() {
            res.set_toolkit_path(toolkit_path);
        }
    }
    res.set_icon(icon_path.to_string_lossy().as_ref());
    res.compile()
        .expect("failed to compile Windows resources (icon)");
}

/// 函数 `main`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 无
#[cfg(not(windows))]
fn main() {}
