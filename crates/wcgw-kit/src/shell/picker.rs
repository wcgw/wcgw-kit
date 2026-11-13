//! Pickers (D14): a floating list narrowed by what is typed, `<C-n>`/`<C-p>` to choose, `<CR>` to open, `<Esc>` to
//! leave. A picker narrows its list fuzzily, or asks the application to search once typing pauses.

#[cfg(test)]
mod tests;

use std::fmt;
use std::rc::Rc;
use std::time::Duration;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::text::{Line as TextLine, Span as TextSpan};
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::cmdline::{Cmdline, Edit, Purpose};
use super::{Kit, Mode_};
use crate::app::{App, BufferId, Effect, Effects, Wake, WakePurpose};
use crate::hl::Group;
use crate::key::{Code, Key, Mods};

/// How long typing must pause before a searching picker searches.
const SEARCH_DELAY: Duration = Duration::from_millis(120);

/// The widest a picker is, in cells.
const MAX_WIDTH: u16 = 72;

/// The most rows a picker lists.
const MAX_ROWS: u16 = 12;

/// What the query follows.
const PROMPT: &str = "> ";

/// Something a picker can open.
#[derive(Debug, Clone)]
pub struct Choice<T> {
    /// How it is listed, as text and the group each part is drawn in.
    pub label: Vec<(String, Group)>,
    /// What the query is matched against; ignored by a picker that searches.
    pub text: String,
    /// What picking it opens.
    pub pick: Pick<T>,
}

/// What picking a choice opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick<T> {
    /// A buffer.
    Buffer(BufferId),
    /// What the target refers to, as [`App::open`] opens it.
    Target(T),
}

/// What a search does: asks for what the query finds, to come back through [`Picker::found`].
pub type Search<A> = Rc<dyn Fn(&mut A, &mut Kit<A>, String) -> Effects<A>>;

/// What makes an application's picker, as `:pick {name}` opens it.
pub type Make<A> = Rc<dyn Fn(&mut A, &mut Kit<A>) -> Picker<A>>;

/// Where a picker's choices come from.
pub enum Source<A: App> {
    /// A list given whole, narrowed fuzzily as the query changes.
    List,
    /// A search the application runs once typing pauses, saying `idle` while there is no query.
    Search {
        /// Asks for what the query finds.
        search: Search<A>,
        /// What to say while there is no query.
        idle: String,
    },
}

/// A picker open over the windows.
pub struct Picker<A: App> {
    title: String,
    /// The query, edited as a command line is.
    query: Cmdline<A>,
    /// What can be picked, in their own order.
    choices: Vec<Choice<A::Target>>,
    /// The choices matching the query, best first, as indices into `choices`, each with the characters of its text
    /// that matched.
    shown: Vec<(usize, Vec<usize>)>,
    /// The index into `shown` of the chosen one.
    selected: usize,
    /// What to say while nothing is shown.
    hint: String,
    source: Source<A>,
}

impl<A: App> fmt::Debug for Picker<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Picker")
            .field("title", &self.title)
            .field("choices", &self.choices)
            .finish_non_exhaustive()
    }
}

/// What a key did to a picker.
enum Outcome<T> {
    /// Nothing the kit needs to know.
    Stay,
    /// The query changed.
    Changed,
    /// `<CR>`: open the chosen one, if any.
    Picked(Option<Pick<T>>),
    /// `<Esc>`, `<C-c>`.
    Leave,
}

/// Ranks texts against a query as fzf does (D14): every space-separated term must match, fuzzily unless it starts
/// with `'` (exact) or `^` (prefix); lower case matches either case. Keeps nucleo's scratch space between texts.
struct Ranker {
    pattern: Pattern,
    matcher: Matcher,
    buffer: Vec<char>,
}

impl Ranker {
    fn new(query: &str) -> Self {
        Self {
            pattern: Pattern::parse(query, CaseMatching::Smart, Normalization::Smart),
            matcher: Matcher::new(Config::DEFAULT),
            buffer: Vec::new(),
        }
    }

    /// How well the query matches `text`, and which characters of it matched, by index; `None` when it doesn't.
    fn rank(&mut self, text: &str) -> Option<(i64, Vec<usize>)> {
        let mut indices = Vec::new();
        let haystack = Utf32Str::new(text, &mut self.buffer);
        let score = self
            .pattern
            .indices(haystack, &mut self.matcher, &mut indices)?;
        // Several terms can match the same character; positions come back in no particular order.
        indices.sort_unstable();
        indices.dedup();
        let positions = indices
            .into_iter()
            .map(|i| usize::try_from(i).unwrap_or(usize::MAX))
            .collect();
        Some((i64::from(score), positions))
    }
}

/// How well `query` matches `text`, as fzf does it, and which characters of `text` matched, by index.
#[cfg(test)]
fn best(query: &str, text: &str) -> Option<(i64, Vec<usize>)> {
    Ranker::new(query).rank(text)
}

impl<A: App> Picker<A> {
    /// A picker titled `title` over `choices`, narrowed fuzzily, saying `hint` when none matches.
    #[must_use]
    pub fn list(
        title: impl Into<String>,
        choices: Vec<Choice<A::Target>>,
        hint: impl Into<String>,
    ) -> Self {
        let mut picker = Self {
            title: title.into(),
            query: Cmdline::new(Purpose::Ex, None),
            choices,
            shown: Vec::new(),
            selected: 0,
            hint: hint.into(),
            source: Source::List,
        };
        picker.narrow();
        picker
    }

    /// A picker titled `title` whose choices `search` finds once typing pauses, saying `idle` while there is no
    /// query; results come back through [`Picker::found`].
    #[must_use]
    pub fn search(
        title: impl Into<String>,
        idle: impl Into<String>,
        search: impl Fn(&mut A, &mut Kit<A>, String) -> Effects<A> + 'static,
    ) -> Self {
        let idle = idle.into();
        Self {
            title: title.into(),
            query: Cmdline::new(Purpose::Ex, None),
            choices: Vec::new(),
            shown: Vec::new(),
            selected: 0,
            hint: idle.clone(),
            source: Source::Search {
                search: Rc::new(search),
                idle,
            },
        }
    }

    /// Its title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Starts with `query` typed.
    #[must_use]
    pub fn with_query(mut self, query: &str) -> Self {
        if !query.is_empty() {
            self.query.paste(query);
            self.narrow();
        }
        self
    }

    /// Starts on the `index`th choice shown, when there is one.
    #[must_use]
    pub fn selecting(mut self, index: usize) -> Self {
        if index < self.shown.len() {
            self.selected = index;
        }
        self
    }

    /// What a search found for `term`, in the order found, or `hint` to say when it found nothing; ignored unless
    /// the picker searches and its query is still `term`. Returns whether it was taken.
    pub fn found(
        &mut self,
        term: &str,
        choices: Vec<Choice<A::Target>>,
        hint: impl Into<String>,
    ) -> bool {
        if !self.is_searching() || self.term() != term {
            return false;
        }
        self.choices = choices;
        // The search ranks them: every one is shown, in its order.
        self.shown = (0..self.choices.len()).map(|i| (i, Vec::new())).collect();
        self.selected = 0;
        self.hint = hint.into();
        true
    }

    /// Whether its choices come from a search.
    #[must_use]
    pub fn is_searching(&self) -> bool {
        matches!(self.source, Source::Search { .. })
    }

    /// Whether it searches and has a query to search for now.
    pub(super) fn wants_search(&self) -> bool {
        self.is_searching() && !self.term().is_empty()
    }

    /// The choices matching the query, best first; ties keep their order.
    fn narrow(&mut self) {
        if self.is_searching() {
            return;
        }
        let mut ranker = Ranker::new(&self.query.text());
        let mut scored: Vec<(usize, i64, Vec<usize>)> = self
            .choices
            .iter()
            .enumerate()
            .filter_map(|(i, choice)| {
                ranker
                    .rank(&choice.text)
                    .map(|(score, matched)| (i, score, matched))
            })
            .collect();
        scored.sort_by_key(|&(_, score, _)| std::cmp::Reverse(score));
        self.shown = scored
            .into_iter()
            .map(|(i, _, matched)| (i, matched))
            .collect();
        self.selected = 0;
    }

    fn key(&mut self, key: Key) -> Outcome<A::Target> {
        let ctrl = |ch| Key {
            code: Code::Char(ch),
            mods: Mods::CTRL,
        };
        let count = self.shown.len();
        match key {
            k if k == ctrl('n') || k.code == Code::Down => {
                if count > 0 {
                    self.selected = (self.selected + 1) % count;
                }
                Outcome::Stay
            }
            k if k == ctrl('p') || k.code == Code::Up => {
                if count > 0 {
                    self.selected = (self.selected + count - 1) % count;
                }
                Outcome::Stay
            }
            // Unlike the command line, an empty picker stays open on <BS>.
            k if k.code == Code::Backspace && self.query.text().is_empty() => Outcome::Stay,
            k => {
                let before = self.query.text();
                match self.query.key(k, &[]) {
                    Edit::Submit(_) => Outcome::Picked(self.chosen()),
                    Edit::Cancel => Outcome::Leave,
                    Edit::Editing if self.query.text() == before => Outcome::Stay,
                    Edit::Editing => {
                        self.narrow();
                        Outcome::Changed
                    }
                }
            }
        }
    }

    /// Pasted text, up to its first line break, goes into the query.
    fn paste(&mut self, text: &str) -> Outcome<A::Target> {
        self.query.paste(text);
        self.narrow();
        Outcome::Changed
    }

    fn chosen(&self) -> Option<Pick<A::Target>> {
        let &(index, _) = self.shown.get(self.selected)?;
        Some(self.choices[index].pick.clone())
    }

    /// The search term the query stands for; empty searches nothing.
    #[must_use]
    pub fn term(&self) -> String {
        self.query.text().trim().to_owned()
    }
}

impl<A: App> Kit<A> {
    /// `:pick buffers [query]`: the listed buffers, the one shown first, then the others from the last shown; it
    /// starts on the second.
    pub(super) fn pick(&mut self, app: &mut A, args: &str) -> Result<Effects<A>, String> {
        let (list, query) = crate::ex::required(args)?
            .split_once(' ')
            .map_or((args, ""), |(list, query)| (list, query.trim_start()));
        let picker = if list == "buffers" {
            self.buffer_picker()
        } else {
            let make = self
                .pickers
                .iter()
                .find(|(name, _)| *name == list)
                .map(|(_, make)| Rc::clone(make))
                .ok_or_else(|| crate::ex::ExError::InvalidArgument(list.to_owned()))?;
            make(app, self)
        };
        Ok(self.open_picker(app, picker.with_query(query)))
    }

    /// Makes `:pick {name}` open the picker `make` makes.
    pub fn add_picker(
        &mut self,
        name: &'static str,
        make: impl Fn(&mut A, &mut Kit<A>) -> Picker<A> + 'static,
    ) {
        self.pickers.retain(|(known, _)| *known != name);
        self.pickers.push((name, Rc::new(make)));
    }

    /// The names `:pick` takes.
    pub(super) fn picker_names(&self) -> Vec<String> {
        std::iter::once("buffers")
            .chain(self.pickers.iter().map(|(name, _)| *name))
            .map(str::to_owned)
            .collect()
    }

    fn buffer_picker(&self) -> Picker<A> {
        let current = self.windows.current().buffer;
        let mut order: Vec<BufferId> = std::iter::once(current)
            .chain(self.recent.iter().copied())
            .chain(self.listed())
            .filter(|&id| self.buffers[id.0].listed)
            .collect();
        let mut seen = std::collections::HashSet::new();
        order.retain(|id| seen.insert(*id));
        let choices = order
            .into_iter()
            .map(|id| {
                let name = self.buffers[id.0].name.clone();
                let group = if id == current {
                    Group::COMMENT
                } else {
                    Group::NORMAL_FLOAT
                };
                Choice {
                    label: vec![(name.clone(), group)],
                    text: name,
                    pick: Pick::Buffer(id),
                }
            })
            .collect();
        Picker::list("Buffers", choices, "No matching buffer").selecting(1)
    }

    /// A key while a picker is open.
    pub(super) fn picker_key(&mut self, app: &mut A, key: Key) -> Effects<A> {
        let Mode_::Picker(picker) = &mut self.mode else {
            return Vec::new();
        };
        let outcome = picker.key(key);
        self.picked(app, outcome)
    }

    /// Text pasted while a picker is open.
    pub(super) fn picker_paste(&mut self, app: &mut A, text: &str) -> Effects<A> {
        let Mode_::Picker(picker) = &mut self.mode else {
            return Vec::new();
        };
        let outcome = picker.paste(text);
        self.picked(app, outcome)
    }

    /// What the open picker does about what a key or a paste did to it.
    fn picked(&mut self, app: &mut A, outcome: Outcome<A::Target>) -> Effects<A> {
        let Mode_::Picker(picker) = &mut self.mode else {
            return Vec::new();
        };
        match outcome {
            Outcome::Stay => Vec::new(),
            Outcome::Changed => {
                let Source::Search { idle, .. } = &picker.source else {
                    return Vec::new();
                };
                self.search_generation += 1;
                if picker.term().is_empty() {
                    picker.choices.clear();
                    picker.shown.clear();
                    picker.selected = 0;
                    picker.hint.clone_from(idle);
                    return Vec::new();
                }
                "Searching…".clone_into(&mut picker.hint);
                vec![Effect::Wake {
                    after: SEARCH_DELAY,
                    token: Wake {
                        purpose: WakePurpose::Search,
                        generation: self.search_generation,
                    },
                }]
            }
            Outcome::Leave => {
                self.mode = Mode_::Normal;
                Vec::new()
            }
            Outcome::Picked(pick) => {
                self.mode = Mode_::Normal;
                match pick {
                    Some(Pick::Buffer(id)) => {
                        self.show_buffer(id);
                        Vec::new()
                    }
                    Some(Pick::Target(target)) => match app.open(self, target) {
                        Ok(effects) => effects,
                        Err(error) => {
                            self.error(error);
                            Vec::new()
                        }
                    },
                    None => Vec::new(),
                }
            }
        }
    }

    /// Typing paused in a picker that searches: search for what it says.
    pub(super) fn search_now_with(&mut self, app: &mut A) -> Effects<A> {
        let Mode_::Picker(picker) = &self.mode else {
            return Vec::new();
        };
        let Source::Search { search, .. } = &picker.source else {
            return Vec::new();
        };
        let term = picker.term();
        if term.is_empty() {
            return Vec::new();
        }
        let search = Rc::clone(search);
        search(app, self, term)
    }

    /// Draws `picker` in the middle of the screen, and returns where the cursor goes: in its query.
    /// Where `picker` goes: its card, in the middle of the screen.
    fn picker_area(&self, picker: &Picker<A>) -> Rect {
        let (width, height) = self.size;
        let card_width = MAX_WIDTH.min(width.saturating_sub(4)).max(10).min(width);
        let rows = u16::try_from(picker.shown.len())
            .unwrap_or(u16::MAX)
            .clamp(1, MAX_ROWS);
        let card_height = (rows + 3).min(height.saturating_sub(2)).max(3).min(height);
        Rect::new(
            (width - card_width) / 2,
            height.saturating_sub(card_height) / 3,
            card_width,
            card_height,
        )
    }

    /// The rows of `picker`'s list on screen, and the index into what it shows of the first.
    fn picker_list(&self, picker: &Picker<A>) -> (Rect, usize) {
        let inner = Block::bordered().inner(self.picker_area(picker));
        let list = Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(1),
        );
        let visible = usize::from(list.height);
        // Scrolled so the chosen one is in view.
        (
            list,
            picker.selected.saturating_sub(visible.saturating_sub(1)),
        )
    }

    /// A click on the open picker: on an item of its list, chooses it, and a double click opens it.
    pub(super) fn picker_click(
        &mut self,
        app: &mut A,
        mouse: crate::key::Mouse,
        column: u16,
        row: u16,
    ) -> Effects<A> {
        let Mode_::Picker(picker) = &self.mode else {
            return Vec::new();
        };
        let (list, first) = self.picker_list(picker);
        if !list.contains((column, row).into()) {
            return Vec::new();
        }
        let index = first + usize::from(row - list.y);
        let Mode_::Picker(picker) = &mut self.mode else {
            return Vec::new();
        };
        if index >= picker.shown.len() {
            return Vec::new();
        }
        picker.selected = index;
        match mouse {
            crate::key::Mouse::DoubleLeft => {
                let outcome = Outcome::Picked(picker.chosen());
                self.picked(app, outcome)
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn render_picker(&self, frame: &mut Frame<'_>, picker: &Picker<A>) -> Position {
        let area = self.picker_area(picker);
        let title = format!(" {} ", picker.title);
        let block = Block::bordered()
            .title(title)
            .border_style(self.theme.style(Group::FLOAT_BORDER))
            .style(self.theme.style(Group::NORMAL_FLOAT));
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        let prompt = format!("{PROMPT}{}", picker.query.text());
        frame.render_widget(
            Paragraph::new(prompt),
            Rect::new(inner.x, inner.y, inner.width, 1.min(inner.height)),
        );
        let (list, first) = self.picker_list(picker);
        let visible = usize::from(list.height);
        let lines: Vec<TextLine<'_>> = if picker.shown.is_empty() {
            vec![TextLine::from(vec![
                TextSpan::raw("  "),
                TextSpan::styled(picker.hint.as_str(), self.theme.style(Group::COMMENT)),
            ])]
        } else {
            picker
                .shown
                .iter()
                .skip(first)
                .take(visible)
                .map(|(index, matched)| {
                    let mut spans = vec![TextSpan::raw("  ")];
                    // The label's characters, those that matched in `PmenuMatch`.
                    let mut at = 0;
                    for (text, group) in &picker.choices[*index].label {
                        for ch in text.chars() {
                            let group = if matched.contains(&at) {
                                Group::PMENU_MATCH
                            } else {
                                *group
                            };
                            spans.push(TextSpan::styled(ch.to_string(), self.theme.style(group)));
                            at += 1;
                        }
                    }
                    TextLine::from(spans)
                })
                .collect()
        };
        frame.render_widget(Paragraph::new(lines), list);
        if !picker.shown.is_empty() {
            // Over the labels' own colours, as Visual mode's selection.
            let row = u16::try_from(picker.selected - first).unwrap_or(0);
            if row < list.height {
                frame.buffer_mut().set_style(
                    Rect::new(list.x, list.y + row, list.width, 1),
                    self.theme.style(Group::VISUAL),
                );
            }
        }
        let x = inner.x + u16::try_from(PROMPT.width() + picker.query.cursor_cells()).unwrap_or(0);
        Position::new(x.min(inner.right().saturating_sub(1)), inner.y)
    }
}
