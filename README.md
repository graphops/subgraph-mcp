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

## Requirements

- Rust (latest stable version recommended: 1.75+). \
  You can install it using the following command on macOS, Linux, or other Unix-like systems: \
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
  Follow the on-screen instructions. For other platforms, see the [official Rust installation guide](https://www.rust-lang.org/tools/install).
- A Gateway API key for The Graph Network.
- For Claude Desktop users: Latest Claude Desktop version

## Installation

```bash
# Clone the repository
git clone git@github.com:graphops/subgraph-mcp.git
cd subgraph-mcp

# Build the project
cargo build --release
```

## Usage

The server can be run in two modes: STDIO or SSE.

### STDIO Mode (e.g., for Claude Desktop)

This mode is typically used for direct integration with local MCP clients like Claude Desktop.

#### Configuration with Claude Desktop

Add the server to your `claude_desktop_config.json`. You need to replace `/path/to/subgraph-mcp` with the **absolute path** to the compiled binary you built in the Installation step.

**Finding the command path:**
After running `cargo build --release`, the executable will typically be located at `target/release/subgraph-mcp` inside your project directory (`subgraph-mcp`).

1. Navigate to your `subgraph-mcp` directory in the terminal.
2. Run `pwd` (print working directory) to get the full path to the `subgraph-mcp` directory.
3. Combine the output of `pwd` with `/target/release/subgraph-mcp`.

For example, if `pwd` outputs `/Users/user/subgraph-mcp`, the full command path would be `/Users/user/subgraph-mcp/target/release/subgraph-mcp`.

**Configuration Example:**

```json
{
  "mcpServers": {
    "subgraph": {
      "command": "/path/to/your/subgraph-mcp/target/release/subgraph-mcp", // <-- Replace this with the actual path!
      "env": {
        "GATEWAY_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

After adding the configuration, restart Claude Desktop.

**Important**: Claude Desktop may not automatically utilize server resources. To ensure proper functionality, manually add "Subgraph MCP LLM Guidence" resource to your chat context by clicking on the context menu and adding the resource `graphql://subgraph`.

## Available Tools

The server exposes the following tools:

- **`search_subgraphs_by_keyword`**: Search for subgraphs by keyword in their display names. Ordered by signal. Returns top 10 results if total results ≤ 100, or square root of total otherwise. (Corresponds to Step 2 of the workflow).
- **`get_deployment_30day_query_counts`**: Get the aggregate query count over the last 30 days for multiple subgraph deployments (using their IPFS hashes), sorted by query count. (Corresponds to Step 3 of the workflow).
- **`get_schema_by_deployment_id`**: Get the GraphQL schema for a specific subgraph deployment using its _deployment ID_ (e.g., `0x...`).
- **`get_schema_by_subgraph_id`**: Get the GraphQL schema for the _current_ deployment associated with a _subgraph ID_ (e.g., `5zvR82...`).
- **`get_schema_by_ipfs_hash`**: Get the GraphQL schema for a specific subgraph deployment using its manifest's _IPFS hash_ (e.g., `Qm...`).
- **`execute_query_by_deployment_id`**: Execute a GraphQL query against a specific, immutable subgraph deployment using its _deployment ID_ (e.g., `0x...`).
- **`execute_query_by_subgraph_id`**: Execute a GraphQL query against the _latest_ deployment associated with a _subgraph ID_ (e.g., `5zvR82...`).
- **`execute_query_by_ipfs_hash`**: Execute a GraphQL query against a specific, immutable subgraph deployment using its _IPFS hash_ (e.g., `Qm...`).
- **`get_top_subgraph_deployments`**: Get the top 3 subgraph deployments indexing a given contract address on a specific chain, ordered by query fees. (Used in the "Contract Address Lookup" special case).

**Key Identifier Types:**

- **Subgraph ID** (e.g., `5zvR82...`): Logical identifier for a subgraph. Use `execute_query_by_subgraph_id` or `get_schema_by_subgraph_id`.
- **Deployment ID** (e.g., `0x4d7c...`): Identifier for a specific, immutable deployment. Use `execute_query_by_deployment_id` or `get_schema_by_deployment_id`.
- **IPFS Hash** (e.g., `QmTZ8e...`): Identifier for the manifest of a specific, immutable deployment. Use `execute_query_by_ipfs_hash` or `get_schema_by_ipfs_hash`.

Example usage in Claude (or other MCP clients), keeping the workflow in mind:

```
User: Find subgraphs for Uniswap.

Assistant (after `search_subgraphs_by_keyword` and `get_deployment_30day_query_counts`):
I found several Uniswap subgraphs.
- Uniswap v3 on Ethereum is the most active (X queries last 30 days).
- Uniswap v2 on Ethereum (Y queries last 30 days).
- Uniswap v3 on Arbitrum (Z queries last 30 days).
Which specific version and network are you interested in?

Find the top subgraphs for contract 0x1f98431c8ad98523631ae4a59f267346ea31f984 on arbitrum-one
```

### Natural Language Queries (Following the Workflow)

Once connected to an LLM with this MCP server, you can ask natural language questions. The LLM should adhere to the `SERVER_INSTRUCTIONS` workflow:

```
What are the most active pools on Uniswap v3 on Ethereum?

(LLM internally performs search, query volume check, selects subgraph, potentially gets schema, then formulates and executes the query)
```

The LLM will automatically:

1.  Follow the **Server Instructions & Workflow** described above.
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

- `graphql://subgraph`: Provides the detailed `SERVER_INSTRUCTIONS` used by the LLM, including the workflow for different user goals (address lookup, finding subgraphs for a contract, querying by ID, getting schema) and important usage notes.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

Apache-2.0
