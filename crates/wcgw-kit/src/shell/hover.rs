//! `K`: a card about what is under the cursor, floating next to it until the next key. What it says is the
//! application's ([`Kit::show_hover`]).

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::text::{Line as TextLine, Span as TextSpan};
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::Kit;
use crate::app::App;
use crate::doc::Layout;
use crate::hl::Group;

/// The widest a card's text is, in cells, before it wraps.
const MAX_WIDTH: u16 = 60;

impl<A: App> Kit<A> {
    /// Draws the card, if any, just below the cursor at `cursor`, or above it when there is more room there.
    pub(super) fn render_hover(&self, frame: &mut Frame<'_>, cursor: Position) {
        let Some(hover) = &self.hover else { return };
        let (width, _) = self.size;
        let max_inner = MAX_WIDTH.min(width.saturating_sub(2));
        // Rows between the tabline and the statusline.
        let windows = self.windows_area();
        let (first, last) = (windows.y, windows.bottom().saturating_sub(1));
        if max_inner < 4 || last < first + 2 {
            return;
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
        let inner_width = u16::try_from(inner_width).unwrap_or(max_inner);
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
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        let rows: Vec<TextLine<'_>> = (0..layout.row_count().min(usize::from(inner.height)))
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
        self.draw_pictures(frame.buffer_mut(), &layout, 0, inner);
    }
}
