use crate::config::Setting;

/// Global editor configuration shared between all views/buffers
pub struct Settings {
    pub picker_split_proportion: Setting<(u16, u16)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { picker_split_proportion: Setting::new((1, 2)) }
    }
}
