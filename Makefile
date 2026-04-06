GRAMMAR_DIR = ~/.local/share/zi/grammars
PLUGIN_DIR  = ~/.local/share/zi/plugins

RUST_VERSION       = v0.24.2
GO_VERSION         = v0.23.4
JSON_VERSION       = v0.24.8
C_VERSION          = v0.23.5
TYPESCRIPT_VERSION = v0.23.2
PYTHON_VERSION     = v0.25.0
BASH_VERSION       = v0.25.1
CSS_VERSION        = v0.25.0
HTML_VERSION       = v0.23.2
YAML_VERSION       = v0.7.2
TOML_VERSION       = v0.7.0
MARKDOWN_VERSION   = v0.5.3
FSHARP_VERSION     = main

GRAMMARS = rust go json c typescript python bash css html yaml toml markdown

.PHONY: all clean install $(addprefix install-,$(GRAMMARS)) config

all: $(addprefix install-,$(GRAMMARS)) config

config:
	cargo build -p plugin-config --target wasm32-wasip1 --release
	mkdir -p $(PLUGIN_DIR)
	cp target/wasm32-wasip1/release/plugin_config.wasm $(PLUGIN_DIR)/plugin_config.wasm

clean:
	rm -rf tree-sitter-*

define grammar_repo
$(if $(filter fsharp,$1),ionide,$(if $(filter yaml toml markdown,$1),tree-sitter-grammars,tree-sitter))
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
$(eval $(call install_grammar,python,PYTHON))
$(eval $(call install_grammar,bash,BASH))
$(eval $(call install_grammar,css,CSS))
$(eval $(call install_grammar,html,HTML))
$(eval $(call install_grammar,yaml,YAML))
$(eval $(call install_grammar,toml,TOML))

install-typescript: $(GRAMMAR_DIR)/typescript/language.wasm $(GRAMMAR_DIR)/typescript/highlights.scm

$(GRAMMAR_DIR)/typescript/language.wasm:
	mkdir -p $(GRAMMAR_DIR)/typescript
	curl --fail --silent --show-error -L \
		https://github.com/tree-sitter/tree-sitter-typescript/releases/download/$(TYPESCRIPT_VERSION)/tree-sitter-typescript.wasm \
		-o $(GRAMMAR_DIR)/typescript/language.wasm

$(GRAMMAR_DIR)/typescript/highlights.scm: tree-sitter-typescript
	mkdir -p $(GRAMMAR_DIR)/typescript
	cp tree-sitter-typescript/queries/highlights.scm $(GRAMMAR_DIR)/typescript/highlights.scm

install-markdown: $(GRAMMAR_DIR)/markdown/language.wasm $(GRAMMAR_DIR)/markdown/highlights.scm

$(GRAMMAR_DIR)/markdown/language.wasm:
	mkdir -p $(GRAMMAR_DIR)/markdown
	curl --fail --silent --show-error -L \
		https://github.com/tree-sitter-grammars/tree-sitter-markdown/releases/download/$(MARKDOWN_VERSION)/tree-sitter-markdown.wasm \
		-o $(GRAMMAR_DIR)/markdown/language.wasm

$(GRAMMAR_DIR)/markdown/highlights.scm: tree-sitter-markdown
	mkdir -p $(GRAMMAR_DIR)/markdown
	cp tree-sitter-markdown/tree-sitter-markdown/queries/highlights.scm $(GRAMMAR_DIR)/markdown/highlights.scm

define fetch_source
tree-sitter-$1:
	curl --silent --show-error -L https://github.com/$(call grammar_repo,$1)/tree-sitter-$1/archive/refs/tags/$($2_VERSION).tar.gz | tar xz
	mv tree-sitter-$1-$$(subst v,,$($2_VERSION)) $$@
endef

$(eval $(call fetch_source,rust,RUST))
$(eval $(call fetch_source,go,GO))
$(eval $(call fetch_source,json,JSON))
$(eval $(call fetch_source,c,C))
$(eval $(call fetch_source,typescript,TYPESCRIPT))
$(eval $(call fetch_source,python,PYTHON))
$(eval $(call fetch_source,bash,BASH))
$(eval $(call fetch_source,css,CSS))
$(eval $(call fetch_source,html,HTML))
$(eval $(call fetch_source,yaml,YAML))
$(eval $(call fetch_source,toml,TOML))
$(eval $(call fetch_source,markdown,MARKDOWN))
