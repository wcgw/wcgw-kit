" tokyonight-night, from folke/tokyonight.nvim (Apache-2.0, Copyright Folke Lemaitre)
" extras/lua/tokyonight_night.lua at cdc07ac78467a233fd62c493de29a17e0cf2b2b6, with transparent = true and floats transparent.
" Written by scripts/tokyonight.py: regenerate rather than edit.
hi clear
hi Normal guifg=#c0caf5
hi NormalFloat guifg=#c0caf5
hi FloatBorder guifg=#27a1b9
hi FloatTitle guifg=#27a1b9
hi NonText guifg=#545c7e
hi EndOfBuffer guifg=#1a1b26
hi Conceal guifg=#737aa2
hi StatusLine guifg=#a9b1d6 guibg=#16161e
hi StatusLineNC guifg=#3b4261 guibg=#16161e
hi StatusLineModeNormal gui=bold guifg=#15161e guibg=#7aa2f7
hi StatusLineModeVisual gui=bold guifg=#15161e guibg=#bb9af7
hi StatusLineModeCommand gui=bold guifg=#15161e guibg=#e0af68
hi StatusLineModeInsert gui=bold guifg=#15161e guibg=#9ece6a
hi TabLine guifg=#3b4261 guibg=#16161e
hi TabLineSel guifg=#15161e guibg=#7aa2f7
hi TabLineFill NONE
hi link WinBar StatusLine
hi link WinBarNC StatusLineNC
hi WinSeparator gui=bold guifg=#15161e
hi CursorLine guibg=#292e42
hi Visual guibg=#283457
hi Search guifg=#c0caf5 guibg=#3d59a1
hi link CurSearch IncSearch
hi IncSearch guifg=#15161e guibg=#ff9e64
hi Pmenu guifg=#c0caf5 guibg=#16161e
hi PmenuSel guibg=#343a55
hi PmenuMatch guifg=#2ac3de guibg=#16161e
hi WhichKey guifg=#7dcfff
hi WhichKeySeparator guifg=#565f89
hi WhichKeyDesc guifg=#bb9af7
hi WhichKeyGroup guifg=#7aa2f7
hi Folded guifg=#7aa2f7 guibg=#3b4261
hi ErrorMsg guifg=#db4b4b
hi WarningMsg guifg=#e0af68
hi MoreMsg guifg=#7aa2f7
hi ModeMsg gui=bold guifg=#a9b1d6
hi Question guifg=#7aa2f7
hi Title gui=bold guifg=#7aa2f7
hi Directory guifg=#7aa2f7
hi Bold gui=bold guifg=#c0caf5
hi Italic gui=italic guifg=#c0caf5
hi Comment gui=italic guifg=#565f89
hi Constant guifg=#ff9e64
hi String guifg=#9ece6a
hi Character guifg=#9ece6a
hi link Number Constant
hi link Boolean Constant
hi Identifier guifg=#bb9af7
hi Function guifg=#7aa2f7
hi Statement guifg=#bb9af7
hi Keyword gui=italic guifg=#7dcfff
hi Operator guifg=#89ddff
hi PreProc guifg=#7dcfff
hi Type guifg=#2ac3de
hi Special guifg=#2ac3de
hi link SpecialChar Special
hi link Delimiter Special
hi Underlined gui=underline
hi Todo guifg=#1a1b26 guibg=#e0af68
hi Error guifg=#db4b4b
hi DiagnosticError guifg=#db4b4b
hi DiagnosticWarn guifg=#e0af68
hi DiagnosticInfo guifg=#0db9d7
hi DiagnosticHint guifg=#1abc9c
hi DiagnosticOk guifg=#73daca
hi Added guifg=#449dab
hi Changed guifg=#6183bb
hi Removed guifg=#914c54
hi DiffAdd guibg=#243e4a
hi DiffChange guibg=#1f2231
hi DiffDelete guibg=#4a272f
hi DiffText guibg=#394b70
hi link @markup.heading Title
hi @markup.strong gui=bold
hi @markup.italic gui=italic
hi @markup.strikethrough gui=strikethrough
hi @markup.underline gui=underline
hi @markup.link guifg=#1abc9c
hi link @markup.link.url Underlined
hi link @markup.link.label SpecialChar
hi link @markup.raw String
hi link @markup.quote Comment
hi @markup.list guifg=#89ddff
