---
phase: 6
slug: task-management-api-documentation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-25
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust native) |
| **Config file** | none — Cargo.toml [dev-dependencies] to be added |
| **Quick run command** | `cargo build` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** ~60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 06-01-01 | 01 | 1 | API-02 | T-06-01 / T-06-02 | Valid state transitions enforced server-side | compile + integration | `cargo build` then `cargo test --test task_api` | ❌ W0 | ⬜ pending |
| 06-01-02 | 01 | 1 | API-02 | — | N/A | compile | `cargo build` | ❌ W0 | ⬜ pending |
| 06-01-03 | 01 | 2 | API-05 | — | N/A | compile + integration | `cargo test --test openapi_spec` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/task_api.rs` — integration tests for task CRUD + lifecycle
- [ ] `tests/openapi_spec.rs` — integration test validating OpenAPI spec structure

*Existing infrastructure (cargo test) covers basic compilation checks. Integration tests need Wave 0 setup.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Swagger UI renders in browser | API-05 | Browser-based UI rendering not testable via cargo test | Start server, open `/swagger-ui` in browser, verify all endpoints listed and "Try it out" works |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
