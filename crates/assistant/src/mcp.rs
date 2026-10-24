//! The chat uses the same tools as the player's own agent. exile-companion-mcp runs as a child
//! process and this side is its MCP client.

use std::path::Path;
use std::process::Stdio;

use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt};
use serde_json::Value;

use crate::turn::{ToolDef, ToolOutput, Tools};
use crate::ChatError;

pub struct Server {
    client: RunningService<RoleClient, ()>,
    pub instructions: String,
    /// In the server's order, so the prompt cache holds.
    pub tools: Vec<ToolDef>,
}

impl Server {
    pub async fn start(exe: &Path, args: &[String]) -> Result<Server, ChatError> {
        let mut cmd = tokio::process::Command::new(exe);
        cmd.args(args);
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let failed =
            |e: String| ChatError::Tools(format!("exile-companion-mcp didn't start ({e})"));
        let (transport, _) = TokioChildProcess::builder(cmd)
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| failed(e.to_string()))?;
        let client = ().serve(transport).await.map_err(|e| failed(e.to_string()))?;
        let instructions = client
            .peer_info()
            .and_then(|i| i.instructions.clone())
            .unwrap_or_default();
        let listed = client
            .list_all_tools()
            .await
            .map_err(|e| failed(e.to_string()))?;
        let tools = listed
            .iter()
            .map(|t| {
                let mut schema = (*t.input_schema).clone();
                // Some providers behind OpenRouter reject a schema that names its own dialect.
                schema.remove("$schema");
                ToolDef {
                    name: t.name.to_string(),
                    description: t.description.as_deref().unwrap_or("").to_string(),
                    schema: Value::Object(schema),
                }
            })
            .collect();
        Ok(Server {
            client,
            instructions,
            tools,
        })
    }

    pub fn is_running(&self) -> bool {
        !self.client.is_closed()
    }
}

fn output(result: CallToolResult) -> ToolOutput {
    let text = result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    ToolOutput {
        text,
        is_error: result.is_error.unwrap_or(false),
    }
}

impl Tools for Server {
    async fn call(&self, name: &str, input: Value) -> ToolOutput {
        let mut params = CallToolRequestParams::new(name.to_string());
        if let Value::Object(args) = input {
            params = params.with_arguments(args);
        }
        match self.client.call_tool(params).await {
            Ok(r) => output(r),
            Err(e) => ToolOutput {
                text: format!("The tool failed: {e}"),
                is_error: true,
            },
        }
    }
}
