# wcgw-kit: plan and decisions

A Rust/ratatui toolkit giving four terminal apps one Neovim look & feel:
[devnull](https://github.com/wcgw/devnull) (`dn`), [ghd](https://github.com/wcgw/ghd),
[muttrs](https://github.com/wcgw/muttrs) and [slackers](https://github.com/wcgw/slackers). Extracted mostly from devnull,
whose `docs/plan.md` (D1, D4–D7, D10) these decisions build on.

## Starting point (surveyed 2026-11-12)

All four on branch `wcgw-kit`, ratatui 0.30, crossterm 0.29, edition 2024, tokio. Surveyed at devnull `740b95f`, ghd
`e92ea6b`, muttrs `9a7bbb4`, slackers `061287b`.

| | Keys | `:` commands | Theme | Pictures | Icons | voight |
| --- | --- | --- | --- | --- | --- | --- |
| devnull | Keymap: counts, `:map`, leader, which-key | Closed enum and table, completion, history | `hl::Group`, `:hi`, `:colorscheme`, generated tokyonight ×4 | Own: PNG `a=T`, native animation, half blocks, env detection | Assumed Nerd Font, one glyph used | — |
| ghd | No modes; `match` on keys | String `match`, wildmenu, history per run | ~70 app slots, TOML, at startup | None | Nerd or text, off by default | `assessment.rs`, label, `H` heatmap |
| muttrs | No modes; `match`; key→keys remaps in TOML | String `match` plus a separate list, inline Tab, history on disk | ~22 slots, TOML | `muttrs-gfx`: kitty and half blocks, ported from slackers | `NERD`/`PLAIN`, `auto` | As ghd, drifted (~300 diff lines) |
| slackers | `Mode` enum; `match`; mouse | `:q`, `:vw` | `const Style`s | `slackers-gfx`: kitty only, `/dev/shm` RGBA, frame swapping | Nerd only | — |

Each app has its own `update`, `select!` loop and `$EDITOR` hand-off; all use tokyonight-moon's palette (devnull defaults
to night) and leave the background to the terminal.

## Decisions

### D1. Framework for input and chrome

- The kit owns input dispatch (keys → keymap → motions, actions, ex commands), the `:` line, the registries (D13) and
  the chrome (D10, D14). Apps own their domain state and what their buffers contain; they register commands, maps,
  highlight groups, options, icons and statusline segments.
- Why: the feel (counts, remaps, `:map`, completion, history, which-key) is what drifted while each app hand-rolled its
  dispatch. A toolbox of widgets would let it drift again.
- Layered crates (D18), so leaf pieces such as pictures can be adopted on their own.

### D2. The kit owns the loop

- `async fn run(app)`, awaited inside the app's own tokio runtime (current-thread or multi-thread, the app's choice).
- **Terminal session**: raw mode, alternate screen, focus reporting, bracketed paste, kitty keyboard flags, buttons-only
  mouse (D5), panic hook, restore.
- **Loop**: `select!` over crossterm's `EventStream`, the app's inbox and the kit's timers (key timeout, which-key,
  half-block frames, spinner); one redraw per wake.
- **Hand-offs**: suspending for `$EDITOR` or a shell (drop and rebuild the `EventStream`, forget pictures), OSC 52 copy,
  OSC 777 notifications, `xdg-open`/`open`.
- **Pictures**: prepared uploads written between draws; animation paused while unfocused.
- **App contract**: Elm-style `update(&mut self, Msg) -> Effects`, as devnull and slackers have. Effects are the kit's
  (`Copy`, `Edit`, `Open`, `Notify`, `Wake`, `Quit`) or the app's own. Async work posts messages back through an inbox
  `Sender` the kit hands over.
- Cost: ghd and muttrs mutate state and `tokio::spawn` from handlers today; they move to messages.

### D3. Every pane is a buffer

- devnull's document model moves into the kit. A buffer is app-rendered styled lines whose spans carry an app-defined
  `Target` (generic: it replaces `doc::Target`/`AccountRef` and their `devnull_api` import) and mark item boundaries (a
  commit, a notification, a Message, a mail).
- The kit owns the cursor, every motion, `]]`/`[[` by item, `/ ? n N * #`, `v`/`V`/`y`, folds, `zz zt zb` and
  `scrolloff`, wrapping, pictures in lines, and "an action applies to the item under the cursor".
- No pane draws its own content. Two valves in the buffer contract instead:
  - **Lazy lines**: a buffer can be a provider (`len()`, `lines(range, width)`), for a 50k-Message mailbox or a long
    Slack history.
  - **Bottom anchoring**, for chat timelines.
- Consequences: ghd's notification table becomes column-aligned `nowrap` lines laid out for the window's width, and
  the badges in its borders move to the statusline. muttrs' and slackers' sidebars become buffers with folds.
  slackers' conversation (author grouping, the new-messages rule) becomes app-rendered lines.

### D4. One composer

- The only editable thing: a composer docked under a window.
  - Insert mode with readline keys (`<C-w>`, `<C-u>`, arrows, Home/End), multi-line, `<CR>` sending or breaking the
    line as configured; `<Esc>` leaves. No Vim modal editing inside: `<C-x><C-e>` hands the text to `$EDITOR`.
  - Completion: apps register triggers (`@`, `:`, `/`) with sources, shown in the `Pmenu` the wildmenu uses.
  - Built on ratatui-textarea, which stays out of the kit's API.
- The `:` line and every prompt (search, masked passphrase, muttrs' To and Subject) share its line editing; muttrs'
  append-only prompt goes.
- Apps choose per action: devnull and muttrs keep `$EDITOR` for commits and mail; slackers uses the composer.
- Rejected: `$EDITOR` only (unusable for chat); a Vim-modal editor in the kit (a project of its own).

### D5. Mouse

- `:set mouse=`, as Neovim, on by default. Buttons and wheel only (`?1000h ?1006h`): moving the pointer never redraws.
- The kit hit-tests, since it owns windows and buffers: the wheel scrolls the window under the pointer; a click focuses
  its window and moves the cursor to the cell; a click on a picker, `Pmenu` or which-key item selects it.
- Mouse events are keys (`<LeftMouse>`, `<2-LeftMouse>`, `<ScrollWheelDown>`, …) and go through the keymap. slackers
  keeps click-to-open as a default map; devnull may default to `mouse=` (its D1).
- Trade-off: while the mouse is captured, selecting text in the terminal takes Shift-drag in Ghostty.

### D6. Configuration

- **`config.toml`**: app-owned, domain only (accounts, workspaces, the voight device, polling). The kit never reads
  it; slackers keeps rewriting its own with `toml_edit`.
- **`init.vim`**: kit-owned, all of the UI: `:set`, the `:map` family, `:hi`, `:colorscheme`, `:source`. One parser,
  shared with the `:` line.
- **Shared layer first**: `~/.config/wcgw/init.vim` and `~/.config/wcgw/colors/`, then the app's `~/.config/<app>/`.
  One colorscheme and one leader apply to all four apps.
- Apps register their options (`graphics`, `icons`, …) in the kit's option registry.
- Going away: ghd's and muttrs' `themes/*.toml` (including `catppuccin-mocha.toml`), muttrs' `[keys.*]`, ghd's
  `icons =`. A one-off script converts them to `:hi` and `:nnoremap` lines.

### D7. Highlight groups and colorschemes

- The kit defines Neovim's standard groups:
  - UI: `Normal`, `NormalFloat`, `FloatBorder`, `StatusLine`, `StatusLineNC`, `TabLine`, `TabLineSel`, `TabLineFill`,
    `WinBar`, `WinSeparator`, `CursorLine`, `Visual`, `Search`, `CurSearch`, `IncSearch`, `Pmenu`, `PmenuSel`,
    `PmenuMatch`, `WhichKey*`, `Folded`, `NonText`, `EndOfBuffer`, `ErrorMsg`, `WarningMsg`, `MoreMsg`, `ModeMsg`,
    `Title`.
  - Syntax: `Comment`, `String`, `Constant`, `Function`, `Keyword`, `Type`, `Special`, `Underlined`.
  - `DiagnosticError`, `DiagnosticWarn`, `DiagnosticInfo`, `DiagnosticHint`, `DiagnosticOk`; `Added`, `Changed`,
    `Removed`, `DiffAdd`, `DiffDelete`; `@markup.heading`, `@markup.link`, `@markup.raw`, `@markup.quote`.
- Apps register their groups, each with a default link to a standard one (`GhdStateMerged` → `DiagnosticHint`).
  Registering returns a typed handle that spans carry; `:hi`, `:hi link` and `:hi clear` treat app groups like any
  other.
- Colorschemes define standard groups only. An app may ship overrides of its own groups for a scheme (devnull's tuned
  `Devnull*` tokyonight colours).
- Colorschemes are `.vim` files of `:hi` lines. Built-ins are embedded with `include_str!` and parsed exactly as user
  files in `colors/`; devnull's generated tables (`hl/tokyonight.rs`) go.
- Code: ghd's syntect class markers (`syntax.rs`) map onto the syntax groups.

### D8. Default look

- `tokyonight-moon` in all four (devnull changes from `tokyonight-night`).
- `Normal` and `TabLineFill` without a background, as devnull's `68bb8e8` made them; floats too (`NormalFloat`,
  `FloatBorder`, `FloatTitle`), as tokyonight's `floats = "transparent"` in the author's Neovim.
  `:hi Normal guibg=…` paints one again.
- An ANSI-16 `default` scheme replaces ghd's and muttrs' `terminal`. Chosen automatically when the terminal doesn't
  advertise truecolor (`COLORTERM` isn't `truecolor` or `24bit`, and the terminal isn't a known truecolor one), unless
  the user set a scheme.
- Windows are separated by `│` (`WinSeparator`), without boxes. Floats are rounded (`FloatBorder`); `winborder`
  changes that.

### D9. Icons

- One registry: the kit's chrome icons (unread/bell, lock, picture, link, folds, error, warning, spinner) and the apps'
  (`GhdPullRequest`, `MuttrsJunk`, …). Each is a pair `(nerd, plain)`.
- `:set icons=auto|nerd|plain`, default `auto`: nerd in Ghostty, kitty and WezTerm, which ship the symbols
  (`muttrs-gfx/src/lib.rs:46`), plain elsewhere.
- A fixed width per icon, the same in both modes: the glyph and a space it can spill into. Plain fallbacks are
  single-width (`●`, `★`, `↩`, `⚑`) or blank, padded alike, so `:set icons` never shifts a column.
- Icons take their colours from highlight groups.

### D10. Screen anatomy

- **Tabline**: listed buffers (devnull's bufferline; ghd's inbox views become buffers). Sidebar buffers are unlisted.
  `showtabline=2`, as bufferline shows it always in the author's Neovim (it was 1, Vim's).
- **Winbar** per window: titles and breadcrumbs, supplied by the buffer (slackers' breadcrumbs, muttrs'
  `account › mailbox`, ghd's Preview title).
- **One global statusline** (`laststatus=3`) of segments, each with an alignment and a priority; the lowest go first
  when narrow. Kit segments: mode, search count `[3/7]`, Top/Bot/%, the ruler `6:4`, a spinner with "N more". App
  segments: unread counts, connection state, voight. As lualine draws it in the author's Neovim: the mode and the
  ruler in the mode's colours, rounded with a Nerd Font. Pending keys show only with `:set showcmd` (off, as there).
- **Cmdline row** (`cmdheight=1`; `cmdheight=0` merges it into the statusline). Messages as Neovim's: they stay until
  replaced, errors in `ErrorMsg`, history in `:messages`. No toasts that expire.
- **No permanent key hints**: which-key after a prefix, and `:help`.
- Cost: ghd, muttrs and slackers lose a row unless `cmdheight=0`; slackers loses its hints.

### D11. Windows

- devnull's split tree (`:sp`, `:vs`, `<C-w>hjkl`, `:only`). Sidebars are windows.
- `winfixwidth`/`winfixheight`, and size constraints per window: fixed, a percentage, or `clamp(min, max)`.
- Apps build their layout through the window API the ex commands use; toggles are ex commands (`:Sidebar`, mapped to
  `\` in muttrs and slackers).
- No layout engine reacting to size: `Resize` reaches the app, which reshapes the tree (slackers folds the thread away
  below 120 columns, ghd hides the Preview below 12 rows). A hidden window's buffer keeps its cursor.

### D12. Keys

devnull's D5, for all four:

1. **The kit owns motions, and they stay pure Vim**: counts, `gg G H M L / ? n N * #`, `]]`/`[[` by item, folds,
   `<C-o>`/`<C-i>`/`<C-t>`, Visual mode and yanks. App defaults never rebind them; users may, with `:map`.
2. **Keys that edit text in Vim become app actions** (`c r R s d x o i a u`).
3. **Everything else goes under `<leader>`**, `<Space>` in every app, surfaced by which-key.
4. **Every action has an ex command**, so `:map` can rebind it.

What changes: `g` is a prefix (`gg gx gd gy`) in ghd, muttrs and slackers, where it now jumps to the top. `?` searches
backwards; help is `:help`, `<F1>` and `<leader>?`, no longer `?` in ghd and muttrs. voight's heatmap is `<leader>h`,
no longer `H`. `q` closes a window; muttrs' Pager is one, so `q` still goes back to the Index.

Shared verbs (a convention in the docs, not enforced): apps use these keys wherever their domain has the verb.

| Key | Verb |
| --- | --- |
| `<CR>`, a double click | Open (`:open`; the kit binds both) |
| `K` | Details card (`:details`); again, into it |
| `gx` | The link under the cursor in the browser (`:browse link`) |
| `gX` | The thing itself on the web (`:browse`): a commit, a Subject, a Message in Slack; mail has none |
| `gy` | Yank the link under the cursor, else the thing's (`:yanklink`) |
| `c` | New |
| `r` / `R` | Reply / quote |
| `s` | Star, flag |
| `x` / `A` | Select and move down / all shown, or none (`:toggle`, `:toggle!`) |
| `<Esc>` | Hide the search's matches (`:nohlsearch`, the kit's); in ghd and muttrs, then the selection, then the filter (`:clear`) |
| `]b` / `[b` | Next and previous buffer, as `gt` / `gT` (the kit's) |
| `<leader>f…` | Find, as fzf-lua's: `ff` the app's own (people, Notifications by filter, a Mailbox, a Conversation), `fb` buffers, `fh` help, `fk` keys, `fC` commands, `fj` jumps (the kit's) |
| `]h` / `[h` | Next and previous hunk, as gitsigns' (ghd's diffs) |
| `a` | Out of the inbox: archive (muttrs), Done (ghd) |
| `dd` | Delete |
| `\` | The sidebar: go to it, hide it from within, bring it back (`:sidebar`) |
| `q` | Close the window; quit rather than leave a sidebar alone |
| `<C-l>` | Refresh (`:e!`); `:e NAME` opens by name |
| `<F1>`, `<leader>?`, `gt`, `gT` | Help, next and previous buffer (the kit binds them) |
| `!` | Run a command with the terminal (`:shell`, `:!CMD`; the kit binds it) |

Commands shared by apps mean the same in each and are said the same in `:help`: `:details`, `:browse` (the thing on
the web) and `:browse link` (the link under the cursor, else the only one of what holds it, or a list of them),
`:yanklink`, `:toggle(!)`, `:select`, `:unselect`, `:filter`, `:clear`, `:sidebar`, `:exit`, `:voight` (`heatmap`
shows or hides; `on` and `off` start and stop assessing). Help texts say "yank", and an app's glossary terms
capitalised. One-letter abbreviations are the kit's (`:e`, `:b`, `:q`, `:h`, `:o`).

Dates are said the kit's way (`wcgw_kit::when`): `ago` in a feed's column (`5m`, `3h`, `2d`, `6w`, `1y`; ghd,
devnull), `short` in a list of mail or chat (`14:05`, `5 Nov`, `Nov 2025`; muttrs, slackers), `long` and `day` in
full (`Thu 5 Nov 2026, 14:05`).

### D13. Commands, registries, plugins

- One registry, `Commands<A>`, the kit's own commands included: devnull's closed `Command` enum and ghd's and muttrs'
  `match` plus name list dissolve into it. An entry: name and minimum abbreviation, whether it takes `!`, an argument
  completer `fn(&A, &str) -> Vec<String>`, a handler `fn(&mut A, Invocation) -> Result<Effects, ExError>`, and a help
  line and topic (feeding `:help` and which-key).
- Keys bind only to ex commands or to the kit's motions and actions.
- A plugin is a function, `install(&mut Kit<A>, …)`, that registers commands, default maps, highlight groups, options,
  icons and statusline segments. No plugin trait, lifecycle or dynamic loading. Its state lives in the app's `A`,
  reached through an accessor it's given.
- **Range highlights**, as Neovim's extmarks: overlays of (byte range in a buffer's source text → group or style)
  that the kit maps onto rendered spans. They replace ghd's `drawn::Finder` and muttrs' heatmap finder, and serve
  search and diff marks too.

### D14. Floats: picker, hover, confirm, help

- **Picker**: nucleo with fzf syntax and match positions (slackers' `Finder`), replacing devnull's scorer and muttrs'
  substring narrowing. Sources are a list, or async and streaming through the inbox (devnull's Users search, slackers'
  people). Verbs are the app's, each on a key (`<CR>` the default; muttrs' Copy and Save on `<C-y>`, `<C-s>`). Number
  selection optional per source (muttrs' Links). Items are rendered lines with groups and icons. No preview pane in
  v1.
- **Hover**: a float next to the cursor (devnull's and slackers' placement merged). Its content is a buffer: motions,
  yanks and pictures work inside, and `K` again enters it, as in Neovim.
- **Confirm**: `confirm(prompt, choices)` on the cmdline, `[y]es/[e]dit/[n]o` as devnull's. It replaces slackers'
  Details and React modes where they are choices.
- **`:help`** is generated from the registries, by topic; `:help zo` goes to one. ghd's `COMMAND_HELP` and muttrs'
  `help()` go, and their drift with them.

### D15. Graphics (`wcgw-kit-gfx`)

- **Base: slackers-gfx/muttrs-gfx.** Kitty graphics with Unicode placeholders, RGBA (`f=32`), all 297 diacritics, the
  `Picture::row` → `Span` API, deleting by id range (Ghostty skips virtual placements on `d=A`), eviction of the
  least recently drawn pictures over a memory budget (slackers' `pictures.rs`), half blocks (muttrs' `blocks.rs`).
- **From devnull**:
  - Native animation: frames with `a=f`, looped with `a=a`, verified inside placeholders in Ghostty 1.4.0 (devnull
    D7). slackers' ADR 0002 planned that move once Ghostty shipped it. The kit's frame timer only drives half blocks.
  - Pausing while unfocused through `a=a` (`s=1` to stop, `s=3` to loop): to verify (M0.1).
- **Detection**: environment variables, then an `a=q` query with a short timeout before the loop starts (devnull's D7
  asks for one; no app has it). tmux, zellij and screen get half blocks unless `:set graphics=kitty`.
- **Transfer**: `/dev/shm` (`t=t`) only when local, with `SSH_CONNECTION` and `SSH_TTY` unset; otherwise in-band.
- **Floor**: Ghostty 1.4 for kitty mode; older terminals get half blocks.
- `:set graphics=auto|kitty|halfblocks|off`. No sixel, no iTerm2 (devnull D7, muttrs ADR 0002).

### D16. Rich text (`wcgw-kit-text`), after the shell

- One block form, muttrs-render's (Flow, Stack, Gap, Rule, Table). Front ends behind features, as their dependencies
  are heavy: `html` (muttrs' reader: scraper, html5ever, its CSS subset and contrast-safe colours), `markdown` (new:
  pulldown-cmark to blocks), `syntax` (ghd's syntect class markers with two-face's syntaxes).
- Output goes into kit buffers: spans carry highlight groups (not `Style`s), link targets, picture slots and source byte
  ranges, which range highlights (D13) need.
- Domain rules stay in the apps, as passes or options: mail's format=flowed, signatures and attribution; GitHub's
  mentions and "HTML as GitHub shows it"; Slack's mrkdwn.
- tui-markdown goes: it can't carry groups or source ranges.

### D17. voight

- The kit knows nothing of voight, and voight nothing of the kit.
- The voight library gains a UI-agnostic `Assessor`, moved out of ghd's and muttrs' `assessment.rs`: a background
  thread, falling back to the CPU when the device fails, a cache by text, one assessment at a time, the 50-word
  minimum, `Assessment`/`Window`, and events over a channel.
- Each app keeps a `voight.rs` (about 50–80 lines): `install(&mut Kit<A>, …)` registers `:voight [heatmap|on|off]`, a
  statusline segment, `VoightHeat*` groups, a default `<leader>h` map, and range highlights over the text the app
  picks (ghd: Markdown normalised as GitHub shows it; muttrs: `Document::prose()`).
- The `voight`, `cuda`, `metal` and `wgpu` features stay in the apps.
- Rejected: a `voight-kit` crate (it would have to pin the same kit revision as every app, or Cargo builds two kits
  whose types don't unify); the per-app copies as they are (drifted already).

### D18. Packaging

- Workspace: `wcgw-kit` (the shell), `wcgw-kit-gfx` (pictures; `image`, `base64`, no tokio), `wcgw-kit-text` (D16,
  later).
- Apache-2.0. tokyonight's attribution ([folke/tokyonight.nvim](https://github.com/folke/tokyonight.nvim), Apache-2.0)
  comes over from devnull. slackers' code (GPL-3.0-or-later) is relicensed by its author as it moves in.
- **For now**: each app has a path dependency on the local checkout. CI on the apps' `wcgw-kit` branches can't resolve
  it until the kit is pushed.
- **Once this all works and is agreed**: `wcgw/wcgw-kit` is pushed, and the apps pin a git revision, as they pin
  voight; `publish = false`. Local work against a checkout then goes through an uncommitted
  `~/src/github.com/wcgw/.cargo/config.toml` with `[patch."https://github.com/wcgw/wcgw-kit"]`; its `Cargo.lock`
  churn isn't committed.
- Edition 2024, MSRV 1.99, `unsafe_code = "forbid"`, `missing_docs`, `missing_debug_implementations`,
  `rust_2018_idioms`, clippy pedantic.

### D19. Testing

- `wcgw_kit::testing`, behind a `testing` feature the apps enable for their tests. `Harness<A>` runs the shell and the
  app's `update` without a runtime or a terminal:
  - `press("<leader>1")` in Vim notation, `type_text`, `click_on("text")`, `scroll`, `resize`, `send(msg)`;
    `effects()`; `screen()` as rows and the cursor.
  - A fake clock, `advance(Duration)`: key timeouts, which-key, the spinner and half-block animation are
    deterministic.
  - `group_at(x, y)` beside `style_at`: tests assert the highlight group, so they survive a change of colorscheme.
  - Injected terminal facts: graphics, cell size (10×20), icons, focus.
- String rows by default; insta optional for whole-screen layouts.
- The kit's own suite tests chrome, motions, the cmdline and pickers with a small dummy app; apps don't re-test them.

### D20. Process

- Test-first (red → green → refactor); each step reports what ran (`fmt`, `clippy`, tests) and what didn't.
- Review checkpoints inside milestones (⏸).
- The user commits, between stages; Claude only when authorized in that session. Conventional-commit subjects, no AI
  attribution. Never push; no GitHub writes.
- Spikes live in the scratchpad, never a repo. Probes that need eyes on Ghostty are run by the user.
- Each app moves on its own `wcgw-kit` branch, with its suite green.

### Defaults (agreed together)

- **History**: the kit's, on disk in `$XDG_STATE_HOME/<app>/history`, with muttrs' mechanics: the file is re-read
  before each write so instances side by side merge; command and search lists apart; Up/Down recall by prefix; 200
  entries.
- **Identity**: the app gives its name, which sets `~/.config/<app>/`, its state directory and the window title. The kit
  parses `+{cmd}` arguments (`dn +cmd`); the app parses the rest.
- **Focus**: the kit tracks it, pauses animation itself, and tells the app (`Focus` messages). Polling cadences and
  notifying while unfocused are the app's.
- **Keyboard**: the kitty keyboard flags (`DISAMBIGUATE_ESCAPE_CODES`) when supported, so `<C-i>` isn't `<Tab>` and
  `<Esc>` acts at once.
- **Long lines**: `wrap` is a buffer option, on by default. `nowrap` buffers (ghd's table, muttrs' Index) get lines laid
  out by the app for the window's width, cut with `…`. No horizontal scrolling in v1.

## Milestones (proposed; to review)

Each app ends each milestone runnable, on its `wcgw-kit` branch.

### M0 Spikes (scratchpad, throwaway)

- [ ] **M0.1** Native animation in placeholders paused and resumed with `a=a,s=1`/`s=3`, in Ghostty 1.4 (user runs).
- [ ] **M0.2** The `a=q` query with a timeout: Ghostty, kitty, a terminal without graphics, and inside tmux.
- [ ] **M0.3** Pictures over SSH: confirm `/dev/shm` transfer fails there, and in-band works.

### M1 The kit, out of devnull (no behaviour changes but those listed)

- [x] **M1.1** Workspace skeleton: crates, lints, license and attribution; devnull's path dependency.
- [x] **M1.2** `key`; `keymap` with generic builtins, devnull's maps supplied by devnull, an open `Topic`.
- [x] **M1.3** `ex` into `Commands<A>`; completion per command. The option registry for applications waits for an
  application with an option of its own (none so far: `graphics`, `icons` and such are the kit's).
- [x] **M1.4** `hl`: standard groups, app groups with links, `.vim` colorschemes (tokyonight ×4, `default`); devnull's
  `Devnull*` colours as devnull's overrides. The generated schemes match devnull's tables group for group.
- [x] **M1.5** `doc` with a generic `Target`; `windows`/`window` laid out over a document. Range highlights wait for
  their first user (M5/M6).
- [x] **M1.6** Chrome out of `impl App`: tabline, statusline segments, cmdline and messages, which-key, picker,
  hover, confirm, generated `:help` (keys, then commands). The winbar waits for a buffer that has one (M3).
- [x] **M1.7** `run`: session, timers, `$EDITOR`, OSC 52; the test harness and fake clock. devnull's 234 tests green
  through a facade with the old `App`'s shape; a run of the real `dn` in tmux (isolated `HOME`): starts on
  `connect`, `:help` opens above with the separator line, `:qa` quits. Expected differences: moon by default, one
  statusline (`'laststatus'` 3), `:help` lists commands. OSC 777 waits for slackers (M3).

### M2 `wcgw-kit-gfx`

- [x] devnull's pictures (in-band PNG, the terminal's own animation, half blocks) with slackers' findings: the
  297-diacritic table (pictures up to 296 cells), still pictures to a local terminal as raw RGBA through `/dev/shm`,
  deleting by id range, pictures out of view freed least recently seen first over 128 MiB. `Terminal::detect`
  (kitty, Ghostty and its version, multiplexers, SSH, true colour, Nerd Font symbols); `:set graphics=off`.
  Without true colour the kit starts on `default` (D8). ⏸ try in Ghostty: not done here, see M0.

### M3 slackers

- [x] **M3.1** In the kit, each tested through the toy application: the composer (D4), the mouse (D5), windows of
  a size and placements (D11), winbars (D10), buffers that follow their end (D3), icons (D9), OSC 777
  notifications, nucleo in pickers (D14).
- [x] **M3.2** slackers on the kit (3948e60 the port; f871cfc, 884ec98, f23d057, 7eb00f9 taking what the kit grew from it):
  the composer with its hooks (`<Up>` edits the last Message, `<Esc>` gives an edit up), first unread on open,
  cursorline per Message, folds on section headers, `:qa` waiting for what is on its way to Slack, Workspaces'
  icons in winbars again, the details card entered with `K` twice. Behaviour changes
  it chose: reopening a Conversation marked unread no longer reads it at once; the sidebar suggests the first unread
  row until a key; Workspaces in the statusline are grouped by how they stand. ⏸ Relicensing the code that moved
  from slackers (GPL-3.0-or-later) into the kit (Apache-2.0): the author's to confirm (see D18).

### M4 `wcgw-kit-text`

- [x] muttrs-render, moved as it was (its blocks, HTML with a CSS subset, plain text with quotes and format=flowed,
  tables, contrast-safe authors' colours), with `Document::markdown` and a `kit` module: styles from the theme's
  groups, and a rendering as a kit document. ghd's syntax classes wait for ghd (M7).

### M5 voight's `Assessor`

- [x] `voight::assessor` on voight's branch `wcgw-kit` (ae23138, from 26c88c9, the revision the apps pinned): the
  thread, the CPU fallback, the 50-word check, `Assessment`/`Window`, events through a callback. Committed through a
  worktree in the scratchpad, removed since: the checkout at `voight/` was on `pangram` with uncommitted work, left
  as it was. Until voight's branch is pushed, the apps depend on it as `git = "file:///…/voight", branch =
  "wcgw-kit"`.

### M6 muttrs, then M7 ghd

- [x] Each with its `voight.rs` on voight's `Assessor`, its TOML themes and keys converted to `init.vim`, and its keys
  moved to D12.
  - **muttrs** (a0a9bdc the port; d14e479, 3c9c076, 04f0667, d9a773a, 21d7572, 75456b3, a9072a5 after): cursors placed by the kit in place of
    reopening windows, `'cursorline'`, widths per window (`:vsplit` works), picker verbs for Links, Attachments and
    Labels, `:qa` waiting for Tasks, `prompt_completing` for To: and Attach:, Links found by the text crate before
    the Heatmap shades them, `!` and `|` with the terminal (`Effect::Shell`, piping bytes), Drafts as `.eml` in
    the configured `editor` again, selecting as ghd does (`x` then down, `<Esc>` clears). Commands
    renamed to letters only (`:notjunk`, `:delmailbox`). History is the kit's, in the same file; `!` lines are no
    longer kept.
  - **ghd** (e81eace the port; a724508, fe49795, 60a0980 after): `CursorLine::Item` in place of drawing the cursor row,
    widths per window, one buffer each for the Preview, files list and file, every View's cursor restored from the
    cache, counts in the tabs (`Inbox (3)`), code coloured through `highlight_code` (a wide line is cut, still
    coloured), Links through `linked_lines`, shaded after. Waits for GitHub in `main` after `run_returning` rather than through `App::quit`.

### M8 Publishing

- [ ] Push `wcgw/wcgw-kit`; the apps pin a git revision; CI green on all four.
  - [x] Pushed, and tagged `v0.1.0` (88762d4) by the author; the four apps build from the tag (devnull 5a6e3e9, ghd
    e200c2c, muttrs 9423f2f, slackers 847c3d7), each tested on it.
  - [ ] voight's `wcgw-kit` branch pushed: ghd and muttrs still pin it in a local checkout (`file://`).
  - [ ] CI green on all four: not checked.

## Implementation notes

Calls made while building, and why; each can be revisited.

- **The app contract** (open question of M1): `App` has associated `Target`, `Msg` (`Clone + PartialEq + Send`, so
  tests compare messages) and `Effect` types; `update` plus hooks with defaults (`start`, `open`, `edit`,
  `edit_names`, `reload`, `shown`, `keyed`, `edited`, `focus`). Commands, pickers and segments are closures over
  `(&mut A, &mut Kit<A>)`; a plugin's state lives in `A`. `Kit::show_buffer` queues the buffer and the kit calls
  `App::shown` after the application returns, so the kit never calls into the application re-entrantly.
- **Effects** are `Effect<M, E>`: the kit's plus `App(E)`; `After { after, msg }` replaces application wake tokens
  (devnull's polling). The editor's result goes back through `App::edited`; only one editor runs at a time.
- **`:open`, `<CR>`, `<C-]>` are the kit's**: `:open` hands the target under the cursor to `App::open`, with the tag
  stack kept by the kit (as any command made with `Command::jumps`).
- **Tabline**: shows the listed buffers, or the current one when none is listed (devnull's `connect` page). devnull
  keeps it always shown (`showtabline=2`), as it always was.
- **The harness has `style_at`, not `group_at` yet**: a cell holds a style, not the groups that made it. To add when a
  test needs it.
- **No `a=q` query** (D15 asked for one): it tells whether the kitty protocol is there, not whether Unicode
  placeholders are (WezTerm and Konsole have the protocol without them), and its answer would race the first keys
  typed. Detection is by environment only; `:set graphics=` overrides it.
- **Animations go in band**, as PNG frames, even to a local terminal: devnull verified them in Ghostty 1.4.0 that
  way; frames through `/dev/shm` are untried. Still pictures use `/dev/shm` when local (slackers' way, unverified
  in 1.4 by this work). A terminal that doesn't animate (Ghostty before 1.4) gets the first frame only, rather than
  slackers' frame swapping.
- **The kit has its own shell suite** now (a toy application, about 50 tests), for what devnull's doesn't reach:
  hooks, the registries, segments, searching pickers, pictures' budget and `graphics=off`, the composer, the mouse,
  windows of a size, winbars, following the end, cursorline, verbs.
- **`<C-w>q`** is bound, as in Vim.
- **The apps built against the kit's working tree** (path dependencies, D18) while it changed: twice an agent caught
  it mid-edit and checked against a `git archive` of the kit's HEAD instead. The kit was tested before each commit; M8's
  pinned revisions end this.
- **One way across the apps** (a pass after the ports, comparing their keys, commands and looks): what was the same
  everywhere went into the kit's defaults (`<F1>`, `<leader>?`, `gt`, `gT`, a double click opening) and `when`;
  what differed took one app's way, as the table in D12 says. The calls: feeds keep ages in three cells (devnull's
  dates after a week went), lists of mail and chat put the day first (`5 Nov`) and the month for another year, so
  muttrs' date column is two cells wider; `:browse` alone is the thing on the web in every app, so muttrs' says a
  Message has none; ghd's Done went to `a`, as muttrs archives on it, leaving `s` to star or flag; slackers' `o` and
  single click no longer open; muttrs' `theme`, `icons`, `images` and `mouse` are read no more, as ghd's. Commits:
  the kit d964a82, 166461f, 46bf749, f647dd2; devnull 1afa472, 8285b6c, 7c0b259; ghd 3c7bfdf, 74e5723, 033c800,
  51535cc, 11e7c31, efaa01c; muttrs a9072a5, fb1db7a, 24d3c66, 6826bbd, b91a842, 25a3525, b614a9f, f6c6cc2;
  slackers e69e1e5 (`q` left the sidebar alone), 03147d4, a51d19c, d8a8f06.
- **Commands the same way** (a second pass): muttrs' duplicates went (`:refresh` and `:cd` were `:e!` and `:e NAME`;
  `:sync` lists the open Mailbox anew), `:limit` is `:filter` as ghd's, `:shell` went into the kit, `:browse link`
  off a link lists what holds it in devnull and ghd too (the kit's `targets_in_block_under_cursor`), muttrs'
  `:voight on`/`off` start and stop assessing as ghd's, and the wording and abbreviations follow D12. Commits: the
  kit a3f1955, ad39f88; devnull c61dbbc, f76ca47; ghd df18dda, c981398, b80f0c6 (with a clippy slip, 07e0fe2);
  muttrs 0d35d77, e4e9fe4, aee8819, 64f3254, 95590fd, d46290f, 3c6d76a; slackers 1afe686 (a doc comment misplaced
  in a51d19c), 953e5fe, f4bd41f.
- **The author's Neovim, as defaults** (a third pass, from `~/.config/nvim`): `<Esc>` hides the search's matches,
  `]b`/`[b` go through buffers, `<leader>f` finds (fzf-lua's maps), the statusline is lualine's, floats leave the
  background to the terminal (tokyonight's transparent floats), `noshowcmd`, `showtabline=2` (bufferline's),
  sidebars 35 cells (neo-tree's), prose up to `textwidth=120`, and ghd's diffs go by hunk (gitsigns' `]h`). The calls:
  ghd's filter is its `<leader>ff`, muttrs' forward moved to `<leader>F` and moving to `:pick move`, muttrs'
  `pager_width` is read no more for `textwidth`, devnull's `<leader>/` is `<leader>ff`. Commits: the kit 37fde61,
  2d055ee, 5eca281, 37c0090, 34ead85, e3b32a5, e090fee, 952b14d, e08f989, c9f11c6; devnull 9e6716f, 8d1707a,
  1d23220, 944f45b; ghd ec48f97, e76ce00, 2769fdb, 5c0506f; muttrs 8a4d0bf, f734ab3, 2544856, 16b26dc, 6c8980e;
  slackers 79b2705, 1ee0537, d6fa271, b0de83a, 39370e4.
- **Still open, from the ports**: a span carries one highlight group, so ghd lays its read/unread/selected styles
  over a View's when drawing it, and `:hi` on those shows at the next redraw; a buffer shown in two windows of
  different widths is laid out (by the application) for one of them.
- **History is written when quitting**, merged with the file as it is then, rather than at every entry as muttrs
  did: the kit's core stays free of I/O; a crash loses that run's entries.
- **Lazy lines** (D3) are not built: no application has needed them yet; slackers' and muttrs' histories are paged
  in by the application, as devnull's logs are.
- **Following the end**: a window follows a buffer's end only when what changed came from elsewhere than a key, and
  only if its cursor was on the last row after the previous update; a key moving up stops it, `G` resumes it.
- **Rich text is laid out by the text crate, not the kit's wrapping**: muttrs' renderer draws quote bars and table
  borders on every row a paragraph wraps to, which the kit's document can't do; a rendering's lines become a
  document laid out for its window's width, made again when the width changes. Its colours (theme's and authors')
  go on spans as colours of their own (`Span::with_style`), so the theme's groups still decide them through
  `kit::styles`. Spans don't carry link targets yet: links are numbered, as muttrs has them.
- **Markdown goes through HTML** (pulldown-cmark's), rather than a reader of its own: one layout for both.
- **The mail-only parts stay in the text crate for now** (plain text's quotes, format=flowed and signatures): they
  read any plain text well enough, and muttrs is their only user. D16 put them in the application.
- **Quitting can wait** (muttrs' sends and saves): `App::quit` answers `Quitting::Later` to `:q` on the last window
  or `:qa`, and the application sends `Effect::Quit` once done. `:qa!` doesn't ask. The kit doesn't show that it is
  waiting; the application says so (`kit.info`).
- **Pictures from the start**: `Kit::start` asks for the pictures in view, and the runtime keeps the first resize's
  effects; before, a picture on the first screen waited for the first key.
- **What an application may read back**: `Kit::graphics` (`:set graphics=`), `Kit::on_closed_fold`,
  `Kit::window_cursor`, `Kit::text_width`. Options aren't announced when they change; an application reads them when
  it needs them.
- **Command names are letters only** (`ex.rs` splits the name at the first non-letter), as Vim's: muttrs renamed
  `:not-junk` to `:notjunk` and `:delete-mailbox` to `:delmailbox` (its ADR 0007).
- **Picker verbs** act on the choice under the cursor and leave the picker open, for the verb to change its
  choices (`Picker::set_choices`, after deleting a draft, say) or close it (`Kit::close_picker`, to open what was
  chosen in a split, say); the border's bottom says them. A pick still closes it. (Before 8634e00 a verb closed
  the picker, against what this note said; slackers found it.)
- **The hover card can be entered** as Neovim's: showing it again by the key that showed it (`K` twice) enters
  it. `j`/`k`, `<C-d>`/`<C-u>`, `gg`/`G` and the wheel go through it with a cursorline, `<CR>` opens what its row
  refers to through `App::open`, and `q`, `<Esc>`, `K` or a click leave it. It isn't a window: no search, no visual
  selection, no `<C-w>` into it.
- **Pictures in winbars**: `Buffer::set_winbar_spans` takes the document's spans, so `Span::picture` (an
  avatar, a channel's icon) goes in a winbar as in text; `set_winbar` stays for text in groups.
- **The terminal handed to a command** (muttrs' `!` and `|`): `Effect::Shell(Shell::new(cmd).stdin(text).wait())`
  runs `sh -c` with the terminal, as `$EDITOR` gets it, and sends `Msg::Shelled` with the exit code to
  `App::shelled`; the kit says `shell returned N` first, as Vim. `Effect::Edit` became `Effect::Edit(Edit)` with
  `.suffix("eml")` and `.editor(cmd)` (a breaking change; devnull and muttrs follow).
- **Any quit frees the pictures**: `Kit::update` puts the escapes deleting what the terminal holds before an
  `Effect::Quit`, the application's own (after `Quitting::Later`) as the kit's.
- **The composer's hooks**: `on_key` sees a key before the text area does and may take it, `on_word` hears the word
  before the cursor as it changes, `on_leave` hears insert mode end (to keep a draft, say).
- **The composer's completion** gives the application the word before the cursor; the application decides what it
  is (an @-mention, an emoji, a slash command), returns suggestions, and hears which was chosen, to keep what it
  means (slackers' picked references stay slackers').

## Found in the apps while surveying (not caused by this plan)

- slackers and muttrs send pictures through `/dev/shm` whenever it's writable. Over SSH that path is on the remote
  host, where the terminal can't read it. Follows from the code; not run over SSH (M0.3).
- devnull's D7 calls for an `a=q` query; the code detects from the environment only.
- muttrs carries out `:login` (`crates/muttrs/src/ui.rs:2518`), but `COMMANDS`, its completion list, lacks it.
- muttrs declares no license.

## Open questions

- The shape of `A`'s contract: associated `Msg`, `Effect` and `Target` types, and how a plugin reaches its state.
  Settled in M1.
- Lazy buffers: how `/` searches lines that were never laid out (ask the provider? search the source text?).
- Where the one-off TOML-to-`init.vim` converter lives, and whether it ships.
