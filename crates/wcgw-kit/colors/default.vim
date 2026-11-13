" default: the terminal's own sixteen colours, for terminals without true colour and for those who want the
" applications in their terminal's theme.
hi clear
hi Normal NONE
hi NormalFloat NONE
hi FloatBorder guifg=darkgray
hi FloatTitle gui=bold
hi NonText guifg=blue
hi EndOfBuffer guifg=blue
hi Conceal guifg=darkgray
hi StatusLine gui=reverse
hi StatusLineNC gui=reverse guifg=darkgray
hi StatusLineModeNormal gui=bold
hi StatusLineModeVisual gui=bold
hi StatusLineModeCommand gui=bold
hi StatusLineModeInsert gui=bold
hi TabLine guifg=darkgray
hi TabLineSel gui=bold
hi TabLineFill guifg=darkgray
hi link WinBar StatusLine
hi link WinBarNC StatusLineNC
hi WinSeparator guifg=darkgray
hi CursorLine gui=underline
hi Visual gui=reverse
hi Search gui=reverse
hi CurSearch gui=reverse
hi IncSearch gui=reverse
hi Pmenu guibg=darkgray
hi PmenuSel gui=reverse
hi PmenuMatch gui=bold
hi WhichKey guifg=cyan
hi WhichKeySeparator guifg=darkgray
hi WhichKeyDesc NONE
hi WhichKeyGroup guifg=magenta
hi Folded guifg=cyan
hi ErrorMsg guifg=red
hi WarningMsg guifg=yellow
hi MoreMsg guifg=green
hi ModeMsg gui=bold
hi Question guifg=green
hi Title gui=bold guifg=magenta
hi Directory guifg=blue
hi Bold gui=bold
hi Italic gui=italic
hi Comment guifg=darkgray
hi Constant guifg=red
hi String guifg=green
hi link Character String
hi link Number Constant
hi link Boolean Constant
hi Identifier guifg=cyan
hi Function guifg=blue
hi Statement guifg=yellow
hi link Keyword Statement
hi link Operator Statement
hi PreProc guifg=magenta
hi Type guifg=green
hi Special guifg=magenta
hi link SpecialChar Special
hi link Delimiter Special
hi Underlined gui=underline guifg=blue
hi Todo gui=reverse guifg=yellow
hi Error guifg=red
hi DiagnosticError guifg=red
hi DiagnosticWarn guifg=yellow
hi DiagnosticInfo guifg=blue
hi DiagnosticHint guifg=cyan
hi DiagnosticOk guifg=green
hi Added guifg=green
hi Changed guifg=blue
hi Removed guifg=red
hi DiffAdd guifg=green
hi DiffChange guifg=blue
hi DiffDelete guifg=red
hi DiffText gui=bold guifg=blue
hi link @markup.heading Title
hi @markup.strong gui=bold
hi @markup.italic gui=italic
hi @markup.strikethrough gui=strikethrough
hi @markup.underline gui=underline
hi link @markup.link Underlined
hi link @markup.link.url Underlined
hi link @markup.link.label Special
hi link @markup.raw String
hi link @markup.quote Comment
hi link @markup.list Special
