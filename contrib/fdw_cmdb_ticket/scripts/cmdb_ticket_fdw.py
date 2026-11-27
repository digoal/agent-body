#!/usr/bin/env python3
"""轨 B 进程外 CMDB / 变更工单 JSON-RPC 2.0 只读适配器"""

import json
import sys
import time
from pathlib import Path


def default_items(target_host: str | None) -> list[dict]:
    host = target_host or "gov-node-01"
    return [
        {
            "entity_type": "entity:cmdb_ci",
            "entity_id": f"ci:{host}",
            "host_id": host,
            "host_labels": {"cmdb_tier": "L1_PORTAL"},
            "payload": {
                "ci_code": "CI-ZJ-PORTAL-01",
                "service_name": "gov-health-portal",
                "owner_team": "health-sre",
                "env": "prod",
            },
        },
        {
            "entity_type": "entity:change_ticket",
            "entity_id": "CHG-20261127-1402",
            "host_id": host,
            "host_labels": {"ticket_state": "completed"},
            "payload": {
                "ticket_id": "CHG-20261127-1402",
                "title": "gov-health-portal v2 hotfix rollout",
                "change_time": "2026-11-27T14:02:00+08:00",
                "operator": "ops-release-bot",
                "status": "completed",
                "notes": "Hotfix applied to port 8081 after ticket approval.",
            },
        },
    ]


def load_fixture(fixture_path: str | None) -> dict:
    if not fixture_path:
        return {}
    p = Path(fixture_path)
    if not p.exists():
        return {}
    return json.loads(p.read_text(encoding="utf-8"))


def respond(rpc_id, result=None, error=None):
    frame = {"jsonrpc": "2.0", "id": rpc_id if rpc_id is not None else 1}
    if error is not None:
        frame["error"] = error
    else:
        frame["result"] = result
    sys.stdout.write(json.dumps(frame, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def main() -> int:
    fixture_path = sys.argv[1] if len(sys.argv) > 1 else None
    fixture = load_fixture(fixture_path)

    raw_line = sys.stdin.readline()
    if not raw_line.strip():
        respond(1, error={"code": -32700, "message": "empty stdin"})
        return 0

    req = json.loads(raw_line)
    rpc_id = req.get("id", 1)
    method = req.get("method", "")
    params = req.get("params") or {}

    sleep_ms = fixture.get("sleep_ms") or (params.get("params") or {}).get("sleep_ms")
    if sleep_ms:
        time.sleep(float(sleep_ms) / 1000.0)

    if method == "ext.handshake":
        respond(rpc_id, {"ok": True, "ext_id": "fdw_cmdb_ticket", "protocol": "2.0"})
    elif method == "fdw.explain_source":
        respond(
            rpc_id,
            {
                "healthy": fixture.get("healthy", True),
                "degraded_reason": fixture.get("degraded_reason"),
                "missing_deps": fixture.get("missing_deps", []),
            },
        )
    elif method == "fdw.scan":
        items = fixture.get("items")
        if items is None:
            items = default_items(params.get("target_host"))
        respond(rpc_id, {"items": items})
    elif method == "fdw.poll_drift":
        items = fixture.get("drift_items", [])
        cursor = fixture.get("next_cursor", "cmdb-cursor-1")
        respond(rpc_id, {"items": items, "next_cursor": cursor})
    else:
        respond(rpc_id, error={"code": -32601, "message": f"unknown method: {method}"})
    return 0


if __name__ == "__main__":
    sys.exit(main())
