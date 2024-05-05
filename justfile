test *FLAGS: build
    mold -run cargo test {{FLAGS}}

run *FLAGS:
    @RUST_BACKTRACE=1 cargo run -p zi-term {{FLAGS}}

# Trying to build plugins with a standard cargo build results in linker issues hence the exclude
build *FLAGS: build-plugins
    @echo "building zi"
    @cargo build --workspace --exclude 'plugin-*' {{FLAGS}}

wasm-target := "wasm32-unknown-unknown"

build-plugins:
    @echo "building plugins"
    @for dir in ./plugins/*/; do cargo -Zunstable-options -C $dir component build --target {{wasm-target}} --release; done
    mkdir -p runtime/plugins
    cp target/{{wasm-target}}/release/*.wasm runtime/plugins

