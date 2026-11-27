use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use wasmtime::Engine;
use wasmtime::component::Linker;

use super::state::HostState;
use crate::wit::Plugin;

pub type Store = wasmtime::Store<HostState>;

const EPOCH_TICK: Duration = Duration::from_millis(1);

pub(super) fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true);
        config.epoch_interruption(true);
        Engine::new(&config).expect("configuration should be valid")
    })
}

pub(super) fn new_store(engine: &Engine, state: HostState) -> Store {
    let mut store = Store::new(engine, state);
    store.epoch_deadline_async_yield_and_update(1);
    store
}

pub(super) struct EpochTicker {
    stop: Arc<AtomicBool>,
}

impl EpochTicker {
    pub(super) fn start(engine: &'static Engine) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let ticking = stop.clone();
        std::thread::Builder::new()
            .name("zi-wasm-epoch".into())
            .spawn(move || {
                while !ticking.load(Ordering::Relaxed) {
                    std::thread::sleep(EPOCH_TICK);
                    engine.increment_epoch();
                }
            })
            .expect("couldn't spawn the epoch ticker thread");
        Self { stop }
    }
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub(super) fn linker(engine: &Engine) -> wasmtime::Result<Linker<HostState>> {
    let mut linker = Linker::new(engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
    Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)?;
    Ok(linker)
}
