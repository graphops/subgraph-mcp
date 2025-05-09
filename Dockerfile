# -----------------------------------------------------------------------------
# 1. Global Build Arguments & Metadata
# -----------------------------------------------------------------------------
ARG RUST_VERSION=1.86
ARG DEBIAN_VERSION=bookworm
ARG CHEF_IMAGE=lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION}-slim-${DEBIAN_VERSION}
ARG RUNTIME_BASE=gcr.io/distroless/cc-debian12:nonroot

ARG TINI_VERSION=v0.19.0
ARG TINI_SHA256=c5b0666b4cb676901f90dfcb37106783c5fe2077b04590973b885950611b30ee

ARG BUILD_VERSION="unknown"
ARG VCS_REF="unknown"
ARG BUILD_DATE="unknown"

# -----------------------------------------------------------------------------
# 2. Cargo Chef Stage - Prepare Dependency Recipe
# -----------------------------------------------------------------------------
FROM ${CHEF_IMAGE} AS chef
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# -----------------------------------------------------------------------------
# 3. Builder Stage - Compile Rust with Cached Dependencies
# -----------------------------------------------------------------------------
FROM ${CHEF_IMAGE} AS builder

ARG CARGO_BUILD_JOBS="default"
ARG CARGO_PROFILE=release
ENV RUSTFLAGS="-C opt-level=3 -C codegen-units=1"

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev binutils ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy and build dependencies first
COPY --from=chef /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Now build the actual application
COPY . .
RUN cargo build --profile ${CARGO_PROFILE} --locked --jobs "${CARGO_BUILD_JOBS}" \
    && strip target/${CARGO_PROFILE}/subgraph-mcp \
    && cp target/${CARGO_PROFILE}/subgraph-mcp /app/subgraph-mcp

# -----------------------------------------------------------------------------
# 4. Preparation Stage (Tini + Directory Setup)
# -----------------------------------------------------------------------------
FROM debian:${DEBIAN_VERSION}-slim AS preparation

ARG TINI_VERSION
ARG TINI_SHA256

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Download Tini and verify its checksum
RUN curl -fsSL -o /tini \
      https://github.com/krallin/tini/releases/download/${TINI_VERSION}/tini-static-amd64 \
    && echo "${TINI_SHA256}  /tini" | sha256sum --check \
    && chmod +x /tini

# Create log directory (owned by non-root UID 65532)
RUN mkdir -p /var/log/subgraph-mcp \
    && chown 65532:65532 /var/log/subgraph-mcp

# -----------------------------------------------------------------------------
# 5. Final Stage - Minimal Distroless Image
# -----------------------------------------------------------------------------
FROM ${RUNTIME_BASE}

COPY --from=preparation /tini /usr/local/bin/tini
COPY --from=builder /app/subgraph-mcp /usr/local/bin/subgraph-mcp
COPY --from=preparation /var/log/subgraph-mcp /var/log/subgraph-mcp

ENV TZ=UTC \
    LANG=C.UTF-8 \
    SSL_CERT_DIR=/etc/ssl/certs \
    RUST_LOG=info

WORKDIR /
STOPSIGNAL SIGTERM

# Distroless:nonroot automatically runs as UID 65532
USER nonroot

ENTRYPOINT ["/usr/local/bin/tini", "--", "/usr/local/bin/subgraph-mcp"]

# -----------------------------------------------------------------------------
# 6. OCI Labels for Metadata
# -----------------------------------------------------------------------------
LABEL org.opencontainers.image.version="${BUILD_VERSION}" \
      org.opencontainers.image.revision="${VCS_REF}" \
      org.opencontainers.image.created="${BUILD_DATE}" \
      org.opencontainers.image.source="https://github.com/graphops/subgraph-mcp" \
      org.opencontainers.image.description="Subgraph MCP Service" \
      org.opencontainers.image.vendor="GraphOps" \
      org.opencontainers.image.title="subgraph-mcp" \
      maintainer="GraphOps <support@graphops.xyz>" 