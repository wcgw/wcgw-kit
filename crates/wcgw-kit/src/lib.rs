//! A Neovim look & feel for ratatui applications.
//!
//! The kit owns what makes four terminal applications feel like one editor: keys and the keymap, ex commands, the
//! command line, highlight groups and colorschemes, buffers of read-only text with a cursor, windows, and the chrome
//! around them. Applications bring their own content and commands. Decisions and their reasons live in
//! `docs/plan.md`; module docs name the relevant entries (D1 and so on).

pub mod app;
pub mod doc;
pub mod ex;
mod history;
pub mod hl;
pub mod key;
pub mod keymap;
pub mod runtime;
pub mod shell;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use app::{App, BufferId, Edit, Effect, Effects, Msg, Quitting, Shell};
pub use shell::Kit;
/// Pictures in the terminal (D15).
pub use wcgw_kit_gfx as gfx;
