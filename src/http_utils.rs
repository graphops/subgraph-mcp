// SPDX-License-Identifier: Apache-2.0
use http::HeaderMap;
use rmcp::{
    handler::server::tool::{FromToolCallContextPart, ToolCallContext},
    Error as McpError,
};

#[derive(Debug)]
pub struct HttpRequestHeaders(pub Option<HeaderMap>);

impl<'a, S> FromToolCallContextPart<'a, S> for HttpRequestHeaders {
    fn from_tool_call_context_part(
        context: ToolCallContext<'a, S>,
    ) -> Result<(Self, ToolCallContext<'a, S>), McpError> {
        let headers_opt = context
            .request_context()
            .extensions
            .get::<HeaderMap>()
            .cloned();
        Ok((HttpRequestHeaders(headers_opt), context))
    }
}
