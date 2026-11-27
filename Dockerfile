# ==============================================================================
# Agent Body — 生产级多阶段容器镜像构建文件（支持旗舰版默认与 --target probe 零 C++ 探针镜像）
# ==============================================================================

# ------------------------------------------------------------------------------
# 阶段 1A: 极简政务云探针编译构建 (零 DuckDB / 零 C++ 构建)
# ------------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder-probe

WORKDIR /usr/src/agent-body
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY kernel ./kernel
COPY contrib ./contrib
COPY distributions ./distributions
COPY docs ./docs
COPY tests ./tests
COPY presentation ./presentation

RUN cargo build --release -p agent-body-probe -p agent-body-dba -p agent-body-kernel-minimal

# ------------------------------------------------------------------------------
# 可选目标 `--target probe`: 零 libstdc++6 极简探针运行镜像
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim AS probe

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    procps \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder-probe /usr/src/agent-body/target/release/agent-body-probe /usr/local/bin/agent-body-probe
COPY --from=builder-probe /usr/src/agent-body/target/release/agent-body-dba /usr/local/bin/agent-body-dba

ENTRYPOINT ["agent-body-probe"]
CMD ["self-check"]

# ------------------------------------------------------------------------------
# 阶段 1B: 旗舰版全功能编译构建 (SQLite & DuckDB bundled)
# ------------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    g++ \
    make \
    pkg-config \
    git \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/agent-body

COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY kernel ./kernel
COPY contrib ./contrib
COPY distributions ./distributions
COPY docs ./docs
COPY tests ./tests
COPY presentation ./presentation

RUN cargo build --release

# ------------------------------------------------------------------------------
# 阶段 2: 默认旗舰生产运行镜像 (Runtime Stage)
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libstdc++6 \
    ca-certificates \
    curl \
    procps \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/agent-body/target/release/agent-body /usr/local/bin/agent-body

VOLUME ["/root/.agent-body"]

EXPOSE 8765

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://127.0.0.1:8765/api/v1/health || exit 1

ENTRYPOINT ["agent-body"]
CMD ["daemon"]
