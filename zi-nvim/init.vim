inoremap fd <ESC>

" https://unix.stackexchange.com/questions/445989/vim-cw-dw-whitespace-inconsistency
" Remove this inconsistency as it can be accomplished with `ce` anyway.
" The `map cw dwi` and remapping solutions in general have side effects.
" Press `cc` for example and you'll notice vim is waiting for another input.
" onoremap <expr> w <SID>ExpectedChangeWord('w', v:operator, v:count)
" onoremap <expr> W <SID>ExpectedChangeWord('W', v:operator, v:count)
" function! s:ExpectedChangeWord(w, op, c)
"     return a:op != 'c' ? a:w : "\<Esc>d" .. (a:c > 1 ? a:c : '') .. a:w .. 'i'
" endfunction
"
" both solutions have the issue that `cw` on the final character of the buffer doesn't work correctly (the cursor is one too left)

" make it obvious that something is wrong if this is hit
set timeoutlen=5000
" deal with this behaviour later
" set noautoindent

function! ClearUndoHistory()
    let old_undolevels = &undolevels
    set undolevels=-1
    exe "normal ax\<BS>\<Esc>"
    let &undolevels = old_undolevels
    unlet old_undolevels
endfunction

let mapleader = "\<Space>"

nnoremap <silent> <leader>u <cmd>UndotreeToggle<CR> <bar> UndoTreeFocus<CR>
