#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a.to_string_lossy().starts_with("--lumen-"))
    {
        let exe = std::env::current_exe().expect("Executable location");
        let daemon = exe.parent().unwrap().join(if cfg!(windows) {
            "lumen-daemon.exe"
        } else {
            "lumen-daemon"
        });
        let mut c = std::process::Command::new(daemon);
        c.args(std::env::args_os().skip(1));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000);
        }
        let code = c.status().ok().and_then(|s| s.code()).unwrap_or(1);
        std::process::exit(code);
    }
    lumen_lib::run();
}
