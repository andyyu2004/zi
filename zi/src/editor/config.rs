use crate::config::Setting;

/// Global editor configuration shared between all views/buffers
pub struct Config {
    pub picker_split_proportion: Setting<(u16, u16)>,
}

impl Default for Config {
    fn default() -> Self {
        Self { picker_split_proportion: Setting::new((1, 2)) }
    }
}
