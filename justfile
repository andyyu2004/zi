plugin-output := "target/zi-plugins"
grammar-output := "target/zi-grammars"

build-plugins:
    mkdir -p target
    nix build --out-link {{plugin-output}} .#plugins

install-plugins:
    @if [ ! -f "{{plugin-output}}/plugin_example.wasm" ]; then \
        echo "Run just build-plugins before installing plugins." >&2; exit 1; \
    fi
    plugin_dir="${ZI_PLUGIN_DIR:-runtime/plugins}" && \
    mkdir -p "$plugin_dir" && \
    install -m 0644 "{{plugin-output}}/plugin_example.wasm" "$plugin_dir/plugin_example.wasm" && \
    if [ "$plugin_dir" = runtime/plugins ]; then rm -f "$plugin_dir/plugin_test.wasm"; fi

build-grammars:
    mkdir -p target
    nix build --out-link {{grammar-output}} .#grammars

install-grammars:
    @set -eu; \
    if [ ! -d "{{grammar-output}}" ]; then \
        echo "Run just build-grammars before installing grammars." >&2; exit 1; \
    fi; \
    grammar_dir="${ZI_GRAMMAR_DIR:-}"; \
    if [ -z "$grammar_dir" ] || [ "$grammar_dir" = "${ZI_NIX_GRAMMAR_DIR:-}" ]; then \
        case "$(uname -s)" in \
            Darwin) grammar_dir="$HOME/Library/Application Support/zi/grammars" ;; \
            *) case "${XDG_DATA_HOME:-}" in \
                /*) grammar_dir="$XDG_DATA_HOME/zi/grammars" ;; \
                *) grammar_dir="$HOME/.local/share/zi/grammars" ;; \
            esac ;; \
        esac; \
    fi; \
    for source_dir in "{{grammar-output}}"/*/; do \
        language_dir="$grammar_dir/$(basename "$source_dir")"; \
        mkdir -p "$language_dir"; \
        install -m 0644 "$source_dir/language.wasm" "$language_dir/language.wasm"; \
        install -m 0644 "$source_dir/highlights.scm" "$language_dir/highlights.scm"; \
    done

test *FLAGS:
    cargo nextest run --workspace --exclude 'plugin-*' {{FLAGS}}
    cargo test --workspace --exclude 'plugin-*' --doc

# Test coverage, e.g. `just coverage -p zi-text --html --open`. Needs the dev shell.
coverage +FLAGS="--workspace --exclude plugin-*":
    cargo llvm-cov nextest {{FLAGS}}

run *FLAGS:
    @if [ -d runtime/plugins ]; then export ZI_PLUGIN_DIR="${ZI_PLUGIN_DIR:-$PWD/runtime/plugins}"; fi; \
    RUST_BACKTRACE=1 cargo run -p zi-term -- {{FLAGS}}

# Trying to build plugins with a standard cargo build results in linker issues hence the exclude
build *FLAGS: build-plugins
    @echo "building zi"
    @cargo build --workspace --exclude 'plugin-*' {{FLAGS}}

