use std::path::Path;
use std::process::Command;

use crate::models;
use crate::paths;
use crate::permissions;
use crate::term;

const URL_SCREEN_RECORDING: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
const URL_MICROPHONE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";

pub enum Level { Ok, Warn, Fail }

pub struct Check {
    pub name:        &'static str,
    pub level:       Level,
    pub detail:      String,
    pub remediation: Option<String>,
}

pub fn run(model: &str, diarizer: &str) -> i32 {
    let checks = vec![
        check_macos_version(),
        check_apple_silicon(),
        check_screen_recording(),
        check_microphone(),
        check_nabu_home(),
        check_uv(),
        check_whisper_model(model),
        check_diarizer(diarizer),
        check_disk_space(),
    ];

    println!("nabu doctor");
    println!();
    let mut has_fail = false;
    for c in &checks {
        let tag = match c.level {
            Level::Ok   => "[OK]",
            Level::Warn => "[WARN]",
            Level::Fail => { has_fail = true; "[FAIL]" }
        };
        println!("  {:<6} {:<28} {}", tag, c.name, c.detail);
        if let Some(r) = &c.remediation {
            for line in r.lines() {
                // Each remediation message is responsible for its own
                // bullets / numbering. We just indent under the FAIL line.
                println!("         {line}");
            }
        }
    }
    println!();
    if has_fail {
        println!("One or more checks failed. Fix the items above before running nabu.");
        1
    } else {
        println!("All checks passed.");
        0
    }
}

fn check_macos_version() -> Check {
    match Command::new("sw_vers").arg("-productVersion").output() {
        Ok(out) if out.status.success() => {
            let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let major: u32 = ver.split('.').next().and_then(|s| s.parse().ok()).unwrap_or(0);
            if major >= 13 {
                Check { name: "macOS version", level: Level::Ok, detail: format!("{ver} (≥ 13.0 required)"), remediation: None }
            } else {
                Check {
                    name: "macOS version", level: Level::Fail,
                    detail: format!("{ver} — nabu needs macOS 13+ for ScreenCaptureKit"),
                    remediation: Some("Upgrade macOS to 13.0 (Ventura) or later.".into()),
                }
            }
        }
        _ => Check {
            name: "macOS version", level: Level::Warn,
            detail: "could not run sw_vers".into(),
            remediation: None,
        },
    }
}

fn check_apple_silicon() -> Check {
    let arch = Command::new("uname").arg("-m").output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if arch == "arm64" {
        Check { name: "Apple Silicon", level: Level::Ok, detail: "arm64".into(), remediation: None }
    } else {
        Check {
            name: "Apple Silicon", level: Level::Fail,
            detail: format!("architecture is {arch} — nabu requires arm64"),
            remediation: Some("Run on an M1/M2/M3/M4 Mac.".into()),
        }
    }
}

fn check_screen_recording() -> Check {
    if permissions::has_screen_recording() {
        Check { name: "screen recording perm", level: Level::Ok, detail: "granted".into(), remediation: None }
    } else {
        Check {
            name: "screen recording perm", level: Level::Fail,
            detail: "not granted — system audio cannot be captured".into(),
            remediation: Some(remediation_screen_recording()),
        }
    }
}

fn remediation_screen_recording() -> String {
    let app = term::detect();
    let (target, quit_line, header_note) = match &app {
        Some(t) => (
            format!("{} (drag it from {})", t.name, t.path),
            format!("Quit {} completely (⌘ + Q) and reopen it", t.name),
            format!("How to fix (detected terminal: {})", t.name),
        ),
        None => (
            "your terminal app (Terminal lives at /System/Applications/Utilities/Terminal.app, \
             most others at /Applications/<name>.app)".to_string(),
            "Quit your terminal completely (⌘ + Q) and reopen it".to_string(),
            "How to fix".to_string(),
        ),
    };
    format!(
        "{header}:\n  \
         1. Open the Screen Recording pane. Paste this in Terminal:\n     \
                open \"{url}\"\n     \
                (or: System Settings → Privacy & Security → Screen Recording)\n  \
         2. Click + and add {target}\n  \
         3. Toggle it ON in the list\n  \
         4. {quit_line}\n     \
                macOS caches the permission decision until the terminal process restarts.\n  \
         5. Run `nabu --doctor` again to confirm this check turns green",
        header = header_note,
        url = URL_SCREEN_RECORDING,
        target = target,
        quit_line = quit_line,
    )
}

fn check_microphone() -> Check {
    // cpal will return an error opening the default input device if mic
    // access has been denied, so a probe is the simplest reliable check.
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        return Check {
            name: "microphone", level: Level::Fail,
            detail: "no default input device".into(),
            remediation: Some("Connect a microphone, then re-run `nabu --doctor`.".into()),
        };
    };
    match device.default_input_config() {
        Ok(_) => Check { name: "microphone", level: Level::Ok, detail: "default input device available".into(), remediation: None },
        Err(e) => Check {
            name: "microphone", level: Level::Fail,
            detail: format!("cannot open input ({e})"),
            remediation: Some(remediation_microphone()),
        },
    }
}

fn remediation_microphone() -> String {
    let app = term::detect();
    let (target, header_note) = match &app {
        Some(t) => (t.name.to_string(), format!("How to fix (detected terminal: {})", t.name)),
        None    => ("your terminal app".to_string(), "How to fix".to_string()),
    };
    format!(
        "{header}:\n  \
         1. Open the Microphone pane. Paste this in Terminal:\n     \
                open \"{url}\"\n     \
                (or: System Settings → Privacy & Security → Microphone)\n  \
         2. Toggle {target} ON in the list (if it is missing, run nabu once so macOS adds it)\n  \
         3. Run `nabu --doctor` again to confirm this check turns green",
        header = header_note,
        url = URL_MICROPHONE,
        target = target,
    )
}

fn check_nabu_home() -> Check {
    let p = match paths::resolve() { Ok(p) => p, Err(e) => {
        return Check { name: "~/.nabu", level: Level::Fail, detail: e.to_string(), remediation: None };
    }};
    if !p.nabu_home.exists() {
        return Check {
            name: "~/.nabu", level: Level::Fail,
            detail: "missing — first-run extraction has not happened".into(),
            remediation: Some("Run: nabu --setup".into()),
        };
    }
    if !p.transcribe.join(".venv").exists() {
        return Check {
            name: "~/.nabu/transcribe", level: Level::Fail,
            detail: "no Python venv — setup did not complete".into(),
            remediation: Some("Run: nabu --setup".into()),
        };
    }
    Check { name: "~/.nabu", level: Level::Ok, detail: "present with venv".into(), remediation: None }
}

fn check_uv() -> Check {
    let p = paths::resolve().ok();
    let uv = p.as_ref().map(|p| p.uv.clone());
    let Some(uv) = uv else {
        return Check { name: "uv runtime", level: Level::Warn, detail: "could not resolve path".into(), remediation: None };
    };
    if !uv.exists() {
        return Check {
            name: "uv runtime", level: Level::Fail,
            detail: "~/.nabu/bin/uv missing".into(),
            remediation: Some("Run: nabu --setup".into()),
        };
    }
    match Command::new(&uv).arg("--version").output() {
        Ok(o) if o.status.success() => {
            let ver = String::from_utf8_lossy(&o.stdout).trim().to_string();
            Check { name: "uv runtime", level: Level::Ok, detail: ver, remediation: None }
        }
        _ => Check {
            name: "uv runtime", level: Level::Fail,
            detail: "uv extracted but failed to run".into(),
            remediation: Some("Remove ~/.nabu and re-run: nabu --setup".into()),
        },
    }
}

fn check_whisper_model(model: &str) -> Check {
    if models::whisper_cached(model) {
        Check { name: "whisper model", level: Level::Ok, detail: format!("{model} cached"), remediation: None }
    } else {
        Check {
            name: "whisper model", level: Level::Warn,
            detail: format!("{model} not in cache — first run will download it"),
            remediation: Some(format!("Pre-download with: nabu --setup --model {model}")),
        }
    }
}

fn check_diarizer(diarizer: &str) -> Check {
    match diarizer {
        "wespeaker" => {
            if models::wespeaker_cached() {
                Check { name: "diarizer (wespeaker)", level: Level::Ok, detail: "model present".into(), remediation: None }
            } else {
                Check {
                    name: "diarizer (wespeaker)", level: Level::Warn,
                    detail: "model not cached — first run will download it".into(),
                    remediation: Some("Pre-download with: nabu --setup".into()),
                }
            }
        }
        "pyannote" => {
            if models::pyannote_cached() {
                Check { name: "diarizer (pyannote)", level: Level::Ok, detail: "models present".into(), remediation: None }
            } else {
                Check {
                    name: "diarizer (pyannote)", level: Level::Fail,
                    detail: "models not cached".into(),
                    remediation: Some("Accept terms on HuggingFace, then: nabu --setup --hf-token <TOKEN>".into()),
                }
            }
        }
        other => Check {
            name: "diarizer", level: Level::Fail,
            detail: format!("unknown diarizer '{other}' — expected 'wespeaker' or 'pyannote'"),
            remediation: None,
        },
    }
}

fn check_disk_space() -> Check {
    let home = match dirs::home_dir() { Some(h) => h, None => return Check { name: "disk space", level: Level::Warn, detail: "no home dir".into(), remediation: None } };
    let free_mb = free_megabytes(&home).unwrap_or(0);
    if free_mb == 0 {
        Check { name: "disk space", level: Level::Warn, detail: "could not determine".into(), remediation: None }
    } else if free_mb < 5_000 {
        Check {
            name: "disk space", level: Level::Warn,
            detail: format!("{} MB free on home volume — recordings may fill the disk", free_mb),
            remediation: Some("Free up space, or use --out to write to another volume.".into()),
        }
    } else {
        Check { name: "disk space", level: Level::Ok, detail: format!("{} MB free", free_mb), remediation: None }
    }
}

fn free_megabytes(path: &Path) -> Option<u64> {
    let out = Command::new("df").arg("-k").arg(path).output().ok()?;
    if !out.status.success() { return None; }
    let text = String::from_utf8_lossy(&out.stdout);
    let last = text.lines().last()?;
    let cols: Vec<&str> = last.split_whitespace().collect();
    // BSD df: Filesystem 1024-blocks Used Available Capacity ...
    let avail_kb: u64 = cols.get(3)?.parse().ok()?;
    Some(avail_kb / 1024)
}
