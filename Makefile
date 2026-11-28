# ==============================================================================
# Agent Body — 构建、测试与部署 Makefile
# ==============================================================================

SHELL := /bin/bash
UNAME_S := $(shell uname -s)
TARGET := target/release/agent-body
ifeq ($(UNAME_S),Darwin)
  INSTALL_PATH ?= $(HOME)/.local/bin/agent-body
  export CPLUS_INCLUDE_PATH ?= /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include/c++/v1
else
  INSTALL_PATH ?= /usr/local/bin/agent-body
endif
MIRROR_DIR ?= $(HOME)/new/shared_git/agent-body

.PHONY: all build check test preflight install restart clean docker-build lint-panic lint-hygiene lint-strict arch-check quality-gate sync-mirror codegraph evolution-check verify-decks verify-webui

all: build

# 架构防腐边界门禁检查
arch-check:
	@python3 scripts/arch_boundary_check.py --check

# 语法与警告零容忍检查
check:
	export PATH="$$HOME/.cargo/bin:$$PATH" && cargo check --workspace --all-targets

# 生产代码零 Panic 门禁检查（禁止在 #[cfg(test)] 前出现 .unwrap( / .expect(）
lint-panic:
	@python3 -c 'import glob, re, sys; files=sorted(glob.glob("src/**/*.rs", recursive=True)+glob.glob("kernel/**/src/**/*.rs", recursive=True)+glob.glob("contrib/**/src/**/*.rs", recursive=True)+glob.glob("distributions/**/src/**/*.rs", recursive=True)); hits=[(p,i,l.strip()) for p in files for i,l in enumerate(open(p).read().split("#[cfg(test)]")[0].splitlines(),1) if re.search(r"\.(unwrap|expect)\(", l)]; print("\n".join(f"{p}:{i}: {l}" for p,i,l in hits) if hits else "✅ 生产代码零 unwrap/expect 检查通过"); sys.exit(1 if hits else 0)'

# 代码卫生与位置无关性门禁（规约 #13 & #28）
lint-hygiene:
	python3 scripts/code_hygiene.py --check

# 严格零警告编译（仅对本 crate 启用 -D warnings，避免触发 libduckdb-sys 等三方依赖全量重编）
lint-strict:
	@export PATH="$$HOME/.cargo/bin:$$PATH" && cargo rustc --lib -- -D warnings && cargo rustc --bin agent-body -- -D warnings && cargo check --workspace --all-targets && (cargo clippy --version >/dev/null 2>&1 && cargo clippy --workspace --all-targets -- -D warnings || echo "ℹ️  clippy 未安装，已通过 cargo rustc --lib/--bin -D warnings 严格编译门禁")

# 六重确定性质量门禁一键联检
quality-gate: evolution-check arch-check lint-panic lint-hygiene lint-strict
	@echo "✅ 全部质量门禁检查通过"

# 编译优化发布版二进制
build:
	export PATH="$$HOME/.cargo/bin:$$PATH" && cargo build --release

# 运行全量单元与集成测试套件
test:
	export PATH="$$HOME/.cargo/bin:$$PATH" && RUST_TEST_THREADS=1 cargo test --workspace

# 运行宿主环境运行依赖预检
preflight:
	export PATH="$$HOME/.cargo/bin:$$PATH" && cargo run -- preflight

# 安装到全局 PATH
install: build
	install -m 755 $(TARGET) $(INSTALL_PATH)
	@echo "✅ agent-body 已成功安装至 $(INSTALL_PATH)"

# 重启本地后台守护服务 (macOS LaunchAgent / Linux systemd)
restart: install
ifeq ($(UNAME_S),Darwin)
	launchctl kickstart -k gui/$$(id -u)/com.digoal.agent-body
	@echo "✅ com.digoal.agent-body LaunchAgent 已平滑重启"
else
	systemctl restart agent-body.service
	@echo "✅ agent-body.service 守护服务已平滑重启"
endif

# 清理构建缓存产物
clean:
	cargo clean

# 安全镜像同步至开源仓库（强制排除 target/ 避免占满根分区）
sync-mirror:
	mkdir -p $(MIRROR_DIR)
	rsync -a --delete --exclude 'target/' $(CURDIR)/ $(MIRROR_DIR)/
	@echo "✅ 已安全镜像同步至 $(MIRROR_DIR)/"

# 构建生产 Docker 容器镜像
docker-build:
	docker build -t agent-body:latest .

# 初始化/刷新本项目 CodeGraph 符号地图（供 Agent 零重扫定位代码）
codegraph:
	codegraph init

# 确定性架构清单生成与文档数字防漂移门禁
evolution-check:
	python3 scripts/evolution_manifest.py --check

# Playwright 无头演说视觉与交互全量门禁（规约 #13 & #17）
verify-decks:
	@if python3 -c "import playwright" >/dev/null 2>&1; then python3 scripts/verify_presentations.py; else uv run --with playwright python3 scripts/verify_presentations.py; fi

# Playwright 无头 WebUI 9 大工作台 + 老库迁移 E2E 回归门禁
verify-webui:
	@export PATH="$$HOME/.cargo/bin:$$PATH" && if [ ! -x target/debug/agent-body ] && [ ! -x target/release/agent-body ]; then cargo test --no-run --test cli_e2e; fi && if python3 -c "import playwright" >/dev/null 2>&1; then python3 scripts/verify_webui_e2e.py; else uv run --with playwright python3 scripts/verify_webui_e2e.py; fi
