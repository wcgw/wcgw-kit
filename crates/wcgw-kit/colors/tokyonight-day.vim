" tokyonight-day, from folke/tokyonight.nvim (Apache-2.0, Copyright Folke Lemaitre)
" extras/lua/tokyonight_day.lua at cdc07ac78467a233fd62c493de29a17e0cf2b2b6, with transparent = true and floats transparent.
" Written by scripts/tokyonight.py: regenerate rather than edit.
hi clear
hi Normal guifg=#3760bf
hi NormalFloat guifg=#3760bf
hi FloatBorder guifg=#4094a3
hi FloatTitle guifg=#4094a3
hi NonText guifg=#8990b3
hi EndOfBuffer guifg=#e1e2e7
hi Conceal guifg=#68709a
hi StatusLine guifg=#6172b0 guibg=#d0d5e3
hi StatusLineNC guifg=#a8aecb guibg=#d0d5e3
hi StatusLineModeNormal gui=bold guifg=#b4b5b9 guibg=#2e7de9
hi StatusLineModeVisual gui=bold guifg=#b4b5b9 guibg=#9854f1
hi StatusLineModeCommand gui=bold guifg=#b4b5b9 guibg=#8c6c3e
hi StatusLineModeInsert gui=bold guifg=#b4b5b9 guibg=#587539
hi TabLine guifg=#a8aecb guibg=#d0d5e3
hi TabLineSel guifg=#b4b5b9 guibg=#2e7de9
hi TabLineFill NONE
hi link WinBar StatusLine
hi link WinBarNC StatusLineNC
hi WinSeparator gui=bold guifg=#b4b5b9
hi CursorLine guibg=#c4c8da
hi Visual guibg=#b7c1e3
hi Search guifg=#3760bf guibg=#7890dd
hi link CurSearch IncSearch
hi IncSearch guifg=#b4b5b9 guibg=#b15c00
hi Pmenu guifg=#3760bf guibg=#d0d5e3
hi PmenuSel guibg=#b3b8d1
hi PmenuMatch guifg=#188092 guibg=#d0d5e3
hi WhichKey guifg=#007197
hi WhichKeySeparator guifg=#848cb5
hi WhichKeyDesc guifg=#9854f1
hi WhichKeyGroup guifg=#2e7de9
hi Folded guifg=#2e7de9 guibg=#a8aecb
hi ErrorMsg guifg=#c64343
hi WarningMsg guifg=#8c6c3e
hi MoreMsg guifg=#2e7de9
hi ModeMsg gui=bold guifg=#6172b0
hi Question guifg=#2e7de9
hi Title gui=bold guifg=#2e7de9
hi Directory guifg=#2e7de9
hi Bold gui=bold guifg=#3760bf
hi Italic gui=italic guifg=#3760bf
hi Comment gui=italic guifg=#848cb5
hi Constant guifg=#b15c00
hi String guifg=#587539
hi Character guifg=#587539
hi link Number Constant
hi link Boolean Constant
hi Identifier guifg=#9854f1
hi Function guifg=#2e7de9
hi Statement guifg=#9854f1
hi Keyword gui=italic guifg=#007197
hi Operator guifg=#006a83
hi PreProc guifg=#007197
hi Type guifg=#188092
hi Special guifg=#188092
hi link SpecialChar Special
hi link Delimiter Special
hi Underlined gui=underline
hi Todo guifg=#e1e2e7 guibg=#8c6c3e
hi Error guifg=#c64343
hi DiagnosticError guifg=#c64343
hi DiagnosticWarn guifg=#8c6c3e
hi DiagnosticInfo guifg=#07879d
hi DiagnosticHint guifg=#118c74
hi DiagnosticOk guifg=#387068
hi Added guifg=#4197a4
hi Changed guifg=#506d9c
hi Removed guifg=#c47981
hi DiffAdd guibg=#b7ced5
hi DiffChange guibg=#d5d9e4
hi DiffDelete guibg=#dababe
hi DiffText guibg=#92a6d5
hi link @markup.heading Title
hi @markup.strong gui=bold
hi @markup.italic gui=italic
hi @markup.strikethrough gui=strikethrough
hi @markup.underline gui=underline
hi @markup.link guifg=#118c74
hi link @markup.link.url Underlined
hi link @markup.link.label SpecialChar
hi link @markup.raw String
hi link @markup.quote Comment
hi @markup.list guifg=#006a83
