use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalKind {
    Kitty,
    WezTerm,
    Other,
}

pub fn detect() -> TerminalKind {
    if env::var("WEZTERM_EXECUTABLE").is_ok()
        || env::var("WEZTERM_PANE").is_ok()
        || term_program_eq("WezTerm")
    {
        return TerminalKind::WezTerm;
    }

    if env::var("KITTY_WINDOW_ID").is_ok()
        || term_program_eq("kitty")
        || env::var("TERM").map(|v| v == "xterm-kitty").unwrap_or(false)
    {
        return TerminalKind::Kitty;
    }

    TerminalKind::Other
}

pub fn is_kitty() -> bool {
    detect() == TerminalKind::Kitty
}

pub fn is_wezterm() -> bool {
    detect() == TerminalKind::WezTerm
}

fn term_program_eq(name: &str) -> bool {
    env::var("TERM_PROGRAM")
        .map(|v| v.eq_ignore_ascii_case(name))
        .unwrap_or(false)
}
