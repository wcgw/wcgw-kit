//! Ex command lines (D13): what `:` runs and what `init.vim` holds.
//!
//! Only whole commands, one per line: no ranges and no `|` chaining. A line names a command, in full or
//! abbreviated as Vim allows, maybe a `!`, then its arguments; the [`Kit`](crate::Kit)'s registry says which
//! command the name is and runs it.

use std::fmt;

/// A command line split into the name typed, its `!` and its arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Split<'a> {
    /// The command's name as typed, maybe abbreviated.
    pub name: &'a str,
    /// Whether a `!` follows the name.
    pub bang: bool,
    /// The rest of the line, trimmed.
    pub args: &'a str,
}

/// Splits one command line, as typed after `:` or written in `init.vim`. Blank lines and `"` comments are `None`;
/// leading `:` and whitespace are ignored.
#[must_use]
pub fn split(line: &str) -> Option<Split<'_>> {
    let trimmed = line.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
    if trimmed.is_empty() || trimmed.starts_with('"') {
        return None;
    }
    let name_len = trimmed
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(trimmed.len());
    let (name, rest) = trimmed.split_at(name_len);
    let (bang, args) = match rest.strip_prefix('!') {
        Some(args) => (true, args.trim()),
        None => (false, rest.trim()),
    };
    Some(Split { name, bang, args })
}

/// Why a command line cannot run, worded as Vim words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExError {
    /// E492: the command name is unknown.
    NotACommand(String),
    /// E477: the command takes no `!`.
    NoBang,
    /// E471: an argument is missing.
    ArgumentRequired,
    /// E475: an argument is malformed.
    InvalidArgument(String),
}

impl fmt::Display for ExError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotACommand(line) => write!(f, "E492: Not an editor command: {line}"),
            Self::NoBang => f.write_str("E477: No ! allowed"),
            Self::ArgumentRequired => f.write_str("E471: Argument required"),
            Self::InvalidArgument(arg) => write!(f, "E475: Invalid argument: {arg}"),
        }
    }
}

impl std::error::Error for ExError {}

impl From<ExError> for String {
    fn from(error: ExError) -> Self {
        error.to_string()
    }
}

/// `args`, which must not be empty.
///
/// # Errors
///
/// E471 when it is.
pub fn required(args: &str) -> Result<&str, ExError> {
    if args.is_empty() {
        Err(ExError::ArgumentRequired)
    } else {
        Ok(args)
    }
}

/// One `:set` argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetArg {
    /// The option's name as typed, including any `no` or `inv` prefix: only the options know whether `nowrap` is
    /// `wrap` turned off or an option of its own.
    pub name: String,
    /// What to do with it.
    pub op: SetOp,
}

/// What a `:set` argument does to its option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetOp {
    /// `name`: turn a flag on (or off, or flip it, through a `no` or `inv` prefix), or show another option.
    On,
    /// `name!`: flip a flag.
    Toggle,
    /// `name?`: show the value.
    Query,
    /// `name=value` or `name:value`.
    Assign(String),
}

/// The arguments of `:set`.
pub(crate) fn set_args(args: &str) -> Vec<SetArg> {
    args.split_whitespace().map(set_arg).collect()
}

fn set_arg(arg: &str) -> SetArg {
    let named = |name: &str, op| SetArg {
        name: name.to_owned(),
        op,
    };
    if let Some((name, value)) = arg.split_once(['=', ':']) {
        return named(name, SetOp::Assign(value.to_owned()));
    }
    if let Some(name) = arg.strip_suffix('?') {
        return named(name, SetOp::Query);
    }
    if let Some(name) = arg.strip_suffix('!') {
        return named(name, SetOp::Toggle);
    }
    named(arg, SetOp::On)
}

/// The left side and the rest of the line of a `:map`, after map modifiers like `<silent>`, which the kit accepts
/// but has no use for. `None` when either side is missing.
pub(crate) fn map_args(args: &str) -> Option<(&str, &str)> {
    let mut rest = args;
    while let Some(after) = ["<silent>", "<nowait>", "<unique>"]
        .iter()
        .find_map(|m| rest.strip_prefix(m))
    {
        rest = after.trim_start();
    }
    let (lhs, rhs) = rest.split_once(char::is_whitespace)?;
    let rhs = rhs.trim_start();
    (!rhs.is_empty()).then_some((lhs, rhs))
}

/// What `:highlight` asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Highlight {
    /// `:hi {group} {key}={value} ...`.
    Set {
        /// The highlight group's name, as typed.
        group: String,
        /// Attribute keys and values, in order, such as `guifg` and `#c0caf5`.
        attrs: Vec<(String, String)>,
    },
    /// `:hi clear`: every group back to the colorscheme's defaults.
    Clear,
    /// `:hi link {from} {to}`: draws `from` as `to`; `:hi link {from} NONE` (`to` is `None`) undoes it.
    Link {
        /// The group drawn as another.
        from: String,
        /// The group it is drawn as; `None` to draw it as itself again.
        to: Option<String>,
    },
}

/// The arguments of `:highlight`.
pub(crate) fn highlight(args: &str) -> Result<Highlight, ExError> {
    let mut words = args.split_whitespace();
    let group = words.next().ok_or(ExError::ArgumentRequired)?;
    if group == "clear" {
        return Ok(Highlight::Clear);
    }
    if group == "link" {
        let from = words.next().ok_or(ExError::ArgumentRequired)?;
        let to = words.next().ok_or(ExError::ArgumentRequired)?;
        return Ok(Highlight::Link {
            from: from.to_owned(),
            to: (to != "NONE").then(|| to.to_owned()),
        });
    }
    let attrs = words
        .map(|word| {
            word.split_once('=')
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .ok_or_else(|| ExError::InvalidArgument(word.to_owned()))
        })
        .collect::<Result<_, _>>()?;
    Ok(Highlight::Set {
        group: group.to_owned(),
        attrs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_splits_into_name_bang_and_arguments() {
        assert_eq!(
            split(":  quit!  "),
            Some(Split {
                name: "quit",
                bang: true,
                args: ""
            })
        );
        assert_eq!(
            split("map <leader>x :star<CR>"),
            Some(Split {
                name: "map",
                bang: false,
                args: "<leader>x :star<CR>"
            })
        );
        assert_eq!(
            split("e!"),
            Some(Split {
                name: "e",
                bang: true,
                args: ""
            })
        );
        assert_eq!(split("   "), None);
        assert_eq!(split("\" a comment"), None);
    }

    #[test]
    fn set_arguments_say_what_to_do_with_each_option() {
        assert_eq!(
            set_args("ic noic so=5 hls? wrap! tm:200"),
            [
                SetArg {
                    name: "ic".into(),
                    op: SetOp::On
                },
                SetArg {
                    name: "noic".into(),
                    op: SetOp::On
                },
                SetArg {
                    name: "so".into(),
                    op: SetOp::Assign("5".into())
                },
                SetArg {
                    name: "hls".into(),
                    op: SetOp::Query
                },
                SetArg {
                    name: "wrap".into(),
                    op: SetOp::Toggle
                },
                SetArg {
                    name: "tm".into(),
                    op: SetOp::Assign("200".into())
                },
            ]
        );
    }

    #[test]
    fn maps_take_modifiers_and_need_both_sides() {
        assert_eq!(
            map_args("<silent> <leader>x  :star<CR>"),
            Some(("<leader>x", ":star<CR>"))
        );
        assert_eq!(map_args("x"), None);
    }

    #[test]
    fn highlight_sets_clears_and_links() {
        assert_eq!(highlight("clear"), Ok(Highlight::Clear));
        assert_eq!(
            highlight("link A NONE"),
            Ok(Highlight::Link {
                from: "A".into(),
                to: None
            })
        );
        assert_eq!(
            highlight("Normal guifg=#000000"),
            Ok(Highlight::Set {
                group: "Normal".into(),
                attrs: vec![("guifg".into(), "#000000".into())]
            })
        );
        assert_eq!(
            highlight("Normal bold"),
            Err(ExError::InvalidArgument("bold".into()))
        );
        assert_eq!(highlight(""), Err(ExError::ArgumentRequired));
    }
}
