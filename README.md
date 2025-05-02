# Subgraph MCP Server

A Model Context Protocol (MCP) server that allows LLMs to interact with The Graph protocol's subgraphs through GraphOps Gateway.

## Features

- Get the GraphQL schema for any subgraph/deployment
- Execute GraphQL queries against any subgraph/deployment
- Find the top subgraph deployments for a contract address on a specific chain
- Supports MCP resources, tools, and prompts

## Requirements

- Rust (latest stable version recommended: 1.75+)
- A GraphOps API key
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

### Integration with MCP clients

#### Configuration with Claude Desktop

Add the server to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "subgraph": {
      "command": "/path/to/subgraph-mcp",
      "env": {
        "GRAPHOPS_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

After adding the configuration, restart Claude Desktop.

**Important**: Claude Desktop may not automatically utilize server resources. To ensure proper functionality, manually add "The Graph" resource to your chat context by clicking on the context menu and adding the resource `graphql://subgraph`.

## Available Tools

The server exposes the following tools:

- **`get_schema_by_deployment_id`**: Get the GraphQL schema for a specific subgraph deployment using its _deployment ID_ (e.g., `0x...`).
- **`get_schema_by_subgraph_id`**: Get the GraphQL schema for the _current_ deployment associated with a _subgraph ID_ (e.g., `5zvR82...`).
- **`get_schema_by_ipfs_hash`**: Get the GraphQL schema for a specific subgraph deployment using its manifest's _IPFS hash_ (e.g., `Qm...`).
- **`execute_query_by_deployment_id`**: Execute a GraphQL query against a specific, immutable subgraph deployment using its _deployment ID_ (e.g., `0x...`) or _IPFS hash_ (e.g., `Qm...`).
- **`execute_query_by_subgraph_id`**: Execute a GraphQL query against the _latest_ deployment associated with a _subgraph ID_ (e.g., `5zvR82...`).
- **`get_top_subgraph_deployments`**: Get the top 3 subgraph deployments indexing a given contract address on a specific chain, ordered by query fees.

**Key Identifier Types:**

- **Subgraph ID** (e.g., `5zvR82...`): Logical identifier for a subgraph. Use `execute_query_by_subgraph_id` or `get_schema_by_subgraph_id`.
- **Deployment ID** (e.g., `0x4d7c...`): Identifier for a specific, immutable deployment. Use `execute_query_by_deployment_id` or `get_schema_by_deployment_id`.
- **IPFS Hash** (e.g., `QmTZ8e...`): Identifier for the manifest of a specific, immutable deployment. Use `execute_query_by_deployment_id` (the gateway treats it like a deployment ID for querying) or `get_schema_by_ipfs_hash`.

Example usage in Claude:

```
Get the schema for subgraph deployment 0xYourDeploymentIdHere

Run this query against subgraph 5zvR82YourSubgraphIdHere: { users(first: 1) { id } }

Find the top subgraphs for contract 0x1f98431c8ad98523631ae4a59f267346ea31f984 on arbitrum-one
```

### Natural Language Queries

Once connected to Claude with this MCP server, you can ask natural language questions about subgraph data without writing GraphQL queries manually:

```
What are the pairs with maximum volume on 0xde0a7b5368f846f7d863d9f64949b688ad9818243151d488b4c6b206145b9ea3?

Which tokens have the highest market cap in this subgraph?

Show me the most recent 5 swaps for the USDC/ETH pair
```

Claude will automatically (given that you added The Graph resource to the session context):

1.  Identify the user's goal (lookup, find subgraphs, query, get schema).
2.  Use `get_top_subgraph_deployments` if necessary to find relevant deployment IDs.
3.  Fetch and understand the subgraph schema using the appropriate `get_schema_by_*` tool.
4.  Convert your question into an appropriate GraphQL query.
5.  Execute the query using the correct `execute_query_by_*` tool based on the identifier type.
6.  Present the results in a readable format.

## Prompts

The server provides predefined prompts for each tool:

- `get_schema_by_deployment_id`: Get the schema for a deployment ID.
- `get_schema_by_subgraph_id`: Get the schema for a subgraph ID.
- `get_schema_by_ipfs_hash`: Get the schema for an IPFS hash.
- `execute_query_by_deployment_id`: Run a GraphQL query against a deployment ID/hash.
- `execute_query_by_subgraph_id`: Run a GraphQL query against a subgraph ID.
- `get_top_subgraph_deployments`: Get top subgraphs for a contract on a specific chain.

## Resources

The server exposes one resource:

- `graphql://subgraph`: Provides the detailed `SERVER_INSTRUCTIONS` used by the LLM, including the workflow for different user goals (address lookup, finding subgraphs for a contract, querying by ID, getting schema) and important usage notes.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

Apache-2.0
