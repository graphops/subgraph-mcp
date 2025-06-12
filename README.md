# Subgraph MCP Server

A Model Context Protocol (MCP) server that allows LLMs to interact with Subgraphs available on The Graph Network.

## Features

- Get the GraphQL schema for any subgraph/deployment
- Execute GraphQL queries against any subgraph/deployment
- Find the top subgraph deployments for a contract address on a specific chain
- Search for subgraphs by keyword
- Get 30-day query volume for subgraph deployments
- Supports MCP resources, tools, and prompts
- Can run in STDIO mode or as an SSE (Server-Sent Events) server

## Usage

The `subgraph-mcp` server offers two primary ways to interact with The Graph Network:

1.  **Connecting to the Remote Hosted MCP Service (Recommended for most users)**
2.  **Building and Running the Server Locally**

### Connecting to the Remote Hosted MCP Service

This is the quickest way to get started. You can configure your MCP client (e.g., Claude Desktop) to connect to our hosted `subgraph-mcp` service.

#### Requirements

- A Gateway API key for The Graph Network.

#### Configuration

Add the following to your configuration file of your client (e.g.,`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "subgraph-mcp": {
      "command": "npx",
      "args": [
        "mcp-remote",
        "--header",
        "Authorization:${AUTH_HEADER}",
        "https://subgraphs.mcp.thegraph.com/sse"
      ],
      "env": {
        "AUTH_HEADER": "Bearer YOUR_GATEWAY_API_KEY" // <-- Replace with your actual key
      }
    }
  }
}
```

Replace `YOUR_GATEWAY_API_KEY` with your actual Gateway API key. After adding the configuration, restart your MCP client.

Once configured, you can skip to the "Available Tools" or "Natural Language Queries" sections to learn how to interact with the service.

### Building and Running the Server Locally

This option is for users who prefer to build, run, and potentially modify the server on their own machine.

#### Requirements (for Local Execution)

- Rust (latest stable version recommended: 1.75+). \
  You can install it using the following command on macOS, Linux, or other Unix-like systems: \
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
  Follow the on-screen instructions. For other platforms, see the [official Rust installation guide](https://www.rust-lang.org/tools/install).
- A Gateway API key for The Graph Network.

#### Installation (for Local Execution)

```bash
# Clone the repository
git clone git@github.com:graphops/subgraph-mcp.git
cd subgraph-mcp

# Build the project
cargo build --release
```

#### Configuration (for Local Execution)

Add the following to your configuration file of your client (e.g.,`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "subgraph-mcp": {
      "command": "/path/to/your/subgraph-mcp/target/release/subgraph-mcp", // <-- Replace this with the actual path!
      "env": {
        "GATEWAY_API_KEY": "YOUR_GATEWAY_API_KEY" // <-- Replace with your actual key
      }
    }
  }
}
```

You need to replace `/path/to/subgraph-mcp` with the **absolute path** to the compiled binary you built in the Installation step.

**Finding the command path:**

After running `cargo build --release`, the executable will typically be located at `target/release/subgraph-mcp` inside your project directory (`subgraph-mcp`).

1. Navigate to your `subgraph-mcp` directory in the terminal.
2. Run `pwd` (print working directory) to get the full path to the `subgraph-mcp` directory.
3. Combine the output of `pwd` with `/target/release/subgraph-mcp`.

For example, if `pwd` outputs `/Users/user/subgraph-mcp`, the full command path would be `/Users/user/subgraph-mcp/target/release/subgraph-mcp`.

After adding the configuration, restart Claude Desktop.

**Important**: Claude Desktop may not automatically utilize server resources. To ensure proper functionality, manually add `Subgraph Server Instructions` resource to your chat context by clicking on the context menu and adding the resource.

## Available Tools

The server exposes the following tools:

- **`search_subgraphs_by_keyword`**: Search for subgraphs by keyword in their display names. Ordered by signal. Returns top 10 results if total results ≤ 100, or square root of total otherwise.
- **`get_deployment_30day_query_counts`**: Get the aggregate query count over the last 30 days for multiple subgraph deployments (using their IPFS hashes), sorted by query count.
- **`get_schema_by_deployment_id`**: Get the GraphQL schema for a specific subgraph deployment using its _deployment ID_ (e.g., `0x...`).
- **`get_schema_by_subgraph_id`**: Get the GraphQL schema for the _current_ deployment associated with a _subgraph ID_ (e.g., `5zvR82...`).
- **`get_schema_by_ipfs_hash`**: Get the GraphQL schema for a specific subgraph deployment using its manifest's _IPFS hash_ (e.g., `Qm...`).
- **`execute_query_by_deployment_id`**: Execute a GraphQL query against a specific, immutable subgraph deployment using its _deployment ID_ (e.g., `0x...`).
- **`execute_query_by_subgraph_id`**: Execute a GraphQL query against the _latest_ deployment associated with a _subgraph ID_ (e.g., `5zvR82...`).
- **`execute_query_by_ipfs_hash`**: Execute a GraphQL query against a specific, immutable subgraph deployment using its _IPFS hash_ (e.g., `Qm...`).
- **`get_top_subgraph_deployments`**: Get the top 3 subgraph deployments indexing a given contract address on a specific chain, ordered by query fees.

### Natural Language Queries

Once connected to an LLM with this MCP server, you can ask natural language questions.

**Important**: Claude Desktop may not automatically utilize server resources. To ensure proper functionality, manually add `Subgraph Server Instructions` resource to your chat context by clicking on the context menu and adding the resource.

Example usage in Claude (or other MCP clients), assuming you added `Subgraph Server Instructions` to your prompt:

```
User: List the 20 most recently registered .eth names.

Assistant (after `search_subgraphs_by_keyword`, `get_deployment_30day_query_counts` and other tool usage):
Perfect! I've successfully retrieved the 20 most recently registered .eth names using the ENS subgraph, which has 68.1 million queries in the last 30 days, making it the most active and reliable source for ENS data.
Here are the 20 most recently registered .eth names:

...

```

The LLM will automatically:

1.  Follow the **Subgraph Server Instructions**.
2.  Use `search_subgraphs_by_keyword` to find candidate subgraphs.
3.  Use `get_deployment_30day_query_counts` to verify activity and aid selection.
4.  Use `get_top_subgraph_deployments` if a contract address is provided.
5.  Fetch and understand the subgraph schema using the appropriate `get_schema_by_*` tool.
6.  Convert your question into an appropriate GraphQL query.
7.  Execute the query using the correct `execute_query_by_*` tool based on the identifier type and confirmed active deployment.
8.  Present the results in a readable format.

## Prompts

The server provides predefined prompts for most tools (as discoverable via MCP's `list_prompts`):

- `get_schema_by_deployment_id`: Get the schema for a deployment ID.
- `get_schema_by_subgraph_id`: Get the schema for a subgraph ID.
- `get_schema_by_ipfs_hash`: Get the schema for an IPFS hash.
- `execute_query_by_deployment_id`: Run a GraphQL query against a deployment ID.
- `execute_query_by_subgraph_id`: Run a GraphQL query against a subgraph ID.
- `execute_query_by_ipfs_hash`: Run a GraphQL query against an IPFS hash.
- `get_top_subgraph_deployments`: Get top subgraphs for a contract on a specific chain.

## Resources

The server exposes one resource:

- `graphql://subgraph`: Provides the detailed `Subgraph Server Instructions` used by the LLM, including the workflow for different user goals (address lookup, finding subgraphs for a contract, querying by ID, getting schema) and important usage notes.

Below is a reference for the `Subgraph Server Instructions`:

```
**Interacting with The Graph Subgraphs**
**IMPORTANT: ALWAYS verify query volumes using `get_deployment_30day_query_counts` for any potential subgraph candidate *before* selecting or querying it. This step is NON-OPTIONAL. Failure to do so may result in using outdated or irrelevant data.**
**Follow this sequence strictly:**
1.  **Analyze User Request:**
    *   Identify the **protocol name** (e.g., "Uniswap", "Aave", "ENS").
    *   Note any specific **version** or **blockchain network** mentioned by the user.
    *   Determine the **goal**: Query data? Get schema?
2.  **Initial Search & Preliminary Analysis:**
    *   Use `search_subgraphs_by_keyword` with the most generic term for the protocol (e.g., if "Uniswap v3 on Ethereum", initially search only for "Uniswap").
    *   Examine `displayName` and other metadata in the search results for version and network information.
3.  **Mandatory Query Volume Check & Clarification (If Needed):**
    *   **ALWAYS** extract the IPFS hashes (`ipfsHash`) for all potentially relevant subgraphs identified in Step 2.
    *   **ALWAYS** use `get_deployment_30day_query_counts` for these IPFS hashes.
    *   **If Ambiguous (Multiple Versions/Chains with significant volume):**
        *   Present a summary to the user, **including the 30-day query counts for each option**. For example: "I found several Uniswap subgraphs. Uniswap v3 on Ethereum is the most active (X queries last 30 days). I also see Uniswap v2 on Ethereum (Y queries) and Uniswap v3 on Arbitrum (Z queries). Which specific version and network are you interested in?"
    *   **If Still Unclear (Information Missing and Not Inferable even with query volumes):**
        *   If version/chain information is genuinely missing from search results and user input, and query volumes don't offer a clear path (e.g. all relevant subgraphs have very low or no volume), ask for clarification directly. Example: "I found several subgraphs for 'ExampleProtocol', but none have significant query activity. Could you please specify the version and blockchain network you're interested in?"
    *   **Do NOT proceed to Step 4 without completing this query volume verification.**
4.  **Select Final Subgraph (Post Query Volume Check & Clarification):**
    *   After the keyword search, mandatory query volume check, and any necessary clarification, you should have a clear target protocol, version, and network.
    *   Identify all candidate subgraphs from your Step 2 `search_subgraphs_by_keyword` results that match these clarified criteria.
    *   **If there is more than one such matching subgraph:**
        *   You should have already fetched their query counts in Step 3.
        *   **Select the subgraph with the highest `total_query_count`** among them.
    *   **If only one subgraph precisely matches the criteria**, that is your selected subgraph.
    *   When presenting your chosen subgraph or asking for final confirmation before querying, **ALWAYS state its 30-day query volume** to demonstrate this check has been performed. For example: "I've selected the 'Uniswap v3 Ethereum' subgraph, which has X queries in the last 30 days. Shall I proceed to get its schema?"
    *   If the selected subgraph's query count is very low (and this wasn't already discussed during clarification), briefly inform the user.
5.  **Execute Action Using the Identified Subgraph:**
    *   **Identify the ID Type:** (Subgraph ID, Deployment ID, or IPFS Hash - note that `search_subgraphs_by_keyword` returns `id` for Subgraph ID and `ipfsHash` for current deployment's IPFS hash).
    *   **Determine the Correct Tool based on Goal & ID Type:**
        *   **Goal: Query Data**
            *   Subgraph ID (`id` from search) → `execute_query_by_subgraph_id`
            *   Deployment ID (0x...) → `execute_query_by_deployment_id`
            *   IPFS Hash (`ipfsHash` from search) → `execute_query_by_ipfs_hash`
        *   **Goal: Get Schema**
            *   Subgraph ID → `get_schema_by_subgraph_id`
            *   Deployment ID → `get_schema_by_deployment_id`
            *   IPFS Hash → `get_schema_by_ipfs_hash`
    *   **Write Clean GraphQL Queries:** Simple structure, omit 'variables' if unused, include only essential fields.
**Special Case: Contract Address Lookup**
*   ONLY when a user explicitly provides a **contract address** (0x...) AND asks for subgraphs related to it:
    *   Identify the blockchain network for the address (ask user if unclear).
    *   Use `get_top_subgraph_deployments` with the provided contract address and chain name.
    *   Process and use the resulting IPFS hashes as needed. **Crucially, before using any of these IPFS hashes for querying, first use `get_deployment_30day_query_counts` with their IPFS hashes to verify recent activity.**
**ID Type Reference:**
*   **Subgraph ID**: Typically starts with digits and letters (e.g., 5zvR82...)
*   **Contract Address**: A shorter hexadecimal string, typically 42 characters long including the "0x" prefix (e.g., 0x1a3c9b1d2f0529d97f2afc5136cc23e58f1fd35b).
*   **Deployment ID**: A longer hexadecimal string, typically 66 characters long including the "0x" prefix (e.g., 0xc5b4d246cf890b0b468e005224622d4c85a8b723cc0b8fa7db6d1a93ddd2e5de). Use length to distinguish from a Contract Address.
*   **IPFS Hash**: Typically starts with Qm... For the purpose of `get_deployment_30day_query_counts`, use the \'IPFS Hash\' (Qm...).
*   Note `search_subgraphs_by_keyword` and `get_top_subgraph_deployments` returns `ipfsHash`.

**Best Practices:**
*   When using GraphQL, if unsure about the structure, first get the schema to understand available entities and fields.
*   Create focused queries that only request necessary fields.
*   For paginated data, use appropriate limit parameters.
*   Use variables for dynamic values in queries.
```

## Monitoring

The server exposes Prometheus metrics for monitoring its performance and behavior.

### Metrics Endpoint

When running in SSE mode, a metrics server is started on a separate port.

- **Endpoint**: `/metrics`
- **Default Port**: `9091`

You can configure the port and host for the metrics server using the `METRICS_PORT` and `METRICS_HOST` environment variables.

### Exposed Metrics

The following application-specific metrics are exposed:

- `mcp_tool_calls_total{tool_name, status}`: A counter for the number of MCP tool calls.
  - `tool_name`: The name of the MCP tool being called (e.g., `get_schema_by_deployment_id`).
  - `status`: The result of the call (`success` or `error`).
- `mcp_tool_call_duration_seconds{tool_name}`: A histogram of the duration of MCP tool calls.
- `gateway_requests_total{endpoint_type, status}`: A counter for outgoing requests to The Graph's Gateway.
  - `endpoint_type`: The type of query or endpoint being hit (e.g., `get_schema_by_deployment_id`, `subgraphs/id`).
  - `status`: The result of the request (`success` or `error`).
- `gateway_request_duration_seconds{endpoint_type}`: A histogram of the duration of Gateway requests.

Additionally, the `axum-prometheus` library provides standard HTTP request metrics for the metrics server itself (prefixed with `http_`).

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

Apache-2.0
