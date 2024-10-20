use std::sync::Arc;

use crate::{Client, Result};

#[async_trait::async_trait]
pub trait Plugin {
    async fn init(&self, client: Client) -> Result<()>;
}

#[async_trait::async_trait]
pub trait PluginManager {
    fn name(&self) -> &'static str;

    async fn start(self: Arc<Self>, client: Client) -> Result<()>;
}
