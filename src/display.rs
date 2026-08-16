use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal;

use crate::meter::Meter;

const WARN_BEFORE: Duration = Duration::from_secs(10 * 60);
const BAR_WIDTH: usize = 24;

/// Format a `u64` second count as `HH:MM:SS`.
pub fn format_hms(secs: u64) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}

/// Render a 0..1 level as a fixed-width bar of filled/empty glyphs. Non-finite
/// or out-of-range input is clamped into [0, 1].
pub fn render_bar(level: f32, width: usize) -> String {
    let level = if level.is_finite() {
        level.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = ((level * width as f32).round() as usize).min(width);
    let mut s = String::with_capacity(width * 3);
    for _ in 0..filled {
        s.push('█');
    }
    for _ in 0..width - filled {
        s.push('░');
    }
    s
}

/// Draw or redraw the 3-line live region (timer + mic bar + sys bar) in place.
/// On a repeat draw the cursor is first moved up 2 lines back to the timer row;
/// the first draw (`*drawn == false`) prints fresh with no up-move. Each line is
/// cleared to EOL and carriage-returned so it works in raw mode too.
fn draw_region(drawn: &mut bool, timer: &str, mic: &str, sys: &str) {
    let mut o = String::new();
    if *drawn {
        o.push_str("\x1b[2A");
    } // move up to the timer line
    o.push_str("\r\x1b[2K");
    o.push_str(timer);
    o.push('\n');
    o.push_str("\r\x1b[2K");
    o.push_str(mic);
    o.push('\n');
    o.push_str("\r\x1b[2K");
    o.push_str(sys);
    print!("{o}");
    io::stdout().flush().ok();
    *drawn = true;
}

/// Erase the live region so a transient message can be printed cleanly above the
/// next redraw. No-op if nothing is drawn (first message, or non-interactive).
fn clear_region(drawn: &mut bool) {
    if !*drawn {
        return;
    }
    // Cursor sits on the sys (bottom) line: go up to the timer line, clear all 3.
    print!("\x1b[2A\r\x1b[2K\x1b[1B\x1b[2K\x1b[1B\x1b[2K\x1b[2A\r");
    io::stdout().flush().ok();
    *drawn = false;
}

/// Parse a human duration like `4h`, `90m`, `1h30m`, `2h15m30s` into a `Duration`.
/// Returns an error on empty input, a missing unit, or an unknown unit.
pub fn parse_duration(input: &str) -> Result<Duration, String> {
    let text = input.trim();
    if text.is_empty() {
        return Err("empty duration".into());
    }

    let mut total: u64 = 0;
    let mut num: u64 = 0;
    let mut have_num = false;
    let mut have_any = false;

    for ch in text.chars() {
        if ch.is_ascii_digit() {
            num = num
                .checked_mul(10)
                .and_then(|scaled| scaled.checked_add((ch as u64) - ('0' as u64)))
                .ok_or_else(|| "duration overflow".to_string())?;
            have_num = true;
            continue;
        }
        if !have_num {
            return Err(format!("expected number before '{ch}'"));
        }
        let unit_secs = match ch {
            'h' | 'H' => 3600,
            'm' | 'M' => 60,
            's' | 'S' => 1,
            other => return Err(format!("unknown duration unit '{other}' (use h/m/s)")),
        };
        total = total
            .checked_add(
                num.checked_mul(unit_secs)
                    .ok_or_else(|| "duration overflow".to_string())?,
            )
            .ok_or_else(|| "duration overflow".to_string())?;
        num = 0;
        have_num = false;
        have_any = true;
    }
    if have_num {
        return Err("trailing number with no unit (e.g. '90' should be '90m' or '90s')".into());
    }
    if !have_any {
        return Err("no duration components parsed".into());
    }
    Ok(Duration::from_secs(total))
}

pub struct Handle {
    pub stop: Arc<AtomicBool>,
    pub thread: JoinHandle<()>,
}

/// Spawn a thread that prints a live recording timer plus per-stream input-level
/// bars, and watches for a max-duration deadline. Within `WARN_BEFORE` of the
/// deadline, the user can press `e` to extend by `extend_by`; pressing Ctrl-C in
/// raw mode also stops.
pub fn spawn(
    start: Instant,
    max_duration: Duration,
    extend_by: Duration,
    mic_meter: Meter,
    sys_meter: Meter,
) -> Handle {
    let stop = Arc::new(AtomicBool::new(false));
    let deadline = Arc::new(Mutex::new(start + max_duration));
    let stop_bg = stop.clone();
    let deadline_bg = deadline.clone();

    let thread = thread::spawn(move || {
        let interactive = io::stdout().is_terminal();
        let mut raw_guard = if interactive {
            RawModeGuard::enter().ok()
        } else {
            None
        };

        let dots = ['●', '◉'];
        let mut warned = false;
        let mut region_drawn = false;

        loop {
            if stop_bg.load(Ordering::Relaxed) {
                break;
            }

            let now = Instant::now();
            let deadline_now = *deadline_bg.lock().unwrap();
            if now >= deadline_now {
                if interactive {
                    clear_region(&mut region_drawn);
                }
                println!("  ■ max-duration reached — stopping");
                stop_bg.store(true, Ordering::Relaxed);
                break;
            }

            let elapsed = now.duration_since(start).as_secs();
            // Blink the REC dot on a fixed ~2 Hz wall-clock cadence, independent
            // of the (much faster) redraw loop that keeps the level bars smooth.
            let blink = dots[(now.duration_since(start).as_millis() / 500) as usize % dots.len()];
            let remaining = deadline_now.duration_since(now);
            let approaching = remaining <= WARN_BEFORE;

            if approaching && !warned {
                warned = true;
                if interactive {
                    clear_region(&mut region_drawn);
                }
                println!();
                println!(
                    "  ⚠  Recording will auto-stop in {} — press [e] to extend +{}, Ctrl-C to stop now",
                    format_hms(remaining.as_secs()),
                    human_short(extend_by)
                );
            }

            let suffix = if approaching {
                format!(" — auto-stop in {}", format_hms(remaining.as_secs()))
            } else {
                String::new()
            };
            let timer = format!(
                "  {} REC  {}  —  Ctrl-C to stop{}",
                blink,
                format_hms(elapsed),
                suffix
            );

            if interactive {
                let mic = format!("  mic  {}", render_bar(mic_meter.level(), BAR_WIDTH));
                let sys = format!("  sys  {}", render_bar(sys_meter.level(), BAR_WIDTH));
                draw_region(&mut region_drawn, &timer, &mic, &sys);
            } else {
                print!("\r{timer}  ");
                io::stdout().flush().ok();
            }
            // 50 ms loop → ~20 fps level bars and snappy keypress polling. The
            // REC blink and timer are wall-clock derived, so the fast redraw
            // costs nothing but smoothness.
            if interactive {
                if let Ok(true) = event::poll(Duration::from_millis(50)) {
                    if let Ok(Event::Key(k)) = event::read() {
                        if k.kind != KeyEventKind::Release {
                            match k.code {
                                KeyCode::Char('e') | KeyCode::Char('E') if approaching => {
                                    let mut d = deadline_bg.lock().unwrap();
                                    *d += extend_by;
                                    warned = false;
                                    clear_region(&mut region_drawn);
                                    println!();
                                    println!(
                                        "  ✓ Extended by {} — new deadline {}",
                                        human_short(extend_by),
                                        format_hms((*d - start).as_secs())
                                    );
                                }
                                KeyCode::Char('c')
                                    if k.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                                {
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
        if interactive {
            clear_region(&mut region_drawn);
            // Exit raw mode before the final line so its newline is a real
            // CR+LF — otherwise the cursor stays mid-column and whatever main
            // prints next (e.g. the "Transcribe now?" prompt) is indented.
            drop(raw_guard.take());
            print!("\r");
            println!("  ■ stopped  —  {} recorded", format_hms(total));
        } else {
            println!(
                "\r  ■ stopped  —  {} recorded                                ",
                format_hms(total)
            );
        }
    });

    drop(deadline);
    Handle { stop, thread }
}

fn human_short(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs % 3600 == 0 {
        format!("{}h", secs / 3600)
    } else if secs % 60 == 0 {
        format!("{}m", secs / 60)
    } else {
        format!("{}s", secs)
    }
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
        assert_eq!(parse_duration("4h").unwrap(), Duration::from_secs(4 * 3600));
        assert_eq!(parse_duration("90m").unwrap(), Duration::from_secs(90 * 60));
        assert_eq!(parse_duration("1h30m").unwrap(), Duration::from_secs(90 * 60));
        assert_eq!(
            parse_duration("2h15m30s").unwrap(),
            Duration::from_secs(2 * 3600 + 15 * 60 + 30)
        );
        assert_eq!(parse_duration("45s").unwrap(), Duration::from_secs(45));
    }

    #[test]
    fn parse_durations_err() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("90").is_err()); // missing unit
        assert!(parse_duration("h").is_err()); // missing number
        assert!(parse_duration("1x").is_err()); // unknown unit
        assert!(parse_duration("1h2x").is_err());
    }

    #[test]
    fn bar_full_and_empty() {
        assert_eq!(render_bar(1.0, 4), "████");
        assert_eq!(render_bar(0.0, 4), "░░░░");
    }

    #[test]
    fn bar_half_rounds() {
        assert_eq!(render_bar(0.5, 4), "██░░");
    }

    #[test]
    fn bar_clamps_out_of_range() {
        assert_eq!(render_bar(2.0, 4), "████");
        assert_eq!(render_bar(-1.0, 4), "░░░░");
    }

    #[test]
    fn bar_nonfinite_is_empty() {
        assert_eq!(render_bar(f32::NAN, 4), "░░░░");
        assert_eq!(render_bar(f32::INFINITY, 4), "░░░░");
    }
}
