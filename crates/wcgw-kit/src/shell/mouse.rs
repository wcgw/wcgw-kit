//! The mouse (D5), as Neovim has it with `'mouse'` set: the wheel scrolls the window under the pointer, a click
//! focuses the window it lands in and puts the cursor there, a click on the tabline shows that buffer, and a click on a
//! picker's list chooses. Clicks are keys too (`<LeftMouse>`, `<2-LeftMouse>`), so a map can make one open what it
//! lands on.

use ratatui::layout::Rect;

use super::{Kit, Mode_};
use crate::app::{App, Effects};
use crate::doc::Cursor;
use crate::key::{Code, Key, Mouse};
use crate::keymap::Scroll;

/// How many rows a turn of the wheel scrolls, as Neovim's `'mousescroll'`.
const WHEEL_ROWS: usize = 3;

impl<A: App> Kit<A> {
    /// The mouse did `mouse` over the cell at `column`, `row`.
    pub(super) fn mouse(&mut self, app: &mut A, mouse: Mouse, column: u16, row: u16) -> Effects<A> {
        if !self.options.mouse {
            return Vec::new();
        }
        match &self.mode {
            Mode_::Picker(_) => return self.picker_click(app, mouse, column, row),
            Mode_::Cmdline(_) | Mode_::Confirm(_) | Mode_::MorePrompt { .. } => return Vec::new(),
            Mode_::Insert(_) if matches!(mouse, Mouse::Left | Mouse::DoubleLeft) => {
                self.mode = Mode_::Normal;
            }
            Mode_::Normal | Mode_::Visual(_) | Mode_::Insert(_) => {}
        }
        if matches!(mouse, Mouse::WheelUp | Mouse::WheelDown) {
            self.wheel(mouse == Mouse::WheelDown, column, row);
            return Vec::new();
        }
        if row < self.tabline_rows() {
            if let Some(id) = self.tab_at(column) {
                self.show_buffer(id);
            }
            return Vec::new();
        }
        if mouse == Mouse::Middle || mouse == Mouse::Right {
            return Vec::new();
        }
        let Some((window, text, composer)) = self.window_at(column, row) else {
            return Vec::new();
        };
        self.windows.focus_id(window);
        self.hover = None;
        if composer.is_some_and(|composer| composer.contains((column, row).into())) {
            let _ = self.start_insert();
            return Vec::new();
        }
        if !text.contains((column, row).into()) {
            return Vec::new();
        }
        let viewport = self.viewport();
        let buffers = &self.buffers;
        let current = self.windows.current_mut();
        let layout = current.layout(buffers, text);
        let top = current.top(&layout, viewport);
        let clicked = top + usize::from(row - text.y);
        if clicked < layout.row_count() {
            current.cursor = Cursor::at_screen(&layout, clicked, usize::from(column - text.x));
        }
        // As any key, a click can be mapped: `<LeftMouse>` to open what it lands on, say.
        let ran = self
            .keymap
            .feed(self.keymap_mode(), Key::new(Code::Mouse(mouse)));
        self.run_all(app, ran)
    }

    /// Scrolls the window under `column`, `row` by a turn of the wheel, down or up, without making it current.
    fn wheel(&mut self, down: bool, column: u16, row: u16) {
        let Some((id, text, _)) = self.window_at(column, row) else {
            return;
        };
        let viewport = super::window::Viewport {
            height: usize::from(text.height),
            scrolloff: self.options.scrolloff,
        };
        let buffers = &self.buffers;
        let Some(window) = self.windows.find_mut(id) else {
            return;
        };
        let layout = window.layout(buffers, text);
        let scroll = if down {
            Scroll::LineDown
        } else {
            Scroll::LineUp
        };
        window.scroll(&layout, viewport, scroll, WHEEL_ROWS);
    }

    /// The window over the cell at `column`, `row`, with its text area and its composer's, if it shows one.
    fn window_at(&self, column: u16, row: u16) -> Option<(super::WindowId, Rect, Option<Rect>)> {
        let area = self.windows_area();
        self.windows
            .arrange(area)
            .windows
            .into_iter()
            .find(|(_, rect)| rect.contains((column, row).into()))
            .and_then(|(id, rect)| {
                let buffer = self.windows.find(id)?.buffer;
                let text = self.text_area(rect, area, buffer);
                let rows = self.composer_rows(buffer);
                let composer =
                    (rows > 0).then(|| Rect::new(rect.x, text.bottom(), rect.width, rows));
                Some((id, text, composer))
            })
    }

    /// The buffer whose name the tabline shows at `column`.
    fn tab_at(&self, column: u16) -> Option<crate::app::BufferId> {
        let mut shown = self.listed();
        if shown.is_empty() {
            shown.push(self.windows.current().buffer);
        }
        let mut left = 0u16;
        for id in shown {
            let width = u16::try_from(
                unicode_width::UnicodeWidthStr::width(self.buffers[id.0].name.as_str()) + 2,
            )
            .unwrap_or(u16::MAX);
            if column < left.saturating_add(width) {
                return Some(id);
            }
            left = left.saturating_add(width);
        }
        None
    }
}
