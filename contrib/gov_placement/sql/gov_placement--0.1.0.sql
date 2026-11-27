-- ext-gov-placement v0.1.0: 工作区资产放置规则与历史记录私有表
CREATE TABLE IF NOT EXISTS ext_gov_placement__rules (
    rule_id        TEXT PRIMARY KEY,
    category       TEXT NOT NULL,
    target_subdir  TEXT NOT NULL,
    naming_pattern TEXT NOT NULL,
    description    TEXT NOT NULL,
    enabled        INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS ext_gov_placement__history (
    history_id     TEXT PRIMARY KEY,
    caller_id      TEXT NOT NULL,
    category       TEXT NOT NULL,
    raw_name       TEXT NOT NULL,
    suggested_path TEXT NOT NULL,
    verdict        TEXT NOT NULL,
    created_at     TEXT NOT NULL
);

INSERT INTO ext_gov_placement__rules (rule_id, category, target_subdir, naming_pattern, description, enabled)
VALUES ('rule_script', 'script', 'shared_proj', '{snake_name}', '可复用脚本与工具目录（snake_case 命名）', 1)
ON CONFLICT(rule_id) DO NOTHING;

INSERT INTO ext_gov_placement__rules (rule_id, category, target_subdir, naming_pattern, description, enabled)
VALUES ('rule_project', 'project', 'work', '{snake_name}', '常规立项工作工程目录（snake_case 命名）', 1)
ON CONFLICT(rule_id) DO NOTHING;

INSERT INTO ext_gov_placement__rules (rule_id, category, target_subdir, naming_pattern, description, enabled)
VALUES ('rule_document', 'document', 'note', '{snake_name}', '笔记、清单与知识文档目录（snake_case 命名）', 1)
ON CONFLICT(rule_id) DO NOTHING;

INSERT INTO ext_gov_placement__rules (rule_id, category, target_subdir, naming_pattern, description, enabled)
VALUES ('rule_temp', 'temp', 'tmp', '{snake_name}', '临时脚本与调试产物隔离目录（tmp）', 1)
ON CONFLICT(rule_id) DO NOTHING;
