use std::sync::Arc;

use rmcp::ErrorData as McpError;
use serde_json::Value;

use crate::{
    external_control::ExternalControlRuntime,
    mcp::backend::{InProcessMcpBackend, McpBackend},
};

#[derive(Clone)]
pub struct McpControlService {
    backend: Arc<dyn McpBackend>,
}

impl McpControlService {
    pub fn new<B>(backend: B) -> Self
    where
        B: McpBackend + 'static,
    {
        Self {
            backend: Arc::new(backend),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn in_process(runtime: ExternalControlRuntime) -> Self {
        Self::new(InProcessMcpBackend::new(runtime))
    }

    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value, McpError> {
        self.backend.call_tool(name, args).await
    }
}
