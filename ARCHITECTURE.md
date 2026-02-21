# Architecture

This document describes the internal structure of vbmc-rs for contributors.

## Overview

vbmc-rs is a Redfish virtual BMC — it presents a standard Redfish REST API that maps to hypervisor operations on virtual machines. One instance manages multiple VMs (blade chassis model), all using the same backend type.

```
┌─────────────────────────────────────────────┐
│                HTTP Clients                  │
│          (curl, Redfish tools, BMaaS)        │
└──────────────────┬──────────────────────────┘
                   │ Redfish REST API
┌──────────────────▼──────────────────────────┐
│              axum Router                     │
│  ┌─────────────────────────────────────┐    │
│  │   OData Compliance Layer (Tower)     │    │
│  │   • OData-Version header             │    │
│  │   • Link header                      │    │
│  │   • HEAD → GET + strip body          │    │
│  └─────────────────────────────────────┘    │
│  ┌─────────────────────────────────────┐    │
│  │   Redfish Handlers (~30 modules)     │    │
│  │   systems, power, storage, memory,   │    │
│  │   ethernet, bios, managers, ...      │    │
│  └─────────────┬───────────────────────┘    │
└────────────────┼────────────────────────────┘
                 │ backend-agnostic types
┌────────────────▼────────────────────────────┐
│           VmmBackend trait                    │
│  ┌──────────┬──────────┬──────────┐         │
│  │   Cloud  │   QEMU   │ Libvirt  │         │
│  │Hypervisor│  (QMP)   │ (virsh)  │         │
│  └────┬─────┴────┬─────┴────┬─────┘         │
└───────┼──────────┼──────────┼───────────────┘
        │          │          │
   Unix socket  Unix socket  virsh CLI
        │          │          │
   ┌────▼───┐ ┌───▼────┐ ┌───▼────┐
   │  CH    │ │  QEMU  │ │libvirtd│
   │process │ │process │ │        │
   └────────┘ └────────┘ └────────┘
```

## Directory structure

```
src/
├── main.rs                  Entry point, CLI, backend dispatch
├── app_state.rs             Shared application state (Arc<AppState>)
├── config.rs                TOML configuration types and loading
├── state.rs                 Persistent per-VM state (JSON files)
├── tasks.rs                 Async task tracking
├── media.rs                 Virtual media image downloads
├── prometheus.rs            Prometheus metrics server
├── telemetry.rs             Request metrics middleware
│
├── backend/
│   ├── mod.rs               VmmBackend trait, Backend enum, BackendError
│   ├── types.rs             Backend-agnostic types (VmInfo, VmPowerState, ...)
│   ├── cloud_hypervisor/
│   │   ├── mod.rs           CloudHypervisorBackend + type conversions
│   │   ├── client.rs        Raw HTTP/1.1 over Unix socket
│   │   └── types.rs         Cloud-Hypervisor API wire types
│   ├── qemu/
│   │   ├── mod.rs           QemuBackend (manage-only)
│   │   ├── client.rs        QMP JSON protocol over Unix socket
│   │   └── types.rs         QMP response structs
│   └── libvirt/
│       ├── mod.rs           LibvirtBackend (virsh CLI)
│       └── xml.rs           Domain XML parser (quick-xml)
│
├── redfish/
│   ├── mod.rs               Router with ~60 routes + compliance layer
│   ├── types.rs             ODataId, Collection<T>, Status, StatusRollup
│   ├── error.rs             RedfishApiError → HTTP status + JSON body
│   ├── compliance.rs        OData headers Tower middleware
│   ├── odata.rs             $metadata (CSDL XML), /odata service doc
│   ├── service_root.rs      Service root with all resource links
│   ├── systems.rs           ComputerSystem collection + individual
│   ├── power.rs             Reset actions (On, Off, Reboot, ...)
│   ├── processors.rs        CPU information
│   ├── memory.rs            Memory (DIMM) information
│   ├── ethernet.rs          Network interfaces
│   ├── storage.rs           SimpleStorage (legacy)
│   ├── storage_controllers.rs  Full Storage/Drives/Volumes
│   ├── pcie.rs              PCIe Devices and Functions
│   ├── bios.rs              BIOS settings with pending changes
│   ├── secure_boot.rs       Secure Boot configuration
│   ├── virtual_media.rs     CD/DVD insertion and ejection
│   ├── log_service.rs       System console + manager audit logs
│   ├── managers.rs          BMC manager resource
│   ├── trusted_component.rs Chassis + trusted components
│   ├── chassis_power.rs     Synthetic power readings
│   ├── chassis_thermal.rs   Synthetic thermal readings
│   ├── network_adapter.rs   Chassis-level network adapters
│   ├── update_service.rs    Firmware inventory
│   ├── license_service.rs   License management
│   ├── session_service.rs   Session CRUD
│   ├── account_service.rs   User accounts + roles
│   ├── event_service.rs     Event subscriptions + SSE
│   ├── task_service.rs      Task monitoring
│   ├── certificate_service.rs  Certificate management
│   ├── security_policy.rs   Security policy settings
│   ├── telemetry.rs         Telemetry metric reports
│   └── component_integrity.rs  Attestation status
│
├── auth/
│   ├── mod.rs               AuthenticatedUser extractor
│   ├── accounts.rs          AccountStore with argon2 password hashing
│   ├── sessions.rs          SessionStore with token generation + sweeper
│   └── rbac.rs              Role-based privilege mapping
│
├── events/
│   ├── mod.rs               EventBus (tokio broadcast channel)
│   ├── subscriptions.rs     SubscriptionStore + webhook delivery
│   ├── audit_log.rs         JSONL audit log writer
│   └── registry.rs          Event message ID constants
│
├── attestation/
│   ├── mod.rs               Attestation polling coordinator
│   ├── trust_chain.rs       Verification status types
│   ├── keylime.rs           Keylime agent client
│   └── trustee.rs           Trustee attestation client
│
├── integration_tests.rs     HTTP-level tests with MockBackend
│
data/
└── metadata.xml             CSDL metadata document

examples/
├── config.toml              Cloud-Hypervisor example config
├── config-qemu.toml         QEMU example config
└── config-libvirt.toml      Libvirt example config
```

## Key design decisions

### Backend abstraction

The `VmmBackend` trait defines 11 async methods (`vm_info`, `vm_create`, `vm_boot`, etc.) that all return backend-agnostic types from `backend::types`. The `Backend` enum dispatches to concrete implementations at zero overhead (no `dyn` / vtable).

Backend-specific wire types (CH `VmConfig`, QMP `QmpStatus`, libvirt domain XML) are converted to `backend::types::VmInfo` inside each backend module. Redfish handlers never touch backend-specific types.

### Feature gating

Each backend is behind a compile-time feature flag:

- `cloud-hypervisor` (default) — always available
- `qemu` — QMP client compiled in
- `libvirt` — adds `quick-xml` dependency

The `Backend` enum variants and their match arms use `#[cfg(feature = "...")]`.

### Multi-system model

Configuration maps system IDs to backend connection details. For Cloud-Hypervisor and QEMU this is a Unix socket path per VM. For libvirt it's a connection URI + domain name.

`AppState` holds a `DashMap<String, VmState>` for concurrent per-system state access with per-system locks for atomic power operations.

### State persistence

Each VM's mutable state (`VmState`) is persisted as a JSON file in `state_directory`. This includes boot override settings, virtual media insertion state, secure boot toggle, BIOS settings, and licenses. State is loaded on startup and saved atomically (write to temp + rename).

### OData compliance

A Tower middleware layer (`ODataComplianceLayer`) wraps the entire router:

- Adds `OData-Version: 4.0` and `Link: </redfish/v1/$metadata>; rel=describedby` to every response
- Converts HEAD requests to GET internally, then strips the response body

The `$metadata` endpoint serves a static CSDL XML document (`data/metadata.xml`) referencing all implemented Redfish schema namespaces.

### Authentication

When `auth.enabled = true`:

- Sessions created via `POST /SessionService/Sessions` return an `X-Auth-Token`
- Subsequent requests authenticated via `X-Auth-Token` header or HTTP Basic
- Passwords hashed with argon2
- Role-based access control: Administrator, Operator, ReadOnly
- Account lockout after configurable failed attempts
- Background sweeper removes expired sessions

### Event system

`EventBus` uses a `tokio::sync::broadcast` channel. Power actions, virtual media changes, and attestation state changes emit `RedfishEvent`s. Subscribers include:

- Audit log writer (JSONL file)
- Webhook delivery (with exponential backoff)
- SSE stream (`/EventService/SSE`)

## Testing

Tests are organized in three layers:

1. **Pure function tests** (in each module's `#[cfg(test)]` block) — type conversions, enum mappings, parsing, serialization, string formatting. No async, no I/O.

2. **State management tests** — `TaskManager` lifecycle, `AccountStore` password verification, `SessionStore` token handling, `EventBus` broadcast, `SubscriptionStore` CRUD. May use `tempfile` for disk I/O.

3. **Integration tests** (`src/integration_tests.rs`) — full HTTP request/response testing via `axum::Router::oneshot()` with a `MockBackend`. Tests OData headers, resource JSON structure, PATCH state mutations, error responses, and routing.

The `MockBackend` (compiled only in test builds via `#[cfg(test)]`) implements `VmmBackend` with configurable per-system `VmInfo` responses and no-op mutation methods.

## Adding a new Redfish resource

1. Create `src/redfish/<resource>.rs` with handler functions
2. Add resource structs with `#[serde(rename = "...")]` for Redfish property names
3. Register routes in `src/redfish/mod.rs`
4. Add links from parent resources (e.g., `ComputerSystem` struct in `systems.rs`)
5. If the resource references schema types, add them to `data/metadata.xml`
6. Add integration tests in `src/integration_tests.rs`

## Adding a new backend

1. Create `src/backend/<name>/mod.rs` implementing `VmmBackend`
2. Add wire types in `src/backend/<name>/types.rs`
3. Add `<name> = [...]` to `[features]` in `Cargo.toml`
4. Add variant to `Backend` enum and all match arms in `src/backend/mod.rs` (behind `#[cfg(feature = "...")]`)
5. Add config variant to `BackendType` in `src/config.rs`
6. Add construction logic in `src/main.rs`
7. Add example config in `examples/`
