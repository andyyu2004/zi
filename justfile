test *FLAGS: build
    mold -run cargo test {{FLAGS}}

build *FLAGS: build-plugins
    @echo "building zi"
    # Trying to build plugins with a standard cargo build results in linker issues
    @cargo build --workspace --exclude 'plugin-*' {{FLAGS}}

wasm-target := "wasm32-unknown-unknown"

build-plugins:
    @echo "building plugins"
    @for dir in ./plugins/*/; do cargo -Zunstable-options -C $dir component build --target {{wasm-target}} --release; done
    mkdir -p runtime/plugins
    cp target/{{wasm-target}}/release/*.wasm runtime/plugins

