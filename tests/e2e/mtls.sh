#!/usr/bin/env bash
set -euo pipefail

# mTLS e2e test: verifies sidecar TLS + aggregator mTLS client auth.
# Requires: CRC running with KubeVirt, vbmc-rs Helm chart deployed in vbmc-system.

eval "$(crc oc-env)"

VBMC_NS="vbmc-system"
VM_NS="mtls-test"
VM_NAME="mtls-vm"
SYSTEM_ID="mtls-vm"

log() { echo "=== $1 ===" >&2; }

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

cleanup() {
    log "Cleaning up mTLS test resources"
    kill "$PF_PID" 2>/dev/null || true
    oc delete vm "$VM_NAME" -n "$VM_NS" --ignore-not-found 2>/dev/null
    oc delete namespace "$VM_NS" --ignore-not-found 2>/dev/null
    oc delete secret vbmc-rs-bmc-tls -n "$VBMC_NS" --ignore-not-found 2>/dev/null
    # Revert mTLS settings
    helm upgrade vbmc-rs charts/vbmc-rs/ \
        --set webhook.tlsSecret="" \
        --set aggregator.mtls.enabled=false \
        --reuse-values 2>/dev/null || true
    rm -f /tmp/bmc-ca.key /tmp/bmc-ca.crt /tmp/bmc-ca.srl \
          /tmp/sidecar.key /tmp/sidecar.crt /tmp/sidecar.csr \
          /tmp/agg-client.key /tmp/agg-client.crt /tmp/agg-client.csr
}
trap cleanup EXIT
PF_PID=""

PASS=0
FAIL=0

check() {
    local desc="$1" expected="$2"
    shift 2
    local status
    status=$(curl -s -o /dev/null -w "%{http_code}" "$@" 2>/dev/null) || status="000"
    if [ "$status" = "$expected" ]; then
        echo "  PASS: $desc (HTTP $status)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc (expected $expected, got $status)"
        FAIL=$((FAIL + 1))
    fi
}

log "Generating BMC mTLS certificates"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout /tmp/bmc-ca.key -out /tmp/bmc-ca.crt -days 1 -nodes \
    -subj "/CN=vbmc-rs BMC CA" 2>/dev/null

openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout /tmp/sidecar.key -out /tmp/sidecar.csr -nodes \
    -subj "/CN=vbmc-rs-sidecar" \
    -addext "subjectAltName=IP:0.0.0.0" 2>/dev/null
openssl x509 -req -in /tmp/sidecar.csr -CA /tmp/bmc-ca.crt -CAkey /tmp/bmc-ca.key \
    -CAcreateserial -out /tmp/sidecar.crt -days 1 \
    -copy_extensions copy 2>/dev/null

openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout /tmp/agg-client.key -out /tmp/agg-client.csr -nodes \
    -subj "/CN=vbmc-rs-aggregator" 2>/dev/null
openssl x509 -req -in /tmp/agg-client.csr -CA /tmp/bmc-ca.crt -CAkey /tmp/bmc-ca.key \
    -CAcreateserial -out /tmp/agg-client.crt -days 1 2>/dev/null

log "Creating VM namespace and BMC TLS secrets"
oc create namespace "$VM_NS" --dry-run=client -o yaml | oc apply -f -

# Sidecar server cert in VM namespace (webhook injects this into pods)
oc create secret generic vbmc-rs-bmc-tls -n "$VM_NS" \
    --from-file=ca.crt=/tmp/bmc-ca.crt \
    --from-file=tls.crt=/tmp/sidecar.crt \
    --from-file=tls.key=/tmp/sidecar.key \
    --dry-run=client -o yaml | oc apply -f -

# Aggregator client cert in vbmc-system (aggregator uses this for mTLS to sidecars)
oc create secret generic vbmc-rs-bmc-tls -n "$VBMC_NS" \
    --from-file=ca.crt=/tmp/bmc-ca.crt \
    --from-file=client.crt=/tmp/agg-client.crt \
    --from-file=client.key=/tmp/agg-client.key \
    --dry-run=client -o yaml | oc apply -f -

log "Upgrading Helm chart with mTLS"
helm upgrade vbmc-rs charts/vbmc-rs/ \
    --set webhook.tlsSecret=vbmc-rs-bmc-tls \
    --set aggregator.mtls.enabled=true \
    --reuse-values
oc rollout status deployment/vbmc-rs-webhook -n "$VBMC_NS" --timeout=60s

log "Creating test VM"
oc apply -n "$VM_NS" -f - <<EOF
apiVersion: kubevirt.io/v1
kind: VirtualMachine
metadata:
  name: ${VM_NAME}
  labels:
    vbmc-rs/system-id: ${SYSTEM_ID}
spec:
  runStrategy: Always
  template:
    metadata:
      labels:
        vbmc-rs/system-id: ${SYSTEM_ID}
    spec:
      domain:
        cpu:
          cores: 1
        memory:
          guest: "512Mi"
        devices:
          disks:
            - name: rootdisk
              disk:
                bus: virtio
      volumes:
        - name: rootdisk
          containerDisk:
            image: quay.io/containerdisks/fedora:latest
EOF

log "Waiting for VM to start"
wait_for "VMI Running" "oc get vmi ${VM_NAME} -n ${VM_NS} -o jsonpath='{.status.phase}' | grep -q Running" 300

LAUNCHER=$(oc get pods -n "$VM_NS" -o jsonpath='{.items[0].metadata.name}')
log "Launcher pod: $LAUNCHER"

log "Verifying sidecar has TLS volume mounted"
TLS_MOUNT=$(oc get pod "$LAUNCHER" -n "$VM_NS" \
    -o jsonpath='{.spec.containers[?(@.name=="vbmc-rs")].volumeMounts[?(@.name=="vbmc-tls")].mountPath}')
if [ "$TLS_MOUNT" = "/etc/vbmc-tls" ]; then
    echo "  PASS: sidecar has vbmc-tls volume at /etc/vbmc-tls"
    PASS=$((PASS + 1))
else
    echo "  FAIL: sidecar missing vbmc-tls volume (got: ${TLS_MOUNT:-none})"
    FAIL=$((FAIL + 1))
fi

log "Verifying sidecar config has TLS enabled"
CONFIG=$(oc exec -n "$VM_NS" "$LAUNCHER" -c vbmc-rs -- cat /tmp/vbmc-config.toml 2>/dev/null)
if echo "$CONFIG" | grep -q "tls_cert"; then
    echo "  PASS: sidecar config contains TLS settings"
    PASS=$((PASS + 1))
else
    echo "  FAIL: sidecar config missing TLS settings"
    FAIL=$((FAIL + 1))
fi

log "Testing via aggregator (mTLS to sidecar)"
oc port-forward -n "$VBMC_NS" svc/vbmc-rs-aggregator 18080:8080 &
PF_PID=$!
sleep 2
TOKEN=$(oc whoami -t)

check "Aggregator sees mTLS VM" "200" \
    -H "Authorization: Bearer $TOKEN" \
    http://localhost:18080/redfish/v1/Systems/${SYSTEM_ID}

kill $PF_PID 2>/dev/null || true
PF_PID=""

log "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
