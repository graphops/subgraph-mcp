// SPDX-License-Identifier: Apache-2.0
pub const GATEWAY_URL: &str = "https://gateway.thegraph.com/api";
pub const GRAPH_NETWORK_SUBGRAPH_ARBITRUM: &str = "QmdKXcBUHR3UyURqVRQHu1oV6VUkBrhi2vNvMx3bNDnUCc";
pub const GATEWAY_QOS_ORACLE: &str = "QmZmb6z87QmqBLmkMhaqWy7h2GLF1ey8Qj7YSRuqSGMjeH";

pub const SERVER_INSTRUCTIONS: &str = "**Interacting with The Graph Subgraphs**
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
