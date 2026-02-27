# vbmc-rs

Redfish-compliant virtual BMC in Rust. One instance manages multiple VMs
(blade chassis model) via a standards-compliant REST API.

## Build & Test

```bash
cargo build --all-features           # Build with all backends
cargo test --all-features            # Run all tests
cargo clippy --all-features          # Lint
cargo fmt --check                    # Check formatting
```

The libvirt feature requires `libvirt-devel` (Fedora) or `libvirt-dev`
(Debian/Ubuntu) installed on the system.

## Feature Flags

- `cloud-hypervisor` (default) — Cloud-Hypervisor via HTTP over Unix socket
- `qemu` — QEMU via QMP over Unix socket
- `libvirt` — libvirt via `virt` crate (C API bindings)
- `keylime`, `trustee`, `nvidia-cc` — attestation providers

Always use `--all-features` when building and testing to catch
cross-feature issues.

## Architecture

- `src/backend/` — `VmmBackend` trait + 3 implementations (CH, QEMU, libvirt)
- `src/redfish/` — ~35 Redfish resource modules, router, OData compliance
- `src/auth/` — sessions, accounts, RBAC (argon2 passwords)
- `src/events/` — broadcast event bus, webhooks, SSE, audit log
- `src/attestation/` — Keylime/Trustee polling

Key files: `app_state.rs` (shared state), `config.rs` (TOML config),
`state.rs` (per-VM persistent JSON state), `backend/types.rs`
(backend-agnostic VM types).

## Conventions

- Handlers are async, take `State(state): State<Arc<AppState>>` via axum extractors
- Backend-specific wire types are converted inside each backend module;
  Redfish handlers only use `backend::types`
- `Backend` enum dispatches to implementations — no `dyn` / vtable
- Feature gating via `#[cfg(feature = "...")]` on enum variants and match arms
- Per-VM mutable state persisted as JSON (write to temp + rename)
- Redfish structs use `#[serde(rename = "...")]` for property names
  and `#[serde(skip_serializing_if = "Option::is_none")]` for optional fields
- Error types: `BackendError` (backend layer) → `RedfishApiError` (HTTP layer)
- Tests: unit tests in `#[cfg(test)]` blocks, integration tests in
  `src/integration_tests.rs` using `MockBackend` + `axum::Router::oneshot()`

## Adding a Redfish Resource

1. Create `src/redfish/<resource>.rs` with handler functions and structs
2. Register routes in `src/redfish/mod.rs`
3. Add links from parent resources
4. Add schema refs to `data/metadata.xml`
5. Add integration tests in `src/integration_tests.rs`

## DMTF Validation

`scripts/dmtf-validate.sh` runs the DMTF Redfish Service Validator
against a live instance. Reports go to `dmtf-reports/`. Conformance
details in `docs/conformance.md`.

## Do Not

- Do not use `dyn VmmBackend` — use the `Backend` enum
- Do not add `.rustfmt.toml` or `clippy.toml` — project uses defaults
- Do not expose backend-specific types in Redfish handlers
- Do not skip `#[serde(skip_serializing_if)]` on Option fields in Redfish structs
