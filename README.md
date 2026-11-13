# wcgw-kit

A Neovim look & feel for [ratatui](https://ratatui.rs) applications: the toolkit behind
[devnull](https://github.com/wcgw/devnull) (`dn`), [ghd](https://github.com/wcgw/ghd),
[muttrs](https://github.com/wcgw/muttrs) and [slackers](https://github.com/wcgw/slackers).

It owns what makes them feel like one editor: keys and the keymap, ex commands, the command line, highlight groups
and colorschemes, read-only buffers with a cursor and Vim motions, windows, pictures in the terminal, and the chrome
around them. Applications bring their own content and commands.

The design, its decisions and the plan to get there are in [docs/plan.md](docs/plan.md).

## License

Apache License 2.0 ([LICENSE](LICENSE)).

The tokyonight colorschemes come from [folke/tokyonight.nvim](https://github.com/folke/tokyonight.nvim), under the
Apache License 2.0 too ([LICENSES](LICENSES)); `scripts/tokyonight.py` writes them from its `extras`.
