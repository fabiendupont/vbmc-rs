# AGENTS.md

Redfish-compliant virtual BMC in Rust. One instance manages multiple VMs
(blade chassis model) via a standards-compliant REST API.

## Build and test

```bash
cargo build --all-features           # Build with all backends
cargo test --all-features            # Run all tests
cargo clippy --all-features -- -D warnings  # Lint — zero warnings required
cargo fmt --check                    # Check formatting
```

The libvirt feature requires `libvirt-devel` (Fedora) or `libvirt-dev`
(Debian/Ubuntu).

Always use `--all-features` — the project has six feature flags
(`cloud-hypervisor`, `qemu`, `libvirt`, `keylime`, `trustee`, `nvidia-cc`)
and cross-feature issues must be caught.

## Code style

- `cargo fmt` defaults — no `.rustfmt.toml` or `clippy.toml`
- `clippy --all-features -- -D warnings` must pass with zero warnings
- No comments unless the WHY is non-obvious
- Redfish structs use `#[serde(rename = "...")]` for property names
  and `#[serde(skip_serializing_if = "Option::is_none")]` on Option fields
- Error types: `BackendError` (backend layer) → `RedfishApiError` (HTTP layer)
- No bare `unwrap()` in production code — use `.map_err()` or `.ok_or_else()`
- `unwrap()` is fine in test code

## Architecture rules

- `Backend` enum dispatches to implementations — no `dyn VmmBackend`
- Feature gating via `#[cfg(feature = "...")]` on enum variants and match arms
- Backend-specific wire types stay inside each backend module;
  Redfish handlers only use `backend::types`
- Handlers are async, take `State(state): State<Arc<AppState>>` via axum
- Per-VM mutable state persisted as JSON (write to temp + rename)

## Testing

- Unit tests in `#[cfg(test)]` blocks within each module
- Integration tests in `src/integration_tests.rs` using `MockBackend` +
  `axum::Router::oneshot()`
- Run tests after completing a logical unit of work, not after every edit
- CI runs fmt, clippy, build, and test via `.github/workflows/ci.yml`

## Commit conventions

- All commits must include `Signed-off-by` — use `git commit -s`
- Concise messages focused on the why, not the what
- No `git push --force`, `git reset --hard`, or destructive operations
  without explicit approval

## Adding a Redfish resource

1. Create `src/redfish/<resource>.rs` with handler functions and structs
2. Register routes in `src/redfish/mod.rs`
3. Add links from parent resources
4. Add schema refs to `data/metadata.xml`
5. Add integration tests in `src/integration_tests.rs`

## DMTF validation

`scripts/dmtf-validate.sh` runs the DMTF Redfish Service Validator
against a live instance. Reports go to `dmtf-reports/`. Conformance
details in `docs/conformance.md`.

## Do not

- Use `dyn VmmBackend` — use the `Backend` enum
- Add `.rustfmt.toml` or `clippy.toml`
- Expose backend-specific types in Redfish handlers
- Skip `#[serde(skip_serializing_if)]` on Option fields in Redfish structs
- Add `#[allow(dead_code)]` without justification — wire the code or delete it
