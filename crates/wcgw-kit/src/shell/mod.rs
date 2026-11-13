//! The editor around an application (D1): buffers in windows, the keymap, the command line, highlight groups, and
//! the chrome around them. [`Kit::update`] is pure: it returns [`Effect`]s, which the runtime carries out.

mod cmdline;
mod commands;
mod complete;
mod composer;
mod confirm;
mod help;
mod hover;
mod icons;
mod mouse;
mod options;
mod picker;
mod pictures;
mod statusline;
#[cfg(test)]
mod tests;
mod view;
mod window;
mod windows;

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use ratatui::layout::Rect;

use crate::app::{App, BufferId, Effect, Effects, Msg, Wake, WakePurpose};
use crate::doc::{
    Cursor, Direction, Document, Layout, Motion, Pattern, SearchOutcome, Selection, SelectionKind,
};
use crate::ex::{self, ExError};
use crate::hl::Theme;
use crate::key::{Code, Key, Mods};
use crate::keymap::{Action, Invocation as Keys, Keymap, Mode};

use cmdline::{Cmdline, Edit, Purpose};
use commands::{Builtin, Commands, Found};
pub use commands::{Command, Complete, Invocation, Run};
pub use complete::Completions;
pub use composer::{Composer, Suggestion, Word};
use confirm::Confirm;
use options::Options;
pub use picker::{Choice, Pick, Picker};
pub use pictures::Prepared;
pub use statusline::{Segment, Side};
pub use wcgw_kit_gfx::Graphics;
use window::{Viewport, Window};
use windows::{Axis, Toward, Windows};
pub use windows::{Placement, Size, WindowId};

/// A cell's size in pixels until the terminal says otherwise: Ghostty's, as measured in devnull's M0.1.
const DEFAULT_CELL_SIZE: (u16, u16) = (10, 20);

/// How long pending keys wait before the which-key popup shows what can follow them.
const WHICH_KEY_DELAY: Duration = Duration::from_millis(200);

/// A named document shown in windows: a log, a thread, a mailbox, a channel.
#[derive(Debug)]
pub struct Buffer<T> {
    name: String,
    doc: Document<T>,
    listed: bool,
    /// What the winbar of windows showing it says, if it has one (D10).
    winbar: Option<Vec<(String, crate::hl::Group)>>,
    /// Whether windows showing it start at its end and stay there as it grows, as a chat does (D3).
    follows_end: bool,
}

impl<T> Buffer<T> {
    /// Its name, as `:ls` and the tabline show it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What it shows.
    #[must_use]
    pub fn doc(&self) -> &Document<T> {
        &self.doc
    }

    /// What it shows, to change.
    pub fn doc_mut(&mut self) -> &mut Document<T> {
        &mut self.doc
    }

    /// Whether `:ls`, `:b` and the tabline show it.
    #[must_use]
    pub fn is_listed(&self) -> bool {
        self.listed
    }

    /// Gives windows showing it a winbar saying `title`, as text in highlight groups; `None` takes it away.
    pub fn set_winbar(&mut self, title: Option<Vec<(String, crate::hl::Group)>>) {
        self.winbar = title;
    }

    /// Makes windows showing it start at its end and stay there as it grows, while their cursor is on its last row:
    /// as a chat's timeline does.
    pub fn set_follows_end(&mut self, follows: bool) {
        self.follows_end = follows;
    }

    fn new(name: String, doc: Document<T>, listed: bool) -> Self {
        Self {
            name,
            doc,
            listed,
            winbar: None,
            follows_end: false,
        }
    }
}

/// What keys are for right now.
enum Mode_<A: App> {
    Normal,
    /// Selecting, from the selection's fixed end to the cursor.
    Visual(Selection),
    /// Typing after `:`, `/`, `?`, or a prompt.
    Cmdline(Cmdline<A>),
    /// Lines shown over the screen (messages, or a listing such as `:ls`) until a key continues.
    MorePrompt {
        lines: Vec<Message>,
    },
    /// A picker over the windows, taking keys until something is picked or it is left.
    Picker(Picker<A>),
    /// A question in the command line, answered by the next key.
    Confirm(Confirm<A>),
    /// Typing into the composer of a buffer.
    Insert(BufferId),
}

/// A line for the command line area.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Message {
    text: String,
    kind: MessageKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageKind {
    Info,
    Warning,
    Error,
}

/// The last search, for `n` and `N`.
#[derive(Debug, Clone)]
struct LastSearch {
    pattern: Pattern,
    direction: Direction,
}

/// The editor: everything the kit knows, and the only place that decides what keys and commands do.
#[allow(clippy::struct_excessive_bools)] // Independent states, not a mode in disguise.
pub struct Kit<A: App> {
    /// The terminal's size in cells, as (width, height).
    size: (u16, u16),
    focused: bool,
    keymap: Keymap,
    options: Options,
    theme: Theme,
    buffers: Vec<Buffer<A::Target>>,
    windows: Windows,
    mode: Mode_<A>,
    /// The message shown in the command line until the next key, if any.
    message: Option<Message>,
    /// Every message shown, oldest first, for `:messages`.
    messages: Vec<Message>,
    /// Command lines run, oldest first.
    ex_history: Vec<String>,
    /// Patterns searched for, oldest first.
    search_history: Vec<String>,
    last_search: Option<LastSearch>,
    /// Counts key presses, so a [`Wake`] asked for before the latest key is recognised as stale.
    generation: u64,
    /// Counts changes to a searching picker's query, so a search asked for before the latest is stale.
    search_generation: u64,
    /// Whether the which-key popup is showing.
    which_key: bool,
    /// The card `K` shows, until the next key.
    hover: Option<Document<A::Target>>,
    /// Buffers in the order they were last shown, the latest first.
    recent: Vec<BufferId>,
    /// The pictures asked for.
    pictures: pictures::Pictures,
    /// Whether the last search's matches are shown, with `'hlsearch'`: `:noh` hides them until the next search.
    highlighting: bool,
    /// A cell's size in pixels, across and down: what a picture at its own size takes cells by.
    cell_size: (u16, u16),
    commands: Commands<A>,
    segments: Vec<Segment<A>>,
    /// The application's pickers, by the name `:pick` takes.
    pickers: Vec<(&'static str, picker::Make<A>)>,
    /// Buffers shown in a window since the application last heard, for [`App::shown`].
    shown: Vec<BufferId>,
    /// The buffer whose composer the editor open now was opened from, rather than by the application.
    editor_ours: Option<BufferId>,
    /// The buffers' composers.
    composers: HashMap<BufferId, Composer<A>>,
    /// The windows reading at the end of a buffer that follows it, after the last update.
    at_end: Vec<WindowId>,
    icons: icons::Icons,
}

impl<A: App> fmt::Debug for Kit<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Kit")
            .field("size", &self.size)
            .field("buffers", &self.buffers.len())
            .field("commands", &self.commands)
            .finish_non_exhaustive()
    }
}

impl<A: App> Kit<A> {
    /// A fresh editor showing `doc` as a buffer called `name`, listed or not, in one window.
    #[must_use]
    pub fn new(name: impl Into<String>, doc: Document<A::Target>, listed: bool) -> Self {
        Self {
            size: (80, 24),
            focused: true,
            keymap: Keymap::new(),
            options: Options::default(),
            theme: Theme::new(),
            buffers: vec![Buffer::new(name.into(), doc, listed)],
            windows: Windows::new(Window::new(BufferId(0))),
            mode: Mode_::Normal,
            message: None,
            messages: Vec::new(),
            ex_history: Vec::new(),
            search_history: Vec::new(),
            last_search: None,
            generation: 0,
            search_generation: 0,
            which_key: false,
            hover: None,
            recent: Vec::new(),
            pictures: pictures::Pictures::default(),
            highlighting: false,
            cell_size: DEFAULT_CELL_SIZE,
            commands: Commands::default(),
            segments: Vec::new(),
            pickers: Vec::new(),
            shown: Vec::new(),
            editor_ours: None,
            composers: HashMap::new(),
            at_end: Vec::new(),
            icons: icons::Icons::default(),
        }
    }

    // --- What an application sets up. ---

    /// The keymap, to bind the application's default keys to its commands.
    pub fn keymap_mut(&mut self) -> &mut Keymap {
        &mut self.keymap
    }

    /// The highlight groups and colorschemes, to define the application's groups.
    pub fn theme_mut(&mut self) -> &mut Theme {
        &mut self.theme
    }

    /// The highlight groups and colorschemes.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Adds one of the application's ex commands.
    ///
    /// # Panics
    ///
    /// When the kit has a command of that name.
    pub fn add_command(&mut self, command: Command<A>) {
        self.commands.add(command);
    }

    /// Makes `name` an icon (D9), drawn as `nerd` with a Nerd Font, else as `plain` (one cell, or empty).
    pub fn define_icon(&mut self, name: &'static str, nerd: &'static str, plain: &'static str) {
        self.icons.define(name, nerd, plain);
    }

    /// The icon `name`, as `'icons'` says to draw it: two cells, its glyph and a space.
    #[must_use]
    pub fn icon(&self, name: &str) -> String {
        self.icons.get(name, self.options.icons)
    }

    /// Tells the kit whether the terminal ships Nerd Font's symbols, for `icons=auto`.
    pub fn set_nerd_font_symbols(&mut self, ships: bool) {
        self.icons.nerd_font_symbols = ships;
    }

    /// Adds a segment of the application's to the statusline.
    pub fn add_segment(&mut self, segment: Segment<A>) {
        self.segments.push(segment);
    }

    /// Runs `line` as an ex command, as typed after `:`: for an application's defaults, such as
    /// `set showtabline=2`.
    ///
    /// # Errors
    ///
    /// The command's error, worded as Vim words it.
    pub fn run_ex(&mut self, app: &mut A, line: &str) -> Result<Effects<A>, String> {
        self.run_line(app, line)
    }

    // --- What the terminal tells. ---

    /// The command lines and searches typed, oldest first.
    pub(crate) fn history(&self) -> crate::history::Entries {
        crate::history::Entries {
            commands: self.ex_history.clone(),
            searches: self.search_history.clone(),
        }
    }

    /// Takes `entries` as the command lines and searches typed before.
    pub(crate) fn set_history(&mut self, entries: crate::history::Entries) {
        self.ex_history = entries.commands;
        self.search_history = entries.searches;
    }

    /// The terminal's size in cells, as (width, height), as last told by [`Msg::Resize`].
    #[must_use]
    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// Takes a cell to be `width` × `height` pixels, as the terminal says, for pictures shown at their own size from
    /// now on; a size of 0 is ignored.
    pub fn set_cell_size(&mut self, width: u16, height: u16) {
        if width > 0 && height > 0 {
            self.cell_size = (width, height);
        }
    }

    /// A cell's size in pixels, across and down.
    #[must_use]
    pub fn cell_size(&self) -> (u16, u16) {
        self.cell_size
    }

    /// Whether the terminal has focus.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    // --- Buffers. ---

    /// Adds a buffer called `name` showing `doc`, listed or not; windows show it once asked to.
    pub fn add_buffer(
        &mut self,
        name: impl Into<String>,
        doc: Document<A::Target>,
        listed: bool,
    ) -> BufferId {
        self.buffers.push(Buffer::new(name.into(), doc, listed));
        BufferId(self.buffers.len() - 1)
    }

    /// The buffer called `name`, if any.
    #[must_use]
    pub fn find_buffer(&self, name: &str) -> Option<BufferId> {
        self.buffers
            .iter()
            .position(|buffer| buffer.name == name)
            .map(BufferId)
    }

    /// Buffer `id`.
    #[must_use]
    pub fn buffer(&self, id: BufferId) -> &Buffer<A::Target> {
        &self.buffers[id.0]
    }

    /// Buffer `id`, to change what it shows.
    pub fn buffer_mut(&mut self, id: BufferId) -> &mut Buffer<A::Target> {
        &mut self.buffers[id.0]
    }

    /// Every buffer, in the order made.
    pub fn buffers(&self) -> impl Iterator<Item = (BufferId, &Buffer<A::Target>)> {
        self.buffers
            .iter()
            .enumerate()
            .map(|(index, buffer)| (BufferId(index), buffer))
    }

    /// Every buffer, to change what they show.
    pub fn buffers_mut(&mut self) -> impl Iterator<Item = (BufferId, &mut Buffer<A::Target>)> {
        self.buffers
            .iter_mut()
            .enumerate()
            .map(|(index, buffer)| (BufferId(index), buffer))
    }

    /// The buffer the current window shows.
    #[must_use]
    pub fn current_buffer(&self) -> BufferId {
        self.windows.current().buffer
    }

    /// Shows `id` in the current window, where the window last left it; [`App::shown`] hears of it.
    pub fn show_buffer(&mut self, id: BufferId) {
        self.show_buffer_at(id, Cursor::new());
    }

    /// As [`Kit::show_buffer`], starting at `start` when the window never showed the buffer. Showing another listed
    /// buffer is a jump, as starting to edit another file is in Vim.
    pub fn show_buffer_at(&mut self, id: BufferId, start: Cursor) {
        let area = self.window_area();
        let window = self.windows.current_mut();
        let shown = &self.buffers[window.buffer.0];
        if id != window.buffer && shown.listed {
            let layout = window.layout(&self.buffers, area);
            window.record_jump(&layout, window.cursor.clone());
        }
        let first = !window.has_shown(id);
        window.switch_to(id, start);
        if first && self.buffers[id.0].follows_end {
            self.cursor_to_end(self.windows.current_id());
        }
        self.visited(id);
        self.shown.push(id);
    }

    /// Shows `id` in window `window` rather than the current one, which stays current.
    pub fn show_buffer_in(&mut self, window: WindowId, id: BufferId) {
        let current = self.windows.current_id();
        if self.windows.focus_id(window) {
            self.show_buffer(id);
            self.windows.focus_id(current);
        }
    }

    // --- Windows. ---

    /// The window the cursor is in.
    #[must_use]
    pub fn current_window(&self) -> WindowId {
        self.windows.current_id()
    }

    /// The windows, top to bottom, left to right.
    #[must_use]
    pub fn window_ids(&self) -> Vec<WindowId> {
        self.windows.ids()
    }

    /// Opens a window showing `buffer` as `placement` says, `size` big, and makes it current: a sidebar is
    /// `open_window(Placement::FarLeft, Size::Cells(30), sidebar)`.
    pub fn open_window(&mut self, placement: Placement, size: Size, buffer: BufferId) -> WindowId {
        let id = self
            .windows
            .open(placement, size, window::Window::new(buffer));
        if self.buffers[buffer.0].follows_end {
            self.cursor_to_end(id);
        }
        self.shown.push(buffer);
        id
    }

    /// Makes `id` the current window; whether it is open.
    pub fn focus_window(&mut self, id: WindowId) -> bool {
        self.windows.focus_id(id)
    }

    /// Closes window `id`, unless it is the last; whether it closed.
    pub fn close_window_id(&mut self, id: WindowId) -> bool {
        self.windows.close_id(id)
    }

    /// The buffer window `id` shows, if it is open.
    #[must_use]
    pub fn window_buffer(&self, id: WindowId) -> Option<BufferId> {
        self.windows.find(id).map(|window| window.buffer)
    }

    /// The windows showing `buffer`, in tree order.
    #[must_use]
    pub fn windows_showing(&self, buffer: BufferId) -> Vec<WindowId> {
        self.windows
            .ids()
            .into_iter()
            .filter(|&id| self.window_buffer(id) == Some(buffer))
            .collect()
    }

    /// Gives window `id` `size`.
    pub fn set_window_size(&mut self, id: WindowId, size: Size) {
        self.windows.resize(id, size);
    }

    /// The text area of window `id`, as laid out now.
    fn window_text_area(&self, id: WindowId) -> Option<Rect> {
        let area = self.windows_area();
        let rect = self
            .windows
            .arrange(area)
            .windows
            .into_iter()
            .find(|(window, _)| *window == id)?
            .1;
        let buffer = self.windows.find(id)?.buffer;
        Some(self.text_area(rect, area, buffer))
    }

    /// Puts the cursor of window `id` on the last row of its buffer.
    fn cursor_to_end(&mut self, id: WindowId) {
        let Some(area) = self.window_text_area(id) else {
            return;
        };
        let viewport = Viewport {
            height: usize::from(area.height),
            scrolloff: self.options.scrolloff,
        };
        let buffers = &self.buffers;
        let Some(window) = self.windows.find_mut(id) else {
            return;
        };
        let layout = window.layout(buffers, area);
        window.cursor = Cursor::at_row(&layout, layout.row_count().saturating_sub(1));
        window.follow_cursor(&layout, viewport);
    }

    /// The windows showing a buffer that follows its end whose cursor is on its last row: those to keep there.
    fn windows_at_end(&self) -> Vec<WindowId> {
        self.windows
            .ids()
            .into_iter()
            .filter(|&id| {
                let Some(window) = self.windows.find(id) else {
                    return false;
                };
                if !self.buffers[window.buffer.0].follows_end {
                    return false;
                }
                let Some(area) = self.window_text_area(id) else {
                    return false;
                };
                let layout = window.layout(&self.buffers, area);
                window.cursor.screen(&layout).row + 1 >= layout.row_count()
            })
            .collect()
    }

    /// The first row the current window shows of `id`, when it shows it: what keeps the view in place as an
    /// application replaces the text above it.
    #[must_use]
    pub fn top_row(&self, id: BufferId) -> Option<usize> {
        let window = self.windows.current();
        (window.buffer == id).then(|| {
            let layout = window.layout(&self.buffers, self.window_area());
            window.top(&layout, self.viewport())
        })
    }

    /// Whether the current window's cursor is within a window's height of the end of what its buffer shows: time to
    /// load more of a long list.
    #[must_use]
    pub fn cursor_near_end(&self) -> bool {
        let area = self.window_area();
        let window = self.windows.current();
        let layout = window.layout(&self.buffers, area);
        window.cursor.screen(&layout).row + usize::from(area.height) >= layout.row_count()
    }

    /// What the text under the current window's cursor refers to, then what the block holding it refers to as a
    /// whole: each if anything.
    #[must_use]
    pub fn targets_under_cursor(&self) -> [Option<&A::Target>; 2] {
        let window = self.windows.current();
        let layout = window.layout(&self.buffers, self.window_area());
        [
            window.cursor.target(&layout),
            window.cursor.block_target(&layout),
        ]
    }

    /// What the text under the current window's cursor refers to, if anything.
    #[must_use]
    pub fn target_under_cursor(&self) -> Option<A::Target> {
        self.targets_under_cursor()[0].cloned()
    }

    /// The text of the current window's cursor row, as shown.
    #[must_use]
    pub fn cursor_row_text(&self) -> String {
        let window = self.windows.current();
        let layout = window.layout(&self.buffers, self.window_area());
        let row = window.cursor.screen(&layout).row;
        layout.segments(row).map(|(text, _)| text).collect()
    }

    /// The listed buffers, in order: what `:ls`, `:b` and the tabline count.
    #[must_use]
    pub fn listed(&self) -> Vec<BufferId> {
        self.buffers()
            .filter(|(_, buffer)| buffer.listed)
            .map(|(id, _)| id)
            .collect()
    }

    // --- Messages, questions and floats. ---

    /// Shows `text` in the command line, and keeps it for `:messages`.
    pub fn info(&mut self, text: impl Into<String>) {
        self.say(text.into(), MessageKind::Info);
    }

    /// Shows the warning `text`.
    pub fn warn(&mut self, text: impl Into<String>) {
        self.say(text.into(), MessageKind::Warning);
    }

    /// Shows the error `text`.
    pub fn error(&mut self, text: impl Into<String>) {
        self.say(text.into(), MessageKind::Error);
    }

    fn say(&mut self, text: String, kind: MessageKind) {
        let message = Message { text, kind };
        self.messages.push(message.clone());
        self.message = Some(message);
    }

    /// Asks `question` in the command line, `shown` above it when given; the next key answers. `then` hears the
    /// key's character, or `'\0'` for a key that is none.
    pub fn confirm(
        &mut self,
        question: impl Into<String>,
        shown: Option<Document<A::Target>>,
        then: impl FnOnce(&mut A, &mut Kit<A>, char) -> Effects<A> + 'static,
    ) {
        self.mode = Mode_::Confirm(Confirm::new(question.into(), shown, Box::new(then)));
    }

    /// Asks for a line of text after `prompt` in the command line, shown as `*`s when `masked` (for a passphrase,
    /// never remembered); `then` hears it once typed.
    pub fn prompt(
        &mut self,
        prompt: impl Into<String>,
        masked: bool,
        then: impl FnOnce(&mut A, &mut Kit<A>, String) -> Effects<A> + 'static,
    ) {
        let purpose = Purpose::Prompt {
            prompt: prompt.into(),
            masked,
            then: Some(Box::new(then)),
        };
        self.mode = Mode_::Cmdline(Cmdline::new(purpose, None));
    }

    /// Shows `doc` in a card next to the cursor, until the next key, as `K` does in Neovim.
    pub fn show_hover(&mut self, doc: Document<A::Target>) {
        self.hover = Some(doc);
    }

    /// The card shown next to the cursor, if any, to change.
    pub fn hover_mut(&mut self) -> Option<&mut Document<A::Target>> {
        self.hover.as_mut()
    }

    /// Takes away the card shown next to the cursor.
    pub fn close_hover(&mut self) {
        self.hover = None;
    }

    /// Opens `picker` over the windows; one that searches searches for its query at once.
    pub fn open_picker(&mut self, app: &mut A, picker: Picker<A>) -> Effects<A> {
        let search = picker.wants_search();
        self.mode = Mode_::Picker(picker);
        self.search_generation += 1;
        if search {
            return self.search_now_with(app);
        }
        Vec::new()
    }

    /// The picker open, if any, to change what it lists, as when a search answers.
    pub fn picker_mut(&mut self) -> Option<&mut Picker<A>> {
        match &mut self.mode {
            Mode_::Picker(picker) => Some(picker),
            _ => None,
        }
    }

    /// Closes the picker open, if any.
    pub fn close_picker(&mut self) {
        if let Mode_::Picker(_) = self.mode {
            self.mode = Mode_::Normal;
        }
    }

    // --- Reacting. ---

    /// Reacts to `msg`, returning what the runtime must do.
    pub fn update(&mut self, app: &mut A, msg: Msg<A::Msg>) -> Effects<A> {
        let keyed = matches!(msg, Msg::Key(_) | Msg::Paste(_));
        let at_end = std::mem::take(&mut self.at_end);
        let mut effects = self.react(app, msg);
        if !keyed {
            // What arrived went below where the cursor was reading: it goes on reading at the end.
            for id in at_end {
                self.cursor_to_end(id);
            }
        }
        if !effects.contains(&Effect::Quit) {
            effects.extend(self.notify_shown(app));
            effects.extend(self.pictures_in_view());
        }
        self.at_end = self.windows_at_end();
        effects
    }

    /// The effects to start with, once the runtime is ready.
    pub fn start(&mut self, app: &mut A) -> Effects<A> {
        let mut effects = Vec::new();
        if self.options.mouse {
            effects.push(Effect::CaptureMouse(true));
        }
        effects.extend(app.start(self));
        effects.extend(self.notify_shown(app));
        effects
    }

    /// Tells the application of the buffers shown since it last heard.
    fn notify_shown(&mut self, app: &mut A) -> Effects<A> {
        let mut effects = Vec::new();
        while !self.shown.is_empty() {
            for id in std::mem::take(&mut self.shown) {
                effects.extend(app.shown(self, id));
            }
        }
        effects
    }

    fn react(&mut self, app: &mut A, msg: Msg<A::Msg>) -> Effects<A> {
        match msg {
            Msg::Key(key) => {
                self.generation += 1;
                self.which_key = false;
                let mut effects = self.key(app, key);
                effects.extend(self.wake_ups());
                effects.extend(self.notify_shown(app));
                effects.extend(app.keyed(self));
                effects
            }
            Msg::Mouse { mouse, column, row } => {
                self.generation += 1;
                self.which_key = false;
                self.message = None;
                let mut effects = self.mouse(app, mouse, column, row);
                effects.extend(self.notify_shown(app));
                effects.extend(app.keyed(self));
                effects
            }
            Msg::Wake(token) if token.generation != self.generation_of(token.purpose) => Vec::new(),
            Msg::Wake(Wake { purpose, .. }) => match purpose {
                WakePurpose::KeyTimeout => {
                    self.which_key = false;
                    let ran = self.keymap.timeout(self.keymap_mode());
                    self.run_all(app, ran)
                }
                WakePurpose::WhichKey => {
                    self.which_key = !self.keymap.pending_keys().is_empty();
                    Vec::new()
                }
                WakePurpose::Search => self.search_now_with(app),
                WakePurpose::Frame => {
                    self.next_frames();
                    Vec::new()
                }
            },
            Msg::Paste(text) => match &mut self.mode {
                Mode_::Cmdline(cmdline) => {
                    cmdline.paste(&text);
                    Vec::new()
                }
                Mode_::Picker(_) => self.picker_paste(app, &text),
                Mode_::Insert(id) => {
                    let id = *id;
                    self.insert_paste(app, id, &text);
                    Vec::new()
                }
                _ => Vec::new(),
            },
            Msg::Sourced { path, contents } => self.source(app, &path, contents),
            Msg::Edited(saved) => {
                if let Some(id) = self.editor_ours.take() {
                    self.composer_edited(id, saved);
                    Vec::new()
                } else {
                    app.edited(self, saved)
                }
            }
            Msg::Picture {
                source,
                cols,
                rows,
                result,
            } => self.picture_arrived(source, cols, rows, result),
            Msg::FocusGained => {
                self.focused = true;
                app.focus(self, true)
            }
            Msg::FocusLost => {
                self.focused = false;
                app.focus(self, false)
            }
            Msg::Resize { width, height } => {
                self.size = (width, height);
                app.resized(self, width, height)
            }
            Msg::Failed(text) => {
                self.error(text);
                Vec::new()
            }
            Msg::App(msg) => app.update(self, msg),
        }
    }

    fn key(&mut self, app: &mut A, key: Key) -> Effects<A> {
        if let Mode_::Cmdline(cmdline) = &self.mode
            && cmdline.purpose.is_ex()
            && key.code == Code::Tab
        {
            self.complete_command_line(app, !key.mods.contains(Mods::SHIFT));
            return Vec::new();
        }
        match &mut self.mode {
            Mode_::Normal | Mode_::Visual(_) => {
                self.message = None;
                // As Neovim's hover window, the card goes with the next key, which also does its usual job.
                self.hover = None;
                let ran = self.keymap.feed(self.keymap_mode(), key);
                self.run_all(app, ran)
            }
            Mode_::MorePrompt { .. } => {
                self.mode = Mode_::Normal;
                // As in Vim, <CR>, <Space> and <Esc> only dismiss; any other key also does its usual job.
                let dismiss_only = [Key::new(Code::Enter), Key::char(' '), Key::new(Code::Esc)];
                if dismiss_only.contains(&key) {
                    Vec::new()
                } else {
                    self.key(app, key)
                }
            }
            Mode_::Picker(_) => self.picker_key(app, key),
            Mode_::Confirm(_) => self.confirm_key(app, key),
            Mode_::Insert(id) => {
                let id = *id;
                self.insert_key(app, id, key)
            }
            Mode_::Cmdline(cmdline) => {
                let history: &[String] = match cmdline.purpose {
                    Purpose::Ex => &self.ex_history,
                    Purpose::Search(_) => &self.search_history,
                    Purpose::Prompt { .. } => &[],
                };
                match cmdline.key(key, history) {
                    Edit::Editing => Vec::new(),
                    Edit::Cancel => {
                        self.mode = Mode_::Normal;
                        Vec::new()
                    }
                    Edit::Submit(text) => {
                        let Mode_::Cmdline(cmdline) =
                            std::mem::replace(&mut self.mode, Mode_::Normal)
                        else {
                            unreachable!("the mode was just matched as the command line");
                        };
                        let count = cmdline.count;
                        match cmdline.purpose {
                            Purpose::Ex => {
                                remember(&mut self.ex_history, &text);
                                self.ex(app, &text)
                            }
                            Purpose::Search(direction) => {
                                remember(&mut self.search_history, &text);
                                self.search(&text, direction, count.unwrap_or(1));
                                Vec::new()
                            }
                            Purpose::Prompt { then, .. } => match then {
                                Some(then) => then(app, self, text),
                                None => Vec::new(),
                            },
                        }
                    }
                }
            }
        }
    }

    /// The confirmation asked: the next key answers it.
    fn confirm_key(&mut self, app: &mut A, key: Key) -> Effects<A> {
        let Mode_::Confirm(confirm) = std::mem::replace(&mut self.mode, Mode_::Normal) else {
            return Vec::new();
        };
        let answer = match key.code {
            Code::Char(answer) if key.mods == Mods::NONE => answer,
            _ => '\0',
        };
        confirm.answer(app, self, answer)
    }

    fn run_all(&mut self, app: &mut A, ran: Vec<Keys>) -> Effects<A> {
        ran.into_iter()
            .flat_map(|invocation| self.run(app, invocation))
            .collect()
    }

    /// The wake-ups pending keys need: the which-key popup, and the timeout of an ambiguous sequence.
    fn wake_ups(&self) -> Effects<A> {
        let in_keymap_mode = matches!(self.mode, Mode_::Normal | Mode_::Visual(_));
        if !in_keymap_mode || self.keymap.pending_keys().is_empty() {
            return Vec::new();
        }
        let wake = |purpose| Wake {
            purpose,
            generation: self.generation,
        };
        let mut wakes = vec![Effect::Wake {
            after: WHICH_KEY_DELAY,
            token: wake(WakePurpose::WhichKey),
        }];
        if self.keymap.is_ambiguous() {
            wakes.push(Effect::Wake {
                after: Duration::from_millis(self.options.timeoutlen),
                token: wake(WakePurpose::KeyTimeout),
            });
        }
        wakes
    }

    /// The current generation of wake-ups for `purpose`: tokens of an older one are stale.
    fn generation_of(&self, purpose: WakePurpose) -> u64 {
        match purpose {
            WakePurpose::KeyTimeout | WakePurpose::WhichKey => self.generation,
            WakePurpose::Search => self.search_generation,
            WakePurpose::Frame => self.pictures.frame_generation(),
        }
    }

    /// Notes buffer `id` as the last shown.
    fn visited(&mut self, id: BufferId) {
        self.recent.retain(|&recent| recent != id);
        self.recent.insert(0, id);
    }

    /// After the current window jumped from buffer `before`: keeps its cursor in view, and tells the application
    /// of the buffer it jumped to, if it is another.
    fn jumped(&mut self, before: BufferId) {
        self.follow_cursor();
        let now = self.windows.current().buffer;
        if now != before {
            self.visited(now);
            self.shown.push(now);
        }
    }

    /// `:pop` (`<C-t>`): back to where the window was when `:open` or a command like it last left for another
    /// buffer.
    fn pop_tag(&mut self) -> Result<(), String> {
        let area = self.window_area();
        let window = self.windows.current_mut();
        let before = window.buffer;
        let layout = window.layout(&self.buffers, area);
        if !window.pop_tag(&layout) {
            return Err("E73: Tag stack empty".to_owned());
        }
        self.jumped(before);
        Ok(())
    }

    /// `:jumps`: the jumplist as Vim lists it: how far each entry is from where `<C-o>` and `<C-i>` last went, its
    /// row and column, and the text there, or the name of its buffer when it is another; `>` marks where one is.
    fn list_jumps(&mut self) {
        let area = self.window_area();
        let window = self.windows.current();
        let (jumps, at) = window.jumplist();
        let mut lines = vec![" jump line  col file/text".to_owned()];
        for (i, jump) in jumps.iter().enumerate() {
            let layout = window.layout_of(&self.buffers, jump.buffer, area);
            let shown = jump.cursor.screen(&layout);
            let text = if jump.buffer == window.buffer {
                layout.segments(shown.row).map(|(text, _)| text).collect()
            } else {
                self.buffers[jump.buffer.0].name.clone()
            };
            lines.push(format!(
                "{} {:2} {:5} {:4} {text}",
                if i == at { '>' } else { ' ' },
                i.abs_diff(at),
                shown.row + 1,
                shown.col,
            ));
        }
        if at == jumps.len() {
            lines.push(">".to_owned());
        }
        let lines = lines
            .into_iter()
            .map(|text| Message {
                text,
                kind: MessageKind::Info,
            })
            .collect();
        self.mode = Mode_::MorePrompt { lines };
    }

    /// The search being typed after `/` or `?`, with `'incsearch'`: its pattern, direction and count. `None` while
    /// it is empty or not a valid pattern yet.
    fn typed_search(&self) -> Option<(Pattern, Direction, usize)> {
        let Mode_::Cmdline(cmdline) = &self.mode else {
            return None;
        };
        let Purpose::Search(direction) = cmdline.purpose else {
            return None;
        };
        let text = cmdline.text();
        if !self.options.incsearch || text.is_empty() {
            return None;
        }
        let pattern = Pattern::new(&text, self.options.case_mode()).ok()?;
        Some((pattern, direction, cmdline.count.unwrap_or(1)))
    }

    /// The last search's pattern, while its matches are shown: with `'hlsearch'`, and not since `:noh`.
    fn searched(&self) -> Option<&Pattern> {
        self.last_search
            .as_ref()
            .filter(|_| self.highlighting)
            .map(|search| &search.pattern)
    }

    /// The alternate buffer, as Vim calls it: the listed buffer shown last before the current one.
    fn alternate(&self) -> Option<BufferId> {
        let current = self.windows.current().buffer;
        self.recent
            .iter()
            .copied()
            .find(|&id| id != current && self.buffers[id.0].listed)
    }

    /// `:ls`: the listed buffers, the current one marked `%a` and the alternate one `#`.
    fn list_buffers(&mut self) {
        let current = self.windows.current().buffer;
        let alternate = self.alternate();
        let lines = self
            .listed()
            .into_iter()
            .enumerate()
            .map(|(number, id)| Message {
                text: format!(
                    "{:>3} {:2} \"{}\"",
                    number + 1,
                    if id == current {
                        "%a"
                    } else if Some(id) == alternate {
                        "#"
                    } else {
                        ""
                    },
                    self.buffers[id.0].name
                ),
                kind: MessageKind::Info,
            })
            .collect();
        self.mode = Mode_::MorePrompt { lines };
    }

    /// `:b {arg}`: the alternate buffer for `#`, else the listed buffer numbered `arg`, else named exactly `arg`,
    /// else the only one whose name contains it.
    fn buffer_named(&self, arg: &str) -> Result<BufferId, String> {
        if arg == "#" {
            return self
                .alternate()
                .ok_or_else(|| "E23: No alternate file".to_owned());
        }
        let listed = self.listed();
        if let Ok(number) = arg.parse::<usize>() {
            return listed
                .get(number.wrapping_sub(1))
                .copied()
                .ok_or_else(|| format!("E86: Buffer {number} does not exist"));
        }
        if let Some(&id) = listed.iter().find(|&&id| self.buffers[id.0].name == arg) {
            return Ok(id);
        }
        let matching: Vec<BufferId> = listed
            .into_iter()
            .filter(|&id| self.buffers[id.0].name.contains(arg))
            .collect();
        match matching.as_slice() {
            [id] => Ok(*id),
            [] => Err(format!("E94: No matching buffer for {arg}")),
            _ => Err(format!("E93: More than one match for {arg}")),
        }
    }

    /// `:bn` and `:bp`: the next or previous listed buffer, wrapping around.
    fn cycle_buffer(&mut self, forward: bool) {
        let listed = self.listed();
        if listed.is_empty() {
            return;
        }
        let current = self.windows.current().buffer;
        let at = listed.iter().position(|&id| id == current).unwrap_or(0);
        let next = if forward {
            (at + 1) % listed.len()
        } else {
            (at + listed.len() - 1) % listed.len()
        };
        self.show_buffer(listed[next]);
    }

    /// The keymap table for the current mode.
    fn keymap_mode(&self) -> Mode {
        match self.mode {
            Mode_::Visual(_) => Mode::Visual,
            _ => Mode::Normal,
        }
    }

    fn run(&mut self, app: &mut A, invocation: Keys) -> Effects<A> {
        match invocation.action {
            Action::Visual(kind) => {
                self.visual(kind);
                return Vec::new();
            }
            Action::Yank => return self.yank(),
            Action::Escape => {
                self.mode = Mode_::Normal;
                return Vec::new();
            }
            Action::Fold(op) => {
                let window = self.windows.current_mut();
                let doc = &self.buffers[window.buffer.0].doc;
                window.folds.fold(doc, &window.cursor, op);
            }
            Action::FoldAll { open } => {
                let window = self.windows.current_mut();
                window
                    .folds
                    .fold_all(&self.buffers[window.buffer.0].doc, open);
            }
            _ => return self.run_in_window(app, invocation),
        }
        self.follow_cursor();
        Vec::new()
    }

    /// `v` and `V`: start Visual mode, switch its kind, or leave it when it already is that kind.
    fn visual(&mut self, kind: SelectionKind) {
        self.mode = match std::mem::replace(&mut self.mode, Mode_::Normal) {
            Mode_::Visual(selection) if selection.kind() == kind => Mode_::Normal,
            Mode_::Visual(selection) => Mode_::Visual(selection.with_kind(kind)),
            _ => {
                let area = self.window_area();
                let layout = self.windows.current().layout(&self.buffers, area);
                Mode_::Visual(Selection::start(
                    &self.windows.current().cursor,
                    &layout,
                    kind,
                ))
            }
        };
    }

    /// `y` in Visual mode and `yy` in Normal mode: copies the text and, as Vim does, leaves the cursor at its start.
    fn yank(&mut self) -> Effects<A> {
        let area = self.window_area();
        let layout = self.windows.current().layout(&self.buffers, area);
        let cursor = &mut self.windows.current_mut().cursor;
        let selection = match std::mem::replace(&mut self.mode, Mode_::Normal) {
            Mode_::Visual(selection) => selection,
            _ => Selection::start(cursor, &layout, SelectionKind::Rowwise),
        };
        let text = selection.text(cursor, &layout);
        *cursor = selection.start_cursor(cursor, &layout);
        let lines = text.trim_end_matches('\n').matches('\n').count() + 1;
        // Vim reports yanks of more lines than 'report', which is 2 by default.
        if lines > 2 {
            self.info(format!("{lines} lines yanked"));
        }
        self.follow_cursor();
        vec![Effect::Copy(text)]
    }

    /// Keeps the cursor of the current window in view.
    fn follow_cursor(&mut self) {
        let viewport = self.viewport();
        let area = self.window_area();
        let layout = self.windows.current().layout(&self.buffers, area);
        self.windows.current_mut().follow_cursor(&layout, viewport);
    }

    /// Opens the folds hiding the cursor, as Vim's `'foldopen'` does after a search.
    fn open_fold_at_cursor(&mut self) {
        let window = self.windows.current_mut();
        let doc = &self.buffers[window.buffer.0].doc;
        window.folds.reveal(doc, &window.cursor);
        self.follow_cursor();
    }

    fn run_in_window(&mut self, app: &mut A, invocation: Keys) -> Effects<A> {
        let count = invocation.count.unwrap_or(1);
        let viewport = self.viewport();
        let area = self.window_area();
        let window = self.windows.current_mut();
        let layout = window.layout(&self.buffers, area);
        // Searches and these motions are jumps: where they start goes into the jumplist, as in Vim.
        let from = window.cursor.clone();
        match invocation.action {
            Action::Motion(motion) => {
                window.cursor.apply(&layout, motion, count);
                if is_jump(motion) {
                    window.record_jump(&layout, from);
                }
                window.follow_cursor(&layout, viewport);
            }
            Action::JumpOlder => {
                let before = window.buffer;
                window.jump_older(&layout, count);
                self.jumped(before);
            }
            Action::JumpNewer => {
                let before = window.buffer;
                window.jump_newer(count);
                self.jumped(before);
            }
            Action::Scroll(scroll) => window.scroll(&layout, viewport, scroll, count),
            Action::Place(place) => window.place(&layout, viewport, place),
            Action::ViewRow(place) => {
                window.view_row(&layout, viewport, place, count);
                window.record_jump(&layout, from);
            }
            Action::SearchWord(direction) => {
                let case = self.options.case_mode();
                if let Some((pattern, outcome)) =
                    window.cursor.search_word(&layout, direction, count, case)
                {
                    window.record_jump(&layout, from);
                    self.last_search = Some(LastSearch { pattern, direction });
                    self.highlighting = true;
                    self.open_fold_at_cursor();
                    self.report_search(outcome, direction, "");
                } else {
                    self.error("E348: No string under cursor");
                }
            }
            Action::SearchNext { reverse } => match self.last_search.clone() {
                Some(LastSearch { pattern, direction }) => {
                    let direction = if reverse {
                        opposite(direction)
                    } else {
                        direction
                    };
                    let outcome = window.cursor.search(&layout, &pattern, direction, count);
                    if outcome != SearchOutcome::NotFound {
                        window.record_jump(&layout, from);
                    }
                    // A search again: its matches show again, after `:noh`.
                    self.highlighting = true;
                    self.open_fold_at_cursor();
                    self.report_search(outcome, direction, "");
                }
                None => self.error("E35: No previous regular expression"),
            },
            Action::Cmdline(kind) => {
                self.mode = Mode_::Cmdline(Cmdline::new(kind.into(), invocation.count));
            }
            Action::Keys { keys, remap } => {
                return keys
                    .into_iter()
                    .flat_map(|key| self.replay(app, key, remap))
                    .collect();
            }
            Action::Ex(line) => return self.ex(app, &line),
            Action::Error(text) => self.error(text),
            _ => {}
        }
        Vec::new()
    }

    /// Feeds one key of an [`Action::Keys`] to whatever mode is current now.
    fn replay(&mut self, app: &mut A, key: Key, remap: bool) -> Effects<A> {
        match self.mode {
            Mode_::Normal | Mode_::Visual(_) => {
                let ran = self.keymap.replay(self.keymap_mode(), &[key], remap);
                self.run_all(app, ran)
            }
            Mode_::Cmdline(_)
            | Mode_::MorePrompt { .. }
            | Mode_::Picker(_)
            | Mode_::Confirm(_)
            | Mode_::Insert(_) => self.key(app, key),
        }
    }

    /// Runs an ex command line, reporting its error in the message area.
    fn ex(&mut self, app: &mut A, line: &str) -> Effects<A> {
        match self.run_line(app, line) {
            Ok(effects) => effects,
            Err(error) => {
                self.error(error);
                Vec::new()
            }
        }
    }

    /// Parses and runs one command line; blank lines and comments do nothing.
    fn run_line(&mut self, app: &mut A, line: &str) -> Result<Effects<A>, String> {
        let Some(split) = ex::split(line) else {
            return Ok(Vec::new());
        };
        let trimmed = line
            .trim_start_matches(|c: char| c == ':' || c.is_whitespace())
            .trim_end();
        let (name, found) = self
            .commands
            .lookup(split.name)
            .ok_or_else(|| ExError::NotACommand(trimmed.to_owned()))?;
        commands::check_bang(&found, split.bang)?;
        match found {
            Found::Builtin(builtin) => self.builtin(app, builtin, split.bang, split.args),
            Found::App(command) => {
                let invocation = Invocation {
                    name,
                    bang: split.bang,
                    args: split.args,
                };
                if command.jumps {
                    self.following(app, |app, kit| (command.run)(app, kit, invocation))
                } else {
                    (command.run)(app, self, invocation)
                }
            }
        }
    }

    /// Runs `follow` (`:open` and commands like it), noting where the window was on the tag stack if it shows
    /// another buffer after.
    fn following(
        &mut self,
        app: &mut A,
        follow: impl FnOnce(&mut A, &mut Self) -> Result<Effects<A>, String>,
    ) -> Result<Effects<A>, String> {
        let from = self.windows.current().here();
        let outcome = follow(app, self);
        self.windows.current_mut().push_tag(from);
        outcome
    }

    #[allow(clippy::too_many_lines)] // One arm per command, each short.
    fn builtin(
        &mut self,
        app: &mut A,
        builtin: Builtin,
        bang: bool,
        args: &str,
    ) -> Result<Effects<A>, String> {
        let _ = bang;
        match builtin {
            Builtin::Quit if self.windows.count() > 1 => {
                self.windows.close();
            }
            Builtin::Quit | Builtin::QuitAll => {
                let mut effects = self.pictures.clear();
                effects.push(Effect::Quit);
                return Ok(effects);
            }
            Builtin::Split(vertical) => {
                self.windows
                    .split(if vertical { Axis::Columns } else { Axis::Rows });
            }
            Builtin::Close => self.close_window()?,
            Builtin::Only => self.windows.only(),
            Builtin::Wincmd => {
                let mut chars = ex::required(args)?.chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None) => return self.wincmd(app, ch),
                    _ => return Err(ExError::InvalidArgument(args.to_owned()).into()),
                }
            }
            Builtin::Set => {
                let (graphics, mouse) = (self.options.graphics, self.options.mouse);
                for arg in &ex::set_args(args) {
                    if let Some(shown) = self.options.set(arg, &mut self.keymap)? {
                        self.info(shown);
                    }
                }
                let mut effects = Vec::new();
                if self.options.mouse != mouse {
                    effects.push(Effect::CaptureMouse(self.options.mouse));
                }
                if self.options.graphics != graphics {
                    // Drawn otherwise now: what was sent goes, and pictures are asked for again.
                    effects.extend(self.pictures.clear());
                }
                return Ok(effects);
            }
            Builtin::Map(modes, remap) => {
                let (lhs, rhs) = ex::map_args(args).ok_or(ExError::ArgumentRequired)?;
                for &mode in modes {
                    self.keymap.map(mode, lhs, rhs, remap);
                }
            }
            Builtin::Unmap(modes) => {
                let lhs = ex::required(args)?;
                let mut removed = false;
                for &mode in modes {
                    removed |= self.keymap.unmap(mode, lhs);
                }
                if !removed {
                    return Err("E31: No such mapping".to_owned());
                }
            }
            // Like Vim, nothing to show means no prompt either.
            Builtin::Help => self.help((!args.is_empty()).then_some(args))?,
            Builtin::Messages if self.messages.is_empty() => {}
            Builtin::Messages => {
                self.mode = Mode_::MorePrompt {
                    lines: self.messages.clone(),
                };
            }
            Builtin::Highlight => match ex::highlight(args)? {
                ex::Highlight::Set { group, attrs } => {
                    if let Some(shown) = self.theme.highlight(&group, &attrs)? {
                        self.info(shown);
                    }
                }
                ex::Highlight::Clear => self.theme.clear(),
                ex::Highlight::Link { from, to } => self.theme.link(&from, to.as_deref())?,
            },
            Builtin::Colorscheme if args.is_empty() => self.info(self.theme.name().to_owned()),
            Builtin::Colorscheme => self.theme.colorscheme(args)?,
            Builtin::Source => return Ok(vec![Effect::Source(ex::required(args)?.to_owned())]),
            Builtin::Edit if args.is_empty() => {
                let current = self.windows.current().buffer;
                return app
                    .reload(self, current)
                    .ok_or_else(|| "Nothing to reload here".to_owned());
            }
            Builtin::Edit => return self.edit(app, args),
            Builtin::Buffers => self.list_buffers(),
            Builtin::Buffer => {
                let id = self.buffer_named(ex::required(args)?)?;
                self.show_buffer(id);
            }
            Builtin::NextBuffer(forward) => self.cycle_buffer(forward),
            Builtin::Open => {
                return self.following(app, |app, kit| match kit.target_under_cursor() {
                    Some(target) => app.open(kit, target),
                    None => Ok(Vec::new()),
                });
            }
            Builtin::Pop => self.pop_tag()?,
            Builtin::Jumps => self.list_jumps(),
            Builtin::Nohlsearch => self.highlighting = false,
            Builtin::Pick => return self.pick(app, args),
            Builtin::StartInsert => self.start_insert()?,
        }
        Ok(Vec::new())
    }

    /// `:e {name}`: shows the buffer of that name, or else what the application makes of the name.
    fn edit(&mut self, app: &mut A, name: &str) -> Result<Effects<A>, String> {
        if let Some(id) = self.find_buffer(name) {
            self.show_buffer(id);
            return Ok(Vec::new());
        }
        app.edit(self, name)
    }

    /// Runs the lines of a sourced file, carrying on after errors and then listing them as Vim does.
    fn source(&mut self, app: &mut A, path: &str, contents: Result<String, String>) -> Effects<A> {
        let Ok(contents) = contents else {
            self.error(format!("E484: Can't open file {path}"));
            return Vec::new();
        };
        let mut effects = Vec::new();
        let mut errors = Vec::new();
        for (number, line) in (1..).zip(contents.lines()) {
            match self.run_line(app, line) {
                Ok(more) => effects.extend(more),
                Err(error) => errors.push((number, error)),
            }
        }
        if !errors.is_empty() {
            let from = self.messages.len();
            self.error(format!("Error detected while processing {path}:"));
            for (number, error) in errors {
                self.info(format!("line {number}:"));
                self.error(error);
            }
            self.mode = Mode_::MorePrompt {
                lines: self.messages[from..].to_vec(),
            };
        }
        effects
    }

    fn close_window(&mut self) -> Result<(), String> {
        if self.windows.close() {
            Ok(())
        } else {
            Err("E444: Cannot close last window".to_owned())
        }
    }

    /// `:wincmd {ch}`, which `<C-w>{ch}` runs.
    fn wincmd(&mut self, app: &mut A, ch: char) -> Result<Effects<A>, String> {
        let area = self.windows_area();
        match ch {
            'h' => self.windows.focus(area, Toward::Left),
            'j' => self.windows.focus(area, Toward::Down),
            'k' => self.windows.focus(area, Toward::Up),
            'l' => self.windows.focus(area, Toward::Right),
            'w' => self.windows.cycle(true),
            'W' => self.windows.cycle(false),
            's' => self.windows.split(Axis::Rows),
            'v' => self.windows.split(Axis::Columns),
            'c' => self.close_window()?,
            'o' => self.windows.only(),
            'q' => return self.run_line(app, "quit"),
            _ => {}
        }
        Ok(Vec::new())
    }

    /// `/` and `?` submitted with `text`; an empty pattern repeats the last one.
    fn search(&mut self, text: &str, direction: Direction, count: usize) {
        let pattern = if text.is_empty() {
            match &self.last_search {
                Some(last) => last.pattern.clone(),
                None => return self.error("E35: No previous regular expression"),
            }
        } else {
            match Pattern::new(text, self.options.case_mode()) {
                Ok(pattern) => pattern,
                Err(error) => return self.error(error.to_string()),
            }
        };
        let area = self.window_area();
        let window = self.windows.current_mut();
        let layout = window.layout(&self.buffers, area);
        let from = window.cursor.clone();
        let outcome = window.cursor.search(&layout, &pattern, direction, count);
        if outcome != SearchOutcome::NotFound {
            window.record_jump(&layout, from);
        }
        self.last_search = Some(LastSearch { pattern, direction });
        self.highlighting = true;
        self.open_fold_at_cursor();
        self.report_search(outcome, direction, text);
    }

    fn report_search(&mut self, outcome: SearchOutcome, direction: Direction, text: &str) {
        match (outcome, direction) {
            (SearchOutcome::Found, _) => {}
            (SearchOutcome::Wrapped, Direction::Forward) => {
                self.warn("search hit BOTTOM, continuing at TOP");
            }
            (SearchOutcome::Wrapped, Direction::Backward) => {
                self.warn("search hit TOP, continuing at BOTTOM");
            }
            (SearchOutcome::NotFound, _) => self.error(format!("E486: Pattern not found: {text}")),
        }
    }

    // --- Where things are on the screen. ---

    /// The rows the tabline takes: 1 when `'showtabline'` shows it, else 0.
    fn tabline_rows(&self) -> u16 {
        let shown = match self.options.showtabline {
            0 => false,
            1 => self.listed().len() > 1,
            _ => true,
        };
        u16::from(shown)
    }

    /// Where windows go: between the tabline and the command line, above the global statusline when there is one.
    fn windows_area(&self) -> Rect {
        let (width, height) = self.size;
        let top = self.tabline_rows();
        let global = u16::from(self.options.laststatus == 3);
        Rect::new(0, top, width, height.saturating_sub(top + 1 + global))
    }

    /// Where the current window's text goes.
    fn window_area(&self) -> Rect {
        let current = self.windows.current_id();
        let area = self.windows_area();
        let arrangement = self.windows.arrange(area);
        let rect = arrangement
            .windows
            .into_iter()
            .find(|(id, _)| *id == current)
            .map(|(_, rect)| rect);
        self.text_area(
            rect.unwrap_or_default(),
            area,
            self.windows.current().buffer,
        )
    }

    /// A window's text area, showing `buffer`: all of `rect` but its last row, for its statusline, or with a global
    /// statusline (`'laststatus'` 3) for the line separating it from the window below, when there is one; and but
    /// the rows of the buffer's composer, when it shows.
    fn text_area(&self, rect: Rect, windows: Rect, buffer: BufferId) -> Rect {
        let bar = self.options.laststatus != 3 || rect.bottom() < windows.bottom();
        let above = u16::from(self.buffers[buffer.0].winbar.is_some()).min(rect.height);
        let below = u16::from(bar) + self.composer_rows(buffer);
        Rect {
            y: rect.y + above,
            height: rect.height.saturating_sub(below + above),
            ..rect
        }
    }

    fn viewport(&self) -> Viewport {
        Viewport {
            height: usize::from(self.window_area().height),
            scrolloff: self.options.scrolloff,
        }
    }

    /// How `group` is drawn now.
    #[must_use]
    pub fn style(&self, group: crate::hl::Group) -> ratatui::style::Style {
        self.theme.style(group)
    }

    /// The document of the current window, for tests to change as an application would.
    #[doc(hidden)]
    pub fn current_doc_mut(&mut self) -> &mut Document<A::Target> {
        let id = self.windows.current().buffer;
        &mut self.buffers[id.0].doc
    }

    /// A layout of the current window's buffer, for an application to look at what is shown.
    #[must_use]
    pub fn current_layout(&self) -> Layout<'_, A::Target> {
        self.windows
            .current()
            .layout(&self.buffers, self.window_area())
    }
}

/// Whether `motion` is a jump, remembered in the jumplist.
fn is_jump(motion: Motion) -> bool {
    matches!(
        motion,
        Motion::GotoRow(_)
            | Motion::LastRow
            | Motion::ParagraphForward
            | Motion::ParagraphBackward
            | Motion::NextBlock
            | Motion::PrevBlock
    )
}

fn opposite(direction: Direction) -> Direction {
    match direction {
        Direction::Forward => Direction::Backward,
        Direction::Backward => Direction::Forward,
    }
}

/// Adds `entry` to `history` as its newest, unless empty; an older identical entry moves up instead of repeating.
fn remember(history: &mut Vec<String>, entry: &str) {
    if entry.is_empty() {
        return;
    }
    history.retain(|old| old != entry);
    history.push(entry.to_owned());
}
