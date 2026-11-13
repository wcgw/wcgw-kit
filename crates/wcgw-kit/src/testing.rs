//! Testing an application the way a user meets it (D19): keys in, screen out, with no terminal and no runtime.
//!
//! [`Harness`] runs the kit and the application's `update` synchronously, renders to ratatui's `TestBackend`, and
//! keeps a fake clock: the wake-ups and delayed messages effects ask for are delivered by [`Harness::advance`].

use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Style;

use crate::app::{App, Effect, Effects, Msg};
use crate::key::{Key, parse_keys};
use crate::shell::Kit;

/// What the terminal shows: each row's text, trailing blanks trimmed, and the cursor's (column, row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    /// The rows, top to bottom.
    pub rows: Vec<String>,
    /// Where the cursor is, as (column, row).
    pub cursor: (u16, u16),
}

/// An application in the kit, driven by tests.
#[derive(Debug)]
pub struct Harness<A: App> {
    /// The application.
    pub app: A,
    /// The kit around it.
    pub kit: Kit<A>,
    /// The time on the fake clock since the harness started.
    now: Duration,
    /// Messages to deliver once the clock reaches when they are due, in the order asked for.
    timers: Vec<(Duration, Msg<A::Msg>)>,
}

impl<A: App> Harness<A> {
    /// `app` in `kit`, in a `width` × `height` terminal.
    pub fn new(app: A, kit: Kit<A>, width: u16, height: u16) -> Self {
        let mut harness = Self {
            app,
            kit,
            now: Duration::ZERO,
            timers: Vec::new(),
        };
        let _ = harness.update(Msg::Resize { width, height });
        harness
    }

    /// Reacts to `msg`, returning the effects; timers they ask for start on the fake clock.
    pub fn update(&mut self, msg: Msg<A::Msg>) -> Effects<A> {
        let effects = self.kit.update(&mut self.app, msg);
        for effect in &effects {
            match effect {
                Effect::Wake { after, token } => {
                    self.timers.push((self.now + *after, Msg::Wake(*token)));
                }
                Effect::After { after, msg } => {
                    self.timers.push((self.now + *after, Msg::App(msg.clone())));
                }
                _ => {}
            }
        }
        effects
    }

    /// Sends the application `msg`.
    pub fn send(&mut self, msg: A::Msg) -> Effects<A> {
        self.update(Msg::App(msg))
    }

    /// Types `notation` (Vim notation, `<leader>` as `<Space>`) and collects the effects.
    pub fn press(&mut self, notation: &str) -> Effects<A> {
        parse_keys(notation, Key::char(' '))
            .into_iter()
            .flat_map(|key| self.update(Msg::Key(key)))
            .collect()
    }

    /// Moves the fake clock on by `by`, delivering what falls due, in the order due; returns their effects.
    pub fn advance(&mut self, by: Duration) -> Effects<A> {
        let until = self.now + by;
        let mut effects = Vec::new();
        loop {
            let due = self
                .timers
                .iter()
                .enumerate()
                .filter(|(_, (at, _))| *at <= until)
                .min_by_key(|(_, (at, _))| *at)
                .map(|(index, _)| index);
            let Some(index) = due else { break };
            let (at, msg) = self.timers.remove(index);
            self.now = at;
            effects.extend(self.update(msg));
        }
        self.now = until;
        effects
    }

    /// What the terminal shows now.
    #[must_use]
    pub fn screen(&self) -> Screen {
        let (rows, cursor, _) = self.draw();
        Screen { rows, cursor }
    }

    /// How the cell at column `x` of row `y` is drawn.
    #[must_use]
    pub fn style_at(&self, x: u16, y: u16) -> Style {
        let (_, _, styles) = self.draw();
        styles[usize::from(y)][usize::from(x)]
    }

    fn draw(&self) -> (Vec<String>, (u16, u16), Vec<Vec<Style>>) {
        let (width, height) = self.kit.size();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test backend");
        terminal
            .draw(|frame| self.kit.render(&self.app, frame))
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let rows = (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect();
        let styles = (0..height)
            .map(|y| (0..width).map(|x| buffer[(x, y)].style()).collect())
            .collect();
        let cursor = terminal.get_cursor_position().expect("cursor");
        (rows, (cursor.x, cursor.y), styles)
    }
}
