use crate::{Client, Editor, Result};

#[async_trait::async_trait]
pub trait Plugin {
    async fn init(&self, client: Client) -> Result<()>;
}

#[async_trait::async_trait]
pub trait PluginManager {}

impl Editor {}
