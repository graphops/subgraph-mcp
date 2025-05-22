// SPDX-License-Identifier: Apache-2.0
use rmcp::schemars;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetSchemaByDeploymentIdRequest {
    #[schemars(description = "The deployment ID (e.g., 0x...) of the specific deployment")]
    pub deployment_id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchSubgraphsByKeywordRequest {
    #[schemars(description = "Keyword to search for in subgraph names")]
    pub keyword: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetSchemaBySubgraphIdRequest {
    #[schemars(description = "The subgraph ID (e.g., 5zvR82...) to get the current schema for")]
    pub subgraph_id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetSchemaByIpfsHashRequest {
    #[schemars(description = "The IPFS hash (e.g., Qm...) of the specific deployment")]
    pub ipfs_hash: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExecuteQueryByDeploymentIdRequest {
    #[schemars(
        description = "The deployment ID (e.g., 0x...) of the specific subgraph deployment"
    )]
    pub deployment_id: String,
    #[schemars(description = "The GraphQL query string")]
    pub query: String,
    #[schemars(description = "Optional JSON value for GraphQL variables")]
    pub variables: Option<serde_json::Value>,
}
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExecuteQueryByIpfsHashRequest {
    #[schemars(description = "The IPFS hash (e.g., Qm...) of the specific deployment")]
    pub ipfs_hash: String,
    #[schemars(description = "The GraphQL query string")]
    pub query: String,
    #[schemars(description = "Optional JSON value for GraphQL variables")]
    pub variables: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExecuteQueryBySubgraphIdRequest {
    #[schemars(description = "The ID of the subgraph (resolves to the latest deployment)")]
    pub subgraph_id: String,
    #[schemars(description = "The GraphQL query string")]
    pub query: String,
    #[schemars(description = "Optional JSON value for GraphQL variables")]
    pub variables: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetTopSubgraphDeploymentsRequest {
    #[schemars(description = "The contract address to find subgraph deployments for")]
    pub contract_address: String,
    #[schemars(description = "The chain name (e.g., 'mainnet', 'arbitrum-one')")]
    pub chain: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetDeployment30DayQueryCountsRequest {
    #[schemars(
        description = "List of IPFS hashes (Qm...) to get query counts for the last 30 days"
    )]
    pub ipfs_hashes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphQLResponse {
    pub data: Option<serde_json::Value>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphQLError {
    pub message: String,
}
