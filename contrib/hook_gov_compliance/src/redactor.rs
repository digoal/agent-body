//! 政务云合规脱敏引擎：Java `-D` 精准掩码、URI 凭据、PEM 私钥、Bearer、自动化命令与国标 PII

use std::collections::BTreeMap;

use anyhow::Context;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 合规脱敏与标签注入配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovComplianceConfig {
    /// 注入到 `ProbeContext.params`（`host_label:<k>`）的宿主机维度标签
    pub host_labels: BTreeMap<String, String>,
    /// 额外自定义敏感凭据正则规则
    pub custom_patterns: Vec<String>,
    /// 是否启用 18 位身份证号与 11 位手机号脱敏
    pub enable_pii_masking: bool,
}

impl Default for GovComplianceConfig {
    fn default() -> Self {
        Self {
            host_labels: BTreeMap::new(),
            custom_patterns: Vec::new(),
            enable_pii_masking: true,
        }
    }
}

/// 确定性正则脱敏器
pub struct GovComplianceRedactor {
    java_flag_re: Regex,
    uri_cred_re: Regex,
    pem_key_re: Regex,
    bearer_re: Regex,
    env_secret_re: Regex,
    id_card_re: Regex,
    mobile_re: Regex,
    custom_res: Vec<Regex>,
    enable_pii_masking: bool,
}

impl GovComplianceRedactor {
    pub fn from_config(cfg: &GovComplianceConfig) -> anyhow::Result<Self> {
        let mut custom_res = Vec::with_capacity(cfg.custom_patterns.len());
        for pat in &cfg.custom_patterns {
            let compiled = Regex::new(pat)
                .with_context(|| format!("编译自定义合规脱敏正则失败: {pat}"))?;
            custom_res.push(compiled);
        }
        Ok(Self {
            java_flag_re: Regex::new(
                r#"(?i)((?:-D|--)[A-Za-z0-9._-]*(?:password|passwd|pwd|secret|token|apikey|api_key|accesskey|access_key|private_key|ak|sk)=)([^\s"'`]+)"#,
            )?,
            uri_cred_re: Regex::new(
                r"(?i)((?:postgres|postgresql|mysql|redis|mongodb|amqp|https?|jdbc:[a-z0-9]+)://[^:\s/@]+:)([^@\s/]+)(@)",
            )?,
            pem_key_re: Regex::new(
                r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9 ]*PRIVATE KEY-----",
            )?,
            bearer_re: Regex::new(r"(?i)((?:Bearer|Basic)\s+)[A-Za-z0-9._\-+/=]{8,}")?,
            env_secret_re: Regex::new(
                r#"(?i)(\b(?:PGPASSWORD|MYSQL_PWD|API_SECRET|SECRET_KEY|ACCESS_TOKEN|PASSWORD|PASSWD|TOKEN|SECRET)=)([^\s"';`]+)"#,
            )?,
            id_card_re: Regex::new(
                r"(^|[^0-9])([1-9]\d{2})\d{3}(?:19|20)\d{2}(?:0[1-9]|1[0-2])(?:0[1-9]|[12]\d|3[01])(\d{3}[\dXx])([^0-9A-Za-z]|$)",
            )?,
            mobile_re: Regex::new(r"(^|[^0-9])(1[3-9]\d)\d{4}(\d{4})([^0-9]|$)")?,
            custom_res,
            enable_pii_masking: cfg.enable_pii_masking,
        })
    }

    /// 对单个字符串执行精确分段脱敏，返回 `(新字符串, 是否发生修改)`
    pub fn redact_str(&self, input: &str) -> (String, bool) {
        let mut out = input.to_string();

        out = self.pem_key_re.replace_all(&out, "[REDACTED_PEM_KEY]").into_owned();
        out = self.java_flag_re.replace_all(&out, "${1}[REDACTED]").into_owned();
        out = self.uri_cred_re.replace_all(&out, "${1}[REDACTED]${3}").into_owned();
        out = self.bearer_re.replace_all(&out, "${1}[REDACTED]").into_owned();
        out = self.env_secret_re.replace_all(&out, "${1}[REDACTED]").into_owned();

        if self.enable_pii_masking {
            out = self.id_card_re.replace_all(&out, "${1}${2}***********${3}${4}").into_owned();
            out = self.mobile_re.replace_all(&out, "${1}${2}****${3}${4}").into_owned();
        }

        for re in &self.custom_res {
            out = re.replace_all(&out, "[REDACTED]").into_owned();
        }

        let changed = out != input;
        (out, changed)
    }

    /// 递归脱敏 JSON 值（覆盖全量 string / array / object 与敏感字段名）
    pub fn redact_json(&self, val: &mut Value) -> bool {
        match val {
            Value::String(s) => {
                let (masked, changed) = self.redact_str(s);
                if changed {
                    *s = masked;
                }
                changed
            }
            Value::Array(arr) => {
                let mut changed = false;
                for item in arr {
                    if self.redact_json(item) {
                        changed = true;
                    }
                }
                changed
            }
            Value::Object(map) => {
                let mut changed = false;
                for (k, v) in map.iter_mut() {
                    let lk = k.to_ascii_lowercase();
                    let sensitive_key = lk.contains("password")
                        || lk.contains("passwd")
                        || lk.contains("secret")
                        || lk.contains("private_key")
                        || lk.contains("credential")
                        || lk.ends_with("_token")
                        || lk == "token";
                    if sensitive_key {
                        if let Value::String(sv) = v {
                            if !sv.is_empty() && sv != "[REDACTED]" && sv != "***" {
                                *sv = "[REDACTED]".to_string();
                                changed = true;
                                continue;
                            }
                        }
                    }
                    if self.redact_json(v) {
                        changed = true;
                    }
                }
                changed
            }
            _ => false,
        }
    }
}
