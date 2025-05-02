// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use log::{error, info};
use reqwest::Client;
use rmcp::ServiceExt;
use std::env;
use thiserror::Error;
use tokio::io;

use rmcp::{
    model::*, schemars, service::RequestContext, tool, Error as McpError, RoleServer, ServerHandler,
};
use serde_json::json;

const GATEWAY_URL: &str = "https://gateway.thegraph.com/api";
const GRAPH_NETWORK_SUBGRAPH_ARBITRUM: &str = "QmdKXcBUHR3UyURqVRQHu1oV6VUkBrhi2vNvMx3bNDnUCc";
const SERVER_INSTRUCTIONS: &str = "This server interacts with subgraphs on The Graph protocol. \
Workflow: \
1. **Determine the user's goal:** \
    a. Is the user asking for information *about* a specific address (e.g., find ENS name for 0x...)? \
    b. Is the user asking for subgraphs that index a specific *contract address* they provided? \
    c. Is the user trying to query a subgraph using a *subgraph ID* (e.g., 5zvR82...), *deployment ID* (e.g., 0x...), or *IPFS hash* (e.g., Qm...)? \
    d. Is the user asking for the *schema* of a subgraph using one of the above identifiers? \
2. **Identify the chain** (IMPORTANT: use 'mainnet' for Ethereum mainnet, NOT 'ethereum'; use 'arbitrum-one' for Arbitrum, etc.). This is needed for `get_top_subgraph_deployments`. \
3. **If Goal is (a) - Address Lookup (e.g., ENS):** \
    a. Identify the relevant **protocol** (e.g., ENS). \
    b. Find the **protocol's main contract address** on the identified chain. For The Graph protocol contracts, refer to https://thegraph.com/docs/en/contracts/ and default to using Arbitrum addresses as this is the principal deployment. \
    c. Use `get_top_subgraph_deployments` with the **protocol's contract address** and chain to find relevant deployment IDs. \
    d. Use the obtained deployment ID(s) with `execute_query_by_deployment_id`. The query should use the **original user-provided address** (from 1a) in its variables or filters to find the specific information (e.g., the ENS name). \
4. **If Goal is (b) - Find Subgraphs for a Contract:** \
    a. Use the **contract address provided by the user** (from 1b). \
    b. Use `get_top_subgraph_deployments` with this **user-provided contract address** and the identified chain to find relevant deployment IDs. \
    c. Use the obtained deployment ID(s) with `get_schema_by_deployment_id` or `execute_query_by_deployment_id` as needed. \
5. **If Goal is (c) - Query by Subgraph/Deployment ID/IPFS Hash:** \
    a. Identify the type of identifier provided: **subgraph ID** (often alphanumeric, like 5zvR82...), **deployment ID** (starts with `0x...`), or **IPFS hash** (starts with `Qm...`). \
    b. If it's a **subgraph ID**, use `execute_query_by_subgraph_id`. This targets the *latest* deployment associated with that subgraph ID. \
    c. If it's a **deployment ID** or **IPFS hash**, use `execute_query_by_deployment_id`. This targets the *specific, immutable* deployment corresponding to that ID/hash. \
6. **If Goal is (d) - Get Schema:** \
    a. Identify the type of identifier provided: **subgraph ID** (e.g., 5zvR82...), **deployment ID** (e.g., 0x...), or **IPFS hash** (e.g., Qm...). \
    b. If it's a **subgraph ID**, use `get_schema_by_subgraph_id` to get the schema of the *current* deployment for that subgraph. \
    c. If it's a **deployment ID**, use `get_schema_by_deployment_id`. \
    d. If it's an **IPFS hash**, use `get_schema_by_ipfs_hash`. \
7. **Write clean GraphQL queries:** \
    a. Omit the 'variables' parameter when not needed. \
    b. Create simple GraphQL structures without unnecessary complexity. \
    c. Include only the essential fields in your query. \
**Important:** \
*   Distinguish carefully between identifier types: \
    *   **Subgraph ID** (e.g., `5zvR82...`): Logical identifier for a subgraph. Use `execute_query_by_subgraph_id` (queries latest deployment) or `get_schema_by_subgraph_id` (gets schema of latest deployment). \
    *   **Deployment ID** (e.g., `0x4d7c...`): Identifier for a specific, immutable deployment. Use `execute_query_by_deployment_id` or `get_schema_by_deployment_id`. \
    *   **IPFS Hash** (e.g., `QmTZ8e...`): Identifier for the manifest of a specific, immutable deployment. Use `execute_query_by_deployment_id` (the gateway treats it like a deployment ID for querying) or `get_schema_by_ipfs_hash`. \
*   For `get_top_subgraph_deployments`, the `contractAddress` parameter *must* be the address of the contract you want to find indexed subgraphs for. \
*   Chain parameter for `get_top_subgraph_deployments` must be 'mainnet' for Ethereum mainnet, not 'ethereum'. \
*   The Graph protocol has migrated to Arbitrum One. When working with The Graph protocol directly, refer to https://thegraph.com/docs/en/contracts/ and use Arbitrum contract addresses by default unless specifically requested otherwise. \
*   When asked to provide ENS names for any address, always rely on the ENS contracts and subgraphs. \
*   Never use hardcoded deployment IDs/hashes/subgraph IDs from memory unless provided directly by the user. Use `get_top_subgraph_deployments` first to discover relevant deployments if needed. \
*   If a query or schema fetch fails, double-check that the correct tool was used for the given identifier type (subgraph ID vs. deployment ID vs. IPFS hash) and that the identifier itself is correct. \
*   Clean query structure: Keep GraphQL queries simple with only necessary fields, omit the variables parameter when not needed, and use a clear, minimal query structure. \
*   Protocol version awareness: When querying blockchain protocol data (like Uniswap, Aave, Compound, etc.), prioritize the latest major version unless specified otherwise. \
*   Contract address verification: When accessing blockchain protocol data through subgraphs found via `get_top_subgraph_deployments`, verify that the contract address corresponds to the intended protocol by checking the schema before proceeding with further queries. \
*   Clarification thresholds: When a query about blockchain data lacks specificity (protocol version, timeframe, metrics of interest), request clarification if the potential interpretations would lead to significantly different results. \
*   Context inference: For blockchain data queries, infer context from recent protocol developments (e.g., default to Uniswap V3 over V2 if unspecified). ";

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
pub struct GetSchemaByDeploymentIdRequest {
    #[schemars(description = "The deployment ID (e.g., 0x...) of the specific deployment")]
    pub deployment_id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetSchemaBySubgraphIdRequest {
    #[schemars(description = "The subgraph ID (e.g., 5zvR82...) to get the current schema for")]
    pub subgraph_id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetSchemaByIpfsHashRequest {
    #[schemars(description = "The IPFS hash (e.g., Qm...) of the specific deployment")]
    pub ipfs_hash: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecuteQueryByDeploymentIdRequest {
    #[schemars(description = "The deployment ID or IPFS hash of the specific subgraph deployment")]
    pub deployment_id: String,
    #[schemars(description = "The GraphQL query string")]
    pub query: String,
    #[schemars(description = "Optional JSON value for GraphQL variables")]
    pub variables: Option<serde_json::Value>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecuteQueryBySubgraphIdRequest {
    #[schemars(description = "The ID of the subgraph (resolves to the latest deployment)")]
    pub subgraph_id: String,
    #[schemars(description = "The GraphQL query string")]
    pub query: String,
    #[schemars(description = "Optional JSON value for GraphQL variables")]
    pub variables: Option<serde_json::Value>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetTopSubgraphDeploymentsRequest {
    #[schemars(description = "The contract address to find subgraph deployments for")]
    pub contract_address: String,
    #[schemars(description = "The chain name (e.g., 'mainnet', 'arbitrum-one')")]
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
        env::var("GATEWAY_API_KEY").map_err(|_| SubgraphError::ApiKeyNotSet)
    }

    fn get_graph_network_subgraph(&self) -> String {
        env::var("GRAPH_NETWORK_SUBGRAPH")
            .unwrap_or_else(|_| GRAPH_NETWORK_SUBGRAPH_ARBITRUM.to_string())
    }

    fn get_network_subgraph_query_url(&self, api_key: &str) -> String {
        format!(
            "{}/{}/deployments/id/{}",
            GATEWAY_URL,
            api_key,
            self.get_graph_network_subgraph()
        )
    }

    async fn get_schema_by_deployment_id_internal(
        &self,
        deployment_id: &str,
    ) -> Result<String, SubgraphError> {
        let api_key = self.get_api_key()?;
        let url = self.get_network_subgraph_query_url(&api_key);

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

    async fn get_schema_by_subgraph_id_internal(
        &self,
        subgraph_id: &str,
    ) -> Result<String, SubgraphError> {
        let api_key = self.get_api_key()?;
        let url = self.get_network_subgraph_query_url(&api_key);

        let query = r#"
        query SubgraphSchema($id: String!) {
          subgraph(id: $id) {
            currentVersion {
              subgraphDeployment {
                manifest {
                  schema {
                    schema
                  }
                }
              }
            }
          }
        }
        "#;

        let variables = serde_json::json!({ "id": subgraph_id });
        let request_body = serde_json::json!({ "query": query, "variables": variables });

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
            .get("subgraph")
            .and_then(|sg| sg.get("currentVersion"))
            .and_then(|cv| cv.get("subgraphDeployment"))
            .and_then(|dep| dep.get("manifest"))
            .and_then(|manifest| manifest.get("schema"))
            .and_then(|schema| schema.get("schema"))
            .and_then(|schema| schema.as_str())
            .ok_or_else(|| {
                SubgraphError::GraphQlError(
                    "Schema not found for current version in the response".to_string(),
                )
            })?;

        Ok(schema.to_string())
    }

    async fn get_schema_by_ipfs_hash_internal(
        &self,
        ipfs_hash: &str,
    ) -> Result<String, SubgraphError> {
        let api_key = self.get_api_key()?;
        let url = self.get_network_subgraph_query_url(&api_key);

        let query = r#"
        query DeploymentSchemaByIpfsHash($hash: String!) {
          subgraphDeployments(where: {ipfsHash: $hash}, first: 1) {
            manifest {
              schema {
                schema
              }
            }
          }
        }
        "#;

        let variables = serde_json::json!({ "hash": ipfs_hash });
        let request_body = serde_json::json!({ "query": query, "variables": variables });

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
            .get("subgraphDeployments")
            .and_then(|deployments| deployments.get(0))
            .and_then(|dep| dep.get("manifest"))
            .and_then(|manifest| manifest.get("schema"))
            .and_then(|schema| schema.get("schema"))
            .and_then(|schema| schema.as_str())
            .ok_or_else(|| {
                SubgraphError::GraphQlError(
                    "Schema not found for the given IPFS hash in the response".to_string(),
                )
            })?;

        Ok(schema.to_string())
    }

    async fn execute_query_on_endpoint(
        &self,
        endpoint_type: &str,
        id: &str,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, SubgraphError> {
        let api_key = self.get_api_key()?;
        let url = format!("{}/{}/{}/{}", GATEWAY_URL, api_key, endpoint_type, id);

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

    async fn get_top_subgraph_deployments_internal(
        &self,
        contract_address: &str,
        chain: &str,
    ) -> Result<serde_json::Value, SubgraphError> {
        let api_key = self.get_api_key()?;
        let url = self.get_network_subgraph_query_url(&api_key);

        let query = r#"
        query TopSubgraphDeploymentsForContract($network: String!, $contractAddress: String!) {
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

        Ok(data)
    }

    fn _create_resource_text(&self, uri: &str, name: &str) -> Resource {
        RawResource::new(uri, name.to_string()).no_annotation()
    }

    #[tool(
        description = "Get schema for a specific subgraph deployment using its deployment ID (0x...)."
    )]
    async fn get_schema_by_deployment_id(
        &self,
        #[tool(aggr)]
        GetSchemaByDeploymentIdRequest { deployment_id }: GetSchemaByDeploymentIdRequest,
    ) -> Result<CallToolResult, McpError> {
        match self
            .get_schema_by_deployment_id_internal(&deployment_id)
            .await
        {
            Ok(schema) => Ok(CallToolResult::success(vec![Content::text(schema)])),
            Err(e) => match e {
                SubgraphError::ApiKeyNotSet => Err(McpError::invalid_params(
                    "Configuration error: API key not set. Please set the GATEWAY_API_KEY environment variable.",
                    None,
                )),
                _ => Err(McpError::internal_error(
                    e.to_string(),
                    Some(json!({ "details": e.to_string() })),
                )),
            }
        }
    }

    #[tool(
        description = "Get the schema for the current version of a subgraph using its subgraph ID (e.g., 5zvR82...)."
    )]
    async fn get_schema_by_subgraph_id(
        &self,
        #[tool(aggr)] GetSchemaBySubgraphIdRequest { subgraph_id }: GetSchemaBySubgraphIdRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_schema_by_subgraph_id_internal(&subgraph_id).await {
            Ok(schema) => Ok(CallToolResult::success(vec![Content::text(schema)])),
            Err(e) => match e {
                SubgraphError::ApiKeyNotSet => Err(McpError::invalid_params(
                    "Configuration error: API key not set. Please set the GATEWAY_API_KEY environment variable.",
                    None,
                )),
                _ => Err(McpError::internal_error(
                    e.to_string(),
                    Some(json!({ "details": e.to_string() })),
                )),
            }
        }
    }

    #[tool(
        description = "Get schema for a specific subgraph deployment using its IPFS hash (Qm...)."
    )]
    async fn get_schema_by_ipfs_hash(
        &self,
        #[tool(aggr)] GetSchemaByIpfsHashRequest { ipfs_hash }: GetSchemaByIpfsHashRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_schema_by_ipfs_hash_internal(&ipfs_hash).await {
            Ok(schema) => Ok(CallToolResult::success(vec![Content::text(schema)])),
            Err(e) => match e {
                SubgraphError::ApiKeyNotSet => Err(McpError::invalid_params(
                    "Configuration error: API key not set. Please set the GATEWAY_API_KEY environment variable.",
                    None,
                )),
                _ => Err(McpError::internal_error(
                    e.to_string(),
                    Some(json!({ "details": e.to_string() })),
                )),
            }
        }
    }

    #[tool(description = "Execute a GraphQL query against a specific deployment ID or IPFS hash.")]
    async fn execute_query_by_deployment_id(
        &self,
        #[tool(aggr)] ExecuteQueryByDeploymentIdRequest {
            deployment_id,
            query,
            variables,
        }: ExecuteQueryByDeploymentIdRequest,
    ) -> Result<CallToolResult, McpError> {
        match self
            .execute_query_on_endpoint("deployments/id", &deployment_id, &query, variables)
            .await
        {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                "{:#}",
                result
            ))])),
             Err(e) => match e {
                SubgraphError::ApiKeyNotSet => Err(McpError::invalid_params(
                    "Configuration error: API key not set. Please set the GATEWAY_API_KEY environment variable.",
                    None,
                )),
                _ => Err(McpError::internal_error(
                    e.to_string(),
                    Some(json!({ "details": e.to_string() })),
                )),
            }
        }
    }

    #[tool(description = "Execute a GraphQL query against the latest deployment of a subgraph ID.")]
    async fn execute_query_by_subgraph_id(
        &self,
        #[tool(aggr)] ExecuteQueryBySubgraphIdRequest {
            subgraph_id,
            query,
            variables,
        }: ExecuteQueryBySubgraphIdRequest,
    ) -> Result<CallToolResult, McpError> {
        match self
            .execute_query_on_endpoint("subgraphs/id", &subgraph_id, &query, variables)
            .await
        {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                "{:#}",
                result
            ))])),
             Err(e) => match e {
                SubgraphError::ApiKeyNotSet => Err(McpError::invalid_params(
                    "Configuration error: API key not set. Please set the GATEWAY_API_KEY environment variable.",
                    None,
                )),
                _ => Err(McpError::internal_error(
                    e.to_string(),
                    Some(json!({ "details": e.to_string() })),
                )),
            }
        }
    }

    #[tool(
        description = "Get the top 3 subgraph deployments for a given contract address and chain, ordered by query fees. For chain, use 'mainnet' for Ethereum mainnet, NEVER use 'ethereum'."
    )]
    async fn get_top_subgraph_deployments(
        &self,
        #[tool(aggr)]
        #[schemars(description = "Request containing the contract address and chain name")]
        GetTopSubgraphDeploymentsRequest {
            contract_address,
            chain,
        }: GetTopSubgraphDeploymentsRequest,
    ) -> Result<CallToolResult, McpError> {
        match self
            .get_top_subgraph_deployments_internal(&contract_address, &chain)
            .await
        {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(format!(
                "{:#}",
                result
            ))])),
             Err(e) => match e {
                SubgraphError::ApiKeyNotSet => Err(McpError::invalid_params(
                    "Configuration error: API key not set. Please set the GATEWAY_API_KEY environment variable.",
                    None,
                )),
                _ => Err(McpError::internal_error(
                    e.to_string(),
                    Some(json!({ "details": e.to_string() })),
                )),
            }
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
                    "get_schema_by_deployment_id",
                    Some("Get schema for a specific subgraph deployment using its deployment ID (0x...)."),
                    Some(vec![PromptArgument {
                        name: "deploymentId".to_string(),
                        description: Some("The ID of the subgraph deployment".to_string()),
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
                            "Run this GraphQL query against deployment ID/hash {}: {}",
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
            _ => Err(McpError::invalid_params("prompt not found", None)),
        }
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
