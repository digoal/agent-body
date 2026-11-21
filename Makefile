# ==============================================================================
# Agent Host Steward — 构建、测试与部署 Makefile
# ==============================================================================

SHELL := /bin/bash
TARGET := target/release/agent-steward
INSTALL_PATH ?= /usr/local/bin/agent-steward

.PHONY: all build check test preflight install restart clean docker-build lint-panic sync-mirror codegraph evolution-check

all: build

# 语法与警告零容忍检查
check:
	export PATH="$$HOME/.cargo/bin:$$PATH" && cargo check --tests

# 生产代码零 Panic 门禁检查（禁止在 #[cfg(test)] 前出现 .unwrap( / .expect(）
lint-panic:
	@python3 -c 'import glob, re, sys; hits=[(p,i,l.strip()) for p in sorted(glob.glob("src/**/*.rs", recursive=True)) for i,l in enumerate(open(p).read().split("#[cfg(test)]")[0].splitlines(),1) if re.search(r"\.(unwrap|expect)\(", l)]; print("\n".join(f"{p}:{i}: {l}" for p,i,l in hits) if hits else "✅ 生产代码零 unwrap/expect 检查通过"); sys.exit(1 if hits else 0)'

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

# 安全镜像同步至开源仓库（强制排除 target/ 避免占满根分区）
sync-mirror:
	rsync -a --delete --exclude 'target/' /root/new/work/agent-steward/ /root/new/shared_git/agent-steward/
	@echo "✅ 已安全镜像同步至 /root/new/shared_git/agent-steward/"

# 构建生产 Docker 容器镜像
docker-build:
	docker build -t agent-steward:latest .

# 初始化/刷新本项目 CodeGraph 符号地图（供 Agent 零重扫定位代码）
codegraph:
	codegraph init

# 确定性架构清单生成与文档数字防漂移门禁
evolution-check:
	python3 scripts/evolution_manifest.py --check
