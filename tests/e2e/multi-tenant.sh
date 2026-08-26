#!/usr/bin/env bash
set -euo pipefail

# Multi-tenant e2e test: verifies OAuth/SAR filtering across namespaces.
# Requires: CRC running with KubeVirt, vbmc-rs webhook + aggregator deployed.

eval "$(crc oc-env)"

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

PASS=0
FAIL=0

check_systems_count() {
    local desc="$1" token="$2" expected="$3"
    local count
    count=$(curl -s -H "Authorization: Bearer $token" http://localhost:8080/redfish/v1/Systems 2>&1 | \
        python3 -c "import sys,json; print(json.load(sys.stdin).get('Members@odata.count', -1))" 2>/dev/null || echo "-1")
    if [ "$count" = "$expected" ]; then
        echo "  PASS: $desc (count=$count)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc (expected $expected, got $count)"
        FAIL=$((FAIL + 1))
    fi
}

log "Creating tenant namespaces"
oc create namespace tenant-a --dry-run=client -o yaml | oc apply -f -
oc create namespace tenant-b --dry-run=client -o yaml | oc apply -f -

log "Creating tenant ServiceAccounts and RBAC"
for tenant in a b; do
    ns="tenant-${tenant}"
    oc create serviceaccount "user-${tenant}" -n "$ns" --dry-run=client -o yaml | oc apply -f -
    oc apply -f - <<EOF
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: vm-viewer
  namespace: ${ns}
rules:
  - apiGroups: ["kubevirt.io"]
    resources: ["virtualmachines"]
    verbs: ["get", "list"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: user-${tenant}-vm-viewer
  namespace: ${ns}
subjects:
  - kind: ServiceAccount
    name: user-${tenant}
    namespace: ${ns}
roleRef:
  kind: Role
  name: vm-viewer
  apiGroup: rbac.authorization.k8s.io
EOF
done

log "Creating VMs in each namespace"
for tenant in a b; do
    ns="tenant-${tenant}"
    vm_name="vm-${tenant}"
    oc apply -n "$ns" -f - <<EOF
apiVersion: kubevirt.io/v1
kind: VirtualMachine
metadata:
  name: ${vm_name}
  labels:
    vbmc-rs/system-id: ${vm_name}
spec:
  runStrategy: Always
  template:
    metadata:
      labels:
        vbmc-rs/system-id: ${vm_name}
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
done

log "Waiting for VMs to start (120s)"
wait_for "vm-a Running" "oc get vmi vm-a -n tenant-a -o jsonpath='{.status.phase}' | grep -q Running" 300
wait_for "vm-b Running" "oc get vmi vm-b -n tenant-b -o jsonpath='{.status.phase}' | grep -q Running" 300

log "Waiting for aggregator to discover both VMs (60s)"
sleep 60

log "Getting SA tokens"
TOKEN_A=$(oc create token user-a -n tenant-a --duration=600s)
TOKEN_B=$(oc create token user-b -n tenant-b --duration=600s)
TOKEN_ADMIN=$(oc whoami -t)

log "Port-forwarding to aggregator"
oc port-forward svc/vbmc-rs-aggregator 8080:8080 -n default &
PF_PID=$!
sleep 3

log "Testing multi-tenant isolation"

check_systems_count "admin sees all VMs" "$TOKEN_ADMIN" 3
check_systems_count "user-a sees only their VM" "$TOKEN_A" 1
check_systems_count "user-b sees only their VM" "$TOKEN_B" 1

log "Testing per-system access"
STATUS_A_OWN=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN_A" http://localhost:8080/redfish/v1/Systems/vm-a)
STATUS_A_OTHER=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN_A" http://localhost:8080/redfish/v1/Systems/vm-b)
STATUS_B_OWN=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN_B" http://localhost:8080/redfish/v1/Systems/vm-b)
STATUS_B_OTHER=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN_B" http://localhost:8080/redfish/v1/Systems/vm-a)

for check in \
    "user-a accesses own VM:${STATUS_A_OWN}:200" \
    "user-a denied other VM:${STATUS_A_OTHER}:403" \
    "user-b accesses own VM:${STATUS_B_OWN}:200" \
    "user-b denied other VM:${STATUS_B_OTHER}:403"; do
    IFS=: read -r desc actual expected <<< "$check"
    if [ "$actual" = "$expected" ]; then
        echo "  PASS: $desc (HTTP $actual)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc (expected $expected, got $actual)"
        FAIL=$((FAIL + 1))
    fi
done

kill $PF_PID 2>/dev/null || true

log "Cleaning up"
oc delete vm vm-a -n tenant-a --ignore-not-found
oc delete vm vm-b -n tenant-b --ignore-not-found
oc delete namespace tenant-a tenant-b --ignore-not-found

log "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
