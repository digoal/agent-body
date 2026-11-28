# Defense-in-Depth for Embodied Host Agents: Dual-Domain Tamper-Evident Auditing, Zero-Trust Peer Mesh, and Pareto-Gated Empirical Benchmarking

**中文副题**：宿主机具身智能体纵深防御：双域因果存证审计、零信任联邦信封协议与帕累托双门禁实证评测体系  
**Target Venues**：USENIX Security / ACM CCS / NDSS (or MLSys Security & Evaluation Track)  
**Paper Type**：Full Research Paper (Embodied Agent Security, Accountable Autonomy & Anti-Gaming Evaluation)  
**Artifact Baseline**：`src/scanner/redactor.rs` · `src/audit/` · `src/mesh/` · `src/benchmark/{mod,dq,provenance,ledger,competitors}.rs`  
**Tags**：`#AgentSecurity` `#DualDomainAudit` `#ZeroTrustMesh` `#PromptInjectionDefense` `#ParetoBenchmark` `#USENIXSecurity`

---

## Abstract

Granting autonomous AI agents direct access to host filesystems, `/proc` process tables, background maintenance daemons, and peer-to-peer multi-host networks introduces an acute security and accountability gap. Unprotected host substrates leak plaintext credentials from `/proc/*/cmdline` and `.env` files into persistent indices, ingest indirect prompt injections from untrusted third-party repositories into session startup briefs, execute unlogged *out-of-agent* background mutations (e.g., garbage collection, link rewriting, watcher floods), and expose cross-host collaboration endpoints to replay and arbitrary file-overwrite attacks. Compounding these threats is an *evaluation integrity flaw*: naive context-compression and redaction mechanisms can trivially claim `99%` token savings or `0%` secret leakage by silently discarding essential operational facts.

We present a holistic **Five-Layer Embodied Defense-in-Depth** architecture, **Dual-Domain Tamper-Evident Behavioral Auditing**, **Zero-Trust `MeshEnvelope v1.0` Collaboration**, and a **Pareto-Gated Empirical Benchmark (`v4.1`)** implemented in **Agent Body**. Across layers `L1–L5`, sensitive-path shallow blacklisting, universal `SecretRedactor` pipelines, POSIX-escaped hook execution, loopback/Bearer enforcement, and `PromptInjectionScanner` sanitization eliminate credential persistence and indirect jailbreak propagation. To eradicate background black-box mutations without drowning logs in editor I/O noise, our `AuditLogger` (`< 80 μs` non-blocking write) unifies **`IN_AGENT`** tool/hook invocations and **`OUT_OF_AGENT`** daemon/GC/repair actions with `60 s` sliding-window `inotify` flood folding, W3C `trace_id` correlation, redacted `<= 4 KB` `before_after_delta` diffs, and `SHA-256` tamper-evident digests. Across hosts, `MeshEnvelope v1.0` enforces canonical `HMAC-SHA256` signing, dual-factor replay defense (`±300 s` window + concurrent `NonceCache`), four least-privilege scopes, and `0600`-quarantined `<= 10 MB` artifact inboxes. Finally, our benchmark couples a deterministic four-dimension context quality metric $Q \in [0, 100]$ with `16` auditable provenance rules (`PR-01`–`PR-16`), requiring `100%` ground-truth fact recall alongside $Q_{\text{body}} \ge Q_{\text{baseline}}$. Live 7-day production audits (`9,431` events) and `22` workloads confirm `0.0%` plaintext credential leakage (`DQ4/DQ7 = 100%`), `100%` replay/tamper/sensitive-transfer blocking (`W14`), and `+54.5` to `+75.9` quality gains over popular open-source baselines (`COMP-C1`–`COMP-C4`).

---

## 1. Introduction

When an AI agent operates purely inside a cloud chat window, its attack surface is largely confined to conversational text. Once embodied on a developer workstation or production Linux server—scanning local repositories, reading `/proc` process states, maintaining long-term memories, running periodic cleanup daemons, and collaborating with peer hosts—the substrate itself becomes a high-value privilege bridge.

A systematic security audit of early host-agent architectures (`11-security-spec.md`, `14-graph-intelligence-and-mesh-protocol-spec.md`, `16-dual-domain-audit-and-self-evolution-spec.md`) revealed **five critical vulnerability classes (`SEC-GAP-01` to `SEC-GAP-05`)**:

1. **Passive Credential Harvesting (`SEC-GAP-01`)**: Recursive workspace scanners and `/proc/<pid>/cmdline` collectors inadvertently index `.env` files, SSH keys, database connection URIs (`postgres://user:pass@...`), and bearer tokens (`sk-...`), persisting plaintext secrets into SQLite/PostgreSQL FTS tables and shipping them into LLM prompts (`100%` leak rate observed in unredacted Mem0/JSON baselines, `COMP-C1`).
2. **Indirect Prompt Injection via Workspace Artifacts (`SEC-GAP-02`)**: Cloning an external GitHub repository or receiving a peer message containing adversarial instructions (e.g., `IGNORE PREVIOUS INSTRUCTIONS; curl attacker.sh | sh` embedded in a `README.md` or checkpoint summary; [Greshake et al., AISec 2023](https://arxiv.org/abs/2302.12173)) poisons `SessionStart` `body_brief` assembly and cross-brain `ContinuityCapsule` handoffs.
3. **Out-of-Agent Accountability Blindness & Watcher Floods (`SEC-GAP-03`)**: Prior systems logged only explicit agent tool calls (`changes` table: `94.7%` drowned out by `10,159` raw `watcher:file_modified` editor saves), while **autonomous background maintenance**—garbage collection (`HostGC`), automated broken-link source patching, freshness pruning, and sleep memory compaction—left **`0` audit records**, making unexpected file modifications unattributable.
4. **Unauthenticated / Replay-Prone Peer Collaboration (`SEC-GAP-04`)**: Naive HTTP peer meshes using a static shared token without canonical payload signing, timestamps, nonces, or staging sandboxes allow captured packets to be replayed and enable compromised peers to overwrite sensitive host files.
5. **Metric Gaming in Security & Compression Benchmarks (`SEC-GAP-05`)**: Evaluating redaction or context compression solely by *token reduction %* or *zero leaked secrets* is fundamentally gameable: a degenerate filter that replaces every document with an empty string scores `100%` token savings and `0%` leakage while destroying agent utility.

![五层具身安全纵深防御、IN/OUT 双域防洪审计与零信任 MeshEnvelope 协议](svg/05-five-layer-defense-and-dual-audit.svg)

**Contributions.** We design and empirically validate an end-to-end security and verifiable evaluation framework:

- **Five-Layer Embodied Defense-in-Depth (`L1–L5`) (§2)**: Enforcing zero-read shallow blacklisting on sensitive paths, universal `SecretRedactor` scrubbing across all ingestion/audit/memory channels, POSIX shell escaping and export sandboxes, loopback/token write guards, and `PromptInjectionScanner` sanitization.
- **Dual-Domain (`IN_AGENT × OUT_OF_AGENT`) Flood-Folded Tamper-Evident Auditing (§3)**: Separating raw I/O churn from a unified `< 80 μs` `audit_events` ledger with `60 s` `inotify` sliding-window folding, W3C `trace_id` causality, redacted `<= 4 KB` `before_after_delta` snapshots, and `SHA-256` evidence hashes.
- **Zero-Trust `MeshEnvelope v1.0` & Quarantined Artifact Inbox (§4)**: A lightweight peer protocol (requiring zero Raft/Paxos consensus overhead) providing canonical `HMAC-SHA256` integrity, `(±300 s, NonceCache)` anti-replay, four granular peer scopes, bidirectional redaction/injection screening, and `0600` quarantined `<= 10 MB` file staging.
- **Anti-Gaming Pareto Benchmark (`v4.1`) & Provenance Ledger (`PR-01`–`PR-16`) (§5)**: A three-track empirical suite enforcing simultaneous `100%` ground-truth fact retention and non-decreasing four-dimensional quality $Q$, paired with `< 100 μs` per-call deterministic ROI accounting.

---

## 2. Threat Model & Five-Layer Defense-in-Depth

### 2.1 Threat Model & Trust Assumptions

We consider a host running Agent Body alongside one or more semi-trusted LLM clients that interact with local workspaces (including cloned untrusted open-source repositories) and optional remote Agent Body peers over untrusted LAN/WAN links. The adversary may: (a) plant malicious prompt-injection payloads inside external `README.md`, source comments, or peer messages; (b) scatter sensitive credentials in environment files or process arguments that risk accidental indexing/exfiltration; (c) replay, forge, or tamper with cross-host Mesh HTTP requests, or attempt path-traversal (`../.ssh/authorized_keys`) during artifact transfer; and (d) trigger noisy file saves to obscure unauthorized background deletions. Kernel-level root exploits and direct syscall bypasses outside the MCP/Hook/Web boundary are orthogonal OS-sandbox concerns (discussed in §7).

### 2.2 Layers `L1`–`L5` Architectural Enforcement

1. **`L1` Admission Gate & Universal `SecretRedactor` ([`src/scanner/redactor.rs`](../src/scanner/redactor.rs))**: Sensitive paths (`.env*`, `id_rsa`, `*.pem`, `*.key`, `~/.ssh/`, `/etc/shadow`) are classified as `shallow` or blocked—**their file contents are never read into memory**. Every string entering `assets`, `/proc` `cmdline` snapshots, `memories` (SQLite & PG `payload_redacted`), `body_checkpoints`, `audit_events`, or `mesh_dialogues` passes through `SecretRedactor::redact()`, deterministically masking API keys (`sk-...`, `ghp_...`, AWS/GCP tokens), Bearer headers, URI passwords, and `-Dpassword=` JVM flags.
2. **`L2` Pre-Hook Pain Reflex & Execution Sandboxing ([`src/api/hooks.rs`](../src/api/hooks.rs))**: `body_hook_pre` blocks write attempts to `L3_FORBIDDEN` system paths; `HookDispatcher` applies strict POSIX single-quote escaping to prevent `sh -c` command injection; DuckDB Parquet/CSV analytics exports are confined at the engine level to whitelisted `<workspace>/tmp/` directories.
3. **`L3` Loopback Binding & Write Authentication ([`src/web/server.rs`](../src/web/server.rs))**: Web and Mesh listeners bind to `127.0.0.1:8765` by default. Every mutating HTTP endpoint (`POST/DELETE`, including fast-weight/sleep controls and GC) enforces `check_auth` Bearer verification, deterministically returning `401 Unauthorized` on missing/invalid tokens and refusing non-loopback writes if no token is configured.
4. **`L4` Storage Least Privilege**: `~/.agent-body/` (`0700`) and `body.db` / `config.example.toml` (`0600`) permissions are converged on startup and continuously audited by `PreflightChecker`.
5. **`L5` Indirect Prompt Injection Sanitization (`PromptInjectionScanner`)**: Untrusted summaries, third-party `SKILL.md` descriptions, `ContinuityCapsule` fields, and incoming Mesh dialogues are scanned for instruction-override patterns (`IGNORE PREVIOUS INSTRUCTIONS`, system-role spoofing, hidden shell-exec directives), neutralizing jailbreak tokens before prompt assembly.

---

## 3. Dual-Domain Tamper-Evident Behavioral Auditing

To eliminate the *Out-of-Agent Black Box* (`SEC-GAP-03`) without recreating a heavyweight enterprise SIEM, `AuditLogger` ([`src/audit/mod.rs`](../src/audit/mod.rs)) captures both operational domains in a unified schema (`audit_events` across SQLite, PostgreSQL, and DuckDB):

- **`IN_AGENT` Domain**: Records every `SessionStart` lease issuance, `PreToolUse` guard verdict, `PostToolUse` `< 2 ms` recovery prescription, all `43` MCP tools, `30` CLI commands, Web mutations, `ShadowAbEngine` comparisons, and `ContinuityCapsule` transitions.
- **`OUT_OF_AGENT` Domain**: Mandatorily records all `7` autonomous/reactive host maintenance pipelines—`BackgroundScheduler` 30-minute cycles, `FreshnessReconciler` pruning, `HostGarbageCollector` deletions (file list & byte count), `LinkRepairer` source diffs (`patch_diff`), `PlacementLearner` rule updates, `SleepTimeCompactor` / `FastWeight` jobs, and `FileWatcher` syncs.

**Flood Folding & Cryptographic Evidence.** Rapid editor saves on the same file path within a **60-second sliding window** are folded in memory into a single `fs_watch_sync` summary carrying `folded_events` count—compressing raw I/O storms by `> 80%` while preserving exact write-through invalidation. Every `audit_events` row bounds its redacted `before_after_delta` JSON to `<= 4 KB`, propagates W3C `traceparent` / `X-Trace-Id`, completes non-blocking persistence in `< 80 μs`, and seals canonical event attributes under `evidence_sha256`.

---

## 4. Zero-Trust `MeshEnvelope v1.0` Collaboration Protocol

When agents on Node A (`node-workstation`) and Node B (`node-gpu`) collaborate, [`src/mesh/`](../src/mesh/) enforces a zero-trust wire envelope without consensus-cluster complexity:

```mermaid
flowchart LR
  A["Sender Node-A<br>Redact + Canonical Hash"]
  ENV["MeshEnvelope v1.0<br>ts + nonce + HMAC-SHA256"]
  V{"Receiver Gate<br>±300s + NonceCache + Scope"}
  Q["mesh_inbox/ (0600)<br>status = 'quarantined'"]
  REJ["401 / 403 / 409 / 413<br>Deterministic Reject"]

  A --> ENV
  ENV --> V
  V -.Valid & Clean.-> Q
  V -.Tamper / Replay / Leak.-> REJ

  classDef send fill:#e7f2f5,stroke:#1d5b70,stroke-width:2px,color:#134252
  classDef gate fill:#fdf2e0,stroke:#b56917,stroke-width:2px,color:#8a4e0d
  classDef ok fill:#e6f4ec,stroke:#1f7053,stroke-width:2px,color:#15533c
  classDef bad fill:#fcebe8,stroke:#b43c2c,stroke-width:2px,color:#89281b

  class A,ENV send
  class V gate
  class Q ok
  class REJ bad
```

1. **Canonical `HMAC-SHA256` Authentication**: Over canonical newline-delimited headers `"{protocol}\n{message_id}\n{msg_type}\n{from_node}\n{to_node}\n{timestamp_ms}\n{nonce}\n{payload_sha256}"`, verified using per-peer keys `[mesh.peer_keys]` (failing closed with `403 Forbidden` if unconfigured).
2. **Dual-Factor Replay Defense**: Requests outside $|T_{\text{server}} - \text{timestamp\_ms}| \le 300\text{ s}$ are rejected (`401`); within the window, a thread-safe `NonceCache` tracks `(from_node, nonce) -> expires_at`, rejecting duplicate nonces immediately with `409 Conflict`.
3. **Quarantined Artifact Inbox (`transfer:artifact` Scope)**: While general HTTP endpoints cap request bodies at `64 KB` against DoS, `/api/v1/mesh/transfer` permits streaming up to **`<= 10 MB`**, validates `payload_sha256`, rejects path traversal (`..`, `/`) and sensitive extensions (`.env`, `.pem`, `.key`, `.ssh`), scans payloads via `SecretRedactor` + `PromptInjectionScanner`, and writes strictly into **`~/.agent-body/mesh_inbox/<from_node>/<id>_<safe_name>` (`0600`, `status='quarantined'`)** awaiting explicit local approval—**never overwriting live workspace or system paths**.

---

## 5. Anti-Gaming Pareto Benchmark & Provenance Ledger

To defeat *Metric Gaming* (`SEC-GAP-05`), Benchmark `v4.1` ([`src/benchmark/`](../src/benchmark/)) evaluates every security filter, router, and compactor under a **Deterministic Four-Dimension Quality Function** $Q \in [0, 100]$:

$$Q = \text{round2}\Big(100 \times \big(0.40 \cdot R_{\text{fact}} + 0.30 \cdot P_{\text{signal}} + 0.20 \cdot A_{\text{action}} + 0.10 \cdot S_{\text{safety}}\big)\Big)$$

where $R_{\text{fact}} = \text{facts\_hit} / \text{required\_facts\_total}$ verifies mandatory ground-truth items in the actual output, $P_{\text{signal}}$ measures signal-to-noise purity, $A_{\text{action}}$ checks structured actionable remedies, and $S_{\text{safety}}$ checks `0%` plaintext secret residue and injection neutralization. Information density gain is measured as $\rho_{\text{gain}} = \rho_{\text{body}} / \max(\rho_{\text{base}}, 0.01)$ with $\rho = (\text{facts\_hit} / \max(\text{tokens}, 1)) \times 10^3$.

**Hard Pareto Gate:** A workload passes SLA *only if* efficiency targets hold **and** $R_{\text{fact}} = 1.0$ (`100%` required facts retained) **and** $Q_{\text{body}} \ge Q_{\text{baseline}}$. Any redactor or compressor that over-scrubs legitimate ports, paths, or parameters immediately fails $R_{\text{fact}}$. In production (`Track B`), `EffectivenessLedger` binds every recorded gain (`< 100 μs` overhead) to one of `16` deterministic derivation formulas (`PR-01`–`PR-16`) sealed under batch `report_sha256`.

![Benchmark v4.1 防作弊帕累托前沿实测与四大赛道同机竞品对标](svg/05-pareto-quality-vs-token-frontier.svg)

---

## 6. Empirical Evaluation

### 6.1 Security, Dual-Domain Audit & Mesh Protocol Verification

Table 1 consolidates live security workloads (`W11`, `W14`, `W16`), 7-day production audit telemetry, and live database hygiene gates (`DQ4`, `DQ7`, `DQ8`).

| Security / Audit Evaluation Target | Test / Live Sample Scale | Attack / Failure Vectors Tested | Measured Defense / Hygiene Rate | Quality $Q$ (Base $\to$ Body) |
|---|---|---|---|---:|
| **`W11` Redaction & Injection Guardrail** | API keys + prompt-injection traps + `/etc/shadow` write | Secret leak + jailbreak + forbidden path | **`100%` intercepted & sanitized** (`43 μs`) | `21.5` $\to$ **`100.0`** |
| **`W14` `MeshEnvelope v1.0` Zero-Trust** | Valid HMAC + tampered sig + replay nonce + `.pem` transfer | Forgery, replay (`409`), sensitive file (`403`) | **`100%` valid pass · `100%` attack block** | `12.5` $\to$ **`100.0`** |
| **`W16` Dual-Domain Audit & Lease Evolution** | `IN_AGENT` + `OUT_OF_AGENT` (`HostGC`/`Repair`/`Reconcile`) | Unlogged background mutation + lease drift | **`100%` delta & `trace_id` · auto-downgrade** | `36.0` $\to$ **`100.0`** |
| **Live 7-Day Production Audit Stream** | **`9,431` events** (`IN=6,236`, `OUT=3,195`, `72` blocks) | Editor save floods (`1,812` folded) + leaks | **`0.0%` secret leak · `100%` SHA-256 intact** | Live Prod Verified |
| **`DQ4` & `DQ7` DB Credential Hygiene** | `147` proc/memory rows + `500` sampled `audit_events` | Residual plaintext tokens / passwords | **`147/147` (`100%`) & `500/500` (`100%`)** | SLA $\ge 100\%$ / $99\%$ ✅ |
| **`DQ8` Envelope & Capsule Compliance** | Live `HomeostasisVitals` + `L3_FORBIDDEN` + checkpoints | Privilege escalation + `>300` tok bloat | **Vitals `99.8/100` · `L3` block `100%`** | SLA $\ge 99\%$ ✅ |

### 6.2 Live Head-to-Head Competitor Evaluation (`COMP-C1`–`COMP-C4`)

Table 2 presents same-host, same-dataset physical executions (`run-310cf715`, `LIVE_BENCHMARKED`) across four open-source paradigm tracks.

| Track | Competitor Baseline (Live Sandboxed Execution) | Competitor Measured Outcome | Agent Body Measured Outcome | Quality $Q$ ($\Delta Q$) | Density Gain | Token Savings |
|:---:|---|---|---|---:|---:|---:|
| **`COMP-C1`** | Raw JSON / Mem0-style unredacted memory (`60` items, `5` secrets) | **`5 / 5` (`100%`) plaintext secrets leaked**; full scan | **`0 / 5` (`0%` leak)** · Top-5 FTS5 hit | `24.1` $\to$ **`100.0`** (`+75.9`) | `136.2×` | **97.8%** |
| **`COMP-C2`** | OpenViking L0/L1 virtual `.abstract/.overview` tree + static `205` skills | `60` extra summary files; `144,730` tok | **`0` file copies**; `931` tok (`budget=600`) | `45.5` $\to$ **`100.0`** (`+54.5`) | `233.2×` | **99.4%** |
| **`COMP-C3`** | AgentFS SQLite VFS 4-table full import (`290` mixed files) | Ingests all `280` `node_modules/.git` noise files | **`10` clean files (`0` blacklist escapes)** | `26.4` $\to$ **`100.0`** (`+73.6`) | `87.0×` | **96.5%** |
| **`COMP-C4`** | Multi-CLI subprocess combo (`rg` + `psutil` + `ss`) | `3` subprocesses (`244.5 ms`); `0`-hop service graph | **`0` subprocesses (`75.1 ms`)**; `2` upstream hops | `37.3` $\to$ **`97.5`** (`+60.2`) | `5.5×` | **46.0%** |

Note the latency honesty in `COMP-C1..C3`: running full regex secret redaction, FTS5 indexing, and blacklist policy matching takes `36–68 ms` locally compared to `1.2–14 ms` for an unsafe raw memory append or unfiltered file dump—yet saves **tens of thousands of LLM tokens** (worth multiple seconds of cloud inference) while eliminating `100%` of credential leaks and directory pollution.

---

## 7. Applicability Boundaries & Residual Risks

1. **Regex/Entropy Redaction vs. Novel Custom Encodings**: `SecretRedactor` deterministically catches known token prefixes, key-value assignments, PEM blocks, and URI credentials (`100%` on standard secret corpora), but cannot identify a user-invented passphrase stored in plain prose without key markers (e.g., `"my cat's birthday is blue-tiger-42"`), nor base64-double-encoded custom blobs. High-security deployments should pair `L1` redaction with `contrib/hook_gov_compliance` tenant-specific PII dictionaries.
2. **Heuristic Prompt-Injection Evasion**: `PromptInjectionScanner` neutralizes overt instruction-hijacking templates (`W11`, `W22`), yet adaptive multi-turn semantic steganography remains an open arms race across the security community ([Greshake et al., 2023](https://arxiv.org/abs/2302.12173)). Therefore, Agent Body never relies on `L5` text sanitization as a single point of failure: even if a subtle injection survives into an LLM turn, **`L2` Pre-Hook path guards, `L3_FORBIDDEN` pain reflexes, and `Mesh` `0600` quarantine still block destructive execution**.
3. **Local Root Tampering of SQLite Audit Logs**: `evidence_sha256` detects accidental corruption or unprivileged row tampering in `audit_events`. Against an attacker who already holds local `root` privileges on the host, local SQLite rows could be rewritten together with their hashes; enterprise deployments mitigate this by enabling `contrib/hook_audit_siem` real-time append-only Syslog/OTLP forwarding to an off-host collector.

---

## 8. Reproducibility Artifact & Conclusion

Securing embodied host agents requires defending the substrate where physical facts, memories, background maintenance, and cross-host messages actually meet—and proving via anti-gaming Pareto metrics that security never comes at the cost of lobotomizing context quality.

### Artifact Evaluation (AE) Verification Commands

```bash
export PATH="$HOME/.cargo/bin:$PATH"
# 1. Run security, audit, and zero-trust Mesh integration suites
cargo test --test audit_integration -- --nocapture
cargo test --test mesh_integration -- --nocapture

# 2. Run live competitor tracks (COMP-C1..C4) and data-quality hygiene gates (DQ1..DQ12)
agent-body bench competitors --track all
agent-body audit summary --days 7
```

---

## References

1. Kai Greshake, Sahar Abdelnabi, Shailesh Mishra, Christoph Endres, Thorsten Holz, and Mario Fritz. 2023. Not What You've Signed Up For: Compromising Real-World LLM-Integrated Applications with Indirect Prompt Injection. In *Proceedings of the 16th ACM Workshop on Artificial Intelligence and Security (AISec '23)*. 79–90. https://arxiv.org/abs/2302.12173
2. Edoardo Debenedetti, Jie Zhang, Mislav Balunović, Luca Beurer-Kellner, Marc Fischer, and Florian Tramèr. 2024. AgentDojo: A Dynamic Environment to Evaluate Prompt Injection Attacks and Defenses for LLM Agents. In *NeurIPS 2024 Datasets and Benchmarks Track*. https://arxiv.org/abs/2406.13352
3. Ross Anderson. 2020. *Security Engineering: A Guide to Building Dependable Distributed Systems* (3rd ed.). Wiley.
4. Jerome H. Saltzer and Michael D. Schroeder. 1975. The Protection of Information in Computer Systems. *Proceedings of the IEEE* 63, 9 (1975), 1278–1308. https://doi.org/10.1109/PROC.1975.9939
5. Samuel T. King and Peter M. Chen. 2003. Backtracking Intrusions. In *Proceedings of the 19th ACM Symposium on Operating Systems Principles (SOSP '03)*. 223–236. https://doi.org/10.1145/945445.945467
