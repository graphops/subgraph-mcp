// SPDX-License-Identifier: Apache-2.0
use once_cell::sync::Lazy;
use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, family::Family, histogram::Histogram},
    registry::Registry,
};

const DEFAULT_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

#[derive(Clone, Hash, PartialEq, Eq, Debug, EncodeLabelSet)]
pub struct ToolCallLabels {
    pub tool_name: String,
    pub status: String,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug, EncodeLabelSet)]
pub struct ToolCallDurationLabels {
    pub tool_name: String,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug, EncodeLabelSet)]
pub struct GatewayRequestLabels {
    pub endpoint_type: String,
    pub status: String,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug, EncodeLabelSet)]
pub struct GatewayRequestDurationLabels {
    pub endpoint_type: String,
}

#[derive(Clone)]
pub struct Metrics {
    pub mcp_tool_calls_total: Family<ToolCallLabels, Counter>,
    pub mcp_tool_call_duration_seconds: Family<ToolCallDurationLabels, Histogram>,
    pub gateway_requests_total: Family<GatewayRequestLabels, Counter>,
    pub gateway_request_duration_seconds: Family<GatewayRequestDurationLabels, Histogram>,
}

impl Metrics {
    fn new() -> Self {
        Self {
            mcp_tool_calls_total: Family::<ToolCallLabels, Counter>::default(),
            mcp_tool_call_duration_seconds:
                Family::<ToolCallDurationLabels, Histogram>::new_with_constructor(|| {
                    Histogram::new(DEFAULT_BUCKETS)
                }),
            gateway_requests_total: Family::<GatewayRequestLabels, Counter>::default(),
            gateway_request_duration_seconds:
                Family::<GatewayRequestDurationLabels, Histogram>::new_with_constructor(|| {
                    Histogram::new(DEFAULT_BUCKETS)
                }),
        }
    }

    pub fn register(&self, registry: &mut Registry) {
        registry.register(
            "mcp_tool_calls",
            "Total number of MCP tool calls",
            self.mcp_tool_calls_total.clone(),
        );

        registry.register(
            "mcp_tool_call_duration_seconds",
            "Duration of MCP tool calls in seconds",
            self.mcp_tool_call_duration_seconds.clone(),
        );

        registry.register(
            "gateway_requests",
            "Total number of requests to the Graph Gateway",
            self.gateway_requests_total.clone(),
        );

        registry.register(
            "gateway_request_duration_seconds",
            "Duration of Graph Gateway requests in seconds",
            self.gateway_request_duration_seconds.clone(),
        );
    }

    pub async fn observe_tool_call<F, Fut, T>(&self, tool_name: &str, f: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
        T: IsSuccess,
    {
        let start_time = std::time::Instant::now();
        let result = f().await;
        let duration = start_time.elapsed();

        let status = if result.is_success() {
            "success"
        } else {
            "error"
        };

        self.mcp_tool_calls_total
            .get_or_create(&ToolCallLabels {
                tool_name: tool_name.to_string(),
                status: status.to_string(),
            })
            .inc();

        self.mcp_tool_call_duration_seconds
            .get_or_create(&ToolCallDurationLabels {
                tool_name: tool_name.to_string(),
            })
            .observe(duration.as_secs_f64());

        result
    }

    pub async fn observe_gateway_request<F, Fut, T>(&self, endpoint_type: &str, f: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
        T: IsSuccess,
    {
        let start_time = std::time::Instant::now();
        let result = f().await;
        let duration = start_time.elapsed();

        let status = if result.is_success() {
            "success"
        } else {
            "error"
        };

        self.gateway_requests_total
            .get_or_create(&GatewayRequestLabels {
                endpoint_type: endpoint_type.to_string(),
                status: status.to_string(),
            })
            .inc();

        self.gateway_request_duration_seconds
            .get_or_create(&GatewayRequestDurationLabels {
                endpoint_type: endpoint_type.to_string(),
            })
            .observe(duration.as_secs_f64());

        result
    }
}

pub trait IsSuccess {
    fn is_success(&self) -> bool;
}

impl<T, E> IsSuccess for Result<T, E> {
    fn is_success(&self) -> bool {
        self.is_ok()
    }
}

pub static METRICS: Lazy<Metrics> = Lazy::new(Metrics::new);
