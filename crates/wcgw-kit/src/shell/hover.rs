//! `K`: a card about what is under the cursor, floating next to it until the next key; showing it again enters it,
//! as Neovim's hover window. What it says is the application's ([`Kit::show_hover`]).

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::text::{Line as TextLine, Span as TextSpan};
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::{Kit, Mode_};
use crate::app::{App, Effects};
use crate::doc::{Cursor, Document, Layout};
use crate::hl::Group;
use crate::key::{Code, Key, Mods};

/// The widest a card's text is, in cells, before it wraps.
const MAX_WIDTH: u16 = 60;

impl<A: App> Kit<A> {
    /// How wide the card's text is: as wide as its widest line, up to [`MAX_WIDTH`] and the screen; `None` when the
    /// screen is too narrow for a card.
    fn hover_width(&self, hover: &Document<A::Target>) -> Option<u16> {
        let (width, _) = self.size;
        let max_inner = MAX_WIDTH.min(width.saturating_sub(2));
        if max_inner < 4 {
            return None;
        }
        let natural = Layout::new(hover, max_inner);
        let inner_width = (0..natural.row_count())
            .map(|row| {
                natural
                    .segments(row)
                    .map(|(text, _)| text.width())
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(1)
            .clamp(1, usize::from(max_inner));
        Some(u16::try_from(inner_width).unwrap_or(max_inner))
    }

    /// Draws the card, if any, just below the cursor at `cursor`, or above it when there is more room there; once
    /// entered, with its row highlighted and scrolled to.
    pub(super) fn render_hover(&self, frame: &mut Frame<'_>, cursor: Position) {
        let Some(hover) = &self.hover else { return };
        let Some(inner_width) = self.hover_width(hover) else {
            return;
        };
        let (width, _) = self.size;
        // Rows between the tabline and the statusline.
        let windows = self.windows_area();
        let (first, last) = (windows.y, windows.bottom().saturating_sub(1));
        if last < first + 2 {
            return;
        }
        let layout = Layout::new(hover, inner_width);
        let needed = u16::try_from(layout.row_count())
            .unwrap_or(u16::MAX)
            .saturating_add(2);
        let below = (last + 1).saturating_sub(cursor.y + 1);
        let above = cursor.y.saturating_sub(first);
        let (y, card_height) = if needed <= below || below >= above {
            (cursor.y + 1, needed.min(below))
        } else {
            let card_height = needed.min(above);
            (cursor.y - card_height, card_height)
        };
        if card_height < 3 {
            return;
        }
        let card_width = inner_width + 2;
        let x = cursor.x.min(width.saturating_sub(card_width));
        let area = Rect::new(x, y, card_width, card_height);
        let block = Block::bordered()
            .border_style(self.theme.style(Group::FLOAT_BORDER))
            .style(self.theme.style(Group::NORMAL_FLOAT));
        let inner = block.inner(area);
        self.hover_rows.set(inner.height);
        let height = usize::from(inner.height).max(1);
        let (top, entered) = match self.mode {
            // The card may be shorter than when its row was moved to (the cursor it hangs from moved): its row stays
            // in sight.
            Mode_::Hover { row, top, .. } => (top.max((row + 1).saturating_sub(height)), Some(row)),
            _ => (0, None),
        };
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        let rows: Vec<TextLine<'_>> =
            (top..layout.row_count().min(top + usize::from(inner.height)))
                .map(|row| {
                    TextLine::from(
                        layout
                            .segments(row)
                            .map(|(text, group)| {
                                // Plain text takes the card's colours, not the window's.
                                let group = if group == Group::NORMAL {
                                    Group::NORMAL_FLOAT
                                } else {
                                    group
                                };
                                TextSpan::styled(text, self.theme.style(group))
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
        frame.render_widget(Paragraph::new(rows), inner);
        if let Some(row) = entered
            && let Some(shown) = row.checked_sub(top)
            && let Ok(shown) = u16::try_from(shown)
            && shown < inner.height
        {
            let line = Rect::new(inner.x, inner.y + shown, inner.width, 1);
            frame
                .buffer_mut()
                .set_style(line, self.theme.style(Group::CURSOR_LINE));
        }
        self.draw_pictures(frame.buffer_mut(), &layout, top, inner);
    }

    /// A key in the entered card.
    pub(super) fn hover_key(&mut self, app: &mut A, key: Key) -> Effects<A> {
        let Mode_::Hover { row, g, .. } = self.mode else {
            return Vec::new();
        };
        let half = usize::from(self.hover_rows.get() / 2).max(1);
        let ctrl = |c| key == Key::ctrl(c);
        let plain = |c| key == Key::char(c);
        let leave = plain('q') || plain('K') || key.code == Code::Esc;
        if leave {
            self.close_hover();
        } else if key.code == Code::Enter && key.mods == Mods::NONE {
            let target = self.hover_target(row);
            self.close_hover();
            if let Some(target) = target {
                return match self.following(app, |app, kit| app.open(kit, target)) {
                    Ok(effects) => effects,
                    Err(error) => {
                        self.error(error);
                        Vec::new()
                    }
                };
            }
        } else if plain('j') || key.code == Code::Down || ctrl('n') || ctrl('e') {
            self.hover_scroll(true, 1);
        } else if plain('k') || key.code == Code::Up || ctrl('p') || ctrl('y') {
            self.hover_scroll(false, 1);
        } else if ctrl('d') {
            self.hover_scroll(true, half);
        } else if ctrl('u') {
            self.hover_scroll(false, half);
        } else if plain('G') {
            self.hover_scroll(true, usize::MAX);
        } else if plain('g') && g {
            self.hover_scroll(false, usize::MAX);
        } else if plain('g') {
            if let Mode_::Hover { g, .. } = &mut self.mode {
                *g = true;
            }
            return Vec::new();
        }
        if let Mode_::Hover { g, .. } = &mut self.mode {
            *g = false;
        }
        Vec::new()
    }

    /// Moves through the entered card by `rows`, down or up, keeping its row in sight.
    pub(super) fn hover_scroll(&mut self, down: bool, rows: usize) {
        let count = self.hover_row_count();
        let height = usize::from(self.hover_rows.get()).max(1);
        let Mode_::Hover { row, top, .. } = &mut self.mode else {
            return;
        };
        *row = if down {
            row.saturating_add(rows).min(count.saturating_sub(1))
        } else {
            row.saturating_sub(rows)
        };
        if *row < *top {
            *top = *row;
        } else if *row >= *top + height {
            *top = *row + 1 - height;
        }
    }

    /// How many rows the card has, as drawn.
    fn hover_row_count(&self) -> usize {
        let Some(hover) = &self.hover else { return 0 };
        self.hover_width(hover)
            .map_or(0, |width| Layout::new(hover, width).row_count())
    }

    /// What the card's `row` refers to: the first text on it that refers to something, else its block.
    fn hover_target(&self, row: usize) -> Option<A::Target> {
        let hover = self.hover.as_ref()?;
        let width = self.hover_width(hover)?;
        let layout = Layout::new(hover, width);
        if row >= layout.row_count() {
            return None;
        }
        (0..usize::from(width))
            .find_map(|col| {
                Cursor::at_screen(&layout, row, col)
                    .target(&layout)
                    .cloned()
            })
            .or_else(|| Cursor::at_row(&layout, row).block_target(&layout).cloned())
    }
}
