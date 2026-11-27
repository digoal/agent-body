//! `ext-gov-placement`: 工作区规范放置建议算子与非 `tmp` 临时脚本拦截 Hook

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use agent_body_sdk::{
    body_module_magic, AutonomyLevel, BodyExtension, CallerIdentity, ExecutorStartHook,
    ExtensionKind, ExtensionMeta, ExtensionRegistrar, FailPolicy, FunctionCallInfo, GuardVerdict,
    HookRegistration, KernelServices, MigrationScript, ProcRoutine, ProcSpec, ProcVolatility,
    SDK_API_VERSION,
};
use anyhow::bail;

body_module_magic!();

const MIGRATION_0_1_0: &str = include_str!("../sql/gov_placement--0.1.0.sql");

/// 放置治理配置（支持显式注入基准目录，0 硬编码宿主机路径）
#[derive(Debug, Clone, Default)]
pub struct PlacementConfig {
    pub base_dir: Option<PathBuf>,
}

/// 资产放置治理扩展入口
#[derive(Debug, Clone, Default)]
pub struct GovPlacementExtension {
    config: PlacementConfig,
}

impl GovPlacementExtension {
    pub fn new(config: PlacementConfig) -> Self {
        Self { config }
    }

    pub fn with_base_dir(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            config: PlacementConfig {
                base_dir: Some(base_dir.into()),
            },
        }
    }
}

impl BodyExtension for GovPlacementExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "gov_placement".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_gov_placement__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_0_1_0.to_string(),
        });
        reg.register_proc(Arc::new(PlacementSuggestProc {
            config: self.config.clone(),
        }));
        reg.register_hook(HookRegistration::ExecutorStart(Arc::new(
            TempScriptPlacementHook,
        )));
        Ok(())
    }
}

/// 动态推导工作区根路径：传入 `base_dir` -> `PlacementConfig.base_dir` -> `$AGENT_BODY_WORKSPACE_BASE` -> `$HOME/agent-body-workspace`
pub fn resolve_workspace_base(explicit_base: Option<&str>, config_base: Option<&Path>) -> PathBuf {
    if let Some(raw) = explicit_base {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Some(cfg) = config_base {
        if !cfg.as_os_str().is_empty() {
            return cfg.to_path_buf();
        }
    }
    if let Ok(env_base) = std::env::var("AGENT_BODY_WORKSPACE_BASE") {
        let trimmed = env_base.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    home_dir().join("agent-body-workspace")
}

fn home_dir() -> PathBuf {
    for key in ["HOME", "USERPROFILE"] {
        if let Ok(val) = std::env::var(key) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }
    }
    std::env::temp_dir()
}

/// 将任意文件名或项目名规范化为小写蛇形 (`snake_case`)，保留合法文件扩展名
pub fn to_snake_case_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "unnamed_item".to_string();
    }
    if let Some((stem, ext)) = split_safe_extension(trimmed) {
        let snake_stem = convert_segment_to_snake(stem);
        let clean_ext = ext.to_ascii_lowercase();
        if snake_stem.is_empty() {
            return format!("unnamed_item.{clean_ext}");
        }
        return format!("{snake_stem}.{clean_ext}");
    }
    let snake = convert_segment_to_snake(trimmed);
    if snake.is_empty() {
        "unnamed_item".to_string()
    } else {
        snake
    }
}

fn split_safe_extension(name: &str) -> Option<(&str, &str)> {
    let (stem, ext) = name.rsplit_once('.')?;
    let valid_ext = !stem.is_empty()
        && !ext.is_empty()
        && ext.len() <= 10
        && ext.chars().all(|c| c.is_ascii_alphanumeric());
    valid_ext.then_some((stem, ext))
}

fn convert_segment_to_snake(input: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = input.chars().collect();
    for (idx, &ch) in chars.iter().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                let prev_is_lower_or_digit = idx > 0
                    && (chars[idx - 1].is_ascii_lowercase() || chars[idx - 1].is_ascii_digit());
                let prev_upper_next_lower = idx > 0
                    && chars[idx - 1].is_ascii_uppercase()
                    && chars.get(idx + 1).is_some_and(|c| c.is_ascii_lowercase());
                if (prev_is_lower_or_digit || prev_upper_next_lower) && !out.ends_with('_') {
                    out.push('_');
                }
                out.push(ch.to_ascii_lowercase());
            } else {
                out.push(ch);
            }
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

/// 规范化放置类别 (`script` / `project` / `document` / `temp`)
fn normalize_category(raw: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "script" | "tool" | "shared_proj" => {
            Some(("script", "shared_proj", "rule_script"))
        }
        "project" | "work" | "repo" => {
            Some(("project", "work", "rule_project"))
        }
        "document" | "doc" | "note" | "markdown" => {
            Some(("document", "note", "rule_document"))
        }
        "temp" | "tmp" | "scratch" | "debug" => {
            Some(("temp", "tmp", "rule_temp"))
        }
        _ => None,
    }
}

/// `ext_placement_suggest` 算子实现（单点注册自动派生 MCP/CLI/REST/Parity）
pub struct PlacementSuggestProc {
    config: PlacementConfig,
}

impl ProcRoutine for PlacementSuggestProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "ext_placement_suggest",
            cli_path: &["gov", "placement", "suggest"],
            rest_route: ("POST", "/api/v1/gov/placement/suggest"),
            profiles: &["full", "gov"],
            volatility: ProcVolatility::Stable,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "category": {
                        "type": "string",
                        "enum": ["script", "project", "document", "temp"]
                    },
                    "name": { "type": "string" },
                    "base_dir": { "type": "string" },
                    "intent": { "type": "string" },
                    "target_path": { "type": "string" },
                    "warn_only": { "type": "boolean" }
                }
            }),
            provenance_rule: Some("PR-GOV-PLACEMENT"),
        }
    }

    fn invoke(
        &self,
        fcinfo: FunctionCallInfo,
        ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value> {
        let raw_cat = fcinfo
            .args
            .get("category")
            .or_else(|| fcinfo.args.get("object_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("script");

        let raw_name = fcinfo
            .args
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("缺少必填参数 'name'"))?;

        let (category, subdir, rule_id) = normalize_category(raw_cat).ok_or_else(|| {
            anyhow::anyhow!(
                "不支持的 category '{raw_cat}'，允许值: script / project / document / temp"
            )
        })?;

        let intent = fcinfo
            .args
            .get("intent")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (eff_cat, eff_subdir, eff_rule) = if is_temp_intent(intent) && category != "temp" {
            ("temp", "tmp", "rule_temp")
        } else {
            (category, subdir, rule_id)
        };

        let explicit_base = fcinfo.args.get("base_dir").and_then(|v| v.as_str());
        let workspace_base = resolve_workspace_base(explicit_base, self.config.base_dir.as_deref());
        let snake_name = to_snake_case_name(raw_name);
        let suggested_path = workspace_base.join(eff_subdir).join(&snake_name);

        let result = serde_json::json!({
            "category": eff_cat,
            "rule_id": eff_rule,
            "naming_convention": "snake_case",
            "sanitized_name": snake_name,
            "base_dir": workspace_base.to_string_lossy(),
            "suggested_path": suggested_path.to_string_lossy(),
            "conflict_detected": suggested_path.exists(),
        });

        ctx.table_am()
            .kv_put("gov_placement", "last_suggestion", &result)?;
        Ok(result)
    }
}

/// 非 `tmp` 目录临时脚本放置拦截 Hook (`ExecutorStartHook`)
pub struct TempScriptPlacementHook;

impl ExecutorStartHook for TempScriptPlacementHook {
    fn hook_id(&self) -> &'static str {
        "gov_placement:temp_script_guard"
    }

    fn priority(&self) -> i32 {
        50
    }

    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }

    fn pre_execute(
        &self,
        spec: &ProcSpec,
        args: &serde_json::Value,
        _caller: &CallerIdentity,
    ) -> anyhow::Result<GuardVerdict> {
        let Some(target_path) = extract_target_path(args) else {
            return Ok(GuardVerdict::Proceed);
        };

        if !looks_like_temp_script(args, target_path) {
            return Ok(GuardVerdict::Proceed);
        }

        if path_contains_tmp_dir(Path::new(target_path)) {
            return Ok(GuardVerdict::Proceed);
        }

        let warn_only = args
            .get("warn_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let msg = format!(
            "放置治理拦截 ({}): 检测到临时脚本目标路径 '{}' 未位于 tmp 目录，请迁移至 tmp/ 后重试",
            spec.proc_name, target_path
        );
        if warn_only {
            Ok(GuardVerdict::RequireConfirm { prompt: msg })
        } else {
            Ok(GuardVerdict::Block { reason: msg })
        }
    }
}

fn extract_target_path(args: &serde_json::Value) -> Option<&str> {
    ["target_path", "path", "dest_path", "output_path"]
        .iter()
        .find_map(|key| args.get(*key).and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn is_temp_intent(intent: &str) -> bool {
    let lower = intent.to_ascii_lowercase();
    intent.contains("临时")
        || intent.contains("一次性")
        || intent.contains("草稿")
        || lower.contains("temp")
        || lower.contains("tmp")
        || lower.contains("scratch")
        || lower.contains("debug")
}

fn looks_like_temp_script(args: &serde_json::Value, target_path: &str) -> bool {
    if args.get("is_temp").and_then(|v| v.as_bool()) == Some(true) {
        return true;
    }
    let cat = args
        .get("category")
        .or_else(|| args.get("object_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(cat.as_str(), "temp" | "tmp" | "scratch") {
        return true;
    }
    let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("");
    if is_temp_intent(intent) {
        return true;
    }
    let file_name = Path::new(target_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_script_ext = matches!(
        Path::new(&file_name).extension().and_then(|s| s.to_str()),
        Some("sh" | "py" | "rb" | "pl" | "js" | "ts" | "ps1")
    );
    let temp_prefix = file_name.starts_with("tmp_")
        || file_name.starts_with("temp_")
        || file_name.starts_with("scratch_")
        || file_name.starts_with("debug_");
    let explicit_name_temp = args
        .get("name")
        .and_then(|v| v.as_str())
        .map(|n| {
            let nl = n.to_ascii_lowercase();
            nl.starts_with("tmp_") || nl.starts_with("temp_") || nl.contains("scratch")
        })
        .unwrap_or(false);

    (is_script_ext && temp_prefix) || explicit_name_temp
}

fn path_contains_tmp_dir(path: &Path) -> bool {
    let mut comps: Vec<Component<'_>> = path.components().collect();
    // 排除最后一个文件名组件本身（如 work/tmp_a.py 不算在 tmp 目录下）
    if !comps.is_empty() && path.extension().is_some() {
        comps.pop();
    }
    comps.into_iter().any(|c| match c {
        Component::Normal(seg) => seg
            .to_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("tmp")),
        _ => false,
    })
}

/// 校验控制文件合法性的辅助函数（便于外部测试校验）
pub fn verify_non_empty_name(name: &str) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        bail!("name 不能为空");
    }
    Ok(())
}
