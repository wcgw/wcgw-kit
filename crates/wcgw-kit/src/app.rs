//! The contract between the kit and an application (D1, D2).
//!
//! The kit owns the screen, the keys and the commands that make an editor; an application owns its state and what
//! its buffers show. It implements [`App`]: [`App::update`] reacts to its own messages, the other methods are hooks
//! the kit calls at the moments that matter to an application (a buffer shown, a name to open, text edited). Both
//! return [`Effect`]s, which the runtime carries out: the kit's own (copy, open a link, read a file), or the
//! application's, which its [`Perform`](crate::runtime::Perform) does.

use std::fmt::Debug;
use std::time::Duration;

use crate::Kit;
use crate::key::Key;
use crate::shell::Prepared;

/// An application the kit runs: what its text refers to, its messages and its effects, and its hooks.
pub trait App: Sized + 'static {
    /// What text in its buffers refers to, such as a link or the item a block shows: what `:open` (`<CR>`) and
    /// the application's commands act on.
    type Target: Clone + PartialEq + Debug + 'static;
    /// What its own work tells it, such as answers from a server.
    type Msg: Clone + PartialEq + Debug + Send + 'static;
    /// What it asks its [`Perform`](crate::runtime::Perform) to do, such as asking a server.
    type Effect: Clone + PartialEq + Debug + 'static;

    /// Reacts to one of its messages.
    fn update(&mut self, kit: &mut Kit<Self>, msg: Self::Msg) -> Effects<Self>;

    /// The effects to start with, once the runtime is ready.
    fn start(&mut self, kit: &mut Kit<Self>) -> Effects<Self> {
        let _ = kit;
        Vec::new()
    }

    /// `:open` (`<CR>`): opens what `target`, under the cursor, refers to. The window's place goes on its tag
    /// stack when it shows another buffer after, for `<C-t>`.
    ///
    /// # Errors
    ///
    /// What to tell the user when it can't be opened.
    fn open(&mut self, kit: &mut Kit<Self>, target: Self::Target) -> Result<Effects<Self>, String> {
        let _ = (kit, target);
        Ok(Vec::new())
    }

    /// `:edit {name}` for a name no buffer has: the buffer the application would make of it, shown with
    /// [`Kit::show_buffer`].
    ///
    /// # Errors
    ///
    /// Vim's E94 by default: the application knows no such name.
    fn edit(&mut self, kit: &mut Kit<Self>, name: &str) -> Result<Effects<Self>, String> {
        let _ = kit;
        Err(format!("E94: No matching buffer for {name}"))
    }

    /// Names `:edit` completes besides the buffers', such as what [`App::edit`] knows.
    fn edit_names(&self, kit: &Kit<Self>) -> Vec<String> {
        let _ = kit;
        Vec::new()
    }

    /// `:edit` without a name, `:e!`, `<C-l>`: loads `buffer` again. `None` when there is nothing to reload.
    fn reload(&mut self, kit: &mut Kit<Self>, buffer: BufferId) -> Option<Effects<Self>> {
        let _ = (kit, buffer);
        None
    }

    /// A window shows `buffer` now: load what it shows, unless it has it or is getting it.
    fn shown(&mut self, kit: &mut Kit<Self>, buffer: BufferId) -> Effects<Self> {
        let _ = (kit, buffer);
        Vec::new()
    }

    /// A key was handled: the cursor may be near the end of what is loaded, for instance.
    fn keyed(&mut self, kit: &mut Kit<Self>) -> Effects<Self> {
        let _ = kit;
        Vec::new()
    }

    /// The editor [`Effect::Edit`] opened closed: what it saved, or why there is nothing (it failed, or quit with
    /// an error as Vim's `:cq` does).
    fn edited(&mut self, kit: &mut Kit<Self>, saved: Result<String, String>) -> Effects<Self> {
        let _ = (kit, saved);
        Vec::new()
    }

    /// The command [`Effect::Shell`] ran exited with `status`: its code, or why it could not run or was stopped. The
    /// kit has said so already when it failed (`shell returned 1`), as Vim does.
    fn shelled(&mut self, kit: &mut Kit<Self>, status: Result<i32, String>) -> Effects<Self> {
        let _ = (kit, status);
        Vec::new()
    }

    /// The terminal is `width` × `height` cells now: the application may reshape its windows (D11), as a thread
    /// beside its conversation that a narrow screen can't hold.
    fn resized(&mut self, kit: &mut Kit<Self>, width: u16, height: u16) -> Effects<Self> {
        let _ = (kit, width, height);
        Vec::new()
    }

    /// `:q` on the last window, or `:qa`: whether to quit now, or later, once what the application has on its way is
    /// done, when it sends [`Effect::Quit`] itself. `:qa!` quits without asking.
    fn quit(&mut self, kit: &mut Kit<Self>) -> Quitting {
        let _ = kit;
        Quitting::Now
    }

    /// The terminal gained (`true`) or lost focus.
    fn focus(&mut self, kit: &mut Kit<Self>, focused: bool) -> Effects<Self> {
        let _ = (kit, focused);
        Vec::new()
    }
}

/// When to quit, as [`App::quit`] says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quitting {
    /// Now.
    Now,
    /// Once the application sends [`Effect::Quit`].
    Later,
}

/// What the kit and application ask the runtime to do.
pub type Effects<A> = Vec<Effect<<A as App>::Msg, <A as App>::Effect>>;

/// Identifies a buffer for as long as the kit runs: buffers are never removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(pub(crate) usize);

impl BufferId {
    /// Its number, from 0, in the order buffers were made.
    #[must_use]
    pub fn index(self) -> usize {
        self.0
    }
}

/// A token the runtime hands back as [`Msg::Wake`] once the delay of an [`Effect::Wake`] has passed. Only the kit
/// makes and reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wake {
    pub(crate) purpose: WakePurpose,
    /// What it belongs to: a later key or query makes it stale.
    pub(crate) generation: u64,
}

impl Wake {
    /// What it is for.
    #[must_use]
    pub fn purpose(&self) -> WakePurpose {
        self.purpose
    }
}

/// What a [`Wake`] is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WakePurpose {
    /// `'timeoutlen'` passed on an ambiguous key sequence.
    KeyTimeout,
    /// Time to show the which-key popup.
    WhichKey,
    /// Typing paused in a picker that searches: time to search.
    Search,
    /// Time for animations drawn with half blocks to show their next frames.
    Frame,
}

/// Something that happened, for the kit to react to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg<M> {
    /// A key was pressed.
    Key(Key),
    /// The mouse was clicked or its wheel turned (D5), over the cell at `column` and `row`.
    Mouse {
        /// What it did.
        mouse: crate::key::Mouse,
        /// The cell's column.
        column: u16,
        /// The cell's row.
        row: u16,
    },
    /// Text was pasted (bracketed paste). It never runs commands; on the command line it is inserted.
    Paste(String),
    /// The editor opened by [`Effect::Edit`] closed: what it saved, or why there is nothing.
    Edited(Result<String, String>),
    /// The command [`Effect::Shell`] ran exited: with this code, or why it could not run or was stopped.
    Shelled(Result<i32, String>),
    /// A picture asked for by [`Effect::Picture`], prepared as asked, or why it could not be.
    Picture {
        /// Where it is.
        source: String,
        /// Its width in cells.
        cols: u16,
        /// Its height in cells.
        rows: u16,
        /// The picture, or why there is none.
        result: Result<Prepared, String>,
    },
    /// The terminal gained focus.
    FocusGained,
    /// The terminal lost focus.
    FocusLost,
    /// The delay of an [`Effect::Wake`] has passed.
    Wake(Wake),
    /// A file to run as ex commands, such as `init.vim` at startup or what `:source` asked for, or why it could not
    /// be read.
    Sourced {
        /// The file, as named to the user.
        path: String,
        /// Its text, or the reason it could not be read.
        contents: Result<String, String>,
    },
    /// Something the runtime could not do, to tell the user.
    Failed(String),
    /// The terminal has this size, in cells; sent at startup and on every resize.
    Resize {
        /// Columns.
        width: u16,
        /// Rows.
        height: u16,
    },
    /// One of the application's messages.
    App(M),
}

/// What [`Effect::Edit`] hands the editor: a file holding `text`, named with `suffix` so the editor knows its kind
/// (`md` unless told), opened with `editor` when given, else the user's (`$EDITOR`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// What the file holds.
    pub text: String,
    /// What its name ends with, after a dot: `md`, `eml`.
    pub suffix: String,
    /// A shell command to open it with, in place of the user's editor; the file's path follows it.
    pub editor: Option<String>,
}

impl Edit {
    /// `text`, in a Markdown file, for the user's editor.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            suffix: "md".to_owned(),
            editor: None,
        }
    }

    /// In a file whose name ends with `.suffix`, as `eml` for a mail.
    #[must_use]
    pub fn suffix(self, suffix: impl Into<String>) -> Self {
        Self {
            suffix: suffix.into(),
            ..self
        }
    }

    /// Opened with `editor`, a shell command the file's path follows, rather than the user's.
    #[must_use]
    pub fn editor(self, editor: impl Into<String>) -> Self {
        Self {
            editor: Some(editor.into()),
            ..self
        }
    }
}

/// What [`Effect::Shell`] runs: `command`, by `sh -c`, with the terminal; reading `stdin` when given, else the
/// terminal; and, when it should `wait`, asking for Enter once it exits, so what it printed can be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shell {
    /// The command line.
    pub command: String,
    /// What it reads, as a message piped to it; the terminal when `None`.
    pub stdin: Option<String>,
    /// Whether to wait for Enter once it exits, before the application shows again.
    pub wait: bool,
}

impl Shell {
    /// `command`, reading the terminal, not waiting once it exits.
    #[must_use]
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            stdin: None,
            wait: false,
        }
    }

    /// Reading `text`, as `|` pipes a message.
    #[must_use]
    pub fn stdin(self, text: impl Into<String>) -> Self {
        Self {
            stdin: Some(text.into()),
            ..self
        }
    }

    /// Asking for Enter once it exits, as `:!` does, so its output can be read.
    #[must_use]
    pub fn wait(self) -> Self {
        Self { wait: true, ..self }
    }
}

/// Something the kit or the application asks the runtime to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect<M, E> {
    /// Restore the terminal and exit.
    Quit,
    /// Put this text on the system clipboard (OSC 52).
    Copy(String),
    /// Read this file (`~` is the home directory) and send its contents back as [`Msg::Sourced`].
    Source(String),
    /// Get the picture at `source` and prepare it for `cols` × `rows` cells, then send [`Msg::Picture`]: for the
    /// kitty protocol as image `kitty` when given, as half blocks otherwise.
    Picture {
        /// Where it is.
        source: String,
        /// Its width in cells.
        cols: u16,
        /// Its height in cells.
        rows: u16,
        /// Its id in the terminal, for the kitty protocol.
        kitty: Option<u32>,
    },
    /// Write these graphics protocol escapes to the terminal, as they are.
    Graphics(String),
    /// Have the terminal report mouse clicks and the wheel (D5), or stop.
    CaptureMouse(bool),
    /// Show a desktop notification through the terminal (OSC 777), as when something arrives while it lacks focus.
    Notify {
        /// Its title.
        title: String,
        /// What it says.
        body: String,
    },
    /// Hand the terminal to the user's editor on a file of text, then send [`Msg::Edited`].
    Edit(Edit),
    /// Hand the terminal to a shell command until it exits, then send [`Msg::Shelled`]: `!` and `|`, as Vim's and
    /// mutt's.
    Shell(Shell),
    /// Open this link in the browser.
    OpenUrl(String),
    /// Send [`Msg::Wake`] with `token` once `after` has passed.
    Wake {
        /// How long to wait.
        after: Duration,
        /// What to send back.
        token: Wake,
    },
    /// Send the application `msg` once `after` has passed, as for polling.
    After {
        /// How long to wait.
        after: Duration,
        /// What to send back.
        msg: M,
    },
    /// One of the application's effects, for its [`Perform`](crate::runtime::Perform).
    App(E),
}
