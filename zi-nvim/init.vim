inoremap fd <ESC>

" https://unix.stackexchange.com/questions/445989/vim-cw-dw-whitespace-inconsistency
" remove this inconsistency as it can be accomplished with `ce` anyway
map cw dwi

" make it obvious that something is wrong if this is hit
set timeoutlen=5000
