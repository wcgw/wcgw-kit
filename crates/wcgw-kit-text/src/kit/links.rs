//! Which of a rendering's cells are a Link's, found by their styles, the rendering not saying.

use ratatui_core::style::Style;

use super::Piece;
use crate::{Document, Rendering};

/// `rendering`'s lines in pieces, a Link's number cut from what it was laid out with, each with the Link it is
/// part of.
pub(super) fn find(text: &Document, rendering: &Rendering) -> Vec<Vec<Piece>> {
    let styles = &text.styles;
    let mut lines: Vec<Vec<Piece>> = rendering
        .lines
        .iter()
        .map(|line| {
            let mut pieces = Vec::new();
            for span in &line.spans {
                if text.numbered && span.style == styles.link_number {
                    numbers(&span.content, span.style, text.links.len(), &mut pieces);
                } else {
                    pieces.push(Piece {
                        content: span.content.to_string(),
                        style: span.style,
                        link: None,
                    });
                }
            }
            pieces
        })
        .collect();
    let is_link = |style: Style| is_link_style(style, styles.link);
    if text.numbered {
        before_numbers(&mut lines, is_link);
    } else {
        by_address(&mut lines, text, is_link);
    }
    lines
}

/// Whether a piece in `style` is in the Link style `link`: its colour, and at least its modifiers. A Link style of
/// neither finds nothing.
fn is_link_style(style: Style, link: Style) -> bool {
    let marked = link.fg.is_some() || !link.add_modifier.is_empty();
    marked
        && link.fg.is_none_or(|fg| style.fg == Some(fg))
        && style.add_modifier.contains(link.add_modifier)
}

/// `content` in pieces, each Link number in it (`[3]`, of Links `1..=count`) a piece of its own aimed at that Link.
fn numbers(content: &str, style: Style, count: usize, pieces: &mut Vec<Piece>) {
    let mut rest = content;
    let mut plain = String::new();
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let digits = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        let number = after[..digits].parse::<usize>().ok();
        match number {
            Some(number) if (1..=count).contains(&number) && after[digits..].starts_with(']') => {
                plain.push_str(&rest[..open]);
                if !plain.is_empty() {
                    pieces.push(Piece {
                        content: std::mem::take(&mut plain),
                        style,
                        link: None,
                    });
                }
                let end = open + 1 + digits + 1;
                pieces.push(Piece {
                    content: rest[open..end].to_owned(),
                    style,
                    link: Some(number - 1),
                });
                rest = &rest[end..];
            }
            _ => {
                plain.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    plain.push_str(rest);
    if !plain.is_empty() {
        pieces.push(Piece {
            content: plain,
            style,
            link: None,
        });
    }
}

/// HTML's Links: the pieces in the Link style before a Link's number are its text, across lines; words not in it
/// between them and the number mean they were another's. Blanks, quote bars and bullets come between lines of a
/// Link's text, and don't count.
fn before_numbers(lines: &mut [Vec<Piece>], is_link: impl Fn(Style) -> bool) {
    let mut pending: Vec<(usize, usize)> = Vec::new();
    for row in 0..lines.len() {
        for at in 0..lines[row].len() {
            let piece = &lines[row][at];
            if let Some(link) = piece.link {
                for &(row, at) in &pending {
                    lines[row][at].link = Some(link);
                }
                pending.clear();
            } else if is_link(piece.style) {
                pending.push((row, at));
            } else if piece.content.chars().any(char::is_alphanumeric) {
                pending.clear();
            }
        }
    }
}

/// Plain text's Links are their addresses: a run of pieces in the Link style is the Link whose address starts with
/// it, or goes on with it, cut across lines.
fn by_address(lines: &mut [Vec<Piece>], text: &Document, is_link: impl Fn(Style) -> bool) {
    // The Link being read, and how much of its address has been.
    let mut current: Option<(usize, usize)> = None;
    for line in lines.iter_mut() {
        let mut at = 0;
        while at < line.len() {
            if !is_link(line[at].style) {
                at += 1;
                continue;
            }
            let end = (at..line.len())
                .find(|&end| !is_link(line[end].style))
                .unwrap_or(line.len());
            let run: String = line[at..end]
                .iter()
                .map(|piece| piece.content.as_str())
                .collect();
            let goes_on = current.filter(|&(link, read)| {
                text.links[link]
                    .url
                    .get(read..)
                    .is_some_and(|rest| !rest.is_empty() && rest.starts_with(&run))
            });
            current = match goes_on {
                Some((link, read)) => Some((link, read + run.len())),
                None => text
                    .links
                    .iter()
                    .position(|link| link.url.starts_with(&run))
                    .map(|link| (link, run.len())),
            };
            if let Some((link, _)) = current {
                for piece in &mut line[at..end] {
                    piece.link = Some(link);
                }
            }
            at = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_cut_from_text_laid_out_with_it() {
        let style = Style::new();
        let mut pieces = Vec::new();

        numbers("quoted[2] and [x] [9][1", style, 3, &mut pieces);

        let cut: Vec<(&str, Option<usize>)> = pieces
            .iter()
            .map(|piece| (piece.content.as_str(), piece.link))
            .collect();
        assert_eq!(
            cut,
            [("quoted", None), ("[2]", Some(1)), (" and [x] [9][1", None)]
        );
    }
}
