//! keyloom: see, audit and improve ZSA keyboard layouts.
//!
//! The engine is a library: no printing, every feature returns data. `keyloom-cli` is the CLI
//! and the MCP server on top.

pub mod diff;
pub mod explore;
pub mod guide;
pub mod knowledge;
pub mod labels;
pub mod model;
pub mod ops;
pub mod oryx;
pub mod positions;
pub mod qmk;
pub mod render;
pub mod source;
pub mod summary;
pub mod time;
pub mod validate;
pub mod workspace;

/// Debug logging on stderr, switched on with `KEYLOOM_DEBUG=1`.
pub fn debug_enabled() -> bool {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("KEYLOOM_DEBUG").is_some_and(|v| v != "0" && !v.is_empty()))
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::debug_enabled() {
            eprintln!("[keyloom] {}", format!($($arg)*));
        }
    };
}
