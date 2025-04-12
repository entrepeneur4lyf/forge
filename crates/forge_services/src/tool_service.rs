use std::collections::HashMap;
use std::sync::Arc;

use forge_domain::{McpService, Tool, ToolCallContext, ToolCallFull, ToolDefinition, ToolName, ToolResult, ToolService, Workflow};
use tokio::time::{timeout, Duration};
use tracing::{debug, error};

use crate::tools::ToolRegistry;
use crate::Infrastructure;

// Timeout duration for tool calls
const TOOL_CALL_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct ForgeToolService {
    tools: Arc<HashMap<ToolName, Tool>>,
    mcp_service: Arc<dyn McpService>,
}

impl ForgeToolService {
    pub fn new<F: Infrastructure>(infra: Arc<F>, mcp: Arc<dyn McpService>) -> Self {
        let registry = ToolRegistry::new(infra.clone());
        let tools: HashMap<ToolName, Tool> = registry
            .tools()
            .into_iter()
            .map(|tool| (tool.definition.name.clone(), tool))
            .collect::<HashMap<_, _>>();

        Self { tools: Arc::new(tools), mcp_service: mcp }
    }
}

#[async_trait::async_trait]
impl ToolService for ForgeToolService {
    async fn call(&self, context: ToolCallContext, call: ToolCallFull, workflow: Option<Workflow>) -> ToolResult {
        let name = call.name.clone();
        if !self
            .tools
            .values()
            .any(|v| v.definition.name.eq(&call.name))
        {
            if let Some(workflow) = workflow {
                debug!(tool_name = ?call.name, arguments = ?call.arguments, "Executing tool call");
                return match self
                    .mcp_service
                    .call_tool(call.name.as_str(), call.arguments.clone(), &workflow)
                    .await
                {
                    Ok(result) => {
                        let ans = ToolResult::from(call);
                        match serde_json::to_string(&result.content) {
                            Ok(val) => ans.success(val),
                            Err(_) => {
                                error!(
                                error = "Failed to serialize tool result",
                                "Tool call failed"
                            );
                                ans.failure(anyhow::anyhow!("Failed to serialize tool result"))
                            }
                        }
                    }
                    Err(err) => {
                        error!(error = ?err, "Tool call failed");
                        ToolResult::from(call).failure(err)
                    }
                };
            }
        }
        let input = call.arguments.clone();
        debug!(tool_name = ?call.name, arguments = ?call.arguments, "Executing tool call");
        let mut available_tools = self
            .tools
            .keys()
            .map(|name| name.as_str())
            .collect::<Vec<_>>();

        available_tools.sort();
        let output = match self.tools.get(&name) {
            Some(tool) => {
                // Wrap tool call with timeout
                match timeout(TOOL_CALL_TIMEOUT, tool.executable.call(context, input)).await {
                    Ok(result) => result,
                    Err(_) => Err(anyhow::anyhow!(
                        "Tool '{}' timed out after {} minutes",
                        name.as_str(),
                        TOOL_CALL_TIMEOUT.as_secs() / 60
                    )),
                }
            }
            None => Err(anyhow::anyhow!(
                "No tool with name '{}' was found. Please try again with one of these tools {}",
                name.as_str(),
                available_tools.join(", ")
            )),
        };

        let result = match output {
            Ok(output) => ToolResult::from(call).success(output),
            Err(output) => {
                error!(error = ?output, "Tool call failed");
                ToolResult::from(call).failure(output)
            }
        };

        debug!(result = ?result, "Tool call result");
        result
    }

    async fn list(&self, workflow: Option<Workflow>) -> anyhow::Result<Vec<ToolDefinition>> {
        let mut tools: Vec<_> = self
            .tools
            .values()
            .map(|tool| tool.definition.clone())
            .collect();

        // Sorting is required to ensure system prompts are exactly the same
        tools.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
        if let Some(workflow) = workflow {
            let mcp_tools = self.mcp_service.list_tools(&workflow).await?;
            tools.extend(mcp_tools);
        }

        Ok(tools)
    }

    fn usage_prompt(&self) -> String {
        let mut tools: Vec<_> = self.tools.values().collect();
        tools.sort_by(|a, b| a.definition.name.as_str().cmp(b.definition.name.as_str()));

        tools
            .iter()
            .enumerate()
            .fold("".to_string(), |mut acc, (i, tool)| {
                acc.push('\n');
                acc.push_str((i + 1).to_string().as_str());
                acc.push_str(". ");
                acc.push_str(tool.definition.usage_prompt().to_string().as_str());
                acc
            })
    }
}

#[cfg(test)]
mod test {
    use anyhow::bail;
    use forge_domain::{Tool, ToolCallContext, ToolCallId, ToolDefinition};
    use rmcp::model::{CallToolResult, Content};
    use serde_json::{json, Value};
    use tokio::time;

    use super::*;

    struct MockMcpTool;

    #[async_trait::async_trait]
    impl McpService for MockMcpTool {
        async fn list_tools(&self, _: &Workflow) -> anyhow::Result<Vec<ToolDefinition>> {
            Ok(vec![])
        }

        async fn call_tool(
            &self,
            _: &str,
            _: Value,
            _: &Workflow,
        ) -> anyhow::Result<CallToolResult> {
            Ok(CallToolResult {
                content: vec![Content::text("No tool found")],
                is_error: Some(true),
            })
        }
    }

    // Mock tool that always succeeds
    struct SuccessTool;

    #[async_trait::async_trait]
    impl forge_domain::ExecutableTool for SuccessTool {
        type Input = Value;

        async fn call(
            &self,
            _context: ToolCallContext,
            input: Self::Input,
        ) -> anyhow::Result<String> {
            Ok(format!("Success with input: {}", input))
        }
    }

    // Mock tool that always fails
    struct FailureTool;

    #[async_trait::async_trait]
    impl forge_domain::ExecutableTool for FailureTool {
        type Input = Value;

        async fn call(
            &self,
            _context: ToolCallContext,
            _input: Self::Input,
        ) -> anyhow::Result<String> {
            bail!("Tool call failed with simulated failure".to_string())
        }
    }

    fn new_tool_service() -> impl ToolService {
        let success_tool = Tool {
            definition: ToolDefinition {
                name: ToolName::new("success_tool"),
                description: "A test tool that always succeeds".to_string(),
                input_schema: schemars::schema_for!(serde_json::Value),
                output_schema: Some(schemars::schema_for!(String)),
            },
            executable: Box::new(SuccessTool),
        };

        let failure_tool = Tool {
            definition: ToolDefinition {
                name: ToolName::new("failure_tool"),
                description: "A test tool that always fails".to_string(),
                input_schema: schemars::schema_for!(serde_json::Value),
                output_schema: Some(schemars::schema_for!(String)),
            },
            executable: Box::new(FailureTool),
        };
        let tools = vec![success_tool, failure_tool]
            .into_iter()
            .map(|tool| (tool.definition.name.clone(), tool))
            .collect::<HashMap<_, _>>();

        ForgeToolService { tools: Arc::new(tools), mcp_service: Arc::new(MockMcpTool) }
    }

    #[tokio::test]
    async fn test_successful_tool_call() {
        let service = new_tool_service();
        let call = ToolCallFull {
            name: ToolName::new("success_tool"),
            arguments: json!("test input"),
            call_id: Some(ToolCallId::new("test")),
        };

        let result = service.call(ToolCallContext::default(), call, None).await;
        insta::assert_snapshot!(result);
    }

    #[tokio::test]
    async fn test_failed_tool_call() {
        let service = new_tool_service();
        let call = ToolCallFull {
            name: ToolName::new("failure_tool"),
            arguments: json!("test input"),
            call_id: Some(ToolCallId::new("test")),
        };

        let result = service.call(ToolCallContext::default(), call, None).await;
        insta::assert_snapshot!(result);
    }

    #[tokio::test]
    async fn test_tool_not_found() {
        let service = new_tool_service();
        let call = ToolCallFull {
            name: ToolName::new("nonexistent_tool"),
            arguments: json!("test input"),
            call_id: Some(ToolCallId::new("test")),
        };

        let result = service.call(ToolCallContext::default(), call, None).await;
        insta::assert_snapshot!(result);
    }

    // Mock tool that simulates a long-running task
    struct SlowTool;

    #[async_trait::async_trait]
    impl forge_domain::ExecutableTool for SlowTool {
        type Input = Value;

        async fn call(
            &self,
            _context: ToolCallContext,
            _input: Self::Input,
        ) -> anyhow::Result<String> {
            // Simulate a long-running task that exceeds the timeout
            tokio::time::sleep(Duration::from_secs(400)).await;
            Ok("Slow tool completed".to_string())
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_tool_timeout() {
        test::time::pause();

        let slow_tool = Tool {
            definition: ToolDefinition {
                name: ToolName::new("slow_tool"),
                description: "A test tool that takes too long".to_string(),
                input_schema: schemars::schema_for!(serde_json::Value),
                output_schema: Some(schemars::schema_for!(String)),
            },
            executable: Box::new(SlowTool),
        };

        let service = ForgeToolService {
            tools: Arc::new(
                vec![slow_tool]
                    .into_iter()
                    .map(|tool| (tool.definition.name.clone(), tool))
                    .collect(),
            ),
            mcp_service: Arc::new(MockMcpTool),
        };
        let call = ToolCallFull {
            name: ToolName::new("slow_tool"),
            arguments: json!("test input"),
            call_id: Some(ToolCallId::new("test")),
        };

        // Advance time to trigger timeout
        test::time::advance(Duration::from_secs(305)).await;

        let result = service.call(ToolCallContext::default(), call, None).await;

        // Assert that the result contains a timeout error message
        let content_str = &result.content;
        assert!(
            content_str.contains("timed out"),
            "Expected timeout error message"
        );
        assert!(result.is_error, "Expected error result for timeout");
    }
}
