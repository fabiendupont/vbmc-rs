#!/usr/bin/env bash
set -euo pipefail

CLUSTER_NAME="${CLUSTER_NAME:-vbmc-test}"
KUBEVIRT_VERSION="${KUBEVIRT_VERSION:-$(curl -sL https://api.github.com/repos/kubevirt/kubevirt/releases/latest | grep tag_name | cut -d'"' -f4)}"
SIDECAR_IMAGE="${SIDECAR_IMAGE:-localhost/vbmc-rs-kubevirt-sidecar:test}"
NAMESPACE="${NAMESPACE:-default}"
VM_NAME="${VM_NAME:-test-vm}"
SYSTEM_ID="${SYSTEM_ID:-test-vm}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

log() { echo "=== $1 ===" >&2; }

cleanup() {
    log "Cleanup"
    kind delete cluster --name "$CLUSTER_NAME" 2>/dev/null || true
}

wait_for() {
    local desc="$1" cmd="$2" timeout="${3:-120}"
    local elapsed=0
    while ! eval "$cmd" &>/dev/null; do
        sleep 5
        elapsed=$((elapsed + 5))
        if [ "$elapsed" -ge "$timeout" ]; then
            echo "FAIL: timed out waiting for $desc" >&2
            return 1
        fi
    done
}

trap cleanup EXIT

log "Creating Kind cluster"
kind create cluster --name "$CLUSTER_NAME" --wait 60s

log "Installing KubeVirt $KUBEVIRT_VERSION"
kubectl create -f "https://github.com/kubevirt/kubevirt/releases/download/${KUBEVIRT_VERSION}/kubevirt-operator.yaml"
kubectl create -f "https://github.com/kubevirt/kubevirt/releases/download/${KUBEVIRT_VERSION}/kubevirt-cr.yaml"

log "Enabling software emulation"
kubectl -n kubevirt patch kubevirt kubevirt --type=merge \
    --patch '{"spec":{"configuration":{"developerConfiguration":{"useEmulation":true}}}}'

log "Waiting for KubeVirt to be ready"
wait_for "KubeVirt" "kubectl -n kubevirt get kv kubevirt -o jsonpath='{.status.phase}' | grep -q Deployed" 300

log "Loading sidecar image into Kind"
kind load docker-image "$SIDECAR_IMAGE" --name "$CLUSTER_NAME"

log "Creating sidecar ConfigMap"
kubectl create configmap "vbmc-rs-${VM_NAME}" --from-literal=config.toml="
backend = \"libvirt\"

[server]
bind_address = \"0.0.0.0\"
port = 8000

[systems.${SYSTEM_ID}]
name = \"Test VM\"
connection_uri = \"qemu:///system\"
domain_name = \"${NAMESPACE}_${VM_NAME}\"

[systems.${SYSTEM_ID}.hardware]
cpu_count = 1
memory_mib = 1024
"

log "Creating VirtualMachine with sidecar"
kubectl apply -f - <<EOF
apiVersion: kubevirt.io/v1
kind: VirtualMachine
metadata:
  name: ${VM_NAME}
  labels:
    app.kubernetes.io/name: vbmc-rs-sidecar
    vbmc-rs/system-id: ${SYSTEM_ID}
spec:
  running: true
  template:
    metadata:
      labels:
        app.kubernetes.io/name: vbmc-rs-sidecar
        vbmc-rs/system-id: ${SYSTEM_ID}
    spec:
      domain:
        cpu:
          cores: 1
        memory:
          guest: "1Gi"
        devices:
          disks:
            - name: rootdisk
              disk:
                bus: virtio
      volumes:
        - name: rootdisk
          containerDisk:
            image: quay.io/containerdisks/fedora:latest
      containers:
        - name: vbmc-rs
          image: ${SIDECAR_IMAGE}
          args: ["-c", "/etc/vbmc-rs/config.toml"]
          ports:
            - containerPort: 8000
          volumeMounts:
            - name: vbmc-config
              mountPath: /etc/vbmc-rs
              readOnly: true
            - name: vbmc-state
              mountPath: /var/lib/vbmc-rs
            - name: libvirt-sock
              mountPath: /var/run/libvirt
      volumes:
        - name: vbmc-config
          configMap:
            name: vbmc-rs-${VM_NAME}
        - name: vbmc-state
          emptyDir: {}
        - name: libvirt-sock
          emptyDir: {}
EOF

log "Waiting for VM to start"
wait_for "VMI Running" "kubectl get vmi ${VM_NAME} -o jsonpath='{.status.phase}' | grep -q Running" 300

log "Waiting for sidecar to be ready"
wait_for "sidecar port" "kubectl exec deploy/${VM_NAME} -c vbmc-rs -- test -f /proc/1/status" 60

SIDECAR_POD=$(kubectl get pods -l "vbmc-rs/system-id=${SYSTEM_ID}" -o jsonpath='{.items[0].metadata.name}')
log "Sidecar pod: $SIDECAR_POD"

log "Port-forwarding to sidecar"
kubectl port-forward "pod/${SIDECAR_POD}" 8000:8000 &
PF_PID=$!
sleep 3

BASE="http://localhost:8000"
PASS=0
FAIL=0

check() {
    local desc="$1" method="$2" path="$3" expected_status="$4" body="${5:-}"
    local args=(-s -o /tmp/e2e-body -w '%{http_code}' -X "$method")
    if [ -n "$body" ]; then
        args+=(-H "Content-Type: application/json" -d "$body")
    fi
    local status
    status=$(curl "${args[@]}" "${BASE}${path}")
    if [ "$status" = "$expected_status" ]; then
        echo "  PASS: $desc (HTTP $status)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc (expected $expected_status, got $status)"
        echo "    Body: $(cat /tmp/e2e-body)"
        FAIL=$((FAIL + 1))
    fi
}

log "Testing Redfish endpoints"

check "Service root" GET "/redfish/v1" 200
check "Systems collection" GET "/redfish/v1/Systems" 200
check "System info" GET "/redfish/v1/Systems/${SYSTEM_ID}" 200
check "Processors" GET "/redfish/v1/Systems/${SYSTEM_ID}/Processors" 200
check "Memory" GET "/redfish/v1/Systems/${SYSTEM_ID}/Memory" 200
check "Ethernet interfaces" GET "/redfish/v1/Systems/${SYSTEM_ID}/EthernetInterfaces" 200
check "Storage" GET "/redfish/v1/Systems/${SYSTEM_ID}/Storage" 200
check "SecureBoot" GET "/redfish/v1/Systems/${SYSTEM_ID}/SecureBoot" 200
check "BIOS" GET "/redfish/v1/Systems/${SYSTEM_ID}/Bios" 200
check "Chassis" GET "/redfish/v1/Chassis" 200
check "Managers" GET "/redfish/v1/Managers" 200
check "Nonexistent system" GET "/redfish/v1/Systems/bogus" 404

check "Graceful shutdown" POST "/redfish/v1/Systems/${SYSTEM_ID}/Actions/ComputerSystem.Reset" 200 \
    '{"ResetType":"GracefulShutdown"}'

sleep 5

check "Power on" POST "/redfish/v1/Systems/${SYSTEM_ID}/Actions/ComputerSystem.Reset" 200 \
    '{"ResetType":"On"}'

kill $PF_PID 2>/dev/null || true

log "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
