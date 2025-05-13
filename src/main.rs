// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use log::{error, info};
use reqwest::Client;
use rmcp::{
    model::*,
    schemars,
    service::RequestContext,
    tool,
    transport::sse_server::{SseServer, SseServerConfig},
    Error as McpError, RoleServer, ServerHandler,
};
use serde_json::json;
use std::{
    env,
    net::SocketAddr,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

use rmcp::ServiceExt;
use tokio::io;
use tokio_util::sync::CancellationToken;

const GATEWAY_URL: &str = "https://gateway.thegraph.com/api";
const GRAPH_NETWORK_SUBGRAPH_ARBITRUM: &str = "QmdKXcBUHR3UyURqVRQHu1oV6VUkBrhi2vNvMx3bNDnUCc";
const GATEWAY_QOS_ORACLE: &str = "QmZmb6z87QmqBLmkMhaqWy7h2GLF1ey8Qj7YSRuqSGMjeH";

// SERVER_INSTRUCTIONS (Full instructions needed here)
const SERVER_INSTRUCTIONS: &str = "**Interacting with The Graph Subgraphs**
**IMPORTANT: ALWAYS verify query volumes using `get_deployment_30day_query_counts` for any potential subgraph candidate *before* selecting or querying it. This step is NON-OPTIONAL. Failure to do so may result in using outdated or irrelevant data.**
**Follow this sequence strictly:**
1.  **Analyze User Request:**
   *   Identify the **protocol name** (e.g., \"Uniswap\", \"Aave\", \"ENS\").
   *   Note any specific **version** or **blockchain network** mentioned by the user.
   *   Determine the **goal**: Query data? Get schema?
2.  **Initial Search & Preliminary Analysis:**
   *   Use `search_subgraphs_by_keyword` with the most generic term for the protocol (e.g., if \"Uniswap v3 on Ethereum\", initially search only for \"Uniswap\").
   *   Examine `displayName` and other metadata in the search results for version and network information.
3.  **Mandatory Query Volume Check & Clarification (If Needed):**
   *   **ALWAYS** extract the IPFS hashes (`ipfsHash`) for all potentially relevant subgraphs identified in Step 2.
   *   **ALWAYS** use `get_deployment_30day_query_counts` for these IPFS hashes.
   *   **If Ambiguous (Multiple Versions/Chains with significant volume):**
       *   Present a summary to the user, **including the 30-day query counts for each option**. For example: \"I found several Uniswap subgraphs. Uniswap v3 on Ethereum is the most active (X queries last 30 days). I also see Uniswap v2 on Ethereum (Y queries) and Uniswap v3 on Arbitrum (Z queries). Which specific version and network are you interested in?\"
   *   **If Still Unclear (Information Missing and Not Inferable even with query volumes):**
       *   If version/chain information is genuinely missing from search results and user input, and query volumes don't offer a clear path (e.g. all relevant subgraphs have very low or no volume), ask for clarification directly. Example: \"I found several subgraphs for 'ExampleProtocol', but none have significant query activity. Could you please specify the version and blockchain network you're interested in?\"
   *   **Do NOT proceed to Step 4 without completing this query volume verification.**
4.  **Select Final Subgraph (Post Query Volume Check & Clarification):**
   *   After the keyword search, mandatory query volume check, and any necessary clarification, you should have a clear target protocol, version, and network.
   *   Identify all candidate subgraphs from your Step 2 `search_subgraphs_by_keyword` results that match these clarified criteria.
   *   **If there is more than one such matching subgraph:**
       *   You should have already fetched their query counts in Step 3.
       *   **Select the subgraph with the highest `total_query_count`** among them.
   *   **If only one subgraph precisely matches the criteria**, that is your selected subgraph.
   *   When presenting your chosen subgraph or asking for final confirmation before querying, **ALWAYS state its 30-day query volume** to demonstrate this check has been performed. For example: \"I've selected the 'Uniswap v3 Ethereum' subgraph, which has X queries in the last 30 days. Shall I proceed to get its schema?\"
   *   If the selected subgraph's query count is very low (and this wasn't already discussed during clarification), briefly inform the user.
5.  **Execute Action Using the Identified Subgraph:**
   *   **Identify the ID Type:** (Subgraph ID, Deployment ID, or IPFS Hash - note that `search_subgraphs_by_keyword` returns `id` for Subgraph ID and `ipfsHash` for current deployment's IPFS hash).
   *   **Determine the Correct Tool based on Goal & ID Type:**
       *   **Goal: Query Data**
           *   Subgraph ID (`id` from search) → `execute_query_by_subgraph_id`
           *   Deployment ID / IPFS Hash (`ipfsHash` from search) → `execute_query_by_deployment_id`
       *   **Goal: Get Schema**
           *   Subgraph ID → `get_schema_by_subgraph_id`
           *   Deployment ID → `get_schema_by_deployment_id`
           *   IPFS Hash → `get_schema_by_ipfs_hash`
   *   **Write Clean GraphQL Queries:** Simple structure, omit 'variables' if unused, include only essential fields.
**Special Case: Contract Address Lookup**
*   ONLY when a user explicitly provides a **contract address** (0x...) AND asks for subgraphs related to it:
    *   Identify the blockchain network for the address (ask user if unclear).
    *   Use `get_top_subgraph_deployments` with the provided contract address and chain name.
    *   Process and use the resulting deployment IDs as needed. **Crucially, before using any of these deployment IDs for querying, first use `get_deployment_30day_query_counts` with their IPFS hashes (which are the deployment IDs themselves in this context) to verify activity.**
**ID Type Reference:**
*   **Subgraph ID**: Typically starts with digits and letters (e.g., 5zvR82...)
*   **Deployment ID / IPFS Hash**: For the purpose of `get_deployment_30day_query_counts`, the 'IPFS Hash' (Qm...) or 'Deployment ID' (0x...) can be used. Note `search_subgraphs_by_keyword` returns `ipfsHash`. `get_top_subgraph_deployments` returns `id` which is the Deployment ID (0x...).

**Best Practices:**
*   When using GraphQL, if unsure about the structure, first get the schema to understand available entities and fields.
*   Create focused queries that only request necessary fields.
*   For paginated data, use appropriate limit parameters.
*   Use variables for dynamic values in queries.";

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
pub struct SearchSubgraphsByKeywordRequest {
    #[schemars(description = "Keyword to search for in subgraph names")]
    pub keyword: String,
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

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetDeployment30DayQueryCountsRequest {
    #[schemars(
        description = "List of IPFS hashes (Qm...) to get query counts for the last 30 days"
    )]
    pub ipfs_hashes: Vec<String>,
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

    async fn search_subgraphs_by_keyword_internal(
        &self,
        keyword: &str,
    ) -> Result<serde_json::Value, SubgraphError> {
        let api_key = self.get_api_key()?;
        let url = self.get_network_subgraph_query_url(&api_key);

        let query = r#"
        query SearchSubgraphsByKeyword($keyword: String!) {
          subgraphs(
            where: {metadata_: {displayName_contains_nocase: $keyword}}
            orderBy: currentSignalledTokens
            orderDirection: desc
            first: 1000
          ) {
            id
            metadata {
              displayName
            }
            currentVersion {
              subgraphDeployment {
                ipfsHash
              }
            }
          }
        }
        "#;

        let variables = serde_json::json!({
            "keyword": keyword
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

        // Process the results based on the requirements
        if let Some(subgraphs) = data.get("subgraphs").and_then(|s| s.as_array()) {
            let total_count = subgraphs.len();

            // Determine how many results to return
            let limit = if total_count <= 100 {
                10 // Return top 10 if total is <= 100
            } else {
                // Return square root of total if > 100
                (total_count as f64).sqrt().ceil() as usize
            };

            // Create a new limited result
            let limited_subgraphs = subgraphs.iter().take(limit).collect::<Vec<_>>();

            // Build the result JSON
            let result = serde_json::json!({
                "subgraphs": limited_subgraphs,
                "total": total_count,
                "returned": limited_subgraphs.len(),
            });

            return Ok(result);
        }

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

    #[tool(
        description = "Search for subgraphs by keyword in their display names, ordered by signal. Returns top 10 results if total results ≤ 100, or square root of total otherwise."
    )]
    async fn search_subgraphs_by_keyword(
        &self,
        #[tool(aggr)]
        #[schemars(description = "Request containing the keyword to search for in subgraph names")]
        SearchSubgraphsByKeywordRequest { keyword }: SearchSubgraphsByKeywordRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.search_subgraphs_by_keyword_internal(&keyword).await {
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

    async fn get_deployment_30day_query_counts_internal(
        &self,
        ipfs_hashes: &[String],
    ) -> Result<serde_json::Value, SubgraphError> {
        let api_key = self.get_api_key()?;
        let url = format!(
            "{}/{}/deployments/id/{}",
            GATEWAY_URL, api_key, GATEWAY_QOS_ORACLE
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SubgraphError::GraphQlError("Error calculating timestamp".to_string()))?
            .as_secs();

        let thirty_days_ago = now - (30 * 24 * 60 * 60);

        let query = r#"
        query GetSubgraphDeployment30DayQueryCounts(
          $deploymentIDs: [ID!]!,
          $thirtyDaysAgoTimestamp: BigInt!
        ) {
          subgraphDeployments(where: { id_in: $deploymentIDs }) {
            id
            queryDailyDataPoints(
              where: { dayStart_gte: $thirtyDaysAgoTimestamp }
              orderBy: dayStart
              orderDirection: asc
              first: 31 # Max ~30 daily entries for 30 days
            ) {
              query_count
              dayStart # For verification/debugging
            }
          }
        }
        "#;

        let variables = serde_json::json!({
            "deploymentIDs": ipfs_hashes,
            "thirtyDaysAgoTimestamp": thirty_days_ago.to_string()
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

        // Process the results to aggregate counts
        let deployments = data
            .get("subgraphDeployments")
            .and_then(|d| d.as_array())
            .ok_or_else(|| SubgraphError::GraphQlError("Unexpected response format".to_string()))?;

        let mut query_counts: Vec<serde_json::Value> = Vec::new();

        for deployment in deployments {
            let id = deployment
                .get("id")
                .and_then(|id| id.as_str())
                .ok_or_else(|| {
                    SubgraphError::GraphQlError("Missing deployment ID in response".to_string())
                })?;

            let data_points = deployment
                .get("queryDailyDataPoints")
                .and_then(|points| points.as_array())
                .ok_or_else(|| {
                    SubgraphError::GraphQlError("Missing data points in response".to_string())
                })?;

            let mut total_query_count: i64 = 0;
            for point in data_points {
                let count = point
                    .get("query_count")
                    .and_then(|c| c.as_str())
                    .ok_or_else(|| {
                        SubgraphError::GraphQlError("Missing query count in data point".to_string())
                    })?;

                total_query_count += count.parse::<i64>().unwrap_or(0);
            }

            query_counts.push(serde_json::json!({
                "ipfs_hash": id,
                "total_query_count": total_query_count,
                "data_points_count": data_points.len()
            }));
        }

        // Sort by query count in descending order
        query_counts.sort_by(|a, b| {
            let count_a = a
                .get("total_query_count")
                .and_then(|c| c.as_i64())
                .unwrap_or(0);
            let count_b = b
                .get("total_query_count")
                .and_then(|c| c.as_i64())
                .unwrap_or(0);
            count_b.cmp(&count_a)
        });

        Ok(serde_json::json!({
            "deployments": query_counts,
            "total_deployments": query_counts.len()
        }))
    }

    #[tool(
        description = "Get the aggregate query count over the last 30 days for multiple subgraph deployments, sorted by query count in descending order."
    )]
    async fn get_deployment_30day_query_counts(
        &self,
        #[tool(aggr)]
        #[schemars(
            description = "Request containing a list of IPFS hashes to get 30-day query counts for"
        )]
        GetDeployment30DayQueryCountsRequest { ipfs_hashes }: GetDeployment30DayQueryCountsRequest,
    ) -> Result<CallToolResult, McpError> {
        match self.get_deployment_30day_query_counts_internal(&ipfs_hashes).await {
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
    let args: Vec<String> = env::args().collect();

    // Check for --sse flag
    if args.iter().any(|arg| arg == "--sse") {
        start_sse_server().await
    } else {
        // Default to stdio if no --sse flag
        start_stdio_server().await
    }
}

async fn start_stdio_server() -> Result<()> {
    env_logger::init();
    info!("Starting STDIO Subgraph MCP Server");

    let server = SubgraphServer::new();

    let transport = (io::stdin(), io::stdout());

    let running = server.serve(transport).await?;
    running.waiting().await?;

    info!("Server shutdown complete");
    Ok(())
}

async fn start_sse_server() -> Result<()> {
    env_logger::init();
    info!("Starting SSE Subgraph MCP Server");
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let bind_addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid BIND address format '{}:{}': {}", host, port, e))?;

    let sse_path = env::var("SSE_PATH").unwrap_or_else(|_| "/sse".to_string());
    let post_path = env::var("POST_PATH").unwrap_or_else(|_| "/messages".to_string());

    let server_shutdown_token = CancellationToken::new();
    let service_shutdown_token;

    let config = SseServerConfig {
        bind: bind_addr,
        sse_path,
        post_path,
        ct: server_shutdown_token.clone(), // Token for the server transport
    };

    let sse_server = SseServer::serve_with_config(config).await?;
    info!("SSE Server listening on {}", sse_server.config.bind);

    // Attach the Subgraph MCP service
    // This returns a token specific to the service task
    service_shutdown_token = sse_server.with_service(SubgraphServer::new);
    info!("Subgraph MCP Service attached");

    // Wait for a shutdown signal (Ctrl+C or SIGTERM)
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Ctrl+C (SIGINT) received, initiating graceful shutdown...");
        },
        _ = sigterm.recv() => {
             info!("SIGTERM received, initiating graceful shutdown...");
        }

    };

    info!("Signalling service and server to shut down...");
    service_shutdown_token.cancel(); // Signal the MCP service task to stop
    server_shutdown_token.cancel(); // Signal the underlying web server to stop

    info!("Shutdown complete.");
    Ok(())
}
