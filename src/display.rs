use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal;

const WARN_BEFORE: Duration = Duration::from_secs(10 * 60);

/// Format a `u64` second count as `HH:MM:SS`.
pub fn format_hms(secs: u64) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}

/// Parse a human duration like `4h`, `90m`, `1h30m`, `2h15m30s` into a `Duration`.
/// Returns an error on empty input, a missing unit, or an unknown unit.
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() { return Err("empty duration".into()); }

    let mut total: u64 = 0;
    let mut num: u64 = 0;
    let mut have_num = false;
    let mut have_any = false;

    for c in s.chars() {
        if c.is_ascii_digit() {
            num = num.checked_mul(10).and_then(|n| n.checked_add((c as u64) - ('0' as u64)))
                .ok_or_else(|| "duration overflow".to_string())?;
            have_num = true;
            continue;
        }
        if !have_num { return Err(format!("expected number before '{c}'")); }
        let unit_secs = match c {
            'h' | 'H' => 3600,
            'm' | 'M' => 60,
            's' | 'S' => 1,
            other => return Err(format!("unknown duration unit '{other}' (use h/m/s)")),
        };
        total = total.checked_add(num.checked_mul(unit_secs).ok_or_else(|| "duration overflow".to_string())?)
            .ok_or_else(|| "duration overflow".to_string())?;
        num = 0; have_num = false; have_any = true;
    }
    if have_num {
        return Err("trailing number with no unit (e.g. '90' should be '90m' or '90s')".into());
    }
    if !have_any { return Err("no duration components parsed".into()); }
    Ok(Duration::from_secs(total))
}

pub struct Handle {
    pub stop:   Arc<AtomicBool>,
    pub thread: JoinHandle<()>,
}

/// Spawn a thread that prints a live recording timer and watches for a
/// max-duration deadline. Within `WARN_BEFORE` of the deadline, the user can
/// press `e` to extend by `extend_by`; pressing Ctrl-C in raw mode also stops.
pub fn spawn(start: Instant, max_duration: Duration, extend_by: Duration) -> Handle {
    let stop     = Arc::new(AtomicBool::new(false));
    let deadline = Arc::new(Mutex::new(start + max_duration));
    let stop_bg     = stop.clone();
    let deadline_bg = deadline.clone();

    let thread = thread::spawn(move || {
        let interactive = io::stdout().is_terminal();
        let _raw = if interactive { RawModeGuard::enter().ok() } else { None };

        let dots = ['●', '◉'];
        let mut i = 0usize;
        let mut warned = false;

        loop {
            if stop_bg.load(Ordering::Relaxed) { break; }

            let now = Instant::now();
            let deadline_now = *deadline_bg.lock().unwrap();
            if now >= deadline_now {
                println!("\r  ■ max-duration reached — stopping                       ");
                stop_bg.store(true, Ordering::Relaxed);
                break;
            }

            let elapsed   = now.duration_since(start).as_secs();
            let remaining = deadline_now.duration_since(now);
            let approaching = remaining <= WARN_BEFORE;

            if approaching && !warned {
                warned = true;
                println!();
                println!("  ⚠  Recording will auto-stop in {} — press [e] to extend +{}, Ctrl-C to stop now",
                    format_hms(remaining.as_secs()), human_short(extend_by));
            }

            let suffix = if approaching {
                format!(" — auto-stop in {}", format_hms(remaining.as_secs()))
            } else { String::new() };
            print!("\r  {} REC  {}  —  Ctrl-C to stop{}  ",
                dots[i % dots.len()], format_hms(elapsed), suffix);
            io::stdout().flush().ok();
            i += 1;

            // 200 ms loop: balances timer smoothness with responsive keypress polling.
            if interactive {
                if let Ok(true) = event::poll(Duration::from_millis(200)) {
                    if let Ok(Event::Key(k)) = event::read() {
                        if k.kind != KeyEventKind::Release {
                            match k.code {
                                KeyCode::Char('e') | KeyCode::Char('E') if approaching => {
                                    let mut d = deadline_bg.lock().unwrap();
                                    *d += extend_by;
                                    warned = false;
                                    println!();
                                    println!("  ✓ Extended by {} — new deadline {}",
                                        human_short(extend_by),
                                        format_hms((*d - start).as_secs()));
                                }
                                KeyCode::Char('c') if k.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                                    // Raw mode swallows the default Ctrl-C → SIGINT path.
                                    stop_bg.store(true, Ordering::Relaxed);
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            } else {
                thread::sleep(Duration::from_millis(600));
            }
        }

        let total = start.elapsed().as_secs();
        println!("\r  ■ stopped  —  {} recorded                                ", format_hms(total));
    });

    drop(deadline);
    Handle { stop, thread }
}

fn human_short(d: Duration) -> String {
    let s = d.as_secs();
    if s % 3600 == 0 { format!("{}h", s / 3600) }
    else if s % 60 == 0 { format!("{}m", s / 60) }
    else { format!("{}s", s) }
}

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Result<Self, std::io::Error> {
        terminal::enable_raw_mode()?;
        Ok(RawModeGuard)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hms_basic() {
        assert_eq!(format_hms(0), "00:00:00");
        assert_eq!(format_hms(59), "00:00:59");
        assert_eq!(format_hms(60), "00:01:00");
        assert_eq!(format_hms(3661), "01:01:01");
    }

    #[test]
    fn parse_durations_ok() {
        assert_eq!(parse_duration("4h").unwrap(),       Duration::from_secs(4 * 3600));
        assert_eq!(parse_duration("90m").unwrap(),      Duration::from_secs(90 * 60));
        assert_eq!(parse_duration("1h30m").unwrap(),    Duration::from_secs(90 * 60));
        assert_eq!(parse_duration("2h15m30s").unwrap(), Duration::from_secs(2 * 3600 + 15 * 60 + 30));
        assert_eq!(parse_duration("45s").unwrap(),      Duration::from_secs(45));
    }

    #[test]
    fn parse_durations_err() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("90").is_err());        // missing unit
        assert!(parse_duration("h").is_err());         // missing number
        assert!(parse_duration("1x").is_err());        // unknown unit
        assert!(parse_duration("1h2x").is_err());
    }
}
