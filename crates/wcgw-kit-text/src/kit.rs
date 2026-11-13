//! A [`Rendering`] as a wcgw-kit buffer's [`Document`](wcgw_kit::doc::Document), in the theme's colours (D16).
//!
//! [`styles`] gives the styles a text is drawn in from the theme's groups, so it follows the colorscheme;
//! [`document`] turns a rendering's lines into a document of one block, its pictures into picture lines for the kit
//! to draw. The lines are laid out for a width already: a document of them is made again when the width changes.

use ratatui_core::style::{Modifier, Style};
use wcgw_kit::doc::{Content, Document, Line, Span};
use wcgw_kit::hl::{Group, Theme};

use crate::{Rendering, Rgb, Styles};

/// The styles to draw a text in with `theme`; authors' colours are judged against `page`, the background they show
/// on, and dropped where they would not read (`None` drops them all).
#[must_use]
pub fn styles(theme: &Theme, page: Option<Rgb>) -> Styles {
    let style = |group| theme.style(group);
    Styles {
        text: style(Group::NORMAL),
        page,
        link: style(Group::MARKUP_LINK),
        link_number: style(Group::COMMENT),
        heading: style(Group::MARKUP_HEADING),
        code: style(Group::MARKUP_RAW),
        mark: style(Group::SEARCH),
        quotes: vec![
            style(Group::MARKUP_QUOTE),
            style(Group::COMMENT),
            style(Group::NON_TEXT),
        ],
        attribution: style(Group::COMMENT).add_modifier(Modifier::ITALIC),
        signature: style(Group::COMMENT),
        rule: style(Group::NON_TEXT),
        table_header: style(Group::BOLD),
        image: style(Group::COMMENT).add_modifier(Modifier::ITALIC),
        glyphs: crate::Glyphs::default(),
    }
}

/// `rendering` as a document of one block keyed `key`: each line as laid out, in its own colours, and each picture
/// a picture line at the size it was laid out for, its source named by `source` from its image's index.
#[must_use]
pub fn document<T>(
    rendering: &Rendering,
    key: &str,
    source: impl Fn(usize) -> String,
) -> Document<T> {
    let mut lines: Vec<Line<T>> = Vec::with_capacity(rendering.lines.len());
    let mut skip = 0;
    for (index, line) in rendering.lines.iter().enumerate() {
        if skip > 0 {
            skip -= 1;
            continue;
        }
        if let Some(slot) = rendering.pictures.iter().find(|slot| slot.line == index) {
            lines.push(Line::Image {
                rows: slot.rows,
                cols: slot.cols,
                source: source(slot.image),
            });
            skip = usize::from(slot.rows.saturating_sub(1));
            continue;
        }
        let spans = line
            .spans
            .iter()
            .map(|span| {
                Span::new(span.content.to_string(), Group::NORMAL)
                    .with_style(line.style.patch(span.style))
            })
            .collect();
        lines.push(Line::Text(spans));
    }
    let mut document = Document::new();
    document.push_back(key, Content::from(lines));
    document
}

/// The background a style draws on, as `page` takes it, when it is a colour of its own.
#[must_use]
pub fn page_of(style: Style) -> Option<Rgb> {
    match style.bg {
        Some(ratatui_core::style::Color::Rgb(r, g, b)) => Some(Rgb(r, g, b)),
        _ => None,
    }
}
