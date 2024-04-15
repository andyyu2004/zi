inoremap fd <ESC>

" https://unix.stackexchange.com/questions/445989/vim-cw-dw-whitespace-inconsistency
" remove this inconsistency as it can be accomplished with `ce` anyway
map cw dwi

" make it obvious that something is wrong if this is hit
set timeoutlen=5000

function! ClearUndoHistory()
    let old_undolevels = &undolevels
    set undolevels=-1
    exe "normal ax\<BS>\<Esc>"
    let &undolevels = old_undolevels
    unlet old_undolevels
endfunction

let mapleader = "\<Space>"

nnoremap <silent> <leader>u <cmd>UndotreeToggle<CR> <bar> UndoTreeFocus<CR>
