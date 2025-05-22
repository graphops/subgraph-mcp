// SPDX-License-Identifier: Apache-2.0
use crate::{error::SubgraphError, http_utils::HttpRequestHeaders, types::*, constants::SERVER_INSTRUCTIONS};
use rmcp::{model::*, service::RequestContext, RoleServer, ServerHandler, tool, Error as McpError};
use serde_json::json;
use reqwest::Client;
#[derive(Clone)]
pub struct SubgraphServer {
    pub(crate) http_client: Client,
}

impl Default for SubgraphServer {
    fn default() -> Self {
        Self::new()
    }
}

impl SubgraphServer {
    pub fn new() -> Self {
        SubgraphServer {
            http_client: Client::new(),
        }
    }
}

#[tool(tool_box)]
impl SubgraphServer {
    #[tool(
        description = "Get schema for a specific subgraph deployment using its deployment ID (0x...)."
    )]
    pub async fn get_schema_by_deployment_id(
        &self,
        headers: HttpRequestHeaders,
        #[tool(aggr)]
        GetSchemaByDeploymentIdRequest { deployment_id }: GetSchemaByDeploymentIdRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_api_key(headers.0.as_ref()) {
            Ok(api_key) => {
                match self
                    .get_schema_by_deployment_id_internal(&api_key, &deployment_id)
                    .await
                {
                    Ok(schema) => Ok(CallToolResult::success(vec![Content::text(schema)])),
                    Err(e) => match e {
                        SubgraphError::GraphQlError(_) => Err(McpError::internal_error(
                            e.to_string(),
                            Some(json!({ "details": e.to_string() })),
                        )),
                        _ => Err(McpError::internal_error(
                            format!("Unexpected error during schema retrieval: {}", e),
                            Some(json!({ "details": e.to_string()})),
                        )),
                    },
                }
            }
            Err(SubgraphError::ApiKeyNotSet) => Err(McpError::invalid_params(
                "Configuration error: API key not found. Please set the GATEWAY_API_KEY environment variable or provide a Bearer token in the Authorization header.",
                None,
            )),
            Err(e) => Err(McpError::internal_error( 
                format!("Error retrieving API key: {}", e),
                Some(json!({ "details": e.to_string() })),
            )),
        }
    }

    #[tool(
        description = "Get the schema for the current version of a subgraph using its subgraph ID (e.g., 5zvR82...)."
    )]
    pub async fn get_schema_by_subgraph_id(
        &self,
        headers: HttpRequestHeaders,
        #[tool(aggr)] GetSchemaBySubgraphIdRequest { subgraph_id }: GetSchemaBySubgraphIdRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_api_key(headers.0.as_ref()) {
            Ok(api_key) => {
                match self
                    .get_schema_by_subgraph_id_internal(&api_key, &subgraph_id)
                    .await
                {
                    Ok(schema_string) => {
                        tracing::info!(target: "mcp_tool_auth", subgraph_id = %subgraph_id, "Internal function call successful.");
                        Ok(CallToolResult::success(vec![Content::text(schema_string)]))
                    }
                    Err(e) => {
                        tracing::error!(
                            target: "mcp_tool_auth",
                            subgraph_id = %subgraph_id,
                            error = %e,
                            "Internal function call failed."
                        );
                        match e {
                            SubgraphError::GraphQlError(_) => Err(McpError::internal_error(
                                e.to_string(),
                                Some(json!({ "details": e.to_string() })),
                            )),
                            _ => Err(McpError::internal_error(
                                format!("Unexpected error during schema retrieval by subgraph ID: {}",e),
                                Some(json!({ "details": e.to_string()})),
                            )),
                        }
                    }
                }
            }
            Err(SubgraphError::ApiKeyNotSet) => Err(McpError::invalid_params(
                "Configuration error: API key not found. Please set the GATEWAY_API_KEY environment variable or provide a Bearer token in the Authorization header.",
                None,
            )),
            Err(e) => Err(McpError::internal_error(
                format!("Error retrieving API key: {}", e),
                Some(json!({ "details": e.to_string() })),
            )),
        }
    }

    #[tool(
        description = "Get schema for a specific subgraph deployment using its IPFS hash (Qm...)."
    )]
    pub async fn get_schema_by_ipfs_hash(
        &self,
        headers: HttpRequestHeaders,
        #[tool(aggr)] GetSchemaByIpfsHashRequest { ipfs_hash }: GetSchemaByIpfsHashRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_api_key(headers.0.as_ref()) {
            Ok(api_key) => {
                match self
                    .get_schema_by_ipfs_hash_internal(&api_key, &ipfs_hash)
                    .await
                {
                    Ok(schema) => Ok(CallToolResult::success(vec![Content::text(schema)])),
                    Err(e) => match e {
                        SubgraphError::GraphQlError(_) => Err(McpError::internal_error(
                            e.to_string(),
                            Some(json!({ "details": e.to_string() })),
                        )),
                        _ => Err(McpError::internal_error(
                            format!("Unexpected error during schema retrieval by IPFS hash: {}",e),
                            Some(json!({ "details": e.to_string()})),
                        )),
                    },
                }
            }
            Err(SubgraphError::ApiKeyNotSet) => Err(McpError::invalid_params(
                 "Configuration error: API key not found. Please set the GATEWAY_API_KEY environment variable or provide a Bearer token in the Authorization header.",
                None,
            )),
            Err(e) => Err(McpError::internal_error(
                format!("Error retrieving API key: {}", e),
                Some(json!({ "details": e.to_string() })),
            )),
        }
    }

    #[tool(description = "Execute a GraphQL query against a specific deployment ID.")]
    pub async fn execute_query_by_deployment_id(
        &self,
        headers: HttpRequestHeaders,
        #[tool(aggr)] ExecuteQueryByDeploymentIdRequest {
            deployment_id,
            query,
            variables,
        }: ExecuteQueryByDeploymentIdRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_api_key(headers.0.as_ref()) {
            Ok(api_key) => {
                match self
                    .execute_query_on_endpoint(&api_key, "deployments/id", &deployment_id, &query, variables)
                    .await
                {
                    Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                        "{:#}",
                        result
                    ))])),
                    Err(e) => match e {
                        SubgraphError::GraphQlError(_) => Err(McpError::internal_error(
                            e.to_string(),
                            Some(json!({ "details": e.to_string() })),
                        )),
                        _ => Err(McpError::internal_error(
                            format!("Unexpected error during query execution by deployment ID: {}",e),
                            Some(json!({ "details": e.to_string()})),
                        )),
                    },
                }
            }
            Err(SubgraphError::ApiKeyNotSet) => Err(McpError::invalid_params(
                 "Configuration error: API key not found. Please set the GATEWAY_API_KEY environment variable or provide a Bearer token in the Authorization header.",
                None,
            )),
            Err(e) => Err(McpError::internal_error(
                format!("Error retrieving API key: {}", e),
                Some(json!({ "details": e.to_string() })),
            )),
        }
    }

    #[tool(description = "Execute a GraphQL query against a specific IPFS hash.")]
    pub async fn execute_query_by_ipfs_hash(
        &self,
        headers: HttpRequestHeaders,
        #[tool(aggr)] ExecuteQueryByIpfsHashRequest {
            ipfs_hash,
            query,
            variables,
        }: ExecuteQueryByIpfsHashRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_api_key(headers.0.as_ref()) {
            Ok(api_key) => {
                match self
                    .execute_query_on_endpoint(&api_key, "deployments/id", &ipfs_hash, &query, variables)
                    .await
                {
                    Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                        "{:#}",
                        result
                    ))])),
                    Err(e) => match e {
                        SubgraphError::GraphQlError(_) => Err(McpError::internal_error(
                            e.to_string(),
                            Some(json!({ "details": e.to_string() })),
                        )),
                        _ => Err(McpError::internal_error(
                            format!("Unexpected error during query execution by IPFS hash: {}",e),
                            Some(json!({ "details": e.to_string()})),
                        )),
                    },
                }
            }
            Err(SubgraphError::ApiKeyNotSet) => Err(McpError::invalid_params(
                 "Configuration error: API key not found. Please set the GATEWAY_API_KEY environment variable or provide a Bearer token in the Authorization header.",
                None,
            )),
            Err(e) => Err(McpError::internal_error(
                format!("Error retrieving API key: {}", e),
                Some(json!({ "details": e.to_string() })),
            )),
        }
    }

    #[tool(description = "Execute a GraphQL query against the latest deployment of a subgraph ID.")]
    pub async fn execute_query_by_subgraph_id(
        &self,
        headers: HttpRequestHeaders,
        #[tool(aggr)] ExecuteQueryBySubgraphIdRequest {
            subgraph_id,
            query,
            variables,
        }: ExecuteQueryBySubgraphIdRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_api_key(headers.0.as_ref()) {
            Ok(api_key) => {
                match self
                    .execute_query_on_endpoint(&api_key, "subgraphs/id", &subgraph_id, &query, variables)
                    .await
                {
                    Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                        "{:#}",
                        result
                    ))])),
                    Err(e) => match e {
                        SubgraphError::GraphQlError(_) => Err(McpError::internal_error(
                            e.to_string(),
                            Some(json!({ "details": e.to_string() })),
                        )),
                        _ => Err(McpError::internal_error(
                            format!("Unexpected error during query execution by subgraph ID: {}",e),
                            Some(json!({ "details": e.to_string()})),
                        )),
                    },
                }
            }
            Err(SubgraphError::ApiKeyNotSet) => Err(McpError::invalid_params(
                 "Configuration error: API key not found. Please set the GATEWAY_API_KEY environment variable or provide a Bearer token in the Authorization header.",
                None,
            )),
            Err(e) => Err(McpError::internal_error(
                format!("Error retrieving API key: {}", e),
                Some(json!({ "details": e.to_string() })),
            )),
        }
    }

    #[tool(
        description = "Get the top 3 subgraph deployments for a given contract address and chain, ordered by query fees. For chain, use 'mainnet' for Ethereum mainnet, NEVER use 'ethereum'."
    )]
    pub async fn get_top_subgraph_deployments(
        &self,
        headers: HttpRequestHeaders,
        #[tool(aggr)] GetTopSubgraphDeploymentsRequest {
            contract_address,
            chain,
        }: GetTopSubgraphDeploymentsRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_api_key(headers.0.as_ref()) {
            Ok(api_key) => {
                match self
                    .get_top_subgraph_deployments_internal(&api_key, &contract_address, &chain)
                    .await
                {
                    Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                        "{:#}",
                        result
                    ))])),
                    Err(e) => match e {
                        SubgraphError::GraphQlError(_) => Err(McpError::internal_error(
                            e.to_string(),
                            Some(json!({ "details": e.to_string() })),
                        )),
                        _ => Err(McpError::internal_error(
                            format!("Unexpected error during top subgraph deployment retrieval: {}",e),
                            Some(json!({ "details": e.to_string()})),
                        )),
                    },
                }
            }
            Err(SubgraphError::ApiKeyNotSet) => Err(McpError::invalid_params(
                 "Configuration error: API key not found. Please set the GATEWAY_API_KEY environment variable or provide a Bearer token in the Authorization header.",
                None,
            )),
            Err(e) => Err(McpError::internal_error(
                format!("Error retrieving API key: {}", e),
                Some(json!({ "details": e.to_string() })),
            )),
        }
    }

    #[tool(
        description = "Search for subgraphs by keyword in their display names, ordered by signal. Returns top 10 results if total results ≤ 100, or square root of total otherwise."
    )]
    pub async fn search_subgraphs_by_keyword(
        &self,
        headers: HttpRequestHeaders,
        #[tool(aggr)] SearchSubgraphsByKeywordRequest { keyword }: SearchSubgraphsByKeywordRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_api_key(headers.0.as_ref()) {
            Ok(api_key) => {
                match self
                    .search_subgraphs_by_keyword_internal(&api_key, &keyword)
                    .await
                {
                    Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                        "{:#}",
                        result
                    ))])),
                    Err(e) => match e {
                        SubgraphError::GraphQlError(_) => Err(McpError::internal_error(
                            e.to_string(),
                            Some(json!({ "details": e.to_string() })),
                        )),
                        _ => Err(McpError::internal_error(
                            format!("Unexpected error during subgraph search: {}", e),
                            Some(json!({ "details": e.to_string()})),
                        )),
                    },
                }
            }
            Err(SubgraphError::ApiKeyNotSet) => Err(McpError::invalid_params(
                 "Configuration error: API key not found. Please set the GATEWAY_API_KEY environment variable or provide a Bearer token in the Authorization header.",
                None,
            )),
            Err(e) => Err(McpError::internal_error(
                format!("Error retrieving API key: {}", e),
                Some(json!({ "details": e.to_string() })),
            )),
        }
    }

    #[tool(
        description = "Get the aggregate query count over the last 30 days for multiple subgraph deployments, sorted by query count in descending order."
    )]
    pub async fn get_deployment_30day_query_counts(
        &self,
        headers: HttpRequestHeaders,
        #[tool(aggr)]
        GetDeployment30DayQueryCountsRequest { ipfs_hashes }: GetDeployment30DayQueryCountsRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_api_key(headers.0.as_ref()) {
            Ok(api_key) => {
                match self
                    .get_deployment_30day_query_counts_internal(&api_key, &ipfs_hashes)
                    .await
                {
                    Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                        "{:#}",
                        result
                    ))])),
                    Err(e) => match e {
                         SubgraphError::GraphQlError(_) => Err(McpError::internal_error(
                            e.to_string(),
                            Some(json!({ "details": e.to_string() })),
                        )),
                        _ => Err(McpError::internal_error(
                            format!("Unexpected error during 30-day query count retrieval: {}",e),
                            Some(json!({ "details": e.to_string()})),
                        )),
                    },
                }
            }
            Err(SubgraphError::ApiKeyNotSet) => Err(McpError::invalid_params(
                 "Configuration error: API key not found. Please set the GATEWAY_API_KEY environment variable or provide a Bearer token in the Authorization header.",
                None,
            )),
            Err(e) => Err(McpError::internal_error(
                format!("Error retrieving API key: {}", e),
                Some(json!({ "details": e.to_string() })),
            )),
        }
    }
}

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
            server_info: Implementation {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some(SERVER_INSTRUCTIONS.to_string()),

        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![self._create_resource_text("graphql://subgraph", "Subgraph MCP LLM Guidence")],
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
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            next_cursor: None,
            prompts: vec![
                Prompt::new(
                    "get_schema_by_deployment_id",
                    Some("Get schema for a specific subgraph deployment using its deployment ID (0x...)."),
                    Some(vec![PromptArgument {
                        name: "deploymentId".to_string(),
                        description: Some("The ID of the subgraph deployment".to_string()),
                        required: Some(true),
                    }]),
                ),
                Prompt::new(
                    "search_subgraphs_by_keyword",
                    Some("Search for subgraphs by keyword in their display names"),
                    Some(vec![PromptArgument {
                        name: "keyword".to_string(),
                        description: Some("The keyword to search for in subgraph names".to_string()),
                        required: Some(true),
                    }]),
                ),
                Prompt::new(
                    "execute_query_by_deployment_id",
                    Some("Execute GraphQL query against a specific deployment ID/hash."),
                    Some(vec![
                        PromptArgument {
                            name: "deploymentId".to_string(),
                            description: Some(
                                "The specific deployment ID (e.g., 0x...) or IPFS hash (e.g., Qm...)"
                                    .to_string(),
                            ),
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
                    "get_schema_by_subgraph_id",
                    Some("Get the schema for the current version of a subgraph using its subgraph ID (e.g., 5zvR82...)."
                    ),
                    Some(vec![
                        PromptArgument {
                            name: "subgraphId".to_string(),
                            description: Some(
                                "The subgraph ID (e.g., 5zvR82...) to get the current schema for"
                                    .to_string(),
                            ),
                            required: Some(true),
                        },
                    ]),
                ),
                Prompt::new(
                    "get_schema_by_ipfs_hash",
                    Some("Get schema for a specific subgraph deployment using its IPFS hash (Qm...)."),
                    Some(vec![
                        PromptArgument {
                            name: "ipfsHash".to_string(),
                            description: Some(
                                "The IPFS hash (e.g., Qm...) of the specific deployment"
                                    .to_string(),
                            ),
                            required: Some(true),
                        },
                    ]),
                ),
                Prompt::new(
                    "get_top_subgraph_deployments",
                    Some("Get top subgraph deployments for a contract."),
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
                Prompt::new(
                    "get_deployment_30day_query_counts",
                    Some("Get 30-day query counts for multiple subgraph deployments by IPFS hash."),
                    Some(vec![PromptArgument {
                        name: "ipfsHashes".to_string(), 
                        description: Some("A list of IPFS hashes (e.g., [\"Qm1...\", \"Qm2...\"])".to_string()),
                        required: Some(true),
                    }]),
                ),
                Prompt::new(
                    "execute_query_by_ipfs_hash",
                    Some("Execute a GraphQL query against a specific IPFS hash."),
                    Some(vec![
                        PromptArgument {
                            name: "ipfsHash".to_string(),
                            description: Some("The IPFS hash (e.g., Qm...) of the specific deployment".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "query".to_string(),
                            description: Some("The GraphQL query to execute".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "variables".to_string(),
                            description: Some("Optional JSON value for GraphQL variables".to_string()),
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
            "get_schema_by_deployment_id" => {
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
            "search_subgraphs_by_keyword" => {
                let keyword = arguments
                    .as_ref()
                    .and_then(|args| args.get("keyword"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{keyword}")
                    .to_string();

                Ok(GetPromptResult {
                    description: Some(
                        "Search for subgraphs by keyword in their display names".to_string(),
                    ),
                    messages: vec![PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(format!(
                            "Find subgraphs related to \"{}\"",
                            keyword
                        )),
                    }],
                })
            }
            "execute_query_by_deployment_id" => {
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
                            "Run this GraphQL query against deployment ID {}: {}",
                            deployment_id, query
                        )),
                    }],
                })
            }
            "get_schema_by_subgraph_id" => {
                let subgraph_id = arguments
                    .as_ref()
                    .and_then(|args| args.get("subgraphId"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{subgraphId}")
                    .to_string();

                Ok(GetPromptResult {
                    description: Some(
                        "Fetch the schema for the current version of a subgraph using its subgraph ID."
                            .to_string(),
                    ),
                    messages: vec![PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(format!(
                            "Get the schema for subgraph ID {}",
                            subgraph_id
                        )),
                    }],
                })
            }
            "get_schema_by_ipfs_hash" => {
                let ipfs_hash = arguments
                    .as_ref()
                    .and_then(|args| args.get("ipfsHash"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{ipfsHash}")
                    .to_string();

                Ok(GetPromptResult {
                    description: Some(
                        "Fetch the schema for a specific subgraph deployment using its IPFS hash."
                            .to_string(),
                    ),
                    messages: vec![PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(format!(
                            "Get the schema for IPFS hash {}",
                            ipfs_hash
                        )),
                    }],
                })
            }
            "get_top_subgraph_deployments" => {
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
                            "Get the top subgraph deployments for contract {} on chain {}",
                            contract_address, chain
                        )),
                    }],
                })
            }
            "get_deployment_30day_query_counts" => {
                let ipfs_hashes_str = arguments
                    .as_ref()
                    .and_then(|args| args.get("ipfsHashes"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("[\"{ipfsHash1}\", \"{ipfsHash2}\"]")
                    .to_string();
                Ok(GetPromptResult {
                    description: Some("Get 30-day query counts for multiple subgraph deployments.".to_string()),
                    messages: vec![PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(format!(
                            "Retrieve the 30-day query counts for subgraph deployments with IPFS hashes: {}",
                            ipfs_hashes_str
                        )),
                    }],
                })
            }
            "execute_query_by_ipfs_hash" => {
                let ipfs_hash = arguments
                    .as_ref()
                    .and_then(|args| args.get("ipfsHash"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{ipfsHash}")
                    .to_string();

                let query = arguments
                    .as_ref()
                    .and_then(|args| args.get("query"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{query}")
                    .to_string();

                let variables_str = arguments
                    .as_ref()
                    .and_then(|args| args.get("variables"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "{}".to_string());

                Ok(GetPromptResult {
                    description: Some(
                        "Execute a GraphQL query against a specific IPFS hash.".to_string(),
                    ),
                    messages: vec![
                        PromptMessage {
                            role: PromptMessageRole::User,
                            content: PromptMessageContent::text(format!(
                                "Run this GraphQL query against IPFS hash {}: {}\nWith variables: {}",
                                ipfs_hash, query, variables_str
                            )),
                        },
                    ],
                })
            }
            _ => Err(McpError::invalid_params("prompt not found", None)),
        }
    }
}
