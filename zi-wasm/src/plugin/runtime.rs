use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::Thread;
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

static TICKING: AtomicUsize = AtomicUsize::new(0);

/// Advances the engine's epoch while any guard is alive, so plugin calls yield and can time out.
pub(super) struct EpochTicking(());

impl EpochTicking {
    pub(super) fn start() -> Self {
        static TICKER: OnceLock<Thread> = OnceLock::new();
        let ticker = TICKER.get_or_init(spawn_ticker);
        if TICKING.fetch_add(1, Ordering::SeqCst) == 0 {
            ticker.unpark();
        }
        Self(())
    }
}

impl Drop for EpochTicking {
    fn drop(&mut self) {
        TICKING.fetch_sub(1, Ordering::SeqCst);
    }
}

fn spawn_ticker() -> Thread {
    let engine = engine();
    std::thread::Builder::new()
        .name("zi-wasm-epoch".into())
        .spawn(move || {
            loop {
                if TICKING.load(Ordering::SeqCst) == 0 {
                    std::thread::park();
                    continue;
                }
                std::thread::sleep(EPOCH_TICK);
                engine.increment_epoch();
            }
        })
        .expect("couldn't spawn the epoch ticker thread")
        .thread()
        .clone()
}

pub(super) fn linker(engine: &Engine) -> wasmtime::Result<Linker<HostState>> {
    let mut linker = Linker::new(engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
    Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)?;
    Ok(linker)
}
