test *FLAGS: build
    cargo test {{FLAGS}}

build *FLAGS: build-plugins
    @echo "building zi"
    @cargo build {{FLAGS}}

target := "wasm32-unknown-unknown"

build-plugins:
    @echo "building plugins"
    @for dir in ./plugins/*/; do cargo -Zunstable-options -C $dir component build --target {{target}} --release; done
    cp target/{{target}}/release/*.wasm runtime/plugins

