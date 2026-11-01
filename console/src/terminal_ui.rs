//! Shared, conservative terminal styling for the OID console.

use std::io::{self, IsTerminal};

/// Whether cursor positioning and an interactive editor are appropriate.
pub fn interactive() -> bool {
    io::stdin().is_terminal()
        && io::stdout().is_terminal()
        && std::env::var("TERM").as_deref() != Ok("dumb")
        && !std::env::var("TALVIK_PLAIN").is_ok_and(|value| value == "1" || value == "true")
}

/// Whether terminal accents may use ANSI styling.
pub fn colors() -> bool {
    interactive() && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
}

/// Apply an optional, theme-relative accent.
pub fn accent(style: &str, text: &str) -> String {
    if colors() {
        format!("\x1b[{style}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}
