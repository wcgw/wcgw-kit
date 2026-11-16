//! The composer (D4): the one editable thing, a text area docked under a window showing its buffer, for writing
//! what an application sends, such as a chat message. Insert mode types into it with readline keys; `<Esc>` leaves;
//! `<C-x><C-e>` hands the text to `$EDITOR`. Long lines wrap at words, and the composer grows with its text up to
//! [`MAX_ROWS`], then scrolls. The application completes words as they are typed: it says what a word may become,
//! the kit shows them in a menu and puts the one chosen in place.

use std::fmt;
use std::rc::Rc;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget as _};
use ratatui_textarea::{CursorMove, Input, Key as TextKey, ScreenCursor, TextArea, WrapMode};
use unicode_width::UnicodeWidthStr;

use super::{Kit, Mode_};
use crate::app::{App, BufferId, Effect, Effects};
use crate::hl::Group;
use crate::key::{Code, Key, Mods};

/// The most rows a composer's text takes; longer text scrolls in it.
const MAX_ROWS: u16 = 8;

/// The most suggestions shown at once.
const MENU_ROWS: usize = 8;

/// What sending does with the text: the application posts it.
pub type Send<A> = Rc<dyn Fn(&mut A, &mut Kit<A>, BufferId, String) -> Effects<A>>;

/// What the word before the cursor may be completed to, as the application sees it; empty for none.
pub type CompleteWord<A> = Rc<dyn Fn(&A, &Kit<A>, BufferId, &Word<'_>) -> Vec<Suggestion>>;

/// What the application does when a suggestion is put in place, such as remembering whom an @-mention names.
pub type Accepted<A> = Rc<dyn Fn(&mut A, &mut Kit<A>, BufferId, &Suggestion) -> Effects<A>>;

/// A key the application takes before the composer does: `Some` when it handled it.
pub type OnKey<A> = Rc<dyn Fn(&mut A, &mut Kit<A>, BufferId, Key) -> Option<Effects<A>>>;

/// What the application does as the word before the cursor changes, such as asking a server whom a name finds.
pub type OnWord<A> = Rc<dyn Fn(&mut A, &mut Kit<A>, BufferId, &Word<'_>) -> Effects<A>>;

/// What the application does when Insert mode is left, such as giving up an edit left empty.
pub type OnLeave<A> = Rc<dyn Fn(&mut A, &mut Kit<A>, BufferId) -> Effects<A>>;

/// The word before the cursor, as completion sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Word<'a> {
    /// The word: from the last blank before the cursor to it.
    pub text: &'a str,
    /// Whether it is the first word of the text, as a slash command is.
    pub first: bool,
}

/// Something a word may be completed to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// How the menu lists it.
    pub label: Vec<(String, Group)>,
    /// What replaces the word when it is chosen.
    pub insert: String,
    /// What the application knows it by, to tell which was chosen.
    pub key: String,
}

/// A composer of a buffer: its text, and what the application does with it.
pub struct Composer<A: App> {
    area: TextArea<'static>,
    placeholder: String,
    title: Option<String>,
    always: bool,
    enter_sends: bool,
    send: Option<Send<A>>,
    complete: Option<CompleteWord<A>>,
    accepted: Option<Accepted<A>>,
    on_key: Option<OnKey<A>>,
    on_word: Option<OnWord<A>>,
    on_leave: Option<OnLeave<A>>,
    /// What the word before the cursor may become, while it may become something.
    suggestions: Vec<Suggestion>,
    selected: usize,
    /// Whether `<C-x>` was typed, waiting for `<C-e>`.
    ctrl_x: bool,
}

impl<A: App> fmt::Debug for Composer<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Composer")
            .field("text", &self.text())
            .field("suggestions", &self.suggestions)
            .finish_non_exhaustive()
    }
}

impl<A: App> Composer<A> {
    /// An empty composer saying `placeholder` while empty; `<CR>` sends.
    #[must_use]
    pub fn new(placeholder: impl Into<String>) -> Self {
        let placeholder = placeholder.into();
        Self {
            area: text_area(""),
            placeholder,
            title: None,
            always: false,
            enter_sends: true,
            send: None,
            complete: None,
            accepted: None,
            on_key: None,
            on_word: None,
            on_leave: None,
            suggestions: Vec::new(),
            selected: 0,
            ctrl_x: false,
        }
    }

    /// Shown under its buffer even when empty and not typed into, as a chat's is.
    #[must_use]
    pub fn always_shown(self) -> Self {
        Self {
            always: true,
            ..self
        }
    }

    /// `<CR>` breaks the line rather than sending; `<C-s>` sends.
    #[must_use]
    pub fn enter_breaks_lines(self) -> Self {
        Self {
            enter_sends: false,
            ..self
        }
    }

    /// Sends with `send`.
    #[must_use]
    pub fn on_send(
        self,
        send: impl Fn(&mut A, &mut Kit<A>, BufferId, String) -> Effects<A> + 'static,
    ) -> Self {
        Self {
            send: Some(Rc::new(send)),
            ..self
        }
    }

    /// Completes the word before the cursor with what `complete` suggests, telling `accepted` which was chosen.
    #[must_use]
    pub fn completing(
        self,
        complete: impl Fn(&A, &Kit<A>, BufferId, &Word<'_>) -> Vec<Suggestion> + 'static,
        accepted: impl Fn(&mut A, &mut Kit<A>, BufferId, &Suggestion) -> Effects<A> + 'static,
    ) -> Self {
        Self {
            complete: Some(Rc::new(complete)),
            accepted: Some(Rc::new(accepted)),
            ..self
        }
    }

    /// Lets the application take keys before the composer does: `on_key` returns `Some` for one it handled, as Up
    /// in an empty composer editing the last message sent.
    #[must_use]
    pub fn on_key(
        self,
        on_key: impl Fn(&mut A, &mut Kit<A>, BufferId, Key) -> Option<Effects<A>> + 'static,
    ) -> Self {
        Self {
            on_key: Some(Rc::new(on_key)),
            ..self
        }
    }

    /// Tells the application of the word before the cursor as it changes, with effects of its own, such as asking
    /// a server whom a name finds; [`Kit::refresh_completion`] then completes with what came back.
    #[must_use]
    pub fn on_word(
        self,
        on_word: impl Fn(&mut A, &mut Kit<A>, BufferId, &Word<'_>) -> Effects<A> + 'static,
    ) -> Self {
        Self {
            on_word: Some(Rc::new(on_word)),
            ..self
        }
    }

    /// Tells the application when Insert mode is left (`<Esc>`, a click elsewhere).
    #[must_use]
    pub fn on_leave(
        self,
        on_leave: impl Fn(&mut A, &mut Kit<A>, BufferId) -> Effects<A> + 'static,
    ) -> Self {
        Self {
            on_leave: Some(Rc::new(on_leave)),
            ..self
        }
    }

    /// Its text, lines joined by `\n`.
    #[must_use]
    pub fn text(&self) -> String {
        self.area.lines().join("\n")
    }

    /// Whether it holds no text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.area.is_empty()
    }

    /// Replaces its text with `text`, the cursor at the end.
    pub fn set_text(&mut self, text: &str) {
        self.area = text_area(text);
        self.suggestions.clear();
    }

    /// What its border says, such as whom the text mentions; `None` for nothing.
    pub fn set_title(&mut self, title: Option<String>) {
        self.title = title;
    }

    /// What the menu suggests now.
    #[must_use]
    pub fn suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }

    /// The word before the cursor, and whether it is the text's first.
    fn word(&self) -> Option<(Word<'_>, usize)> {
        let ratatui_textarea::DataCursor(row, col) = self.area.cursor();
        let line = self.area.lines().get(row)?;
        let before: String = line.chars().take(col).collect();
        let start = before
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map_or(0, |(at, c)| at + c.len_utf8());
        let byte_end = line
            .char_indices()
            .nth(col)
            .map_or(line.len(), |(at, _)| at);
        let text = &line[start..byte_end];
        (!text.is_empty()).then(|| {
            (
                Word {
                    text,
                    first: row == 0 && start == 0,
                },
                text.chars().count(),
            )
        })
    }

    /// Whether it shows under its buffer now: always, or while it holds text, or while `typing`.
    fn is_shown(&self, typing: bool) -> bool {
        self.always || typing || !self.area.is_empty()
    }

    /// The rows it takes when shown `width` cells wide: its border, and its text wrapped at that width, up to
    /// [`MAX_ROWS`].
    fn height(&self, width: u16) -> u16 {
        let rows = u16::try_from(self.rows(width)).unwrap_or(u16::MAX);
        rows.clamp(1, MAX_ROWS) + 1
    }

    /// The rows its whole text takes wrapped at `width` cells, as drawing it that wide wraps it.
    fn rows(&self, width: u16) -> usize {
        // The text area wraps as it is drawn, and keeps how many rows that makes to itself: a copy is drawn that
        // wide, and the row its end is on counted.
        let mut probe = self.area.clone();
        let at = Rect::new(0, 0, width, 1);
        probe.render(at, &mut Buffer::empty(at));
        probe.move_cursor(CursorMove::Bottom);
        probe.move_cursor(CursorMove::End);
        probe.screen_cursor().row + 1
    }
}

/// A text area holding `text`, the cursor at its end, wrapping at words as it is drawn. The kit draws the
/// placeholder and the cursor, and styles the text.
fn text_area(text: &str) -> TextArea<'static> {
    let mut area = TextArea::new(text.lines().map(str::to_owned).collect());
    area.set_wrap_mode(WrapMode::WordOrGlyph);
    area.set_cursor_line_style(Style::new());
    area.set_cursor_style(Style::new());
    area.move_cursor(CursorMove::Bottom);
    area.move_cursor(CursorMove::End);
    area
}

/// `key` as the text area takes it.
fn input(key: Key) -> Input {
    let key_of = match key.code {
        Code::Char(c) => TextKey::Char(c),
        Code::Enter => TextKey::Enter,
        Code::Backspace => TextKey::Backspace,
        Code::Delete => TextKey::Delete,
        Code::Left => TextKey::Left,
        Code::Right => TextKey::Right,
        Code::Up => TextKey::Up,
        Code::Down => TextKey::Down,
        Code::Home => TextKey::Home,
        Code::End => TextKey::End,
        Code::PageUp => TextKey::PageUp,
        Code::PageDown => TextKey::PageDown,
        Code::Tab => TextKey::Tab,
        Code::Esc => TextKey::Esc,
        Code::F(n) => TextKey::F(n),
        Code::Insert | Code::Mouse(_) => TextKey::Null,
    };
    Input {
        key: key_of,
        ctrl: key.mods.contains(Mods::CTRL),
        alt: key.mods.contains(Mods::ALT),
        shift: key.mods.contains(Mods::SHIFT),
    }
}

impl<A: App> Kit<A> {
    /// Gives buffer `id` the composer `composer`, replacing any it had; windows showing the buffer show it docked
    /// under its text.
    pub fn set_composer(&mut self, id: BufferId, composer: Composer<A>) {
        self.composers.insert(id, composer);
    }

    /// The composer of buffer `id`, if it has one.
    #[must_use]
    pub fn composer(&self, id: BufferId) -> Option<&Composer<A>> {
        self.composers.get(&id)
    }

    /// The composer of buffer `id`, if it has one, to change.
    pub fn composer_mut(&mut self, id: BufferId) -> Option<&mut Composer<A>> {
        self.composers.get_mut(&id)
    }

    /// `:startinsert`: types into the composer of the current buffer.
    pub(super) fn start_insert(&mut self) -> Result<(), String> {
        let id = self.windows.current().buffer;
        if !self.composers.contains_key(&id) {
            return Err("Nothing to write in here".to_owned());
        }
        self.mode = Mode_::Insert(id);
        Ok(())
    }

    /// Whether the composer of `id` is being typed into.
    fn typing_in(&self, id: BufferId) -> bool {
        matches!(self.mode, Mode_::Insert(typing) if typing == id)
    }

    /// The rows the composer of the buffer a window `width` cells wide shows takes at its bottom, when it shows.
    pub(super) fn composer_rows(&self, id: BufferId, width: u16) -> u16 {
        self.composers
            .get(&id)
            .filter(|composer| composer.is_shown(self.typing_in(id)))
            .map_or(0, |composer| composer.height(width))
    }

    /// A key typed into the composer of `id`.
    pub(super) fn insert_key(&mut self, app: &mut A, id: BufferId, key: Key) -> Effects<A> {
        if let Some(on_key) = self
            .composers
            .get(&id)
            .and_then(|composer| composer.on_key.clone())
            && let Some(effects) = on_key(app, self, id, key)
        {
            return effects;
        }
        let Some(composer) = self.composers.get_mut(&id) else {
            self.mode = Mode_::Normal;
            return Vec::new();
        };
        let ctrl = |c| Key {
            code: Code::Char(c),
            mods: Mods::CTRL,
        };
        if std::mem::take(&mut composer.ctrl_x) && key == ctrl('e') {
            self.editor_ours = Some(id);
            return vec![Effect::Edit(crate::app::Edit::new(composer.text()))];
        }
        let menu = !composer.suggestions.is_empty();
        let count = composer.suggestions.len();
        match key {
            k if k == ctrl('x') => composer.ctrl_x = true,
            k if k.code == Code::Esc && menu => composer.suggestions.clear(),
            k if k.code == Code::Esc => {
                composer.suggestions.clear();
                self.mode = Mode_::Normal;
                let on_leave = composer.on_leave.clone();
                return on_leave.map_or_else(Vec::new, |on_leave| on_leave(app, self, id));
            }
            k if menu
                && (k == ctrl('n')
                    || k.code == Code::Down
                    || (k.code == Code::Tab && k.mods == Mods::NONE)) =>
            {
                composer.selected = (composer.selected + 1) % count;
            }
            k if menu
                && (k == ctrl('p')
                    || k.code == Code::Up
                    || (k.code == Code::Tab && k.mods == Mods::SHIFT)) =>
            {
                composer.selected = (composer.selected + count - 1) % count;
            }
            k if menu && k.code == Code::Enter && k.mods == Mods::NONE => {
                return self.accept(app, id);
            }
            k if k.code == Code::Enter && k.mods == Mods::NONE && composer.enter_sends => {
                return self.send(app, id);
            }
            k if k == ctrl('s') => return self.send(app, id),
            k if k.code == Code::Enter || k == ctrl('j') => {
                composer.area.insert_newline();
                return self.word_changed(app, id);
            }
            k => {
                composer.area.input(input(k));
                return self.word_changed(app, id);
            }
        }
        Vec::new()
    }

    /// Insert mode in the composer of `id` was left: the application hears of it.
    pub(super) fn left_insert(&mut self, app: &mut A, id: BufferId) -> Effects<A> {
        let on_leave = self
            .composers
            .get(&id)
            .and_then(|composer| composer.on_leave.clone());
        on_leave.map_or_else(Vec::new, |on_leave| on_leave(app, self, id))
    }

    /// The text of the composer of `id` changed: the application hears of the word before the cursor, then
    /// completes it.
    fn word_changed(&mut self, app: &mut A, id: BufferId) -> Effects<A> {
        let mut effects = Vec::new();
        if let Some(composer) = self.composers.get(&id)
            && let (Some(on_word), Some((word, _))) = (composer.on_word.clone(), composer.word())
        {
            let text = word.text.to_owned();
            let first = word.first;
            effects = on_word(app, self, id, &Word { text: &text, first });
        }
        self.refresh_suggestions(app, id);
        effects
    }

    /// Text pasted into the composer of `id`.
    pub(super) fn insert_paste(&mut self, app: &mut A, id: BufferId, text: &str) {
        if let Some(composer) = self.composers.get_mut(&id) {
            composer.area.insert_str(text);
        }
        self.refresh_suggestions(app, id);
    }

    /// Asks the application what the word before the cursor may become.
    fn refresh_suggestions(&mut self, app: &A, id: BufferId) {
        let Some(composer) = self.composers.get(&id) else {
            return;
        };
        let suggestions = match (&composer.complete, composer.word()) {
            (Some(complete), Some((word, _))) => complete(app, self, id, &word),
            _ => Vec::new(),
        };
        if let Some(composer) = self.composers.get_mut(&id) {
            composer.selected = 0;
            composer.suggestions = suggestions;
        }
    }

    /// Asks the application again what the word before the cursor of the composer being typed in may become, as
    /// when what it knows changed (people found, say).
    pub fn refresh_completion(&mut self, app: &A) {
        if let Mode_::Insert(id) = self.mode {
            self.refresh_suggestions(app, id);
        }
    }

    /// Puts the suggestion chosen in place of the word before the cursor.
    fn accept(&mut self, app: &mut A, id: BufferId) -> Effects<A> {
        let Some(composer) = self.composers.get_mut(&id) else {
            return Vec::new();
        };
        let Some(chosen) = composer.suggestions.get(composer.selected).cloned() else {
            return Vec::new();
        };
        if let Some((_, chars)) = composer.word() {
            for _ in 0..chars {
                composer.area.delete_char();
            }
        }
        composer.area.insert_str(&chosen.insert);
        composer.suggestions.clear();
        let accepted = composer.accepted.clone();
        accepted.map_or_else(Vec::new, |accepted| accepted(app, self, id, &chosen))
    }

    /// Sends the text of the composer of `id`, which empties it; nothing for an empty one.
    fn send(&mut self, app: &mut A, id: BufferId) -> Effects<A> {
        let Some(composer) = self.composers.get_mut(&id) else {
            return Vec::new();
        };
        let text = composer.text();
        if text.trim().is_empty() {
            return Vec::new();
        }
        let send = composer.send.clone();
        composer.set_text("");
        send.map_or_else(Vec::new, |send| send(app, self, id, text))
    }

    /// The editor opened from the composer of `id` saved `saved`: it becomes the text.
    pub(super) fn composer_edited(&mut self, id: BufferId, saved: Result<String, String>) {
        match saved {
            Ok(text) => {
                if let Some(composer) = self.composers.get_mut(&id) {
                    composer.set_text(text.trim_end_matches('\n'));
                }
            }
            Err(reason) => self.error(format!("Not edited: {reason}")),
        }
    }

    /// Draws the composer of `id` in `area`, the bottom rows of a window, and returns where the cursor goes when it
    /// is typed into.
    pub(super) fn render_composer(
        &self,
        frame: &mut Frame<'_>,
        id: BufferId,
        area: Rect,
    ) -> Option<Position> {
        let composer = self.composers.get(&id)?;
        let typing = self.typing_in(id);
        let border = if typing {
            Group::STATUS_LINE_MODE_INSERT
        } else {
            Group::WIN_SEPARATOR
        };
        let mut block = Block::default()
            .borders(Borders::TOP)
            .border_style(self.theme.style(border));
        if let Some(title) = &composer.title {
            block = block.title(format!(" {title} "));
        }
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        let cursor = self
            .theme
            .style(Group::NORMAL)
            .add_modifier(Modifier::REVERSED);
        if composer.area.is_empty() {
            let mut placeholder = Vec::new();
            if typing {
                placeholder.push(Span::styled(" ", cursor));
            }
            placeholder.push(Span::styled(
                composer.placeholder.as_str(),
                self.theme.style(Group::COMMENT),
            ));
            frame.render_widget(Paragraph::new(Line::from(placeholder)), inner);
            return typing.then(|| inner.as_position());
        }
        // Drawn itself rather than a copy: the text area learns the width it wraps at and where it scrolled to as
        // it is drawn, which moving its cursor up and down by the rows shown goes by.
        frame.render_widget(&composer.area, inner);
        frame
            .buffer_mut()
            .set_style(inner, self.theme.style(Group::NORMAL));
        if !typing {
            return None;
        }
        let ScreenCursor { row, col, .. } = composer.area.screen_cursor();
        let (top, _) = composer.area.scroll_offset();
        let row = u16::try_from(row).unwrap_or(u16::MAX).saturating_sub(top);
        // At the end of a row as wide as the composer, the cursor stays on its last cell.
        let at = Position::new(
            inner
                .x
                .saturating_add(u16::try_from(col).unwrap_or(u16::MAX))
                .min(inner.right().saturating_sub(1)),
            inner.y.saturating_add(row),
        );
        frame
            .buffer_mut()
            .set_style(Rect::new(at.x, at.y, 1, 1), cursor);
        self.render_suggestions(frame, composer, inner, at);
        Some(at)
    }

    /// The suggestions for the word at the cursor `at`, in a menu just above the composer's text in `inner`.
    fn render_suggestions(
        &self,
        frame: &mut Frame<'_>,
        composer: &Composer<A>,
        inner: Rect,
        at: Position,
    ) {
        let rows = composer.suggestions.len().min(MENU_ROWS);
        if rows == 0 || inner.y < 2 {
            return;
        }
        let first = composer.selected.saturating_sub(rows - 1);
        let width = composer
            .suggestions
            .iter()
            .map(|suggestion| {
                suggestion
                    .label
                    .iter()
                    .map(|(text, _)| text.width())
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0)
            + 2;
        let (screen_width, _) = self.size;
        let width = u16::try_from(width).unwrap_or(u16::MAX).min(screen_width);
        let rows = u16::try_from(rows)
            .unwrap_or(u16::MAX)
            .min(inner.y.saturating_sub(1));
        // The word is on the cursor's row, as wrapping keeps words whole unless wider than the composer.
        let word = composer.word().map_or(0, |(word, _)| word.text.width());
        let column =
            at.x.saturating_sub(u16::try_from(word).unwrap_or(u16::MAX))
                .max(inner.x);
        let x = column
            .saturating_sub(1)
            .min(screen_width.saturating_sub(width));
        let top = inner.y.saturating_sub(1).saturating_sub(rows);
        for (row, index) in (0..rows).zip(first..) {
            let Some(suggestion) = composer.suggestions.get(index) else {
                break;
            };
            let selected = index == composer.selected;
            let base = if selected {
                Group::PMENU_SEL
            } else {
                Group::PMENU
            };
            let area = Rect::new(x, top + row, width, 1);
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new("")
                    .style(self.theme.style(Group::PMENU).patch(self.theme.style(base))),
                area,
            );
            let mut spans = vec![ratatui::text::Span::raw(" ")];
            for (text, group) in &suggestion.label {
                let style = self
                    .theme
                    .style(Group::PMENU)
                    .patch(self.theme.style(*group))
                    .patch(self.theme.style(base));
                spans.push(ratatui::text::Span::styled(text.clone(), style));
            }
            frame.render_widget(Paragraph::new(ratatui::text::Line::from(spans)), area);
        }
    }
}
