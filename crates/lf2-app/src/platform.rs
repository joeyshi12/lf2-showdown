//! What the original asks Windows for: open-file dialogs, message boxes, web pages,
//! and the local time.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Ask for a .lfr file (GetOpenFileNameA in the original), starting in `dir`.
/// Uses the desktop's own dialog program; without one it takes the newest .lfr file
/// in `dir`.
pub fn pick_recording(dir: &Path) -> Option<PathBuf> {
    let d = dir.to_string_lossy().into_owned();
    let tries: Vec<(&str, Vec<String>)> = if cfg!(target_os = "windows") {
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; \
             $f.InitialDirectory = '{}'; $f.Filter = 'LF2 recording files (*.lfr)|*.lfr'; \
             if ($f.ShowDialog() -eq 'OK') {{ $f.FileName }}",
            d.replace('\'', "''")
        );
        vec![("powershell", vec!["-NoProfile".into(), "-Command".into(), script])]
    } else if cfg!(target_os = "macos") {
        let script = format!(
            "POSIX path of (choose file of type {{\"lfr\"}} default location POSIX file \"{}\")",
            d.replace('"', "\\\"")
        );
        vec![("osascript", vec!["-e".into(), script])]
    } else {
        vec![
            ("zenity", vec!["--file-selection".into(), format!("--filename={}/", d), "--file-filter=*.lfr".into()]),
            ("kdialog", vec!["--getopenfilename".into(), d.clone(), "*.lfr".into()]),
        ]
    };
    for (prog, args) in tries {
        if let Ok(out) = Command::new(prog).args(&args).output() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return if out.status.success() && !s.is_empty() { Some(PathBuf::from(s)) } else { None };
        }
    }
    let newest = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("lfr")))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())?;
    eprintln!("no file dialog program found; playing the newest recording");
    Some(newest.path())
}

/// MessageBoxA: shown with the desktop's dialog program when there is one, and
/// always printed.
pub fn message(text: &str) {
    eprintln!("{}", text);
    let _ = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!(
                "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.MessageBox]::Show('{}')",
                text.replace('\'', "''")
            )])
            .status()
    } else if cfg!(target_os = "macos") {
        Command::new("osascript").args(["-e", &format!("display dialog \"{}\" buttons {{\"OK\"}}", text.replace('"', "\\\""))]).status()
    } else {
        Command::new("zenity").args(["--info", "--text", text]).status()
    };
}

/// ShellExecuteA "open": a web page or a folder.
pub fn open(target: &str) {
    let prog = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if Command::new(prog).arg(target).spawn().is_err() {
        eprintln!("link: {}", target);
    }
}

/// The time for GetLocalTime. This is UTC: time zones are not handled.
pub fn local_time() -> [u16; 6] {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    [y as u16, m as u16, d as u16, (rem / 3600) as u16, (rem / 60 % 60) as u16, (rem % 60) as u16]
}
