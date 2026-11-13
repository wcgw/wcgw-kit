//! Icons (D9): the kit's and the application's, each a Nerd Font glyph with a plain fallback, both drawn two cells
//! wide (the glyph and a space it may spill into) so that `:set icons=` never shifts a column.

use unicode_width::UnicodeWidthStr;

/// Which glyphs icons are drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IconSet {
    /// Nerd Font's where the terminal ships them (Ghostty, kitty, WezTerm), plain ones elsewhere.
    Auto,
    /// Nerd Font's.
    Nerd,
    /// Plain characters.
    Plain,
}

impl IconSet {
    pub(super) const NAMES: [&'static str; 3] = ["auto", "nerd", "plain"];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Nerd => "nerd",
            Self::Plain => "plain",
        }
    }

    pub(super) fn named(name: &str) -> Option<Self> {
        match name {
            "auto" => Some(Self::Auto),
            "nerd" => Some(Self::Nerd),
            "plain" => Some(Self::Plain),
            _ => None,
        }
    }
}

/// The cells every icon takes.
const WIDTH: usize = 2;

/// The kit's own icons: name, Nerd Font glyph, plain fallback.
const KIT: [(&str, &str, &str); 9] = [
    ("unread", "\u{f009a}", "●"),
    ("lock", "\u{f023}", "*"),
    ("picture", "\u{f02e9}", "▣"),
    ("link", "\u{f0c1}", "↗"),
    ("fold_open", "\u{f47c}", "▾"),
    ("fold_closed", "\u{f460}", "▸"),
    ("error", "\u{f0159}", "✗"),
    ("warning", "\u{f071}", "!"),
    ("loading", "\u{f110}", "…"),
];

/// Every icon known, and which glyphs draw them.
#[derive(Debug, Clone)]
pub(super) struct Icons {
    known: Vec<(&'static str, &'static str, &'static str)>,
    /// Whether the terminal ships Nerd Font's symbols, for [`IconSet::Auto`].
    pub(super) nerd_font_symbols: bool,
}

impl Default for Icons {
    fn default() -> Self {
        Self {
            known: KIT.to_vec(),
            nerd_font_symbols: true,
        }
    }
}

impl Icons {
    /// Makes `name` an icon drawn as `nerd`, or `plain` without a Nerd Font, replacing one of that name.
    pub(super) fn define(&mut self, name: &'static str, nerd: &'static str, plain: &'static str) {
        self.known.retain(|(known, ..)| *known != name);
        self.known.push((name, nerd, plain));
    }

    /// Whether `set` draws Nerd Font's glyphs here.
    pub(super) fn nerd(&self, set: IconSet) -> bool {
        match set {
            IconSet::Auto => self.nerd_font_symbols,
            IconSet::Nerd => true,
            IconSet::Plain => false,
        }
    }

    /// The icon `name` as `set` draws it, two cells wide; blank for an icon nobody defined.
    pub(super) fn get(&self, name: &str, set: IconSet) -> String {
        let nerd = self.nerd(set);
        let glyph = self.known.iter().find(|(known, ..)| *known == name).map_or(
            "",
            |&(_, with_nerd, plain)| if nerd { with_nerd } else { plain },
        );
        // Nerd Font's glyphs are one cell to Unicode, but drawn up to two: the space is for them to spill into.
        let cells = glyph.width().max(usize::from(!glyph.is_empty()));
        format!("{glyph}{}", " ".repeat(WIDTH.saturating_sub(cells)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_icon_takes_two_cells_whichever_glyphs_draw_it() {
        let mut icons = Icons::default();
        icons.define("junk", "\u{f1f8}", "");

        for set in [IconSet::Nerd, IconSet::Plain] {
            for name in ["unread", "junk", "nobody"] {
                assert_eq!(icons.get(name, set).width(), 2, "{name} {set:?}");
            }
        }
        assert_eq!(icons.get("unread", IconSet::Plain), "● ");
        assert_eq!(icons.get("junk", IconSet::Plain), "  ");
        assert_eq!(icons.get("unread", IconSet::Nerd), "\u{f009a} ");
    }

    #[test]
    fn auto_draws_nerd_glyphs_only_where_the_terminal_ships_them() {
        let mut icons = Icons {
            nerd_font_symbols: false,
            ..Icons::default()
        };

        assert_eq!(icons.get("link", IconSet::Auto), "↗ ");
        icons.nerd_font_symbols = true;
        assert_eq!(icons.get("link", IconSet::Auto), "\u{f0c1} ");
    }
}
