//! The ex command registry (D13): the kit's commands and the application's, found by name or abbreviation.

use std::fmt;
use std::rc::Rc;

use crate::app::{App, Effects};
use crate::ex::ExError;
use crate::keymap::Mode;
use crate::shell::Kit;

/// A command line, as an application's command receives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invocation<'a> {
    /// The command's full name.
    pub name: &'static str,
    /// Whether a `!` followed the name.
    pub bang: bool,
    /// The rest of the line, trimmed.
    pub args: &'a str,
}

/// What an application's command does: reacts to its line, with the application and the kit at hand.
pub type Run<A> = Rc<dyn Fn(&mut A, &mut Kit<A>, Invocation<'_>) -> Result<Effects<A>, String>>;

/// What an application's command completes an argument to: given the arguments before it and the word typed.
pub type Complete<A> = Rc<dyn Fn(&A, &Kit<A>, &[&str], &str) -> Vec<String>>;

/// An application's ex command.
pub struct Command<A: App> {
    pub(crate) name: &'static str,
    pub(crate) min: usize,
    pub(crate) bang: bool,
    pub(crate) jumps: bool,
    pub(crate) help: &'static str,
    pub(crate) run: Run<A>,
    pub(crate) complete: Option<Complete<A>>,
}

impl<A: App> fmt::Debug for Command<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("name", &self.name)
            .field("min", &self.min)
            .finish_non_exhaustive()
    }
}

impl<A: App> Command<A> {
    /// `:name`, which may be shortened down to its first `min` characters, as Vim's are; running `run`.
    pub fn new(
        name: &'static str,
        min: usize,
        run: impl Fn(&mut A, &mut Kit<A>, Invocation<'_>) -> Result<Effects<A>, String> + 'static,
    ) -> Self {
        Self {
            name,
            min: min.clamp(1, name.len()),
            bang: false,
            jumps: false,
            help: "",
            run: Rc::new(run),
            complete: None,
        }
    }

    /// Takes a `!`; without this, `:name!` is Vim's E477.
    #[must_use]
    pub fn bang(self) -> Self {
        Self { bang: true, ..self }
    }

    /// Follows something to another buffer, as `:open` does: where the window was goes on its tag stack when it
    /// shows another buffer after, for `<C-t>`.
    #[must_use]
    pub fn jumps(self) -> Self {
        Self {
            jumps: true,
            ..self
        }
    }

    /// What `:help` says it does.
    #[must_use]
    pub fn help(self, help: &'static str) -> Self {
        Self { help, ..self }
    }

    /// Completes its arguments with `complete`.
    #[must_use]
    pub fn complete(
        self,
        complete: impl Fn(&A, &Kit<A>, &[&str], &str) -> Vec<String> + 'static,
    ) -> Self {
        Self {
            complete: Some(Rc::new(complete)),
            ..self
        }
    }
}

/// One of the kit's commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Builtin {
    Quit,
    QuitAll,
    Map(&'static [Mode], bool),
    Unmap(&'static [Mode]),
    Set,
    Highlight,
    Colorscheme,
    Source,
    Messages,
    Help,
    Split(bool),
    Close,
    Only,
    Wincmd,
    Open,
    Pop,
    Jumps,
    Nohlsearch,
    Pick,
    Edit,
    Buffer,
    Buffers,
    NextBuffer(bool),
}

impl Builtin {
    /// Whether it takes a `!`.
    fn bang(self) -> bool {
        matches!(self, Self::Quit | Self::QuitAll | Self::Edit)
    }
}

const NORMAL: &[Mode] = &[Mode::Normal];
const VISUAL: &[Mode] = &[Mode::Visual];
const NORMAL_VISUAL: &[Mode] = &[Mode::Normal, Mode::Visual];

/// The kit's commands: full name, shortest abbreviation length, command, and what `:help` says of it, in the order
/// Vim resolves ambiguous abbreviations.
const BUILT_IN: &[(&str, usize, Builtin, &str)] = &[
    (
        "edit",
        1,
        Builtin::Edit,
        "open a buffer by name; alone, reload",
    ),
    (
        "buffer",
        1,
        Builtin::Buffer,
        "show a buffer, by number or name",
    ),
    ("buffers", 7, Builtin::Buffers, "list the buffers"),
    ("bnext", 2, Builtin::NextBuffer(true), "next buffer"),
    (
        "bprevious",
        2,
        Builtin::NextBuffer(false),
        "previous buffer",
    ),
    ("ls", 2, Builtin::Buffers, "list the buffers"),
    (
        "quit",
        1,
        Builtin::Quit,
        "close the window; the last one quits",
    ),
    ("qall", 2, Builtin::QuitAll, "quit"),
    ("quitall", 5, Builtin::QuitAll, "quit"),
    ("map", 3, Builtin::Map(NORMAL_VISUAL, true), "map keys"),
    (
        "nmap",
        2,
        Builtin::Map(NORMAL, true),
        "map keys in Normal mode",
    ),
    (
        "vmap",
        2,
        Builtin::Map(VISUAL, true),
        "map keys in Visual mode",
    ),
    (
        "xmap",
        2,
        Builtin::Map(VISUAL, true),
        "map keys in Visual mode",
    ),
    (
        "noremap",
        2,
        Builtin::Map(NORMAL_VISUAL, false),
        "map keys, not remapped",
    ),
    (
        "nnoremap",
        2,
        Builtin::Map(NORMAL, false),
        "map keys in Normal mode, not remapped",
    ),
    (
        "vnoremap",
        2,
        Builtin::Map(VISUAL, false),
        "map keys in Visual mode, not remapped",
    ),
    (
        "xnoremap",
        2,
        Builtin::Map(VISUAL, false),
        "map keys in Visual mode, not remapped",
    ),
    ("unmap", 3, Builtin::Unmap(NORMAL_VISUAL), "remove a map"),
    (
        "nunmap",
        3,
        Builtin::Unmap(NORMAL),
        "remove a Normal mode map",
    ),
    (
        "vunmap",
        2,
        Builtin::Unmap(VISUAL),
        "remove a Visual mode map",
    ),
    (
        "xunmap",
        2,
        Builtin::Unmap(VISUAL),
        "remove a Visual mode map",
    ),
    ("set", 2, Builtin::Set, "set or show options"),
    ("help", 1, Builtin::Help, "the key bindings and commands"),
    (
        "highlight",
        2,
        Builtin::Highlight,
        "set or show a highlight group",
    ),
    (
        "colorscheme",
        4,
        Builtin::Colorscheme,
        "load a colorscheme, or show which",
    ),
    ("source", 2, Builtin::Source, "run a file of ex commands"),
    ("messages", 3, Builtin::Messages, "the messages shown"),
    ("split", 2, Builtin::Split(false), "split the window"),
    (
        "vsplit",
        2,
        Builtin::Split(true),
        "split the window side by side",
    ),
    ("close", 3, Builtin::Close, "close the window"),
    ("open", 1, Builtin::Open, "open what is under the cursor"),
    ("pop", 2, Builtin::Pop, "back from what was opened"),
    ("jumps", 2, Builtin::Jumps, "list the jumplist"),
    (
        "nohlsearch",
        3,
        Builtin::Nohlsearch,
        "hide the search's matches",
    ),
    ("pick", 2, Builtin::Pick, "a picker"),
    ("only", 2, Builtin::Only, "close the other windows"),
    ("wincmd", 4, Builtin::Wincmd, "what <C-w> and a key do"),
];

/// What a command name stands for.
pub(crate) enum Found<A: App> {
    Builtin(Builtin),
    App(Rc<Command<A>>),
}

/// Every command `:` knows: the kit's, then the application's in the order added.
pub(crate) struct Commands<A: App> {
    app: Vec<Rc<Command<A>>>,
}

impl<A: App> fmt::Debug for Commands<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.app).finish()
    }
}

impl<A: App> Default for Commands<A> {
    fn default() -> Self {
        Self { app: Vec::new() }
    }
}

impl<A: App> Commands<A> {
    /// Adds the application's `command`, replacing one of its own of the same name.
    ///
    /// # Panics
    ///
    /// When the kit has a command of that name: the kit's commands are the same in every application.
    pub(crate) fn add(&mut self, command: Command<A>) {
        assert!(
            BUILT_IN.iter().all(|&(name, ..)| name != command.name),
            ":{} is the kit's",
            command.name
        );
        self.app.retain(|known| known.name != command.name);
        self.app.push(Rc::new(command));
    }

    /// The command `typed` names, in full or abbreviated as Vim allows, with its full name: a full name first,
    /// then the first in order whose abbreviation `typed` is.
    pub(crate) fn lookup(&self, typed: &str) -> Option<(&'static str, Found<A>)> {
        let builtin = |exact: bool| {
            BUILT_IN
                .iter()
                .find(|(full, min, ..)| {
                    if exact {
                        *full == typed
                    } else {
                        typed.len() >= *min && full.starts_with(typed)
                    }
                })
                .map(|&(full, _, builtin, _)| (full, Found::Builtin(builtin)))
        };
        let app = |exact: bool| {
            self.app
                .iter()
                .find(|command| {
                    if exact {
                        command.name == typed
                    } else {
                        typed.len() >= command.min && command.name.starts_with(typed)
                    }
                })
                .map(|command| (command.name, Found::App(Rc::clone(command))))
        };
        builtin(true)
            .or_else(|| app(true))
            .or_else(|| builtin(false))
            .or_else(|| app(false))
    }

    /// The full names of every command, the kit's first.
    pub(crate) fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        BUILT_IN
            .iter()
            .map(|&(name, ..)| name)
            .chain(self.app.iter().map(|command| command.name))
    }

    /// Every command with what `:help` says of it, the kit's first.
    pub(crate) fn listing(&self) -> Vec<(&'static str, &'static str)> {
        BUILT_IN
            .iter()
            .map(|&(name, _, _, help)| (name, help))
            .chain(self.app.iter().map(|command| (command.name, command.help)))
            .collect()
    }
}

/// Checks a `!` against what the command found takes.
pub(crate) fn check_bang<A: App>(found: &Found<A>, bang: bool) -> Result<(), ExError> {
    let takes = match found {
        Found::Builtin(builtin) => builtin.bang(),
        Found::App(command) => command.bang,
    };
    if bang && !takes {
        Err(ExError::NoBang)
    } else {
        Ok(())
    }
}
