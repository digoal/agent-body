# ==============================================================================
# Agent Body — 生产级多阶段容器镜像构建文件
# ==============================================================================

# ------------------------------------------------------------------------------
# 阶段 1: 编译构建 (Builder Stage)
# ------------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

# 安装 C/C++ 依赖 (SQLite & DuckDB bundled 构建依赖)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    g++ \
    make \
    pkg-config \
    git \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/agent-body

# 复制源码树、文档、演说资产与构建脚本
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY docs ./docs
COPY tests ./tests
COPY presentation ./presentation

# 执行 Release 工业级全量优化编译
RUN cargo build --release

# ------------------------------------------------------------------------------
# 阶段 2: 极简生产运行镜像 (Runtime Stage)
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim

# 安装基本运行库 (C++ 标准库、curl 与 procfs 辅助工具)
RUN apt-get update && apt-get install -y --no-install-recommends \
    libstdc++6 \
    ca-certificates \
    curl \
    procps \
 && rm -rf /var/lib/apt/lists/*

# 从构建器阶段拷贝二进制产物
COPY --from=builder /usr/src/agent-body/target/release/agent-body /usr/local/bin/agent-body

# 默认数据存储卷
VOLUME ["/root/.agent-body"]

# 暴露 Web / REST API / Mesh 端口
EXPOSE 8765

# 容器健康检查
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://127.0.0.1:8765/api/v1/health || exit 1

# 默认启动守护进程
ENTRYPOINT ["agent-body"]
CMD ["daemon"]
