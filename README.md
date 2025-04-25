# Subgraph MCP Server

A Model Context Protocol (MCP) server that allows LLMs to interact with The Graph protocol's subgraphs through GraphOps Gateway.

## Features

- Get the GraphQL schema for any subgraph deployment
- Execute GraphQL queries against any subgraph deployment
- Find the top subgraphs for a contract address on a specific chain
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

### Running as a standalone server

```bash
cargo run --release
```

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

The server exposes three main tools:

### 1. get_schema

Get the GraphQL schema for a subgraph deployment.

Parameters:

- `deployment_id`: The ID of the subgraph deployment

Example usage in Claude:

```
What's the schema for subgraph QmYourDeploymentIdHere?
```

### 2. execute_query

Execute a GraphQL query against a subgraph deployment.

Parameters:

- `deployment_id`: The ID of the subgraph deployment
- `query`: The GraphQL query string to execute
- `variables` (optional): JSON object with query variables

Example usage in Claude:

```
Run this GraphQL query against the subgraph QmYourDeploymentIdHere:

{
  tokens(first: 5) {
    id
    name
    symbol
  }
}
```

### 3. get_top_subgraphs

Get the top subgraph deployments for a given contract address and chain, ordered by query fees.

Parameters:

- `contract_address`: The contract address to find indexed subgraphs for
- `chain`: The chain name (e.g., 'mainnet' for Ethereum, 'arbitrum-one')

Example usage in Claude:

```
Get the top subgraphs for contract 0x1f98431c8ad98523631ae4a59f267346ea31f984 on chain mainnet
```

### Natural Language Queries

Once connected to Claude with this MCP server, you can ask natural language questions about subgraph data without writing GraphQL queries manually:

```
What are the pairs with maximum volume on 0xde0a7b5368f846f7d863d9f64949b688ad9818243151d488b4c6b206145b9ea3?

Which tokens have the highest market cap in this subgraph?

Show me the most recent 5 swaps for the USDC/ETH pair
```

Claude will automatically:

1. Fetch and understand the subgraph schema
2. Convert your question into an appropriate GraphQL query
3. Execute the query against the specified subgraph
4. Present the results in a readable format

## Prompts

The server provides three predefined prompts:

- `get_schema`: Get the schema for a subgraph deployment
- `execute_query`: Run a GraphQL query against a deployment
- `get_top_subgraphs`: Get top subgraphs for a contract on a specific chain

## Resources

The server exposes one resource:

- `graphql://subgraph`: Provides detailed information about how to use the Subgraph MCP, including workflow guidance for address lookups and contract queries

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT
