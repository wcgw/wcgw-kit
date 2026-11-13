//! A [`Rendering`] as a wcgw-kit buffer's [`Document`](wcgw_kit::doc::Document), in the theme's colours (D16).
//!
//! [`styles`] gives the styles a text is drawn in from the theme's groups, so it follows the colorscheme;
//! [`lines`] turns a rendering's lines into a kit document's, its pictures into picture lines for the kit to draw,
//! and [`linked_lines`] aims its Links at the application's targets too; [`document`] and [`linked_document`] make
//! a document of one block of them. The lines are laid out for a width already: they are made again when the width
//! changes.

use ratatui_core::style::{Modifier, Style};
use wcgw_kit::doc::{Content, Document, Line, Span};
use wcgw_kit::hl::{Group, Theme};

use crate::{Link, Rendering, Rgb, Styles};

mod links;

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

/// `rendering` as a document of one block keyed `key`: its [`lines`].
#[must_use]
pub fn document<T>(
    rendering: &Rendering,
    key: &str,
    source: impl Fn(usize) -> String,
) -> Document<T> {
    one_block(key, lines(rendering, source))
}

/// `rendering`'s lines as a kit document's: each line as laid out, in its own colours, and each picture a picture
/// line at the size it was laid out for, its source named by `source` from its image's index. For an application
/// that puts them in blocks of its own, or colours them further.
#[must_use]
pub fn lines<T>(rendering: &Rendering, source: impl Fn(usize) -> String) -> Vec<Line<T>> {
    let pieces: Vec<Vec<Piece>> = rendering
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| Piece {
                    content: span.content.to_string(),
                    style: span.style,
                    link: None,
                })
                .collect()
        })
        .collect();
    build(rendering, &pieces, source, |_: usize| None)
}

/// `rendering` as a document of one block keyed `key`: its [`linked_lines`].
#[must_use]
pub fn linked_document<T: Clone>(
    text: &crate::Document,
    rendering: &Rendering,
    key: &str,
    source: impl Fn(usize) -> String,
    target: impl Fn(usize, &Link) -> Option<T>,
) -> Document<T> {
    one_block(key, linked_lines(text, rendering, source, target))
}

/// As [`lines`], with each Link's text and number (`[1]`) aimed at what `target` makes of the Link at that index of
/// `text`'s, so that `<CR>` or a click on it opens it. `rendering` is `text`'s, as laid out: Links are found by its
/// styles, so colour the lines further (a heat map's shading) after, not before.
///
/// The rendering doesn't say which cells are a Link's; they are found by its styles: a Link's number in the style
/// for numbers, and its text in the Link style just before. A Link's text wrapped across lines is found whole, but
/// in a table, where the cells beside it come between, only its last line is.
#[must_use]
pub fn linked_lines<T: Clone>(
    text: &crate::Document,
    rendering: &Rendering,
    source: impl Fn(usize) -> String,
    target: impl Fn(usize, &Link) -> Option<T>,
) -> Vec<Line<T>> {
    let pieces = links::find(text, rendering);
    let targets: Vec<Option<T>> = text
        .links()
        .iter()
        .enumerate()
        .map(|(index, link)| target(index, link))
        .collect();
    build(rendering, &pieces, source, |index: usize| {
        targets.get(index).cloned().flatten()
    })
}

fn one_block<T>(key: &str, lines: Vec<Line<T>>) -> Document<T> {
    let mut document = Document::new();
    document.push_back(key, Content::from(lines));
    document
}

/// A piece of a laid out line: its text, its style as laid out, and the index of the Link it is part of.
struct Piece {
    content: String,
    style: Style,
    link: Option<usize>,
}

fn build<T>(
    rendering: &Rendering,
    pieces: &[Vec<Piece>],
    source: impl Fn(usize) -> String,
    target: impl Fn(usize) -> Option<T>,
) -> Vec<Line<T>> {
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
        let spans = pieces[index]
            .iter()
            .map(|piece| {
                let span = Span::new(piece.content.clone(), Group::NORMAL)
                    .with_style(line.style.patch(piece.style));
                match piece.link.and_then(&target) {
                    Some(target) => span.with_target(target),
                    None => span,
                }
            })
            .collect();
        lines.push(Line::Text(spans));
    }
    lines
}

/// The background a style draws on, as `page` takes it, when it is a colour of its own.
#[must_use]
pub fn page_of(style: Style) -> Option<Rgb> {
    match style.bg {
        Some(ratatui_core::style::Color::Rgb(r, g, b)) => Some(Rgb(r, g, b)),
        _ => None,
    }
}
