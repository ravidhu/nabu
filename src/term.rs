//! Detect the parent terminal application so the doctor can give precise
//! "add this exact app at this path" instructions for macOS permission panes.

pub struct TerminalApp {
    pub name: &'static str,
    pub path: &'static str,
}

/// Best-effort detection from `$TERM_PROGRAM`, the canonical signal set by
/// most macOS terminals. Returns `None` when the variable is missing, points
/// to a multiplexer (`tmux`, `screen`), or names a terminal we don't map.
pub fn detect() -> Option<TerminalApp> {
    let prog = std::env::var("TERM_PROGRAM").ok()?;
    Some(match prog.as_str() {
        "Apple_Terminal" => TerminalApp { name: "Terminal",            path: "/System/Applications/Utilities/Terminal.app" },
        "iTerm.app"      => TerminalApp { name: "iTerm2",              path: "/Applications/iTerm.app" },
        "WarpTerminal"   => TerminalApp { name: "Warp",                path: "/Applications/Warp.app" },
        "ghostty"        => TerminalApp { name: "Ghostty",             path: "/Applications/Ghostty.app" },
        "Hyper"          => TerminalApp { name: "Hyper",               path: "/Applications/Hyper.app" },
        "vscode"         => TerminalApp { name: "Visual Studio Code",  path: "/Applications/Visual Studio Code.app" },
        "cursor"         => TerminalApp { name: "Cursor",              path: "/Applications/Cursor.app" },
        "tabby"          => TerminalApp { name: "Tabby",               path: "/Applications/Tabby.app" },
        "Alacritty"      => TerminalApp { name: "Alacritty",           path: "/Applications/Alacritty.app" },
        "kitty"          => TerminalApp { name: "kitty",               path: "/Applications/kitty.app" },
        _                => return None,
    })
}
