#!/usr/bin/env python3
"""Writes the kit's tokyonight colorschemes (`crates/wcgw-kit/colors/tokyonight-*.vim`) from folke/tokyonight.nvim's
`extras/lua/tokyonight_<style>.lua`, as with its `transparent = true`: Normal and TabLineFill leave the background
to the terminal.

    scripts/tokyonight.py <commit> <dir holding tokyonight_{night,storm,moon,day}.lua> <out dir>

Only the kit's standard groups are written (D7 in docs/plan.md); an application's own groups link to them.
"""

import re
import sys

STYLES = ["night", "storm", "moon", "day"]

# The kit's standard groups, in the order the files list them.
GROUPS = [
    "Normal", "NormalFloat", "FloatBorder", "FloatTitle", "NonText", "EndOfBuffer", "Conceal",
    "StatusLine", "StatusLineNC", "StatusLineModeNormal", "StatusLineModeVisual", "StatusLineModeCommand",
    "StatusLineModeInsert", "TabLine", "TabLineSel", "TabLineFill", "WinBar", "WinBarNC", "WinSeparator",
    "CursorLine", "Visual", "Search", "CurSearch", "IncSearch", "Pmenu", "PmenuSel", "PmenuMatch",
    "WhichKey", "WhichKeySeparator", "WhichKeyDesc", "WhichKeyGroup", "Folded", "ErrorMsg", "WarningMsg",
    "MoreMsg", "ModeMsg", "Question", "Title", "Directory", "Bold", "Italic",
    "Comment", "Constant", "String", "Character", "Number", "Boolean", "Identifier", "Function", "Statement",
    "Keyword", "Operator", "PreProc", "Type", "Special", "SpecialChar", "Delimiter", "Underlined", "Todo", "Error",
    "DiagnosticError", "DiagnosticWarn", "DiagnosticInfo", "DiagnosticHint", "DiagnosticOk",
    "Added", "Changed", "Removed", "DiffAdd", "DiffChange", "DiffDelete", "DiffText",
    "@markup.heading", "@markup.strong", "@markup.italic", "@markup.strikethrough", "@markup.underline",
    "@markup.link", "@markup.link.url", "@markup.link.label", "@markup.raw", "@markup.quote", "@markup.list",
]

ATTRS = ["bold", "italic", "underline", "undercurl", "strikethrough", "reverse"]


def parse(text):
    """The palette and the highlights of one extras file."""
    palette = {}
    body = text.split("local highlights = {", 1)
    for name, value in re.findall(r'^  (\w+) = "(#[0-9a-fA-F]{6})"', body[0], re.M):
        palette[name] = value.lower()
    for table, inner in re.findall(r"^  (\w+) = \{\n(.*?)^  \}", body[0], re.M | re.S):
        for name, value in re.findall(r'(\w+) = "(#[0-9a-fA-F]{6})"', inner):
            palette[f"{table}.{name}"] = value.lower()
    groups = {}
    for match in re.finditer(r'^  (?:\["([^"]+)"\]|(\w+)) = (?:"([^"]+)"|\{(.*?)\}),?$', body[1], re.M | re.S):
        name = match.group(1) or match.group(2)
        if match.group(3) is not None:
            groups[name] = match.group(3)
        else:
            spec = {}
            for key, value in re.findall(r'(\w+) = ("[^"]*"|true|false)', match.group(4) or ""):
                spec[key] = value.strip('"')
            groups[name] = spec
    return palette, groups


def line(name, spec):
    """`hi` for one group."""
    if isinstance(spec, str):
        return f"hi link {name} {spec}"
    parts = []
    attrs = [a for a in ATTRS if spec.get(a) == "true"]
    if attrs:
        parts.append("gui=" + ",".join(attrs))
    for key, vim in (("fg", "guifg"), ("bg", "guibg"), ("sp", "guisp")):
        value = spec.get(key)
        if value and value != "NONE":
            parts.append(f"{vim}={value.lower()}")
    return f"hi {name} " + (" ".join(parts) if parts else "NONE")


def scheme(style, palette, groups):
    p = palette
    # What tokyonight leaves to lualine and other plugins, or names otherwise: from its palette.
    extra = {
        "StatusLineModeNormal": {"fg": p["black"], "bg": p["blue"], "bold": "true"},
        "StatusLineModeVisual": {"fg": p["black"], "bg": p["magenta"], "bold": "true"},
        "StatusLineModeCommand": {"fg": p["black"], "bg": p["yellow"], "bold": "true"},
        "StatusLineModeInsert": {"fg": p["black"], "bg": p["green"], "bold": "true"},
        "DiagnosticOk": {"fg": p["green1"]},
        "Added": {"fg": p["git.add"]},
        "Changed": {"fg": p["git.change"]},
        "Removed": {"fg": p["git.delete"]},
        "Number": "Constant",
        "SpecialChar": "Special",
        "Boolean": "Constant",
        "@markup.quote": "Comment",
    }
    out = []
    for name in GROUPS:
        spec = groups.get(name, extra.get(name))
        if spec is None:
            raise SystemExit(f"tokyonight-{style}: no {name}")
        if isinstance(spec, str) and spec not in GROUPS:
            # A link to a group the kit doesn't know: what that group is, followed to the end.
            seen = {name}
            while isinstance(spec, str) and spec not in GROUPS and spec not in seen:
                seen.add(spec)
                spec = groups.get(spec, {})
        if name in ("Normal", "TabLineFill") and isinstance(spec, dict):
            spec = {k: v for k, v in spec.items() if k != "bg"}
        out.append(line(name, spec))
    return out


def main():
    commit, src, dst = sys.argv[1:4]
    for style in STYLES:
        with open(f"{src}/tokyonight_{style}.lua") as f:
            palette, groups = parse(f.read())
        name = "tokyonight-" + style
        lines = [
            f'" {name}, from folke/tokyonight.nvim (Apache-2.0, Copyright Folke Lemaitre)',
            f'" extras/lua/tokyonight_{style}.lua at {commit}, with transparent = true.',
            '" Written by scripts/tokyonight.py: regenerate rather than edit.',
            "hi clear",
        ] + scheme(style, palette, groups)
        with open(f"{dst}/{name}.vim", "w") as f:
            f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
