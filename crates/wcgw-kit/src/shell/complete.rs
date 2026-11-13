//! `<Tab>` on the command line: what the word before the cursor may be completed to, by what the command takes.

use super::cmdline::Purpose;
use super::commands::Found;
use super::{Kit, Mode_, options};
use crate::app::App;
use crate::hl;
use crate::keymap::Mode;

/// What `<Tab>` may put in place of the end of a command line: from char `start` on, one of `candidates`, sorted.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Completions {
    /// Where the word they replace starts, in chars.
    pub start: usize,
    /// What may replace it, sorted.
    pub candidates: Vec<String>,
}

impl<A: App> Kit<A> {
    /// `<Tab>` (`forward`) and `<S-Tab>` on the ex command line or a prompt that completes: the next or previous
    /// completion.
    pub(super) fn complete_command_line(&mut self, app: &A, forward: bool) {
        let Mode_::Cmdline(mut cmdline) = std::mem::replace(&mut self.mode, Mode_::Normal) else {
            return;
        };
        // Taken out while the line it completes changes, then put back.
        let completer = match &mut cmdline.purpose {
            Purpose::Prompt { complete, .. } => complete.take(),
            Purpose::Ex | Purpose::Search(_) => None,
        };
        match &completer {
            Some(complete) => cmdline.complete(forward, |line| {
                completions(0, line, complete(app, self, line))
            }),
            None => cmdline.complete(forward, |line| self.ex_completions(app, line)),
        }
        if let Purpose::Prompt { complete, .. } = &mut cmdline.purpose {
            *complete = completer;
        }
        self.mode = Mode_::Cmdline(cmdline);
    }

    /// What the ex command line `line`, up to the cursor, may be completed to: a command's name while it is the
    /// first word, else an argument of the command, as the command takes them.
    #[must_use]
    pub fn ex_completions(&self, app: &A, line: &str) -> Completions {
        let command = line.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
        let skipped = line.len() - command.len();
        let name_len = command
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(command.len());
        if name_len == command.len() {
            let names = self.commands.names().map(str::to_owned);
            return completions(line[..skipped].chars().count(), command, names);
        }
        let Some((name, found)) = self.commands.lookup(&command[..name_len]) else {
            return Completions::default();
        };
        // `:e! name` as `:e name`.
        let rest = &command[name_len..];
        let rest = rest.strip_prefix('!').unwrap_or(rest);
        if !rest.starts_with(char::is_whitespace) {
            return Completions::default();
        }
        let word_at = line
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map_or(0, |(at, c)| at + c.len_utf8());
        let word = &line[word_at..];
        let start = line[..word_at].chars().count();
        let before: Vec<&str> = rest.split_whitespace().collect();
        let before = &before[..before.len() - usize::from(!word.is_empty())];
        let names = |names: &[&str]| {
            names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
        };
        let candidates: Vec<String> = match (name, before) {
            ("set", _) => {
                let (offset, values) = options::completions(word);
                let value = word.chars().skip(offset).collect::<String>();
                return completions(start + offset, &value, values);
            }
            ("colorscheme", []) => self.theme.colorscheme_names(),
            ("edit", []) => self
                .buffers
                .iter()
                .map(|buffer| buffer.name.clone())
                .chain(app.edit_names(self))
                .collect(),
            ("buffer", []) => self
                .listed()
                .into_iter()
                .map(|id| self.buffers[id.0].name.clone())
                .collect(),
            ("highlight", []) => groups(&self.theme)
                .chain(names(&["clear", "link"]))
                .collect(),
            ("highlight", ["link"] | ["link", _]) => groups(&self.theme).collect(),
            ("help", []) => [Mode::Normal, Mode::Visual]
                .into_iter()
                .flat_map(|mode| self.keymap.listing(mode))
                .map(|listed| listed.keys)
                .collect(),
            ("pick", []) => self.picker_names(),
            _ => match &found {
                Found::App(command) => command
                    .complete
                    .as_ref()
                    .map_or_default(|complete| complete(app, self, before, word)),
                Found::Builtin(_) => Vec::new(),
            },
        };
        completions(start, word, candidates)
    }
}

/// The names of the highlight groups, as `:hi` takes them.
fn groups(theme: &hl::Theme) -> impl Iterator<Item = String> + '_ {
    theme.groups().iter().map(|group| group.name().to_owned())
}

/// Those of `candidates` that complete `word`, from char `start` on: sorted, each once.
fn completions(
    start: usize,
    word: &str,
    candidates: impl IntoIterator<Item = String>,
) -> Completions {
    let mut candidates: Vec<String> = candidates
        .into_iter()
        .filter(|candidate| candidate.starts_with(word))
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    Completions { start, candidates }
}
