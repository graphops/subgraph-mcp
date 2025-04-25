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

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetTopSubgraphsRequest {
    pub contract_address: String,
    pub chain: String,
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

    async fn get_top_subgraphs_internal(
        &self,
        contract_address: &str,
        chain: &str,
    ) -> Result<serde_json::Value, SubgraphError> {
        let api_key = self.get_api_key()?;
        // Use the general GraphOps gateway endpoint for indexer queries
        let url = format!(
            "https://graph-gateway.graphops.xyz/api/{}/deployments/id/{}",
            api_key, "QmdKXcBUHR3UyURqVRQHu1oV6VUkBrhi2vNvMx3bNDnUCc"
        );

        let query = r#"
        query TopSubgraphsForContract($network: String!, $contractAddress: String!) {
          subgraphDeployments(
            where: {manifest_: {network: $network, manifest_contains: $contractAddress}}
            orderBy: queryFeesAmount
            orderDirection: desc
            first: 3
          ) {
            id
            manifest {
              network
            }
            queryFeesAmount
          }
        }
        "#;

        let variables = serde_json::json!({
            "network": chain,
            "contractAddress": contract_address
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
            .json::<GraphQLResponse>() // Expecting GraphQLResponse structure
            .await?;

        if let Some(errors) = response.errors {
            if !errors.is_empty() {
                return Err(SubgraphError::GraphQlError(errors[0].message.clone()));
            }
        }

        let data = response.data.ok_or_else(|| {
            SubgraphError::GraphQlError("No data returned from the GraphQL API".to_string())
        })?;

        Ok(data)
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

    #[tool(
        description = "Get the top 3 subgraph deployments for a given contract address and chain, ordered by query fees. For chain, use 'mainnet' for Ethereum mainnet, NEVER use 'ethereum'."
    )]
    async fn get_top_subgraphs(
        &self,
        #[tool(aggr)]
        #[schemars(description = "Request containing the contract address and chain name")]
        GetTopSubgraphsRequest {
            contract_address,
            chain,
        }: GetTopSubgraphsRequest,
    ) -> Result<CallToolResult, McpError> {
        match self
            .get_top_subgraphs_internal(&contract_address, &chain)
            .await
        {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                "{:#}", // Pretty print the JSON result
                result
            ))])),
            Err(e) => Err(McpError::internal_error(
                "Failed to get top subgraphs",
                Some(json!({ "error": e.to_string() })),
            )),
        }
    }
}

const_string!(GetSchema = "get_schema");
const_string!(ExecuteQuery = "execute_query");
const_string!(GetTopSubgraphs = "get_top_subgraphs");

// Add a constant for the server instructions
const SERVER_INSTRUCTIONS: &str = "This server interacts with subgraphs on The Graph protocol. \
Workflow: \
1. **Determine the user's goal:** \
    a. Is the user asking for information *about* a specific address (e.g., find ENS name for 0x...)? \
    b. Is the user asking for subgraphs that index a specific *contract address* they provided? \
2. **Identify the chain** (IMPORTANT: use 'mainnet' for Ethereum mainnet, NOT 'ethereum'; use 'arbitrum-one' for Arbitrum, etc.). \
3. **If Goal is (a) - Address Lookup (e.g., ENS):** \
    a. Identify the relevant **protocol** (e.g., ENS). \
    b. Find the **protocol's main contract address** on the identified chain. For The Graph protocol contracts, refer to https://thegraph.com/docs/en/contracts/ and default to using Arbitrum addresses as this is the principal deployment. \
    c. Use 'get_top_subgraphs' with the **protocol's contract address** and chain to find relevant deployment IDs. \
    d. Use the obtained deployment ID(s) with 'execute_query'. The query should use the **original user-provided address** (from 1a) in its variables or filters to find the specific information (e.g., the ENS name). \
4. **If Goal is (b) - Find Subgraphs for a Contract:** \
    a. Use the **contract address provided by the user** (from 1b). \
    b. Use 'get_top_subgraphs' with this **user-provided contract address** and the identified chain to find relevant deployment IDs. \
    c. Use the obtained deployment ID(s) with 'get_schema' or 'execute_query' as needed. \
5. **Write clean GraphQL queries:** \
    a. Omit the 'variables' parameter when not needed. \
    b. Create simple GraphQL structures without unnecessary complexity. \
    c. Include only the essential fields in your query. \
**Important:** \
*   For `get_top_subgraphs`, the `contractAddress` parameter *must* be the address of the contract you want to find indexed subgraphs for. This is different from an address you might be looking up information *about* within a subgraph (like an EOA for an ENS lookup). \
*   Chain parameter must be 'mainnet' for Ethereum mainnet, not 'ethereum'. \
*   The Graph protocol has migrated to Arbitrum One, which now hosts the principal deployment. When working with The Graph protocol directly, refer to https://thegraph.com/docs/en/contracts/ and use Arbitrum contract addresses by default unless specifically requested otherwise. \
*   When asked to provide ENS names for any address, always rely on the ENS contracts and subgraphs. \
*   Never use hardcoded deployment IDs from memory. ALWAYS use `get_top_subgraphs` first to discover current valid deployment IDs. \
*   If a query fails, check the chain parameter and try again with the correct chain name before attempting other approaches. \
*   Clean query structure: Keep GraphQL queries simple with only necessary fields, omit the variables parameter when not needed, and use a clear, minimal query structure. \
*   Protocol version awareness: When querying blockchain protocol data (like Uniswap, Aave, Compound, etc.), prioritize the latest major version unless specified otherwise. If unsure which version to query, explain the different versions and their key differences before proceeding. \
*   Contract address verification: When accessing blockchain protocol data through subgraphs, verify that the contract address corresponds to the intended protocol by checking the schema before proceeding with further queries. If the schema indicates a different protocol than expected, notify the user and suggest the correct address. \
*   Clarification thresholds: When a query about blockchain data lacks specificity (protocol version, timeframe, metrics of interest), request clarification if the potential interpretations would lead to significantly different results or if retrieving all possible interpretations would be inefficient. \
*   Context inference: For blockchain data queries, infer context from recent protocol developments. For instance, if a user asks about 'Uniswap pairs' without specifying a version, consider that V3 has largely superseded V2 in terms of volume and liquidity, but include both if the complete picture is valuable.";

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
            instructions: Some(SERVER_INSTRUCTIONS.to_string()),
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

    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match uri.as_str() {
            "graphql://subgraph" => {
                let description = SERVER_INSTRUCTIONS;
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
                    Some("Get subgraph schema."),
                    Some(vec![PromptArgument {
                        name: "deploymentId".to_string(),
                        description: Some("The ID of the subgraph deployment".to_string()),
                        required: Some(true),
                    }]),
                ),
                Prompt::new(
                    "execute_query",
                    Some("Execute GraphQL query."),
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
                Prompt::new(
                    "get_top_subgraphs",
                    Some("Get top subgraphs for a contract."),
                    Some(vec![
                        PromptArgument {
                            name: "contractAddress".to_string(),
                            description: Some("The contract address".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "chain".to_string(),
                            description: Some(
                                "The chain name (e.g., 'mainnet' for Ethereum, 'arbitrum-one')."
                                    .to_string(),
                            ),
                            required: Some(true),
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
                        "Fetch the GraphQL schema for a subgraph deployment.".to_string(),
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
                        "Execute a GraphQL query against a subgraph deployment.".to_string(),
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
            "get_top_subgraphs" => {
                let contract_address = arguments
                    .as_ref()
                    .and_then(|args| args.get("contractAddress"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{contractAddress}")
                    .to_string();

                let chain = arguments
                    .as_ref()
                    .and_then(|args| args.get("chain"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{chain}")
                    .to_string();

                Ok(GetPromptResult {
                    description: Some(
                        "Fetch the top 3 subgraph deployments for a contract on a specific chain. Use 'mainnet' for Ethereum mainnet, NOT 'ethereum'."
                            .to_string(),
                    ),
                    messages: vec![PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(format!(
                            "Get the top subgraphs for contract {} on chain {}",
                            contract_address, chain
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
