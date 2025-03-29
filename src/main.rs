use anyhow::Result;
use log::{error, info};
use reqwest::Client;
use rmcp::ServiceExt;
use std::env;
use thiserror::Error;
use tokio::io;

use rmcp::{
    const_string, model::*, schemars, service::RequestContext, tool, Error as McpError, RoleServer,
    ServerHandler,
};
use serde_json::json;

#[derive(Debug, Error)]
enum SubgraphError {
    #[error("API key not set")]
    ApiKeyNotSet,
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("GraphQL error: {0}")]
    GraphQlError(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

// GraphQL request structures
#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetSchemaRequest {
    pub deployment_id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecuteQueryRequest {
    pub deployment_id: String,
    pub query: String,
    pub variables: Option<serde_json::Value>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct GraphQLResponse {
    data: Option<serde_json::Value>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct GraphQLError {
    message: String,
}

// Constants for protocol versions

#[derive(Clone)]
pub struct SubgraphServer {
    http_client: Client,
}
impl Default for SubgraphServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool(tool_box)]
impl SubgraphServer {
    pub fn new() -> Self {
        SubgraphServer {
            http_client: Client::new(),
        }
    }

    fn get_api_key(&self) -> Result<String, SubgraphError> {
        env::var("GRAPHOPS_API_KEY").map_err(|_| SubgraphError::ApiKeyNotSet)
    }

    async fn get_schema_internal(&self, deployment_id: &str) -> Result<String, SubgraphError> {
        let api_key = self.get_api_key()?;
        let url = format!(
            "https://graph-gateway.graphops.xyz/api/{}/deployments/id/{}",
            api_key, "QmdKXcBUHR3UyURqVRQHu1oV6VUkBrhi2vNvMx3bNDnUCc"
        );

        let query = r#"
        query SubgraphDeploymentSchema($id: String!) {
            subgraphDeployment(id: $id) {
                manifest {
                    schema {
                        schema
                    }
                }
            }
        }
        "#;

        let variables = serde_json::json!({
            "id": deployment_id
        });

        let request_body = serde_json::json!({
            "query": query,
            "variables": variables
        });

        let response = self
            .http_client
            .post(&url)
            .json(&request_body)
            .send()
            .await?
            .json::<GraphQLResponse>()
            .await?;

        if let Some(errors) = response.errors {
            if !errors.is_empty() {
                return Err(SubgraphError::GraphQlError(errors[0].message.clone()));
            }
        }

        let data = response.data.ok_or_else(|| {
            SubgraphError::GraphQlError("No data returned from the GraphQL API".to_string())
        })?;

        let schema = data
            .get("subgraphDeployment")
            .and_then(|dep| dep.get("manifest"))
            .and_then(|manifest| manifest.get("schema"))
            .and_then(|schema| schema.get("schema"))
            .and_then(|schema| schema.as_str())
            .ok_or_else(|| {
                SubgraphError::GraphQlError("Schema not found in the response".to_string())
            })?;

        Ok(schema.to_string())
    }

    async fn execute_query_internal(
        &self,
        deployment_id: &str,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, SubgraphError> {
        let api_key = self.get_api_key()?;
        let url = format!(
            "https://graph-gateway.graphops.xyz/api/{}/deployments/id/{}",
            api_key, deployment_id
        );

        let mut request_body = serde_json::json!({
            "query": query,
        });

        if let Some(vars) = variables {
            request_body["variables"] = vars;
        }

        let response = self
            .http_client
            .post(&url)
            .json(&request_body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        Ok(response)
    }

    fn _create_resource_text(&self, uri: &str, name: &str) -> Resource {
        RawResource::new(uri, name.to_string()).no_annotation()
    }

    #[tool(description = "Get schema for a subgraph deployment")]
    async fn get_schema(
        &self,
        #[tool(param)]
        #[schemars(description = "The ID of the subgraph deployment")]
        deployment_id: String,
    ) -> Result<CallToolResult, McpError> {
        match self.get_schema_internal(&deployment_id).await {
            Ok(schema) => Ok(CallToolResult::success(vec![Content::text(schema)])),
            Err(e) => Err(McpError::internal_error(
                "Failed to get schema",
                Some(json!({ "error": e.to_string() })),
            )),
        }
    }

    #[tool(description = "Execute a GraphQL query against a deployment")]
    async fn execute_query(
        &self,
        #[tool(aggr)] ExecuteQueryRequest {
            deployment_id,
            query,
            variables,
        }: ExecuteQueryRequest,
    ) -> Result<CallToolResult, McpError> {
        match self
            .execute_query_internal(&deployment_id, &query, variables)
            .await
        {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                "{:#}",
                result
            ))])),
            Err(e) => Err(McpError::internal_error(
                "Failed to execute query",
                Some(json!({ "error": e.to_string() })),
            )),
        }
    }
}

const_string!(GetSchema = "get_schema");
const_string!(ExecuteQuery = "execute_query");

#[tool(tool_box)]
impl ServerHandler for SubgraphServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("This server provides tools for interacting with subgraphs on The Graph protocol. You can get the schema for a subgraph deployment and execute GraphQL queries.".to_string()),
        }
    }

    async fn list_resources(
        &self,
        _request: PaginatedRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![self._create_resource_text("graphql://subgraph", "The Graph")],
            next_cursor: None,
        })
    }

    // dummy resource
    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match uri.as_str() {
            "graphql://subgraph" => {
                let description = "Query subgraphs on The Graph protocol";
                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::text(description, uri)],
                })
            }
            _ => Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({
                    "uri": uri
                })),
            )),
        }
    }

    async fn list_prompts(
        &self,
        _request: PaginatedRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            next_cursor: None,
            prompts: vec![
                Prompt::new(
                    "get_schema",
                    Some("Get subgraph schema"),
                    Some(vec![PromptArgument {
                        name: "deploymentId".to_string(),
                        description: Some("The ID of the subgraph deployment".to_string()),
                        required: Some(true),
                    }]),
                ),
                Prompt::new(
                    "execute_query",
                    Some("Execute GraphQL query"),
                    Some(vec![
                        PromptArgument {
                            name: "deploymentId".to_string(),
                            description: Some("The ID of the subgraph deployment".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "query".to_string(),
                            description: Some("The GraphQL query to execute".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "variables".to_string(),
                            description: Some("Variables for the GraphQL query".to_string()),
                            required: Some(false),
                        },
                    ]),
                ),
            ],
        })
    }

    async fn get_prompt(
        &self,
        GetPromptRequestParam { name, arguments }: GetPromptRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        match name.as_str() {
            "get_schema" => {
                let deployment_id = arguments
                    .as_ref()
                    .and_then(|args| args.get("deploymentId"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{deploymentId}")
                    .to_string();

                Ok(GetPromptResult {
                    description: Some(
                        "Fetch the GraphQL schema for a subgraph deployment".to_string(),
                    ),
                    messages: vec![PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(format!(
                            "Get the schema for subgraph deployment {}",
                            deployment_id
                        )),
                    }],
                })
            }
            "execute_query" => {
                let deployment_id = arguments
                    .as_ref()
                    .and_then(|args| args.get("deploymentId"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{deploymentId}")
                    .to_string();

                let query = arguments
                    .as_ref()
                    .and_then(|args| args.get("query"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{query}")
                    .to_string();

                Ok(GetPromptResult {
                    description: Some(
                        "Execute a GraphQL query against a subgraph deployment".to_string(),
                    ),
                    messages: vec![PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(format!(
                            "Run this GraphQL query against deployment {}: {}",
                            deployment_id, query
                        )),
                    }],
                })
            }
            _ => Err(McpError::invalid_params("prompt not found", None)),
        }
    }

    async fn list_resource_templates(
        &self,
        _request: PaginatedRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            next_cursor: None,
            resource_templates: Vec::new(),
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("Starting Subgraph MCP Server");

    let server = SubgraphServer::new();

    let transport = (io::stdin(), io::stdout());

    let running = server.serve(transport).await?;
    running.waiting().await?;

    info!("Server shutdown complete");
    Ok(())
}
