# ==============================================================================
# Agent Host Steward — 构建、测试与部署 Makefile
# ==============================================================================

SHELL := /bin/bash
TARGET := target/release/agent-steward
INSTALL_PATH ?= /usr/local/bin/agent-steward

.PHONY: all build check test preflight install restart clean docker-build

all: build

# 语法与警告零容忍检查
check:
	export PATH="$$HOME/.cargo/bin:$$PATH" && cargo check --tests

# 编译优化发布版二进制
build:
	export PATH="$$HOME/.cargo/bin:$$PATH" && cargo build --release

# 运行全量单元与集成测试套件
test:
	export PATH="$$HOME/.cargo/bin:$$PATH" && cargo test

# 运行宿主环境运行依赖预检
preflight:
	export PATH="$$HOME/.cargo/bin:$$PATH" && cargo run -- preflight

# 安装到全局 PATH
install: build
	install -m 755 $(TARGET) $(INSTALL_PATH)
	@echo "✅ agent-steward 已成功安装至 $(INSTALL_PATH)"

# 重启本地 systemd 守护服务
restart: install
	systemctl restart agent-steward.service
	@echo "✅ agent-steward.service 守护服务已平滑重启"

# 清理构建缓存产物
clean:
	cargo clean

# 构建生产 Docker 容器镜像
docker-build:
	docker build -t agent-steward:latest .
