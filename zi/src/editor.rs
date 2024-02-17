use crate::event::KeyEvent;
use crate::keymap::Keymap;
use crate::plugin::{self, Config, Context, Engine, Plugin, Result, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
}

pub struct Editor {
    mode: Mode,
    keymap: Keymap,
    plugins: Box<[Plugin]>,
}

impl Editor {
    pub async fn load() -> Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true).async_support(true);
        let engine = Engine::new(&config)?;
        let mut store = Store::new(&engine, Context {});
        let plugins = plugin::load(engine, &mut store, &["../runtime/plugins/p1.wasm"]).await?;
        Ok(Self { plugins, keymap: Keymap::new(), mode: Mode::default() })
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        self.keymap.on_key(self.mode, key);
    }
}
