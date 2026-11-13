//! Highlight groups and colorschemes (D7, D8): the names text is styled by, what colours them, and `:hi`.
//!
//! The kit defines Neovim's standard groups ([`Group::STANDARD`]). Applications define their own with a default
//! link to a standard one ([`Theme::define`]), so any colorscheme colours them; they may also ship colours of their
//! own for a colorscheme ([`Theme::add_overrides`]). Colorschemes are Vim files of `:hi` lines: the built-in ones
//! are parsed exactly as a user's.

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::ops::BitOr;

use ratatui::style::{Color, Modifier, Style};

/// A named highlight group. The colorscheme colours it; `:hi` changes it by name.
///
/// Groups compare by name. The kit's are constants of this type; an application declares its own as constants too
/// (`const HANDLE: Group = Group::new("DevnullHandle")`) and defines them with [`Theme::define`] before use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Group(&'static str);

impl Default for Group {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Declares the kit's groups as constants of [`Group`], and [`Group::STANDARD`], from one list.
macro_rules! standard {
    ($($(#[$doc:meta])* $constant:ident => $name:literal,)*) => {
        impl Group {
            $($(#[$doc])* pub const $constant: Group = Group($name);)*

            /// The kit's groups, which every colorscheme defines, in the order colorschemes list them.
            pub const STANDARD: &[Group] = &[$(Group::$constant,)*];
        }
    };
}

standard! {
    /// Plain text; also the background of the whole screen.
    NORMAL => "Normal",
    /// The background of floating windows: popups, pickers, hover cards.
    NORMAL_FLOAT => "NormalFloat",
    /// The border of floating windows.
    FLOAT_BORDER => "FloatBorder",
    /// The title in a floating window's border.
    FLOAT_TITLE => "FloatTitle",
    /// Text that is not part of the content, such as wrap markers.
    NON_TEXT => "NonText",
    /// The `~` filling rows past the end of a buffer.
    END_OF_BUFFER => "EndOfBuffer",
    /// Text standing for other text.
    CONCEAL => "Conceal",
    /// The statusline of the current window.
    STATUS_LINE => "StatusLine",
    /// The statuslines of the other windows.
    STATUS_LINE_NC => "StatusLineNC",
    /// The mode in the statusline, in Normal mode.
    STATUS_LINE_MODE_NORMAL => "StatusLineModeNormal",
    /// The mode in the statusline, in Visual mode.
    STATUS_LINE_MODE_VISUAL => "StatusLineModeVisual",
    /// The mode in the statusline, on the command line.
    STATUS_LINE_MODE_COMMAND => "StatusLineModeCommand",
    /// The mode in the statusline, in Insert mode.
    STATUS_LINE_MODE_INSERT => "StatusLineModeInsert",
    /// The other buffers in the tabline.
    TAB_LINE => "TabLine",
    /// The current buffer in the tabline.
    TAB_LINE_SEL => "TabLineSel",
    /// The rest of the tabline.
    TAB_LINE_FILL => "TabLineFill",
    /// The winbar of the current window.
    WIN_BAR => "WinBar",
    /// The winbars of the other windows.
    WIN_BAR_NC => "WinBarNC",
    /// The column between side-by-side windows.
    WIN_SEPARATOR => "WinSeparator",
    /// The row holding the cursor, where a buffer highlights it.
    CURSOR_LINE => "CursorLine",
    /// The Visual mode selection.
    VISUAL => "Visual",
    /// Matches of the last search, while `'hlsearch'` shows them.
    SEARCH => "Search",
    /// The match of the last search under the cursor.
    CUR_SEARCH => "CurSearch",
    /// The match a search being typed would go to, with `'incsearch'`.
    INC_SEARCH => "IncSearch",
    /// A popup menu, such as the completions of the command line.
    PMENU => "Pmenu",
    /// The chosen item of a popup menu.
    PMENU_SEL => "PmenuSel",
    /// What matched the query in a picker's list.
    PMENU_MATCH => "PmenuMatch",
    /// A key in the which-key popup.
    WHICH_KEY => "WhichKey",
    /// The arrow between a key and what it does.
    WHICH_KEY_SEPARATOR => "WhichKeySeparator",
    /// What a key does, in the which-key popup.
    WHICH_KEY_DESC => "WhichKeyDesc",
    /// A key that leads to more keys, in the which-key popup.
    WHICH_KEY_GROUP => "WhichKeyGroup",
    /// The summary row of a closed fold.
    FOLDED => "Folded",
    /// Error messages.
    ERROR_MSG => "ErrorMsg",
    /// Warnings, such as a search wrapping around.
    WARNING_MSG => "WarningMsg",
    /// "Press ENTER" prompts.
    MORE_MSG => "MoreMsg",
    /// The mode shown in the command line.
    MODE_MSG => "ModeMsg",
    /// Questions, such as a confirmation.
    QUESTION => "Question",
    /// Titles and headings.
    TITLE => "Title",
    /// Directories, mailboxes, channels: places that hold things.
    DIRECTORY => "Directory",
    /// Bold text.
    BOLD => "Bold",
    /// Italic text.
    ITALIC => "Italic",
    /// Comments, and text that matters less.
    COMMENT => "Comment",
    /// Constants.
    CONSTANT => "Constant",
    /// Strings.
    STRING => "String",
    /// Characters.
    CHARACTER => "Character",
    /// Numbers.
    NUMBER => "Number",
    /// Booleans.
    BOOLEAN => "Boolean",
    /// Names.
    IDENTIFIER => "Identifier",
    /// Functions.
    FUNCTION => "Function",
    /// Statements.
    STATEMENT => "Statement",
    /// Keywords.
    KEYWORD => "Keyword",
    /// Operators.
    OPERATOR => "Operator",
    /// Preprocessor directives.
    PRE_PROC => "PreProc",
    /// Types.
    TYPE => "Type",
    /// Special symbols.
    SPECIAL => "Special",
    /// Special characters.
    SPECIAL_CHAR => "SpecialChar",
    /// Delimiters.
    DELIMITER => "Delimiter",
    /// Links, and other text that stands out by being underlined.
    UNDERLINED => "Underlined",
    /// Things to do.
    TODO => "Todo",
    /// Errors.
    ERROR => "Error",
    /// Diagnostics: an error.
    DIAGNOSTIC_ERROR => "DiagnosticError",
    /// Diagnostics: a warning.
    DIAGNOSTIC_WARN => "DiagnosticWarn",
    /// Diagnostics: information.
    DIAGNOSTIC_INFO => "DiagnosticInfo",
    /// Diagnostics: a hint.
    DIAGNOSTIC_HINT => "DiagnosticHint",
    /// Diagnostics: all is well.
    DIAGNOSTIC_OK => "DiagnosticOk",
    /// Something added.
    ADDED => "Added",
    /// Something changed.
    CHANGED => "Changed",
    /// Something removed.
    REMOVED => "Removed",
    /// A line added, in a diff.
    DIFF_ADD => "DiffAdd",
    /// A line changed, in a diff.
    DIFF_CHANGE => "DiffChange",
    /// A line deleted, in a diff.
    DIFF_DELETE => "DiffDelete",
    /// The text changed in a changed line, in a diff.
    DIFF_TEXT => "DiffText",
    /// Headings in rich text.
    MARKUP_HEADING => "@markup.heading",
    /// Strong text.
    MARKUP_STRONG => "@markup.strong",
    /// Emphasised text.
    MARKUP_ITALIC => "@markup.italic",
    /// Struck-through text.
    MARKUP_STRIKETHROUGH => "@markup.strikethrough",
    /// Underlined text.
    MARKUP_UNDERLINE => "@markup.underline",
    /// Links' text.
    MARKUP_LINK => "@markup.link",
    /// Links shown as their address.
    MARKUP_LINK_URL => "@markup.link.url",
    /// Links' labels.
    MARKUP_LINK_LABEL => "@markup.link.label",
    /// Code.
    MARKUP_RAW => "@markup.raw",
    /// Quotations.
    MARKUP_QUOTE => "@markup.quote",
    /// List markers.
    MARKUP_LIST => "@markup.list",
}

impl Group {
    /// The group called `name`; [`Theme::define`] makes an application's known.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// The name `:hi` knows the group by.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.0
    }
}

/// Text attributes, as `:hi … gui=` lists them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attrs(u8);

impl Attrs {
    /// None.
    pub const NONE: Self = Self(0);
    /// `bold`.
    pub const BOLD: Self = Self(1);
    /// `italic`.
    pub const ITALIC: Self = Self(1 << 1);
    /// `underline`.
    pub const UNDERLINE: Self = Self(1 << 2);
    /// `undercurl`, drawn as an underline.
    pub const UNDERCURL: Self = Self(1 << 3);
    /// `strikethrough`.
    pub const STRIKETHROUGH: Self = Self(1 << 4);
    /// `reverse` (also `inverse`).
    pub const REVERSE: Self = Self(1 << 5);

    const NAMED: [(&str, Self); 6] = [
        ("bold", Self::BOLD),
        ("italic", Self::ITALIC),
        ("underline", Self::UNDERLINE),
        ("undercurl", Self::UNDERCURL),
        ("strikethrough", Self::STRIKETHROUGH),
        ("reverse", Self::REVERSE),
    ];

    /// Whether all of `other` is set.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for Attrs {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// How a group looks, as `:hi` sets it: `None` leaves a colour to the terminal (`NONE`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Spec {
    /// `guifg`.
    pub fg: Option<Color>,
    /// `guibg`.
    pub bg: Option<Color>,
    /// `guisp`: the underline colour.
    pub sp: Option<Color>,
    /// `gui`.
    pub attrs: Attrs,
}

impl Spec {
    /// The ratatui style drawing it.
    #[must_use]
    pub fn style(self) -> Style {
        let mut style = Style::new();
        style.fg = self.fg;
        style.bg = self.bg;
        style.underline_color = self.sp;
        let modifiers = [
            (Attrs::BOLD, Modifier::BOLD),
            (Attrs::ITALIC, Modifier::ITALIC),
            (Attrs::UNDERLINE, Modifier::UNDERLINED),
            (Attrs::UNDERCURL, Modifier::UNDERLINED),
            (Attrs::STRIKETHROUGH, Modifier::CROSSED_OUT),
            (Attrs::REVERSE, Modifier::REVERSED),
        ];
        for (attr, modifier) in modifiers {
            if self.attrs.contains(attr) {
                style = style.add_modifier(modifier);
            }
        }
        style
    }

    /// `:hi`'s description of it, in Vim's order: `gui=… guifg=… guibg=… guisp=…`.
    fn describe(self) -> String {
        let mut text = String::new();
        let attrs: Vec<&str> = Attrs::NAMED
            .iter()
            .filter(|(_, attr)| self.attrs.contains(*attr))
            .map(|(name, _)| *name)
            .collect();
        if !attrs.is_empty() {
            let _ = write!(text, " gui={}", attrs.join(","));
        }
        for (key, color) in [("guifg", self.fg), ("guibg", self.bg), ("guisp", self.sp)] {
            if let Some(color) = color {
                let _ = write!(text, " {key}={}", color_name(color));
            }
        }
        if text.is_empty() {
            " cleared".to_owned()
        } else {
            text
        }
    }
}

/// The colorschemes built in, by name: `tokyonight` is `tokyonight-night`, as in tokyonight.nvim.
const BUILT_IN: &[(&str, &str)] = &[
    (
        "tokyonight-moon",
        include_str!("../../colors/tokyonight-moon.vim"),
    ),
    (
        "tokyonight-night",
        include_str!("../../colors/tokyonight-night.vim"),
    ),
    (
        "tokyonight",
        include_str!("../../colors/tokyonight-night.vim"),
    ),
    (
        "tokyonight-storm",
        include_str!("../../colors/tokyonight-storm.vim"),
    ),
    (
        "tokyonight-day",
        include_str!("../../colors/tokyonight-day.vim"),
    ),
    ("default", include_str!("../../colors/default.vim")),
];

/// The colorscheme the kit starts with (D8).
pub const DEFAULT_COLORSCHEME: &str = "tokyonight-moon";

/// The colorscheme for terminals without true colour (D8).
pub const ANSI_COLORSCHEME: &str = "default";

/// The groups known, their colours and links, and the colorschemes that set them.
#[derive(Debug, Clone)]
pub struct Theme {
    /// The colorscheme loaded last.
    scheme: String,
    /// Every group known, standard ones first, in the order defined: what `:hi` completes.
    known: Vec<Group>,
    /// Where an application's group links when the colorscheme says nothing of it, as Vim's `:hi default link`.
    defaults: HashMap<Group, Group>,
    /// Colours the colorscheme and `:hi` gave.
    specs: HashMap<Group, Spec>,
    /// Groups drawn as another, as the colorscheme and `:hi link` said.
    links: HashMap<Group, Group>,
    /// Colorschemes other than the built-in ones, such as files in a `colors/` directory, by name.
    schemes: Vec<(String, Cow<'static, str>)>,
    /// What applications add to a colorscheme of that name, after it loads.
    overrides: HashMap<String, Vec<Cow<'static, str>>>,
}

impl Default for Theme {
    /// The standard groups in [`DEFAULT_COLORSCHEME`].
    fn default() -> Self {
        Self::new()
    }
}

impl Theme {
    /// The standard groups in [`DEFAULT_COLORSCHEME`].
    ///
    /// # Panics
    ///
    /// Never: the default colorscheme is built in, and a test runs every built-in one.
    #[must_use]
    pub fn new() -> Self {
        let mut theme = Self {
            scheme: String::new(),
            known: Group::STANDARD.to_vec(),
            defaults: HashMap::new(),
            specs: HashMap::new(),
            links: HashMap::new(),
            schemes: Vec::new(),
            overrides: HashMap::new(),
        };
        theme
            .colorscheme(DEFAULT_COLORSCHEME)
            .expect("the default colorscheme is built in");
        theme
    }

    /// Makes `group` known, linked to `link` unless the colorscheme or `:hi` say otherwise. Defining it again
    /// changes its default link.
    pub fn define(&mut self, group: Group, link: Option<Group>) {
        if !self.known.contains(&group) {
            self.known.push(group);
        }
        match link {
            Some(link) => self.defaults.insert(group, link),
            None => self.defaults.remove(&group),
        };
    }

    /// Adds `text`, `:hi` lines, to the colorscheme `scheme` whenever it loads, now too if it is loaded: an
    /// application's own colours for that colorscheme.
    ///
    /// # Errors
    ///
    /// When a line is not one a colorscheme may hold; the lines before it apply.
    pub fn add_overrides(
        &mut self,
        scheme: &str,
        text: impl Into<Cow<'static, str>>,
    ) -> Result<(), String> {
        let text = text.into();
        let applies = self.scheme == scheme;
        self.overrides
            .entry(scheme.to_owned())
            .or_default()
            .push(text.clone());
        if applies {
            self.run(&text)?;
        }
        Ok(())
    }

    /// Makes the colorscheme `name` known, as the `:hi` lines `text`, such as a file of a `colors/` directory. A
    /// built-in one of the same name is shadowed.
    pub fn add_colorscheme(&mut self, name: &str, text: impl Into<Cow<'static, str>>) {
        self.schemes.retain(|(known, _)| known != name);
        self.schemes.push((name.to_owned(), text.into()));
    }

    /// The names of the colorschemes `:colorscheme` knows, sorted.
    #[must_use]
    pub fn colorscheme_names(&self) -> Vec<String> {
        let mut names: Vec<String> = BUILT_IN
            .iter()
            .map(|&(name, _)| name.to_owned())
            .chain(self.schemes.iter().map(|(name, _)| name.clone()))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// `:colorscheme {name}`: the colours of the colorscheme `name`, then the applications' for it.
    ///
    /// # Errors
    ///
    /// Vim's E185 when there is no colorscheme by that name; the theme is unchanged. When a line of it can't run,
    /// that line's error; the lines before it apply.
    pub fn colorscheme(&mut self, name: &str) -> Result<(), String> {
        let text = self
            .schemes
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, text)| text.clone())
            .or_else(|| {
                BUILT_IN
                    .iter()
                    .find(|(known, _)| *known == name)
                    .map(|&(_, text)| Cow::Borrowed(text))
            })
            .ok_or_else(|| format!("E185: Cannot find color scheme '{name}'"))?;
        self.specs.clear();
        self.links.clear();
        name.clone_into(&mut self.scheme);
        self.run(&text)?;
        let overrides = self.overrides.get(name).cloned().unwrap_or_default();
        for text in overrides {
            self.run(&text)?;
        }
        Ok(())
    }

    /// Runs the `:hi` lines of a colorscheme: `hi`, `hi!`, `hi link`, `hi clear`, blank lines and `"` comments.
    fn run(&mut self, text: &str) -> Result<(), String> {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('"') {
                continue;
            }
            let mut words = line.split_whitespace();
            let command = words.next().unwrap_or_default();
            if !matches!(command, "hi" | "hi!" | "highlight" | "highlight!") {
                return Err(format!("E492: Not a colorscheme line: {line}"));
            }
            match words.next() {
                Some("clear") => {
                    self.specs.clear();
                    self.links.clear();
                }
                Some("link" | "default") => {
                    let mut rest: Vec<&str> = words.collect();
                    if rest.first() == Some(&"link") {
                        rest.remove(0);
                    }
                    let [from, to] = rest[..] else {
                        return Err(format!("E475: Invalid argument: {line}"));
                    };
                    self.link(from, (to != "NONE").then_some(to))?;
                }
                Some(group) => {
                    let attrs = words
                        .filter(|word| *word != "NONE")
                        .map(|word| {
                            word.split_once('=')
                                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                                .ok_or_else(|| format!("E475: Invalid argument: {word}"))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let group = self.named(group)?;
                    let mut spec = Spec::default();
                    apply(&mut spec, &attrs)?;
                    self.specs.insert(group, spec);
                    self.links.remove(&group);
                }
                None => return Err(format!("E475: Invalid argument: {line}")),
            }
        }
        Ok(())
    }

    /// The colorscheme's name, as `:colorscheme` without arguments shows it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.scheme
    }

    /// Every group known, standard ones first: what `:hi` completes.
    #[must_use]
    pub fn groups(&self) -> &[Group] {
        &self.known
    }

    /// How `group` is drawn.
    #[must_use]
    pub fn style(&self, group: Group) -> Style {
        self.spec(group).style()
    }

    /// The colours `group` is drawn in, following links.
    #[must_use]
    pub fn spec(&self, group: Group) -> Spec {
        self.specs
            .get(&self.linked(group))
            .copied()
            .unwrap_or_default()
    }

    /// The group `group` is drawn as: following `:hi link`s, then default links of groups the colorscheme left
    /// alone. A loop of links ends where it started to repeat.
    fn linked(&self, group: Group) -> Group {
        let mut seen = vec![group];
        let mut current = group;
        loop {
            let next = self.links.get(&current).copied().or_else(|| {
                (!self.specs.contains_key(&current))
                    .then(|| self.defaults.get(&current).copied())
                    .flatten()
            });
            match next {
                Some(next) if !seen.contains(&next) => {
                    seen.push(next);
                    current = next;
                }
                _ => return current,
            }
        }
    }

    /// The group `:hi` calls `name`, ignoring case as Vim does.
    ///
    /// # Errors
    ///
    /// Vim's E411 for an unknown group.
    pub fn named(&self, name: &str) -> Result<Group, String> {
        self.known
            .iter()
            .copied()
            .find(|group| group.name().eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("E411: Highlight group not found: {name}"))
    }

    /// `:hi link {from} {to}`: draws `from` as `to`, or as itself again for `None`.
    ///
    /// # Errors
    ///
    /// Vim's E411 for an unknown group.
    pub fn link(&mut self, from: &str, to: Option<&str>) -> Result<(), String> {
        let from = self.named(from)?;
        match to.map(|to| self.named(to)).transpose()? {
            Some(to) => self.links.insert(from, to),
            None => self.links.remove(&from),
        };
        Ok(())
    }

    /// `:hi clear`: every group back to the colorscheme's definition.
    pub fn clear(&mut self) {
        let name = self.scheme.clone();
        let _ = self.colorscheme(&name);
    }

    /// `:hi {group} {key}={value} …`: changes only the attributes given; with none, describes the group.
    ///
    /// # Errors
    ///
    /// Vim's error for an unknown group (E411), key (E423), colour (E254) or attribute (E418). Nothing changes
    /// when any argument is wrong.
    pub fn highlight(
        &mut self,
        group: &str,
        attrs: &[(String, String)],
    ) -> Result<Option<String>, String> {
        let group = self.named(group)?;
        let mut spec = self.specs.get(&group).copied().unwrap_or_default();
        if attrs.is_empty() {
            let link = self.links.get(&group).copied().or_else(|| {
                (!self.specs.contains_key(&group))
                    .then(|| self.defaults.get(&group).copied())
                    .flatten()
            });
            let shown = match link {
                Some(to) => format!(" links to {}", to.name()),
                None => spec.describe(),
            };
            return Ok(Some(format!("{:<15}xxx{shown}", group.name())));
        }
        apply(&mut spec, attrs)?;
        self.specs.insert(group, spec);
        // As in Vim, highlighting a linked group unlinks it.
        self.links.remove(&group);
        Ok(None)
    }
}

/// Sets the attributes `attrs` of `spec`, or leaves it alone when one is wrong.
fn apply(spec: &mut Spec, attrs: &[(String, String)]) -> Result<(), String> {
    let mut changed = *spec;
    for (key, value) in attrs {
        match key.to_ascii_lowercase().as_str() {
            "guifg" | "fg" => changed.fg = color(value)?,
            "guibg" | "bg" => changed.bg = color(value)?,
            "guisp" | "sp" => changed.sp = color(value)?,
            "gui" => changed.attrs = gui(value)?,
            // Terminal-only keys from vimrcs written for 256 colours: the kit draws in true colour.
            "term" | "cterm" | "ctermfg" | "ctermbg" | "ctermul" | "start" | "stop" => {}
            _ => return Err(format!("E423: Illegal argument: {key}={value}")),
        }
    }
    *spec = changed;
    Ok(())
}

/// A `:hi` colour: `#rrggbb`, one of the terminal's named colours, or `NONE`.
fn color(value: &str) -> Result<Option<Color>, String> {
    let named = [
        ("black", Color::Black),
        ("red", Color::Red),
        ("green", Color::Green),
        ("yellow", Color::Yellow),
        ("blue", Color::Blue),
        ("magenta", Color::Magenta),
        ("cyan", Color::Cyan),
        ("gray", Color::Gray),
        ("grey", Color::Gray),
        ("darkgray", Color::DarkGray),
        ("darkgrey", Color::DarkGray),
        ("white", Color::White),
    ];
    if value.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    if let Some(&(_, color)) = named
        .iter()
        .find(|(name, _)| value.eq_ignore_ascii_case(name))
    {
        return Ok(Some(color));
    }
    let hex = value.strip_prefix('#').filter(|hex| hex.len() == 6);
    let rgb = hex.and_then(|hex| u32::from_str_radix(hex, 16).ok());
    match rgb {
        Some(rgb) => Ok(Some(Color::from_u32(rgb))),
        None => Err(format!("E254: Cannot allocate color {value}")),
    }
}

/// A `gui=` value: attribute names separated by commas, or `NONE`.
fn gui(value: &str) -> Result<Attrs, String> {
    value.split(',').try_fold(Attrs::NONE, |attrs, name| {
        let name = name.to_ascii_lowercase();
        if name == "none" {
            return Ok(attrs);
        }
        let name = if name == "inverse" {
            "reverse".to_owned()
        } else {
            name
        };
        Attrs::NAMED
            .iter()
            .find(|(known, _)| *known == name)
            .map(|(_, attr)| attrs | *attr)
            .ok_or_else(|| format!("E418: Illegal value: {name}"))
    })
}

/// How `:hi` writes a colour back.
fn color_name(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        other => format!("{other:?}"),
    }
}
