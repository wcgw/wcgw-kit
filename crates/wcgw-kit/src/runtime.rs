//! The I/O around the [`Kit`] (D2): owns the terminal, turns terminal events into [`Msg`]s and carries out
//! [`Effect`]s, handing the application's to its [`Perform`].

use std::future::Future;
use std::io::{self, Stdout, stdout};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crossterm::clipboard::CopyToClipboard;
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, EnableBracketedPaste, EnableFocusChange, Event,
    EventStream, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::supports_keyboard_enhancement;
use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use tokio::sync::{Semaphore, mpsc};
use wcgw_kit_gfx::{Graphics, Picture};

use crate::app::{App, Effect, Msg};
use crate::key::Key;
use crate::shell::{Kit, Prepared};

/// A picture's bytes, on their way.
pub type Bytes = Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send>>;

/// What carries out an application's effects, such as asking a server; results go back through its [`Inbox`].
pub trait Perform<A: App> {
    /// Carries out `effect`.
    fn perform(&mut self, effect: A::Effect);

    /// The bytes of the picture at `source`, as the application's buffers name it. None by default.
    fn picture(&mut self, source: &str) -> Bytes {
        let _ = source;
        Box::pin(async { Err("no pictures here".to_owned()) })
    }
}

/// Where an application's work sends its messages; clones send to the same place.
#[derive(Debug)]
pub struct Inbox<M>(mpsc::UnboundedSender<Msg<M>>);

impl<M> Clone for Inbox<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M> Inbox<M> {
    /// Sends `msg` to the application; `false` once the runtime has ended, when nobody needs it any more.
    pub fn send(&self, msg: M) -> bool {
        self.0.send(Msg::App(msg)).is_ok()
    }

    fn kit(&self, msg: Msg<M>) {
        // Fails only once the loop has ended, when nobody needs the message.
        let _ = self.0.send(msg);
    }
}

/// Where the runtime reads what an [`Inbox`] sends.
#[derive(Debug)]
pub struct Mailbox<M>(mpsc::UnboundedReceiver<Msg<M>>);

/// A new inbox, and the mailbox [`run`] reads it from.
#[must_use]
pub fn channel<M>() -> (Inbox<M>, Mailbox<M>) {
    let (sender, receiver) = mpsc::unbounded_channel();
    (Inbox(sender), Mailbox(receiver))
}

/// How many pictures are got at once.
const PICTURE_SLOTS: usize = 2;

/// Runs `app` in `kit` until it quits, restoring the terminal however it ends. `name` is the application's: its
/// configuration is in `~/.config/{name}`, after the one all of them share in `~/.config/wcgw` (D6).
///
/// # Errors
///
/// If the terminal cannot be set up, read from or drawn to.
pub async fn run<A: App, P: Perform<A>>(
    name: &str,
    mut app: A,
    mut kit: Kit<A>,
    mut performer: P,
    (inbox, Mailbox(mut arrived)): (Inbox<A::Msg>, Mailbox<A::Msg>),
) -> io::Result<()> {
    let mut terminal = Session::start()?;
    let var = |name| std::env::var(name).ok();
    let graphics = Graphics::guess(
        var("TERM").as_deref(),
        var("TERM_PROGRAM").as_deref(),
        std::env::var_os("TMUX").is_some(),
    );
    // Nothing was drawn yet, so nothing to free.
    let _ = kit.set_graphics(graphics);
    kit.number_pictures_from(first_picture_id());
    measure_cells(&mut kit);
    let size = terminal.size()?;
    let _ = kit.update(
        &mut app,
        Msg::Resize {
            width: size.width,
            height: size.height,
        },
    );
    let mut events = EventStream::new();
    let env = |name| std::env::var_os(name).map(PathBuf::from);
    let home = env("HOME");
    let config = config_home(env("XDG_CONFIG_HOME"), home.clone());
    let dirs: Vec<PathBuf> = config
        .iter()
        .flat_map(|config| [config.join("wcgw"), config.join(name)])
        .collect();
    for dir in &dirs {
        add_colorschemes(&mut kit, &dir.join("colors"));
    }
    for dir in &dirs {
        let init = dir.join("init.vim");
        let shown = shown_path(&init, home.clone());
        match tokio::fs::read_to_string(&init).await {
            Ok(contents) => inbox.kit(Msg::Sourced {
                path: shown,
                contents: Ok(contents),
            }),
            // No init.vim is the usual case, not an error.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => inbox.kit(Msg::Sourced {
                path: shown,
                contents: Err(error.to_string()),
            }),
        }
    }
    let slots = Arc::new(Semaphore::new(PICTURE_SLOTS));
    let perform = |effect, performer: &mut P| perform(effect, &inbox, performer, &slots);
    for effect in kit.start(&mut app) {
        if let Step::Quit = perform(effect, &mut performer)? {
            return Ok(());
        }
    }
    loop {
        terminal.draw(|frame| kit.render(&app, frame))?;
        // Both branches are cancel-safe: an event or message not taken now stays queued.
        let msg = tokio::select! {
            event = events.next() => {
                let Some(event) = event else {
                    return Ok(());
                };
                match to_msg(event?) {
                    Some(msg) => msg,
                    None => continue,
                }
            }
            Some(msg) = arrived.recv() => msg,
        };
        if let Msg::Resize { .. } = msg {
            // The font may have changed size with the window.
            measure_cells(&mut kit);
        }
        for effect in kit.update(&mut app, msg) {
            match perform(effect, &mut performer)? {
                Step::Go => {}
                Step::Quit => return Ok(()),
                Step::Edit(text) => {
                    // The editor reads the keys until it exits: stop reading them meanwhile.
                    drop(events);
                    let saved = terminal.edit(name, &text)?;
                    events = EventStream::new();
                    inbox.kit(Msg::Edited(saved));
                }
            }
        }
    }
}

/// Makes the colorschemes of `dir`, its `*.vim` files, known by their names.
fn add_colorschemes<A: App>(kit: &mut Kit<A>, dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "vim")
            && let Some(name) = path.file_stem().and_then(|stem| stem.to_str())
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            kit.theme_mut().add_colorscheme(name, text);
        }
    }
}

/// Tells `kit` a cell's size in pixels, when the terminal reports its own in pixels (Ghostty and kitty do; some
/// multiplexers do not, and `kit` keeps what it had).
fn measure_cells<A: App>(kit: &mut Kit<A>) {
    let Ok(size) = crossterm::terminal::window_size() else {
        return;
    };
    if size.columns > 0 && size.rows > 0 {
        kit.set_cell_size(size.width / size.columns, size.height / size.rows);
    }
}

/// The id of the first picture sent to the terminal: from the process and the time, so that two runs, or another
/// program, are unlikely to reuse it while the terminal still holds the first's.
fn first_picture_id() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    (std::process::id().wrapping_mul(2_654_435_761) ^ nanos) & 0x007f_ffff | 0x0001_0000
}

/// What the loop does after an effect.
enum Step {
    Go,
    Quit,
    /// Hand the terminal to the editor on this text.
    Edit(String),
}

/// Carries out one effect, but for the editor, which needs the terminal. Effects that finish later send their result
/// through `inbox`.
fn perform<A: App, P: Perform<A>>(
    effect: Effect<A::Msg, A::Effect>,
    inbox: &Inbox<A::Msg>,
    performer: &mut P,
    slots: &Arc<Semaphore>,
) -> io::Result<Step> {
    match effect {
        Effect::Quit => return Ok(Step::Quit),
        Effect::Edit { text } => return Ok(Step::Edit(text)),
        Effect::Copy(text) => execute!(stdout(), CopyToClipboard::to_clipboard_from(text))?,
        Effect::Wake { after, token } => {
            let inbox = inbox.clone();
            tokio::spawn(async move {
                tokio::time::sleep(after).await;
                inbox.kit(Msg::Wake(token));
            });
        }
        Effect::After { after, msg } => {
            let inbox = inbox.clone();
            tokio::spawn(async move {
                tokio::time::sleep(after).await;
                inbox.send(msg);
            });
        }
        Effect::App(effect) => performer.perform(effect),
        Effect::Picture {
            source,
            cols,
            rows,
            kitty,
        } => {
            let bytes = performer.picture(&source);
            let (inbox, slots) = (inbox.clone(), Arc::clone(slots));
            tokio::spawn(async move {
                // Never closed: a permit always comes.
                let _slot = slots.acquire_owned().await;
                let result = match bytes.await {
                    // Decoding and scaling take a while: off the runtime's thread.
                    Ok(bytes) => {
                        tokio::task::spawn_blocking(move || prepare(&bytes, cols, rows, kitty))
                            .await
                            .unwrap_or_else(|_| Err("cannot prepare it".to_owned()))
                    }
                    Err(error) => Err(error),
                };
                inbox.kit(Msg::Picture {
                    source,
                    cols,
                    rows,
                    result,
                });
            });
        }
        Effect::Graphics(escapes) => {
            let mut out = stdout();
            io::Write::write_all(&mut out, escapes.as_bytes())?;
            io::Write::flush(&mut out)?;
        }
        Effect::OpenUrl(url) => {
            if let Err(error) = open_url(&url) {
                inbox.kit(Msg::Failed(format!("Cannot open the browser: {error}")));
            }
        }
        Effect::Source(path) => {
            let inbox = inbox.clone();
            let file = expand_home(&path, std::env::var_os("HOME").map(PathBuf::from));
            tokio::spawn(async move {
                let contents = tokio::fs::read_to_string(file)
                    .await
                    .map_err(|error| error.to_string());
                inbox.kit(Msg::Sourced { path, contents });
            });
        }
    }
    Ok(Step::Go)
}

/// The picture in `bytes`, prepared for `cols` × `rows` cells: for the kitty protocol as image `kitty`, else as
/// half blocks.
fn prepare(bytes: &[u8], cols: u16, rows: u16, kitty: Option<u32>) -> Result<Prepared, String> {
    let picture = Picture::decode(bytes).ok_or_else(|| "not a picture the kit shows".to_owned())?;
    Ok(match kitty {
        Some(id) => Prepared::Kitty(picture.kitty(id, cols, rows)),
        None => Prepared::HalfBlocks(picture.half_blocks(cols, rows)),
    })
}

/// Opens `url` in the browser, without waiting for it.
///
/// # Errors
///
/// When the opener cannot start.
pub fn open_url(url: &str) -> io::Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let mut child = std::process::Command::new(opener)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    // Reaped on a thread of its own so it does not linger as a zombie.
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Counts drafts opened, so each gets a file of its own.
static EDITS: AtomicUsize = AtomicUsize::new(0);

/// The user's editor: `$EDITOR`, else `$VISUAL`, else `nvim` when installed, else `vi`.
fn editor() -> String {
    let set = |name| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    };
    set("EDITOR").or_else(|| set("VISUAL")).unwrap_or_else(|| {
        let installed = std::env::var_os("PATH")
            .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("nvim").is_file()));
        if installed { "nvim" } else { "vi" }.to_owned()
    })
}

/// Where drafts are written: `$XDG_RUNTIME_DIR` (memory only, the user's alone), else the temporary directory.
fn drafts_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute() && dir.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

/// Runs `editor` (a shell command, as `$EDITOR` may hold arguments) on a file of `dir` holding `text`, named for
/// application `name`, readable by the user alone and removed after; what it saved, or why there is nothing: it
/// could not run, or exited with an error (`:cq`).
fn run_editor(editor: &str, dir: &Path, name: &str, text: &str) -> Result<String, String> {
    let path = dir.join(format!(
        "{name}-draft-{}-{}.md",
        std::process::id(),
        EDITS.fetch_add(1, Ordering::SeqCst)
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options
        .open(&path)
        .and_then(|mut file| io::Write::write_all(&mut file, text.as_bytes()));
    if let Err(error) = written {
        let _ = std::fs::remove_file(&path);
        return Err(format!("cannot write the draft: {error}"));
    }
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg(name)
        .arg(&path)
        .status();
    let saved = std::fs::read_to_string(&path);
    // The draft stays in the application's memory, not on disk.
    let _ = std::fs::remove_file(&path);
    match status {
        Err(error) => Err(format!("cannot run {editor}: {error}")),
        Ok(status) if !status.success() => Err(match status.code() {
            Some(code) => format!("the editor exited with {code}"),
            None => "the editor was stopped".to_owned(),
        }),
        Ok(_) => saved.map_err(|error| format!("cannot read the draft: {error}")),
    }
}

/// `path` with a leading `~` standing for `home`, when there is one.
fn expand_home(path: &str, home: Option<PathBuf>) -> PathBuf {
    match (path.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            home.join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(path),
    }
}

/// `path` as messages show it: under `home`, as `~/…`.
fn shown_path(path: &Path, home: Option<PathBuf>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// Where configuration lives: `$XDG_CONFIG_HOME`, else `~/.config`.
fn config_home(config_home: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    config_home
        .filter(|dir| dir.is_absolute())
        .or_else(|| home.map(|home| home.join(".config")))
}

/// The configuration directory of application `name` (D6): under `$XDG_CONFIG_HOME`, else under `~/.config`.
#[must_use]
pub fn config_dir(name: &str) -> Option<PathBuf> {
    let env = |name| std::env::var_os(name).map(PathBuf::from);
    config_home(env("XDG_CONFIG_HOME"), env("HOME")).map(|config| config.join(name))
}

/// The message for a terminal event, if the kit cares.
fn to_msg<M>(event: Event) -> Option<Msg<M>> {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => {
            Key::try_from(key).ok().map(Msg::Key)
        }
        Event::Paste(text) => Some(Msg::Paste(text)),
        Event::FocusGained => Some(Msg::FocusGained),
        Event::FocusLost => Some(Msg::FocusLost),
        Event::Resize(width, height) => Some(Msg::Resize { width, height }),
        Event::Key(_) | Event::Mouse(_) => None,
    }
}

/// Whether the kitty keyboard protocol was turned on, so that it is turned off again exactly once, also from the
/// panic hook, which cannot reach the [`Session`].
static KEYBOARD_ENHANCED: AtomicBool = AtomicBool::new(false);

/// The terminal in the kit's modes: raw, alternate screen, focus reporting, bracketed paste and, where supported,
/// the kitty keyboard protocol (so `<C-i>` and `<Tab>` differ). Dropping it, or panicking, restores the terminal.
struct Session {
    terminal: DefaultTerminal,
    /// Whether the terminal speaks the kitty keyboard protocol, asked once: asking takes a round trip.
    keyboard: bool,
}

impl Session {
    fn start() -> io::Result<Self> {
        // Raw mode, the alternate screen, and a panic hook that leaves both.
        let terminal = ratatui::try_init()?;
        let session = Self {
            terminal,
            keyboard: supports_keyboard_enhancement().unwrap_or(false),
        };
        enter_modes(session.keyboard)?;
        // Runs before ratatui's hook, which this one wraps.
        let ratatui_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            leave_modes();
            ratatui_hook(info);
        }));
        Ok(session)
    }

    /// Hands the terminal to the user's editor on `text` until it exits, then takes it back; what it saved, or why
    /// there is nothing.
    fn edit(&mut self, name: &str, text: &str) -> io::Result<Result<String, String>> {
        leave_modes();
        ratatui::restore();
        let saved = run_editor(&editor(), &drafts_dir(), name, text);
        crossterm::terminal::enable_raw_mode()?;
        execute!(
            stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        )?;
        enter_modes(self.keyboard)?;
        // A blank screen, and a terminal that knows it, so the next draw paints it all. Not `Terminal::clear`: it
        // asks the terminal where the cursor is, and gives up with an error when the answer is slow.
        self.terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout()))?;
        Ok(saved)
    }
}

/// Turns on the modes the kit adds on top of ratatui's: focus reporting, bracketed paste and, when the terminal
/// speaks it (`keyboard`), the kitty keyboard protocol.
fn enter_modes(keyboard: bool) -> io::Result<()> {
    execute!(stdout(), EnableFocusChange, EnableBracketedPaste)?;
    if keyboard {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
        KEYBOARD_ENHANCED.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// Turns off the modes `Session::start` added on top of ratatui's. Errors are ignored: this runs while exiting or
/// panicking, when there is nothing better to do.
fn leave_modes() {
    let mut out = stdout();
    if KEYBOARD_ENHANCED.swap(false, Ordering::SeqCst) {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = execute!(out, DisableBracketedPaste, DisableFocusChange);
}

impl Drop for Session {
    fn drop(&mut self) {
        leave_modes();
        ratatui::restore();
    }
}

impl Deref for Session {
    type Target = ratatui::Terminal<ratatui::backend::CrosstermBackend<Stdout>>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    use super::*;

    fn key(kind: KeyEventKind) -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind,
            state: KeyEventState::NONE,
        })
    }

    #[test]
    fn a_leading_tilde_is_the_home_directory() {
        let home = Some(PathBuf::from("/home/u"));

        assert_eq!(
            expand_home("~/x.vim", home.clone()),
            PathBuf::from("/home/u/x.vim")
        );
        assert_eq!(expand_home("~", home.clone()), PathBuf::from("/home/u"));
        assert_eq!(
            expand_home("~bob/x.vim", home.clone()),
            PathBuf::from("~bob/x.vim")
        );
        assert_eq!(expand_home("x.vim", home), PathBuf::from("x.vim"));
        assert_eq!(expand_home("~/x.vim", None), PathBuf::from("~/x.vim"));
    }

    #[test]
    fn paths_under_home_are_shown_with_a_tilde() {
        let home = Some(PathBuf::from("/home/u"));

        assert_eq!(
            shown_path(Path::new("/home/u/.config/devnull/init.vim"), home.clone()),
            "~/.config/devnull/init.vim"
        );
        assert_eq!(
            shown_path(Path::new("/etc/init.vim"), home),
            "/etc/init.vim"
        );
    }

    #[test]
    fn configuration_is_under_the_xdg_config_home_or_dot_config() {
        let home = Some(PathBuf::from("/home/u"));

        assert_eq!(
            config_home(Some("/xdg".into()), home.clone()),
            Some(PathBuf::from("/xdg"))
        );
        assert_eq!(
            config_home(None, home.clone()),
            Some(PathBuf::from("/home/u/.config"))
        );
        assert_eq!(
            config_home(Some("relative".into()), home),
            Some(PathBuf::from("/home/u/.config"))
        );
        assert_eq!(config_home(None, None), None);
    }

    #[test]
    fn presses_and_repeats_are_keys_but_releases_are_dropped() {
        assert_eq!(
            to_msg::<()>(key(KeyEventKind::Press)),
            Some(Msg::Key(Key::char('j')))
        );
        assert_eq!(
            to_msg::<()>(key(KeyEventKind::Repeat)),
            Some(Msg::Key(Key::char('j')))
        );
        assert_eq!(to_msg::<()>(key(KeyEventKind::Release)), None);
    }

    #[test]
    fn focus_paste_and_resize_become_messages() {
        assert_eq!(to_msg::<()>(Event::FocusLost), Some(Msg::FocusLost));
        assert_eq!(to_msg::<()>(Event::FocusGained), Some(Msg::FocusGained));
        assert_eq!(
            to_msg::<()>(Event::Paste("hi".into())),
            Some(Msg::Paste("hi".into()))
        );
        assert_eq!(
            to_msg::<()>(Event::Resize(80, 24)),
            Some(Msg::Resize {
                width: 80,
                height: 24
            })
        );
    }

    fn files(dir: &Path) -> usize {
        std::fs::read_dir(dir).expect("dir").count()
    }

    #[test]
    fn what_the_editor_saved_comes_back_and_the_draft_file_goes() {
        let dir = tempfile::tempdir().expect("dir");

        let saved = run_editor("printf 'saved' >", dir.path(), "app", "draft");

        assert_eq!(saved, Ok("saved".to_owned()));
        assert_eq!(files(dir.path()), 0);
    }

    #[test]
    fn an_editor_quitting_with_an_error_gives_nothing() {
        let dir = tempfile::tempdir().expect("dir");

        let saved = run_editor("false", dir.path(), "app", "draft");

        assert_eq!(saved, Err("the editor exited with 1".to_owned()));
        assert_eq!(files(dir.path()), 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_draft_is_the_user_s_alone_and_editors_may_take_arguments() {
        let dir = tempfile::tempdir().expect("dir");

        let mode = run_editor(
            "f() { stat -c %a \"$1\" > \"$1\"; }; f",
            dir.path(),
            "app",
            "draft",
        );
        let edited = run_editor("sed -i s/draft/done/", dir.path(), "app", "a draft");

        assert_eq!(mode, Ok("600\n".to_owned()));
        assert_eq!(edited, Ok("a done".to_owned()));
    }
}
