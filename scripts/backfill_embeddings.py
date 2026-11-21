#!/usr/bin/env python3
"""Agent Host Steward 增量向量补齐脚本 (Ollama Embedding Backfill)。

功能:
  扫描 ~/.steward/steward.db 中缺失 embedding (NULL / '' / '[]') 的
  assets、fact_nodes、memories 记录，调用本地 Ollama /api/embeddings
  (默认 nomic-embed-text) 批量生成 768 维向量并幂等回写 SQLite。

示例:
  python3 scripts/backfill_embeddings.py
  python3 scripts/backfill_embeddings.py --db ~/.steward/steward.db --model nomic-embed-text --limit 500
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
import urllib.request
from pathlib import Path


def fetch_ollama_embedding(ollama_url: str, model: str, text: str, timeout: int = 30) -> list[float]:
    url = f"{ollama_url.rstrip('/')}/api/embeddings"
    payload = json.dumps({"model": model, "prompt": text[:2000]}).encode("utf-8")
    req = urllib.request.Request(url, data=payload, headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        data = json.loads(resp.read().decode("utf-8"))
    emb = data.get("embedding")
    if not isinstance(emb, list) or not emb:
        raise RuntimeError(f"Ollama returned empty embedding: {data}")
    return emb


def backfill_table(
    conn: sqlite3.Connection,
    ollama_url: str,
    model: str,
    table: str,
    key_col: str,
    text_sql: str,
    limit: int,
    dry_run: bool,
) -> tuple[int, int]:
    cur = conn.cursor()
    where_clause = "embedding IS NULL OR embedding = '' OR embedding = '[]'"
    if table == "assets":
        where_clause += " OR json_extract(desc_kv, '$.embedding_status') = 'stale'"
    rows = cur.execute(
        f"SELECT {key_col}, {text_sql} FROM {table} "
        f"WHERE {where_clause} LIMIT ?",
        (limit,),
    ).fetchall()
    if not rows:
        return (0, 0)
    updated = 0
    failed = 0
    for key, text in rows:
        prompt = (text or str(key)).strip() or str(key)
        if dry_run:
            updated += 1
            continue
        try:
            emb = fetch_ollama_embedding(ollama_url, model, prompt)
            if table == "assets":
                cur.execute(
                    "UPDATE assets SET embedding = ?, "
                    "desc_kv = CASE WHEN json_valid(desc_kv) "
                    "THEN json_set(desc_kv, '$.embedding_status', 'ready') "
                    "ELSE json_object('embedding_status', 'ready') END "
                    "WHERE path = ?",
                    (json.dumps(emb), key),
                )
            else:
                cur.execute(
                    f"UPDATE {table} SET embedding = ? WHERE {key_col} = ?",
                    (json.dumps(emb), key),
                )
            updated += 1
        except Exception as exc:
            failed += 1
            print(f"  ⚠️ {table}[{key}] 补齐失败: {exc}", file=sys.stderr)
    if not dry_run:
        conn.commit()
    return (updated, failed)


def main() -> int:
    parser = argparse.ArgumentParser(description="增量补齐 steward.db 本地 Ollama Embedding 向量")
    parser.add_argument("--db", default=str(Path.home() / ".steward/steward.db"), help="SQLite 数据库路径")
    parser.add_argument("--ollama-url", default="http://127.0.0.1:11434", help="Ollama 服务地址")
    parser.add_argument("--model", default="nomic-embed-text", help="Embedding 模型名")
    parser.add_argument("--limit", type=int, default=500, help="每张表单批最大补齐条数")
    parser.add_argument("--dry-run", action="store_true", help="仅统计缺失数，不写入")
    args = parser.parse_args()

    conn = sqlite3.connect(args.db, timeout=30.0)
    conn.execute("PRAGMA busy_timeout = 30000;")

    targets = [
        ("fact_nodes", "id", "COALESCE(node_type, '') || ' ' || COALESCE(name, '') || ' ' || COALESCE(properties, '')"),
        ("assets", "path", "COALESCE(type, '') || ' ' || COALESCE(path, '') || ' ' || COALESCE(description, '')"),
        ("memories", "id", "COALESCE(type, '') || ' ' || COALESCE(summary, '') || ' ' || COALESCE(content, '')"),
    ]

    total_updated = 0
    total_failed = 0
    for table, key_col, text_sql in targets:
        upd, fail = backfill_table(conn, args.ollama_url, args.model, table, key_col, text_sql, args.limit, args.dry_run)
        total_updated += upd
        total_failed += fail
        print(f"[backfill] {table}: updated={upd}, failed={fail}")

    rem_assets = conn.execute(
        "SELECT COUNT(*) FROM assets WHERE embedding IS NULL OR embedding = '' OR embedding = '[]' OR json_extract(desc_kv, '$.embedding_status') = 'stale'"
    ).fetchone()[0]
    rem_facts = conn.execute("SELECT COUNT(*) FROM fact_nodes WHERE embedding IS NULL OR embedding = '' OR embedding = '[]'").fetchone()[0]
    rem_mems = conn.execute("SELECT COUNT(*) FROM memories WHERE embedding IS NULL OR embedding = '' OR embedding = '[]'").fetchone()[0]
    conn.close()

    print(
        f"✅ 补齐完成 (dry_run={args.dry_run}): 本次更新 {total_updated} 条, 失败 {total_failed} 条 | "
        f"剩余缺失: assets={rem_assets}, fact_nodes={rem_facts}, memories={rem_mems}"
    )
    return 1 if total_failed > 0 else 0


if __name__ == "__main__":
    sys.exit(main())
