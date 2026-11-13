//! A Neovim look & feel for ratatui applications.
//!
//! The kit owns what makes four terminal applications feel like one editor: keys and the keymap, ex commands, the
//! command line, highlight groups and colorschemes, buffers of read-only text with a cursor, windows, and the chrome
//! around them. Applications bring their own content and commands. Decisions and their reasons live in
//! `docs/plan.md`; module docs name the relevant entries (D1 and so on).

pub mod hl;
pub mod key;
