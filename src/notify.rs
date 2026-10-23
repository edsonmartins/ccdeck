//! Notificações de desktop (SPEC O3). Melhor esforço: falhas são ignoradas.

use std::process::{Command, Stdio};

pub fn desktop(title: &str, body: &str) {
    let mut cmd = if cfg!(target_os = "macos") {
        let script = format!(
            "display notification {} with title {}",
            applescript_str(body),
            applescript_str(title)
        );
        let mut c = Command::new("osascript");
        c.args(["-e", &script]);
        c
    } else {
        let mut c = Command::new("notify-send");
        c.args(["--app-name=ccdeck", title, body]);
        c
    };
    let _ = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn applescript_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn bell() {
    use std::io::Write;
    let mut out = std::io::stdout();
    let _ = out.write_all(b"\x07");
    let _ = out.flush();
}
