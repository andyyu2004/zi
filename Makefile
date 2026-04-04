GRAMMAR_DIR = ~/.local/share/zi/grammars

RUST_VERSION     = v0.24.2
GO_VERSION       = v0.23.4
JSON_VERSION     = v0.24.8
C_VERSION        = v0.23.5
TYPESCRIPT_VERSION = v0.23.2
FSHARP_VERSION   = main

GRAMMARS = rust go json c typescript

.PHONY: all clean install $(addprefix install-,$(GRAMMARS))

all: $(addprefix install-,$(GRAMMARS))

clean:
	rm -rf tree-sitter-*

define grammar_repo
$(if $(filter fsharp,$1),ionide,tree-sitter)
endef

define grammar_url
https://github.com/$(call grammar_repo,$1)/tree-sitter-$1
endef

define install_grammar
install-$1: $(GRAMMAR_DIR)/$1/language.wasm $(GRAMMAR_DIR)/$1/highlights.scm

$(GRAMMAR_DIR)/$1/language.wasm:
	mkdir -p $(GRAMMAR_DIR)/$1
	curl --fail --silent --show-error -L \
		$(call grammar_url,$1)/releases/download/$($2_VERSION)/tree-sitter-$1.wasm \
		-o $(GRAMMAR_DIR)/$1/language.wasm

$(GRAMMAR_DIR)/$1/highlights.scm: tree-sitter-$1
	mkdir -p $(GRAMMAR_DIR)/$1
	cp tree-sitter-$1/queries/highlights.scm $(GRAMMAR_DIR)/$1/highlights.scm
endef

$(eval $(call install_grammar,rust,RUST))
$(eval $(call install_grammar,go,GO))
$(eval $(call install_grammar,json,JSON))
$(eval $(call install_grammar,c,C))

install-typescript: $(GRAMMAR_DIR)/typescript/language.wasm $(GRAMMAR_DIR)/typescript/highlights.scm

$(GRAMMAR_DIR)/typescript/language.wasm:
	mkdir -p $(GRAMMAR_DIR)/typescript
	curl --fail --silent --show-error -L \
		https://github.com/tree-sitter/tree-sitter-typescript/releases/download/$(TYPESCRIPT_VERSION)/tree-sitter-typescript.wasm \
		-o $(GRAMMAR_DIR)/typescript/language.wasm

$(GRAMMAR_DIR)/typescript/highlights.scm: tree-sitter-typescript
	mkdir -p $(GRAMMAR_DIR)/typescript
	cp tree-sitter-typescript/queries/highlights.scm $(GRAMMAR_DIR)/typescript/highlights.scm

tree-sitter-rust:
	curl --silent --show-error -L https://github.com/tree-sitter/tree-sitter-rust/archive/refs/tags/$(RUST_VERSION).tar.gz | tar xz
	mv tree-sitter-rust-$(subst v,,$(RUST_VERSION)) $@

tree-sitter-go:
	curl --silent --show-error -L https://github.com/tree-sitter/tree-sitter-go/archive/refs/tags/$(GO_VERSION).tar.gz | tar xz
	mv tree-sitter-go-$(subst v,,$(GO_VERSION)) $@

tree-sitter-json:
	curl --silent --show-error -L https://github.com/tree-sitter/tree-sitter-json/archive/refs/tags/$(JSON_VERSION).tar.gz | tar xz
	mv tree-sitter-json-$(subst v,,$(JSON_VERSION)) $@

tree-sitter-c:
	curl --silent --show-error -L https://github.com/tree-sitter/tree-sitter-c/archive/refs/tags/$(C_VERSION).tar.gz | tar xz
	mv tree-sitter-c-$(subst v,,$(C_VERSION)) $@

tree-sitter-typescript:
	curl --silent --show-error -L https://github.com/tree-sitter/tree-sitter-typescript/archive/refs/tags/$(TYPESCRIPT_VERSION).tar.gz | tar xz
	mv tree-sitter-typescript-$(subst v,,$(TYPESCRIPT_VERSION)) $@
