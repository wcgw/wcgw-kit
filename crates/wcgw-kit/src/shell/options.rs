//! The options `:set` changes.

use wcgw_kit_gfx::Graphics;

use super::icons::IconSet;

use crate::doc::CaseMode;
use crate::ex::{SetArg, SetOp};
use crate::key::{Key, parse_keys};
use crate::keymap::Keymap;

/// The options the kit has, with Neovim's defaults.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // Vim's flags, each an option of its own.
pub(super) struct Options {
    pub(super) ignorecase: bool,
    pub(super) smartcase: bool,
    /// Rows of context kept above and below the cursor.
    pub(super) scrolloff: usize,
    /// How long, in milliseconds, an ambiguous key sequence waits for more.
    pub(super) timeoutlen: u64,
    /// How pictures are drawn.
    pub(super) graphics: Graphics,
    /// Whether the last search's matches are shown, as Neovim does by default.
    pub(super) hlsearch: bool,
    /// Whether a search being typed shows where it would go, as Neovim does by default.
    pub(super) incsearch: bool,
    /// When the tabline shows: 0 never, 1 with two listed buffers or more, 2 always (D10).
    pub(super) showtabline: usize,
    /// Where statuslines are: 2 one per window, 3 one for all, at the bottom (D10).
    pub(super) laststatus: usize,
    /// Whether the mouse is the kit's (D5): Neovim's `'mouse'`, which any value but none turns on.
    pub(super) mouse: bool,
    /// Which glyphs icons are drawn with (D9).
    pub(super) icons: IconSet,
    /// Whether keys typed toward a binding show at the right of the command line: off, as the author's Neovim has
    /// it (`noshowcmd`), since which-key shows them.
    pub(super) showcmd: bool,
    /// The widest prose is laid out, in cells, however wide its window: 120, as the author's Neovim has it; 0 for the
    /// window's width. Lists fill their windows whatever it says.
    pub(super) textwidth: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            ignorecase: false,
            smartcase: false,
            scrolloff: 0,
            timeoutlen: 1000,
            graphics: Graphics::HalfBlocks,
            hlsearch: true,
            incsearch: true,
            showtabline: 2,
            laststatus: 3,
            mouse: true,
            icons: IconSet::Auto,
            showcmd: false,
            textwidth: 120,
        }
    }
}

/// What a `no` or `inv` prefix does to a flag.
#[derive(Clone, Copy)]
enum Flip {
    Off,
    Toggle,
}

impl Flip {
    /// The equivalent operation. Turning off is written as assigning nothing, which only flags accept here.
    fn op(self) -> SetOp {
        match self {
            Self::Off => SetOp::Assign(String::new()),
            Self::Toggle => SetOp::Toggle,
        }
    }
}

/// An option's name, its short name, and what it holds.
#[derive(Clone, Copy)]
enum Opt {
    Flag(fn(&mut Options) -> &mut bool),
    Number(fn(&mut Options) -> &mut u64),
    Count(fn(&mut Options) -> &mut usize),
    /// A count of a few values, as `'laststatus'`: those it may take.
    Choice(fn(&mut Options) -> &mut usize, &'static [usize]),
    /// `leader`: the key `<leader>` stands for, kept by the keymap.
    Leader,
    /// `graphics`: how pictures are drawn.
    Graphics,
    /// `mouse`: Neovim's modes (`a`, `nvi`, …), all taken as on; empty for off.
    Mouse,
    /// `icons`: `auto`, `nerd` or `plain`.
    Icons,
}

const OPTIONS: &[(&str, &str, Opt)] = &[
    ("ignorecase", "ic", Opt::Flag(|o| &mut o.ignorecase)),
    ("smartcase", "scs", Opt::Flag(|o| &mut o.smartcase)),
    ("hlsearch", "hls", Opt::Flag(|o| &mut o.hlsearch)),
    ("incsearch", "is", Opt::Flag(|o| &mut o.incsearch)),
    ("showcmd", "sc", Opt::Flag(|o| &mut o.showcmd)),
    ("scrolloff", "so", Opt::Count(|o| &mut o.scrolloff)),
    ("textwidth", "tw", Opt::Count(|o| &mut o.textwidth)),
    ("timeoutlen", "tm", Opt::Number(|o| &mut o.timeoutlen)),
    (
        "showtabline",
        "stal",
        Opt::Choice(|o| &mut o.showtabline, &[0, 1, 2]),
    ),
    (
        "laststatus",
        "ls",
        Opt::Choice(|o| &mut o.laststatus, &[2, 3]),
    ),
    ("leader", "", Opt::Leader),
    ("mouse", "", Opt::Mouse),
    ("icons", "", Opt::Icons),
    ("graphics", "", Opt::Graphics),
];

/// Every option with what `:help` says of it, by its full name.
pub(super) const HELP: &[(&str, &str)] = &[
    ("ignorecase", "searches ignore case"),
    ("smartcase", "unless the pattern has capitals"),
    ("hlsearch", "show the last search's matches"),
    ("incsearch", "show where a search goes while typed"),
    ("showcmd", "show keys typed toward a binding"),
    ("scrolloff", "rows kept in view around the cursor"),
    (
        "textwidth",
        "the widest prose is laid out; 0 for the window's width",
    ),
    ("timeoutlen", "how long keys wait for more, in ms"),
    (
        "showtabline",
        "the tabline: 0 never, 1 with buffers, 2 always",
    ),
    ("laststatus", "statuslines: 2 per window, 3 one for all"),
    ("leader", "the key <leader> stands for"),
    ("mouse", "the mouse is the kit's: a, or empty for none"),
    ("icons", "icons drawn as auto, nerd or plain"),
    ("graphics", "pictures as kitty, halfblocks or off"),
];

impl Options {
    /// Vim's E474 for `arg`.
    fn invalid(arg: &SetArg) -> String {
        let value = match &arg.op {
            SetOp::Assign(value) => format!("={value}"),
            _ => String::new(),
        };
        format!("E474: Invalid argument: {}{value}", arg.name)
    }

    /// How searches treat case, from `'ignorecase'` and `'smartcase'`.
    pub(super) fn case_mode(&self) -> CaseMode {
        match (self.ignorecase, self.smartcase) {
            (false, _) => CaseMode::Sensitive,
            (true, false) => CaseMode::Insensitive,
            (true, true) => CaseMode::Smart,
        }
    }

    /// Applies one `:set` argument. Returns what to show (for `name?`, or `name` on a non-flag), or Vim's error.
    #[allow(clippy::too_many_lines)] // One arm per kind of option and operation.
    pub(super) fn set(
        &mut self,
        arg: &SetArg,
        keymap: &mut Keymap,
    ) -> Result<Option<String>, String> {
        let find = |typed: &str| {
            OPTIONS
                .iter()
                .copied()
                .find(|(name, short, _)| typed == *name || (!short.is_empty() && typed == *short))
        };
        let unknown = || format!("E518: Unknown option: {}", arg.name);
        // `noname` and `invname` turn a flag off or flip it, unless an option has that name itself.
        let (found, op) = match (find(&arg.name), &arg.op) {
            (Some(found), op) => (found, op.clone()),
            (None, SetOp::On) => {
                let off = arg
                    .name
                    .strip_prefix("no")
                    .and_then(find)
                    .map(|found| (found, Flip::Off));
                let inv = arg
                    .name
                    .strip_prefix("inv")
                    .and_then(find)
                    .map(|found| (found, Flip::Toggle));
                match off.or(inv) {
                    Some((found @ (_, _, Opt::Flag(_)), flip)) => (found, flip.op()),
                    Some(_) => return Err(Self::invalid(arg)),
                    None => return Err(unknown()),
                }
            }
            (None, _) => return Err(unknown()),
        };
        let (name, _, opt) = found;
        let invalid = || Self::invalid(arg);
        match (opt, &op) {
            (Opt::Flag(field), SetOp::Query) => {
                let on = *field(self);
                Ok(Some(format!("{}{name}", if on { "" } else { "no" })))
            }
            (Opt::Flag(field), SetOp::On) => {
                *field(self) = true;
                Ok(None)
            }
            (Opt::Flag(field), SetOp::Assign(off)) if off.is_empty() => {
                *field(self) = false;
                Ok(None)
            }
            (Opt::Flag(field), SetOp::Toggle) => {
                let flag = field(self);
                *flag = !*flag;
                Ok(None)
            }
            (Opt::Number(field), SetOp::Assign(value)) => {
                *field(self) = number(&arg.name, value)?;
                Ok(None)
            }
            (Opt::Count(field), SetOp::Assign(value)) => {
                *field(self) = usize::try_from(number(&arg.name, value)?).map_err(|_| invalid())?;
                Ok(None)
            }
            (Opt::Choice(field, values), SetOp::Assign(value)) => {
                let value = usize::try_from(number(&arg.name, value)?).map_err(|_| invalid())?;
                if !values.contains(&value) {
                    return Err(invalid());
                }
                *field(self) = value;
                Ok(None)
            }
            (Opt::Choice(field, _), SetOp::Query | SetOp::On) => {
                Ok(Some(format!("{name}={}", field(self))))
            }
            (Opt::Number(field), SetOp::Query | SetOp::On) => {
                Ok(Some(format!("{name}={}", field(self))))
            }
            (Opt::Count(field), SetOp::Query | SetOp::On) => {
                Ok(Some(format!("{name}={}", field(self))))
            }
            (Opt::Leader, SetOp::Assign(value)) => {
                match parse_keys(value, Key::char(' ')).as_slice() {
                    [key] => {
                        keymap.set_leader(*key);
                        Ok(None)
                    }
                    _ => Err(invalid()),
                }
            }
            (Opt::Leader, SetOp::Query | SetOp::On) => {
                Ok(Some(format!("{name}={}", keymap.leader())))
            }
            (Opt::Mouse, SetOp::Assign(value)) => {
                self.mouse = !value.is_empty();
                Ok(None)
            }
            (Opt::Mouse, SetOp::Query | SetOp::On) => Ok(Some(format!(
                "{name}={}",
                if self.mouse { "a" } else { "" }
            ))),
            (Opt::Icons, SetOp::Assign(value)) => {
                self.icons = IconSet::named(value).ok_or_else(invalid)?;
                Ok(None)
            }
            (Opt::Icons, SetOp::Query | SetOp::On) => {
                Ok(Some(format!("{name}={}", self.icons.name())))
            }
            (Opt::Graphics, SetOp::Assign(value)) => {
                self.graphics = Graphics::named(value).ok_or_else(invalid)?;
                Ok(None)
            }
            (Opt::Graphics, SetOp::Query | SetOp::On) => {
                Ok(Some(format!("{name}={}", self.graphics.name())))
            }
            _ => Err(invalid()),
        }
    }
}

/// What `<Tab>` may complete `word`, an argument of `:set`, to: from that many chars into `word` on, one of the
/// candidates. An option's name, or for a flag `no` and its name; after `graphics=`, how pictures may be drawn.
pub(super) fn completions(word: &str) -> (usize, Vec<String>) {
    if let Some((name, value)) = word.split_once('=') {
        let opt = OPTIONS
            .iter()
            .find(|&&(full, short, _)| name == full || (!short.is_empty() && name == short))
            .map(|&(_, _, opt)| opt);
        let names: Vec<&str> = match opt {
            Some(Opt::Graphics) => Graphics::ALL
                .iter()
                .map(|graphics| graphics.name())
                .collect(),
            Some(Opt::Icons) => IconSet::NAMES.to_vec(),
            _ => Vec::new(),
        };
        let values = names
            .into_iter()
            .filter(|candidate| candidate.starts_with(value))
            .map(str::to_owned)
            .collect();
        return (name.chars().count() + 1, values);
    }
    let names = OPTIONS.iter().map(|&(full, _, _)| full.to_owned());
    let negated = OPTIONS
        .iter()
        .filter(|(_, _, opt)| matches!(opt, Opt::Flag(_)))
        .map(|&(full, _, _)| format!("no{full}"));
    let candidates = names
        .chain(negated)
        .filter(|candidate| candidate.starts_with(word))
        .collect();
    (0, candidates)
}

fn number(name: &str, value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("E521: Number required after =: {name}={value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_option_says_what_it_does() {
        let named: Vec<&str> = OPTIONS.iter().map(|&(name, ..)| name).collect();
        let helped: Vec<&str> = HELP.iter().map(|&(name, _)| name).collect();

        assert_eq!(named, helped);
    }
}
