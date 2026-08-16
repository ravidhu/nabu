//! Interactive language picker for the transcription step.
//!
//! `-l/--language` remains the non-interactive path. This module is only reached
//! on a TTY when that flag was omitted and the chosen model is multilingual —
//! English-only models (`*.en`) have nothing to ask about.

use std::io::{self, IsTerminal, Write};

/// Languages offered as numbered entries. Everything else in
/// [`WHISPER_LANGUAGES`] stays reachable through the "other" entry or `-l`.
const MENU: &[(&str, &str)] = &[
    ("en", "English"),
    ("fr", "French"),
    ("de", "German"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("ja", "Japanese"),
    ("zh", "Chinese"),
];

/// Every language code Whisper accepts. Checked here so a typo fails in
/// milliseconds instead of after a multi-gigabyte model load.
#[rustfmt::skip]
const WHISPER_LANGUAGES: &[&str] = &[
    "en", "zh", "de", "es",  "ru", "ko", "fr", "ja", "pt", "tr",
    "pl", "ca", "nl", "ar",  "sv", "it", "id", "hi", "fi", "vi",
    "he", "uk", "el", "ms",  "cs", "ro", "da", "hu", "ta", "no",
    "th", "ur", "hr", "bg",  "lt", "la", "mi", "ml", "cy", "sk",
    "te", "fa", "lv", "bn",  "sr", "az", "sl", "kn", "et", "mk",
    "br", "eu", "is", "hy",  "ne", "mn", "bs", "kk", "sq", "sw",
    "gl", "mr", "pa", "si",  "km", "sn", "yo", "so", "af", "oc",
    "ka", "be", "tg", "sd",  "gu", "am", "yi", "lo", "uz", "fo",
    "ht", "ps", "tk", "nn",  "mt", "sa", "lb", "my", "bo", "tl",
    "mg", "as", "tt", "haw", "ln", "ha", "ba", "jw", "su", "yue",
];

/// What one line of user input resolved to.
#[derive(Debug, PartialEq)]
enum Choice {
    /// Auto-detect — no `--language` reaches the Python side.
    Auto,
    /// A validated Whisper language code.
    Code(String),
    /// The "other" entry — prompt for a free-text code.
    Other,
    /// Unrecognised input; re-ask.
    Invalid,
}

/// Decide which language to transcribe in, prompting only when a prompt can
/// change the outcome. `None` means auto-detect.
///
/// Returns without printing anything when `-l` already answered the question,
/// when `-y` asked for an unattended run, when the model is English-only, or
/// when stdin/stdout are not both terminals.
pub fn resolve(flag: Option<&str>, model: &str, assume_yes: bool) -> Option<String> {
    if let Some(lang) = flag {
        return Some(lang.to_string());
    }
    if assume_yes || model.ends_with(".en") {
        return None;
    }
    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        return None;
    }
    prompt()
}

/// Draw the menu and read a choice, re-asking until the input parses. EOF or a
/// read error falls back to auto-detect rather than looping forever.
fn prompt() -> Option<String> {
    loop {
        println!("Language:");
        println!("  {:>2}) auto-detect (default)", 1);
        for (index, (code, name)) in MENU.iter().enumerate() {
            println!("  {:>2}) {:<10} ({})", index + 2, name, code);
        }
        println!("  {:>2}) other — type a code", MENU.len() + 2);
        print!("> ");
        let _ = io::stdout().flush();

        let line = read_line()?;
        match parse(&line) {
            Choice::Auto => return None,
            Choice::Code(code) => return Some(code),
            Choice::Other => {
                print!("Language code (e.g. nl, ko, sv): ");
                let _ = io::stdout().flush();
                let line = read_line()?;
                match parse_code(&line) {
                    Choice::Auto => return None,
                    Choice::Code(code) => return Some(code),
                    _ => println!("  Unknown language code — pick again.\n"),
                }
            }
            Choice::Invalid => println!("  Not a valid choice — pick again.\n"),
        }
    }
}

/// Read one line from the TTY. `None` on EOF or read error.
fn read_line() -> Option<String> {
    let mut line = String::new();
    match io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line),
    }
}

/// Parse a menu response: a number, a bare language code typed instead of a
/// number, or empty for the default.
fn parse(input: &str) -> Choice {
    let answer = input.trim().to_ascii_lowercase();
    if answer.is_empty() {
        return Choice::Auto;
    }
    if let Ok(entry) = answer.parse::<usize>() {
        return match entry {
            1 => Choice::Auto,
            entry if (2..=MENU.len() + 1).contains(&entry) => Choice::Code(MENU[entry - 2].0.to_string()),
            entry if entry == MENU.len() + 2 => Choice::Other,
            _ => Choice::Invalid,
        };
    }
    parse_code(&answer)
}

/// Parse a free-text language code. Empty and the literal `auto` both mean
/// auto-detect; anything outside Whisper's language set is rejected.
fn parse_code(input: &str) -> Choice {
    let code = input.trim().to_ascii_lowercase();
    if code.is_empty() || code == "auto" {
        return Choice::Auto;
    }
    if WHISPER_LANGUAGES.contains(&code.as_str()) {
        Choice::Code(code)
    } else {
        Choice::Invalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_first_entry_mean_auto_detect() {
        assert_eq!(parse(""), Choice::Auto);
        assert_eq!(parse("\n"), Choice::Auto);
        assert_eq!(parse("1"), Choice::Auto);
        assert_eq!(parse("auto"), Choice::Auto);
    }

    #[test]
    fn numbers_map_to_menu_entries() {
        assert_eq!(parse("2"), Choice::Code("en".into()));
        assert_eq!(parse("3"), Choice::Code("fr".into()));
        assert_eq!(parse(" 9 "), Choice::Code("zh".into()));
    }

    #[test]
    fn last_entry_is_other() {
        assert_eq!(parse(&(MENU.len() + 2).to_string()), Choice::Other);
    }

    #[test]
    fn out_of_range_and_junk_are_invalid() {
        assert_eq!(parse("0"), Choice::Invalid);
        assert_eq!(parse(&(MENU.len() + 3).to_string()), Choice::Invalid);
        assert_eq!(parse("nope"), Choice::Invalid);
    }

    #[test]
    fn a_code_may_be_typed_instead_of_a_number() {
        assert_eq!(parse("fr"), Choice::Code("fr".into()));
        assert_eq!(parse("FR"), Choice::Code("fr".into()));
        // Not on the menu, but still a language Whisper knows.
        assert_eq!(parse("ko"), Choice::Code("ko".into()));
    }

    #[test]
    fn unknown_codes_are_rejected() {
        assert_eq!(parse_code("xx"), Choice::Invalid);
        assert_eq!(parse_code("english"), Choice::Invalid);
    }

    #[test]
    fn every_menu_code_is_a_real_whisper_language() {
        for (code, _) in MENU {
            assert!(WHISPER_LANGUAGES.contains(code), "{code} is unknown to Whisper");
        }
    }

    #[test]
    fn flag_wins_over_prompting() {
        assert_eq!(resolve(Some("fr"), "large-v3", false), Some("fr".into()));
    }

    #[test]
    fn english_only_models_and_unattended_runs_skip_the_prompt() {
        assert_eq!(resolve(None, "small.en", false), None);
        assert_eq!(resolve(None, "large-v3", true), None);
    }
}
