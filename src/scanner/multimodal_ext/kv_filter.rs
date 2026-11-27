use serde_json::{json, Value};
use crate::storage::models::AssetRecord;

/// 标量多谓词比较算子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
    Exists,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvPredicate {
    pub key: String,
    pub op: KvOp,
    pub value: String,
}

/// 解析多谓词标量过滤表达式（逗号或分号分隔：`key=v`, `!=`, `>=`, `>`, `<=`, `<`, `~`, `has:key` 或裸键 `key`）
pub fn parse_kv_filters(expr: &str) -> Vec<KvPredicate> {
    let mut out = Vec::new();
    for raw_clause in expr.split([',', ';']) {
        let clause = raw_clause.trim();
        if clause.is_empty() {
            continue;
        }
        if let Some(k) = clause.strip_prefix("has:") {
            let key = k.trim();
            if !key.is_empty() {
                out.push(KvPredicate {
                    key: key.to_string(),
                    op: KvOp::Exists,
                    value: String::new(),
                });
            }
            continue;
        }
        let mut matched_op = false;
        for (sep, op) in [
            (">=", KvOp::Ge),
            ("<=", KvOp::Le),
            ("!=", KvOp::Ne),
            ("~", KvOp::Contains),
            (">", KvOp::Gt),
            ("<", KvOp::Lt),
            ("=", KvOp::Eq),
        ] {
            if let Some((k, v)) = clause.split_once(sep) {
                let key = k.trim();
                let val = v.trim().trim_matches('"').trim_matches('\'');
                if !key.is_empty() {
                    out.push(KvPredicate {
                        key: key.to_string(),
                        op,
                        value: val.to_string(),
                    });
                }
                matched_op = true;
                break;
            }
        }
        if !matched_op {
            out.push(KvPredicate {
                key: clause.to_string(),
                op: KvOp::Exists,
                value: String::new(),
            });
        }
    }
    out
}

fn eval_single_predicate(asset: &AssetRecord, pred: &KvPredicate) -> bool {
    let val_opt: Option<Value> = match pred.key.as_str() {
        "asset_type" | "type" => Some(json!(asset.asset_type)),
        "path" => Some(json!(asset.path)),
        "size_bytes" => asset
            .size_bytes
            .map(|s| json!(s))
            .or_else(|| asset.desc_kv.get("size_bytes").cloned()),
        _ => asset.desc_kv.get(&pred.key).cloned(),
    };
    let Some(val) = val_opt else {
        return matches!(pred.op, KvOp::Ne);
    };
    match pred.op {
        KvOp::Exists => match &val {
            Value::Null => false,
            Value::String(s) => !s.trim().is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(m) => !m.is_empty(),
            _ => true,
        },
        KvOp::Gt | KvOp::Ge | KvOp::Lt | KvOp::Le => {
            let (Some(actual), Ok(target)) = (
                val.as_f64()
                    .or_else(|| val.as_str().and_then(|s| s.trim().parse::<f64>().ok())),
                pred.value.parse::<f64>(),
            ) else {
                return false;
            };
            match pred.op {
                KvOp::Gt => actual > target,
                KvOp::Ge => actual >= target,
                KvOp::Lt => actual < target,
                KvOp::Le => actual <= target,
                _ => false,
            }
        }
        KvOp::Eq | KvOp::Ne => {
            let eq = match &val {
                Value::String(s) => s.eq_ignore_ascii_case(&pred.value),
                Value::Number(n) => n
                    .as_f64()
                    .zip(pred.value.parse::<f64>().ok())
                    .map(|(a, b)| (a - b).abs() < 1e-9)
                    .unwrap_or_else(|| n.to_string() == pred.value),
                Value::Bool(b) => pred.value.parse::<bool>().map(|t| *b == t).unwrap_or(false),
                Value::Array(arr) => arr.iter().any(|x| {
                    x.as_str()
                        .map(|s| s.eq_ignore_ascii_case(&pred.value))
                        .unwrap_or(false)
                }),
                _ => val.to_string().eq_ignore_ascii_case(&pred.value),
            };
            if matches!(pred.op, KvOp::Eq) { eq } else { !eq }
        }
        KvOp::Contains => {
            let needle = pred.value.to_ascii_lowercase();
            match &val {
                Value::String(s) => s.to_ascii_lowercase().contains(&needle),
                Value::Array(arr) => arr
                    .iter()
                    .any(|x| x.to_string().to_ascii_lowercase().contains(&needle)),
                _ => val.to_string().to_ascii_lowercase().contains(&needle),
            }
        }
    }
}

/// 对单条资产执行多谓词 AND 求值
pub fn matches_kv_filters(asset: &AssetRecord, predicates: &[KvPredicate]) -> bool {
    predicates.iter().all(|p| eval_single_predicate(asset, p))
}
