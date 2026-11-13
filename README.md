# wcgw-kit

A Neovim look & feel for [ratatui](https://ratatui.rs) applications: the toolkit behind
[devnull](https://github.com/wcgw/devnull) (`dn`), [ghd](https://github.com/wcgw/ghd),
[muttrs](https://github.com/wcgw/muttrs) and [slackers](https://github.com/wcgw/slackers).

It owns what makes them feel like one editor: keys and the keymap, ex commands, the command line, highlight groups
and colorschemes, read-only buffers with a cursor and Vim motions, windows, pictures in the terminal, and the chrome
around them. Applications bring their own content and commands.

The design, its decisions and the plan to get there are in [docs/plan.md](docs/plan.md).

## Crates

- `wcgw-kit`: the editor. `Kit<A>` holds buffers, windows, the keymap, the command line, highlight groups, pictures
  and the composer; an application implements `App`; `runtime::run` owns the terminal; `testing::Harness` drives it
  all in tests.
- `wcgw-kit-gfx`: pictures as text, through the kitty graphics protocol's placeholders or half blocks.
- `wcgw-kit-text`: rich text (HTML, Markdown, plain) laid out as lines, and as the kit's documents.

## An application, in short

```rust
struct Chat { /* the application's own state */ }

impl wcgw_kit::App for Chat {
    type Target = Thing;      // what text refers to: what `<CR>` (`:open`) opens
    type Msg = Event;         // what its own work tells it
    type Effect = Request;    // what its `Perform` does

    fn update(&mut self, kit: &mut Kit<Self>, msg: Event) -> Effects<Self> { /* … */ }
    fn open(&mut self, kit: &mut Kit<Self>, target: Thing) -> Result<Effects<Self>, String> { /* … */ }
}

let mut kit = Kit::new("home", document, true);
kit.add_command(Command::new("star", 4, |chat, kit, line| chat.star(kit)).help("star it"));
kit.keymap_mut().bind(&[Mode::Normal], "s", "star", "star it", Topic::new(75, "Chat"));
kit.theme_mut().define(MENTION, Some(Group::DIAGNOSTIC_WARN));
runtime::run("chat", Chat::new(), kit, network, runtime::channel()).await
```

devnull is the reference: its `app/` shows the hooks, commands, a searching picker and tests through the harness.

## License

Apache License 2.0 ([LICENSE](LICENSE)).

The tokyonight colorschemes come from [folke/tokyonight.nvim](https://github.com/folke/tokyonight.nvim), under the
Apache License 2.0 too ([LICENSES](LICENSES)); `scripts/tokyonight.py` writes them from its `extras`.
