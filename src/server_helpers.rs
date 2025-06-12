// SPDX-License-Identifier: Apache-2.0
use crate::constants::{
    DEFAULT_GATEWAY_ID, GATEWAY_QOS_ORACLE, GATEWAY_REGISTRY, GRAPH_NETWORK_SUBGRAPH_ARBITRUM,
};
use crate::error::SubgraphError;
use crate::metrics::METRICS;
use crate::server::SubgraphServer;
use crate::types::*;
use http::HeaderMap;
use rmcp::model::{AnnotateAble, RawResource, Resource};
use serde_json::json;
use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

impl SubgraphServer {
    pub(crate) fn get_api_key(
        &self,
        headers_opt: Option<&HeaderMap>,
    ) -> Result<String, SubgraphError> {
        if let Some(actual_headers) = headers_opt {
            if let Some(auth_header_value) = actual_headers.get(http::header::AUTHORIZATION) {
                if let Ok(auth_str) = auth_header_value.to_str() {
                    if let Some(token_part) = auth_str.strip_prefix("Bearer ") {
                        if !token_part.is_empty() {
                            tracing::info!(target: "mcp_tool_auth", "Using API key from Authorization header.");
                            return Ok(token_part.to_string());
                        }
                    }
                }
            }
        }
        tracing::info!(target: "mcp_tool_auth", "Using API key from GATEWAY_API_KEY environment variable.");
        env::var("GATEWAY_API_KEY").map_err(|_| SubgraphError::ApiKeyNotSet)
    }

    pub(crate) fn get_gateway_url(
        &self,
        headers_opt: Option<&HeaderMap>,
    ) -> Result<String, SubgraphError> {
        if let Some(actual_headers) = headers_opt {
            if let Some(gateway_id_header) = actual_headers.get("x-gateway-id") {
                if let Ok(gateway_id) = gateway_id_header.to_str() {
                    if !gateway_id.is_empty() {
                        // Look up the gateway URL by ID
                        if let Some(gateway_url) = GATEWAY_REGISTRY.get(gateway_id) {
                            tracing::info!(target: "mcp_gateway", gateway_id = %gateway_id, gateway_url = %gateway_url, "Using gateway from 'x-gateway-id' header");
                            return Ok(gateway_url.to_string());
                        } else {
                            // Invalid gateway ID - return error with available options
                            let valid_ids: Vec<&str> = GATEWAY_REGISTRY.keys().copied().collect();
                            let error_msg = format!(
                                "Invalid gateway ID '{}' from header. Valid gateway IDs are: {}",
                                gateway_id,
                                valid_ids.join(", ")
                            );
                            tracing::warn!(target: "mcp_gateway", gateway_id = %gateway_id, "Invalid gateway ID requested");
                            return Err(SubgraphError::InvalidGatewayId(error_msg));
                        }
                    }
                }
            }
        }
        // Use default gateway
        if let Some(gateway_url) = GATEWAY_REGISTRY.get(DEFAULT_GATEWAY_ID) {
            tracing::info!(target: "mcp_gateway", gateway_id = %DEFAULT_GATEWAY_ID, gateway_url = %gateway_url, "Using default gateway");
            Ok(gateway_url.to_string())
        } else {
            Err(SubgraphError::InvalidGatewayId(
                "Default gateway ID not found in registry".to_string(),
            ))
        }
    }

    pub(crate) fn get_graph_network_subgraph(&self) -> String {
        env::var("GRAPH_NETWORK_SUBGRAPH")
            .unwrap_or_else(|_| GRAPH_NETWORK_SUBGRAPH_ARBITRUM.to_string())
    }

    pub(crate) fn get_network_subgraph_query_url(
        &self,
        api_key: &str,
        gateway_url: &str,
    ) -> String {
        format!(
            "{}/{}/deployments/id/{}",
            gateway_url,
            api_key,
            self.get_graph_network_subgraph()
        )
    }

    pub(crate) async fn get_schema_by_deployment_id_internal(
        &self,
        api_key: &str,
        gateway_url: &str,
        deployment_id: &str,
    ) -> Result<String, SubgraphError> {
        METRICS
            .observe_gateway_request("network_subgraph_query", || async {
                let url = self.get_network_subgraph_query_url(api_key, gateway_url);

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
            })
            .await
    }

    pub(crate) async fn get_schema_by_subgraph_id_internal(
        &self,
        api_key: &str,
        gateway_url: &str,
        subgraph_id: &str,
    ) -> Result<String, SubgraphError> {
        METRICS
            .observe_gateway_request("network_subgraph_query", || async {
                let url = self.get_network_subgraph_query_url(api_key, gateway_url);

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
            })
            .await
    }

    pub(crate) async fn get_schema_by_ipfs_hash_internal(
        &self,
        api_key: &str,
        gateway_url: &str,
        ipfs_hash: &str,
    ) -> Result<String, SubgraphError> {
        METRICS
            .observe_gateway_request("network_subgraph_query", || async {
                let url = self.get_network_subgraph_query_url(api_key, gateway_url);

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
            })
            .await
    }

    pub(crate) async fn execute_query_on_endpoint(
        &self,
        api_key: &str,
        gateway_url: &str,
        endpoint_type: &str,
        id: &str,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, SubgraphError> {
        METRICS
            .observe_gateway_request(endpoint_type, || async {
                let url = format!("{}/{}/{}/{}", gateway_url, api_key, endpoint_type, id);

                let mut request_body = serde_json::json!({
                    "query": query,
                });

                if let Some(vars) = variables {
                    request_body["variables"] = vars;
                }

                let response_val = self
                    .http_client
                    .post(&url)
                    .json(&request_body)
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await?;

                if let Some(errors_val) = response_val.get("errors") {
                    if let Some(errors_arr) = errors_val.as_array() {
                        if !errors_arr.is_empty() {
                            if let Some(first_error) =
                                errors_arr[0].get("message").and_then(|m| m.as_str())
                            {
                                return Err(SubgraphError::GraphQlError(first_error.to_string()));
                            } else {
                                return Err(SubgraphError::GraphQlError(
                                    "Received GraphQL errors without a message.".to_string(),
                                ));
                            }
                        }
                    }
                }
                Ok(response_val)
            })
            .await
    }

    pub(crate) async fn get_top_subgraph_deployments_internal(
        &self,
        api_key: &str,
        gateway_url: &str,
        contract_address: &str,
        chain: &str,
    ) -> Result<serde_json::Value, SubgraphError> {
        METRICS
            .observe_gateway_request("network_subgraph_query", || async {
                let url = self.get_network_subgraph_query_url(api_key, gateway_url);

                let query = r#"
            query TopSubgraphDeploymentsForContract($network: String!, $contractAddress: String!) {
              subgraphDeployments(
                where: {manifest_: {network: $network, manifest_contains: $contractAddress}}
                orderBy: queryFeesAmount
                orderDirection: desc
                first: 3
              ) {
                ipfsHash
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
            })
            .await
    }

    pub(crate) async fn search_subgraphs_by_keyword_internal(
        &self,
        api_key: &str,
        gateway_url: &str,
        keyword: &str,
    ) -> Result<serde_json::Value, SubgraphError> {
        METRICS
            .observe_gateway_request("network_subgraph_query", || async {
                let url = self.get_network_subgraph_query_url(api_key, gateway_url);

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

                if let Some(subgraphs_arr) = data.get("subgraphs").and_then(|s| s.as_array()) {
                    let total_count = subgraphs_arr.len();
                    let limit = if total_count <= 100 {
                        10
                    } else {
                        (total_count as f64).sqrt().ceil() as usize
                    };
                    let limited_subgraphs: Vec<serde_json::Value> =
                        subgraphs_arr.iter().take(limit).cloned().collect();
                    return Ok(json!({
                        "subgraphs": limited_subgraphs,
                        "total": total_count,
                        "returned": limited_subgraphs.len()
                    }));
                }
                Ok(data)
            })
            .await
    }

    pub(crate) async fn get_deployment_30day_query_counts_internal(
        &self,
        api_key: &str,
        gateway_url: &str,
        ipfs_hashes: &[String],
    ) -> Result<serde_json::Value, SubgraphError> {
        METRICS
            .observe_gateway_request("qos_oracle_query", || async {
                let url = format!(
                    "{}/{}/deployments/id/{}",
                    gateway_url, api_key, GATEWAY_QOS_ORACLE
                );

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| {
                        SubgraphError::InternalProcessingError(format!(
                            "Error calculating timestamp: {}",
                            e
                        ))
                    })?
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
                  first: 31
                ) {
                  query_count
                  dayStart
                }
              }
            }
            "#;

                let variables = serde_json::json!({
                    "deploymentIDs": ipfs_hashes,
                    "thirtyDaysAgoTimestamp": thirty_days_ago.to_string()
                });

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

                let deployments = data
                    .get("subgraphDeployments")
                    .and_then(|d| d.as_array())
                    .ok_or_else(|| {
                        SubgraphError::GraphQlError(
                            "Unexpected response format for deployments".to_string(),
                        )
                    })?;

                let mut query_counts_results: Vec<serde_json::Value> = Vec::new();
                for deployment_data in deployments {
                    let id = deployment_data
                        .get("id")
                        .and_then(|id_val| id_val.as_str())
                        .ok_or_else(|| {
                            SubgraphError::GraphQlError(
                                "Missing deployment ID in response".to_string(),
                            )
                        })?;
                    let data_points = deployment_data
                        .get("queryDailyDataPoints")
                        .and_then(|dp| dp.as_array())
                        .ok_or_else(|| {
                            SubgraphError::GraphQlError(
                                "Missing data points in response".to_string(),
                            )
                        })?;

                    let total_query_count: i64 = data_points
                        .iter()
                        .filter_map(|point| {
                            point
                                .get("query_count")
                                .and_then(|qc| qc.as_str())
                                .and_then(|s| s.parse::<i64>().ok())
                        })
                        .sum();

                    query_counts_results.push(json!({
                        "ipfs_hash": id,
                        "total_query_count": total_query_count,
                        "data_points_count": data_points.len()
                    }));
                }

                query_counts_results.sort_by(|a, b| {
                    let count_a = a["total_query_count"].as_i64().unwrap_or(0);
                    let count_b = b["total_query_count"].as_i64().unwrap_or(0);
                    count_b.cmp(&count_a) // Sort descending
                });

                Ok(json!({
                    "deployments": query_counts_results,
                    "total_deployments_processed": query_counts_results.len()
                }))
            })
            .await
    }
    pub(crate) fn _create_resource_text(&self, uri: &str, name: &str) -> Resource {
        RawResource::new(uri, name.to_string()).no_annotation()
    }
}
