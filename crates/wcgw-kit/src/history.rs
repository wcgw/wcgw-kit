//! The command line's history on disk (D21, muttrs' way): command lines (`:`) and searches (`/`, `?`), kept in a file
//! of one entry a line, oldest first, each after what typed it. The file is read again before it is written, so that
//! two of an application running keep what each other typed.

use std::path::{Path, PathBuf};

/// How many entries of each kind are kept.
pub(crate) const KEPT: usize = 200;

/// What starts a command line's entries.
const COMMAND: char = ':';

/// What starts a search's entries.
const SEARCH: char = '/';

/// Command lines and searches, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Entries {
    pub(crate) commands: Vec<String>,
    pub(crate) searches: Vec<String>,
}

/// The file the history of application `name` is kept in: under `$XDG_STATE_HOME`, else `~/.local/state`.
pub(crate) fn path(
    name: &str,
    state_home: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    let state = state_home
        .filter(|dir| dir.is_absolute())
        .or_else(|| home.map(|home| home.join(".local/state")))?;
    Some(state.join(name).join("history"))
}

/// The entries kept at `path`; none when there is no such file, or it can't be read.
pub(crate) fn read(path: &Path) -> Entries {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Entries::default();
    };
    parse(&text)
}

fn parse(text: &str) -> Entries {
    let mut entries = Entries::default();
    for line in text.lines() {
        let mut chars = line.chars();
        match chars.next() {
            Some(COMMAND) => remember(&mut entries.commands, chars.as_str()),
            Some(SEARCH) => remember(&mut entries.searches, chars.as_str()),
            _ => {}
        }
    }
    entries
}

/// Adds `entry` as the newest of `history`, an older identical one moving up, keeping [`KEPT`].
fn remember(history: &mut Vec<String>, entry: &str) {
    if entry.is_empty() {
        return;
    }
    history.retain(|old| old != entry);
    history.push(entry.to_owned());
    let over = history.len().saturating_sub(KEPT);
    history.drain(..over);
}

/// The entries at `path` with `new` after them, newest last: what this run typed joins what the file holds now.
pub(crate) fn merged(on_disk: &Entries, new: &Entries) -> Entries {
    let mut merged = on_disk.clone();
    for entry in &new.commands {
        remember(&mut merged.commands, entry);
    }
    for entry in &new.searches {
        remember(&mut merged.searches, entry);
    }
    merged
}

/// Writes `entries` to `path`, making its directory if need be.
///
/// # Errors
///
/// When it can't be written.
pub(crate) fn write(path: &Path, entries: &Entries) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut text = String::new();
    for (lead, kind) in [(COMMAND, &entries.commands), (SEARCH, &entries.searches)] {
        for entry in kind {
            text.push(lead);
            text.push_str(entry);
            text.push('\n');
        }
    }
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_read_by_what_typed_them_each_once() {
        let entries = parse(":set ic\n/needle\n:q\n:set ic\nbogus\n/\n");

        assert_eq!(entries.commands, ["q", "set ic"], "the newer of two alike");
        assert_eq!(entries.searches, ["needle"]);
    }

    #[test]
    fn what_a_run_typed_joins_what_the_file_holds_now() {
        let on_disk = parse(":a\n:b\n/x\n");
        let new = Entries {
            commands: vec!["c".into(), "a".into()],
            searches: vec!["y".into()],
        };

        let merged = merged(&on_disk, &new);

        assert_eq!(merged.commands, ["b", "c", "a"]);
        assert_eq!(merged.searches, ["x", "y"]);
    }

    #[test]
    fn only_so_many_are_kept() {
        let new = Entries {
            commands: (0..KEPT + 5).map(|n| n.to_string()).collect(),
            searches: Vec::new(),
        };

        let merged = merged(&Entries::default(), &new);

        assert_eq!(merged.commands.len(), KEPT);
        assert_eq!(merged.commands[0], "5");
    }

    #[test]
    fn it_round_trips_through_its_file_under_the_state_directory() {
        let dir = tempfile::tempdir().expect("dir");
        let path = path("app", Some(dir.path().to_path_buf()), None).expect("a path");
        let entries = Entries {
            commands: vec!["set ic".into()],
            searches: vec!["needle".into()],
        };

        write(&path, &entries).expect("written");

        assert_eq!(read(&path), entries);
        assert!(path.ends_with("app/history"));
        assert_eq!(
            super::path("app", None, Some(PathBuf::from("/home/u"))),
            Some(PathBuf::from("/home/u/.local/state/app/history"))
        );
    }
}
