# Mockup Mode

vbmc-rs can serve a DMTF Redfish mockup directory as a live Redfish service, with support for state mutations (power actions, BIOS settings, boot override). This makes it useful for testing Redfish client libraries, BMaaS integrations, and CI pipelines without real hardware or hypervisors.

## Quick start

```sh
# Serve a mockup directory
vbmc-rs -c examples/config-mockup.toml

# Test it
curl -s http://localhost:8000/redfish/v1/Systems | jq .
curl -X POST http://localhost:8000/redfish/v1/Systems/node-01/Actions/ComputerSystem.Reset \
  -H 'Content-Type: application/json' \
  -d '{"ResetType": "ForceOff"}'
```

## Configuration

```toml
backend = "mockup"
mockup_directory = "/path/to/mockup"

[server]
bind_address = "0.0.0.0"
port = 8000

[auth]
enabled = false
```

## Mockup format

The mockup directory follows the DMTF Redfish Mockup format — a directory tree mirroring the Redfish URI structure, with `index.json` files containing the JSON response for each resource:

```
mockup/
  redfish/
    index.json                              → GET /redfish
    v1/
      index.json                            → GET /redfish/v1
      Systems/
        index.json                          → GET /redfish/v1/Systems
        node-01/
          index.json                        → GET /redfish/v1/Systems/node-01
          Processors/
            index.json                      → GET /redfish/v1/Systems/node-01/Processors
          ...
```

## Creating a mockup from a real BMC

Use the DMTF Redfish Mockup Creator to scrape a live BMC:

```sh
pip install redfish
python3 -m redfish.mockup_creator \
  --rhost https://bmc-hostname \
  --user admin --password password \
  --Auth Session \
  --Dir ./my-bmc-mockup
```

Or manually with curl:

```sh
# Scrape a specific resource
mkdir -p mockup/redfish/v1/Systems/1
curl -sk https://bmc-hostname/redfish/v1/Systems/1 \
  -u admin:password > mockup/redfish/v1/Systems/1/index.json
```

## State mutations

The mockup backend supports these mutations on the in-memory JSON:

| Action | Effect |
|--------|--------|
| `ComputerSystem.Reset` (On/ForceOn) | Sets `PowerState` to `"On"` |
| `ComputerSystem.Reset` (ForceOff/GracefulShutdown) | Sets `PowerState` to `"Off"` |
| `PATCH /Systems/{id}` | Merges patch body into stored JSON |
| `PATCH /Systems/{id}/Bios/Settings` | Merges BIOS attributes |

Any resource served from the mockup supports GET. Resources not in the mockup directory return 404.

## Use with libredfish

vbmc-rs in mockup mode can replace the Python `redfishMockupServer.py` used by Redfish client libraries:

```sh
# Instead of:
python redfishMockupServer.py --port 8000 --dir mockups/dell/ --ssl

# Use:
vbmc-rs -c config-mockup.toml
```

Advantages over the Python mockup server:
- Power actions actually change state
- PATCH requests persist changes
- Full OData compliance (ETags, @odata.context, $metadata)
- TLS and mTLS support
- Single static binary, no Python runtime
