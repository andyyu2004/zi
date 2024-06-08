use crate::config::Setting;

/// Global editor configuration shared between all views/buffers
pub struct Settings {
    pub file_picker_split_ratio: Setting<(u16, u16)>,
    pub jump_list_picker_split_ratio: Setting<(u16, u16)>,
    pub diagnostics_picker_split_ratio: Setting<(u16, u16)>,
    pub global_search_split_ratio: Setting<(u16, u16)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            file_picker_split_ratio: Setting::new((1, 2)),
            jump_list_picker_split_ratio: Setting::new((1, 1)),
            diagnostics_picker_split_ratio: Setting::new((2, 1)),
            global_search_split_ratio: Setting::new((2, 1)),
        }
    }
}
