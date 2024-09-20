.PHONY: tree-sitter-install
.PHONY: tree-sitter-install-%
.PHONY: all test clean

all: tree-sitter-install-rust \
	tree-sitter-install-go \
	tree-sitter-install-json \
	tree-sitter-install-c \
	tree-sitter-install-typescript

# clean:
# 	rm -rf tree-sitter-rust tree-sitter-go tree-sitter-json tree-sitter-c

tree-sitter-install-%: tree-sitter-%.wasm
	mkdir -p ~/.local/share/zi/grammars/$*
	cp tree-sitter-$*/tree-sitter-$*.wasm ~/.local/share/zi/grammars/$*/language.wasm
	cp tree-sitter-$*/queries/highlights.scm ~/.local/share/zi/grammars/$*/highlights.scm

# This one doesn't always work for some reason
# tree-sitter build --wasm tree-sitter-$*
tree-sitter-%.wasm: tree-sitter-%
	cd tree-sitter-$* && tree-sitter build --wasm
	cp tree-sitter-$*/tree-sitter-$*.wasm tree-sitter-$*.wasm

tree-sitter-typescript:
	curl --silent --show-error -L https://github.com/tree-sitter/tree-sitter-typescript/archive/refs/tags/v0.20.4.tar.gz | tar xz
	ln -s tree-sitter-typescript-0.20.4/typescript/ tree-sitter-typescript
	ln -s $(CURDIR)/tree-sitter-typescript-0.20.4/queries tree-sitter-typescript/queries

tree-sitter-c:
	curl --silent --show-error -L https://github.com/tree-sitter/tree-sitter-c/archive/refs/tags/v0.20.7.tar.gz | tar xz
	mv tree-sitter-c-0.20.7 tree-sitter-c

tree-sitter-rust:
	curl --silent --show-error -L https://github.com/tree-sitter/tree-sitter-rust/archive/refs/tags/v0.21.2.tar.gz | tar xz
	mv tree-sitter-rust-0.21.2 tree-sitter-rust

tree-sitter-go:
	curl --silent --show-error -L https://github.com/tree-sitter/tree-sitter-go/archive/refs/tags/v0.20.0.tar.gz | tar xz
	mv tree-sitter-go-0.20.0 tree-sitter-go

tree-sitter-json:
	curl --silent --show-error -L https://github.com/tree-sitter/tree-sitter-json/archive/refs/tags/v0.21.0.tar.gz | tar xz
	mv tree-sitter-json-0.21.0 tree-sitter-json

