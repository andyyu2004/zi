use std::fmt;
use std::str::FromStr;

use anyhow::ensure;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginName(String);

impl FromStr for PluginName {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        ensure!(
            !name.is_empty()
                && name.starts_with(|c: char| c.is_ascii_lowercase())
                && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "invalid plugin name `{name}`: use lowercase letters, digits and `-`, starting with a letter"
        );
        Ok(Self(name.to_owned()))
    }
}

impl PluginName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
