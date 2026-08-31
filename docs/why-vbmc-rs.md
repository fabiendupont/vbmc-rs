# Why vbmc-rs

Several tools exist for simulating BMCs in virtual environments. This document explains why vbmc-rs exists alongside them and when to use each.

## The landscape

### VirtualBMC

[VirtualBMC](https://opendev.org/openstack/virtualbmc) is a Python tool from the OpenStack community that presents an IPMI interface backed by libvirt. It is actively maintained (v3.3.0, February 2026) and widely used in OpenStack CI.

VirtualBMC is IPMI-only. It does not speak Redfish. As the industry moves from IPMI to Redfish for server management, tools that only support IPMI cannot be used to test Redfish-based provisioning workflows.

### sushy-tools

[sushy-tools](https://opendev.org/openstack/sushy-tools) is the Redfish counterpart to VirtualBMC, also from the OpenStack community. It provides a Redfish frontend backed by libvirt or OpenStack.

sushy-tools covers a subset of Redfish resources and does not implement full OData compliance ($metadata, ETags, proper Link headers). It has a single backend (libvirt or OpenStack), no authentication, no TLS/mTLS, no event system, and no Kubernetes integration. The Metal3 community has noted recurring stability issues with sushy-tools in CI environments.

### redfishMockupServer

The DMTF's [redfishMockupServer](https://github.com/DMTF/Redfish-Mockup-Server) is a Python HTTP server that serves static mockup directories. It is read-only — power actions do not change state, PATCH requests are not persisted. It requires a Python runtime.

## Where vbmc-rs fits

vbmc-rs is designed for environments where Redfish fidelity matters:

**Standards conformance.** vbmc-rs passes the official DMTF Redfish Service Validator on every CI run. The conformance report is published on the project site. OData compliance (proper `$metadata`, `@odata.context`, ETags, Link headers) is enforced by a Tower middleware layer, not sprinkled across handlers.

**Multi-backend.** Four hypervisor backends (Cloud-Hypervisor, QEMU, libvirt, KubeVirt) plus a mockup backend. The backend abstraction is compile-time dispatched (no vtable overhead) with feature flags for each backend.

**Stateful mockup mode.** vbmc-rs can serve DMTF mockup directories as live Redfish services where power actions change state and PATCH requests persist — replacing both redfishMockupServer and sushy-tools for client testing.

**Production-grade security.** Session-based authentication with argon2 password hashing, role-based access control (Administrator, Operator, ReadOnly), account lockout, TLS and mTLS via rustls, certificate hot-reload, and JSONL audit logging.

**Kubernetes-native.** Sidecar injection webhook, aggregator with OAuth (TokenReview + SubjectAccessReview), mTLS between components, Helm chart. No other virtual BMC tool deploys natively on Kubernetes.

**Attestation.** Keylime, Trustee, and swtpm integration for remote attestation, exposed through Redfish ComponentIntegrity resources. Unique to vbmc-rs.

**Single binary.** Rust, statically compiled, no runtime dependencies (except libvirt-dev when using the libvirt backend). OCI images available for container deployments.

## When to use what

| Scenario | Recommended tool |
|----------|-----------------|
| Testing IPMI-based workflows against libvirt VMs | VirtualBMC |
| Testing Redfish provisioning (Ironic, MAAS, Tinkerbell) | vbmc-rs |
| Redfish client library development and CI | vbmc-rs (mockup mode) |
| KubeVirt VM management via Redfish | vbmc-rs (kubevirt backend) |
| Simulating BMC fleets for monitoring/dashboards | vbmc-rs (mockup mode) |
| Quick read-only mockup serving | redfishMockupServer (if stateful mutations are not needed) |
