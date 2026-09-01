# Contributing to vbmc-rs

Contributions are welcome. Here's how to get started.

## Building

```sh
# Install libvirt-dev (needed for --all-features)
sudo apt-get install -y libvirt-dev   # Debian/Ubuntu
sudo dnf install -y libvirt-devel     # Fedora

# Build and test
cargo build --all-features
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

## Code style

- `cargo fmt` defaults — no `.rustfmt.toml`
- `cargo clippy --all-features -- -D warnings` must pass with zero warnings
- No comments unless the WHY is non-obvious
- No bare `unwrap()` in production code

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the internal structure, backend abstraction, and how to add new Redfish resources or backends.

## Commits

- All commits must include `Signed-off-by` — use `git commit -s`
- Concise messages focused on the why

## Testing

- Unit tests in `#[cfg(test)]` blocks within each module
- Integration tests in `src/integration_tests.rs` using `MockBackend`
- Run the DMTF Redfish Service Validator: `bash scripts/dmtf-validate.sh`
