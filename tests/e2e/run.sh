#!/usr/bin/env bash
set -euo pipefail

PROVIDER="${E2E_PROVIDER:-crc}"
VBMC_IMAGE="${VBMC_IMAGE:-localhost/vbmc-rs-kubevirt-sidecar:test}"
NAMESPACE="${NAMESPACE:-default}"
VM_NAME="${VM_NAME:-test-vm}"
SYSTEM_ID="${SYSTEM_ID:-test-vm}"

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

# --- Cluster setup ---

setup_crc() {
    log "Using CRC (OpenShift Local)"
    if ! crc status 2>&1 | grep -q "Running"; then
        log "Starting CRC"
        crc start
    fi
    eval "$(crc oc-env)"
    local CRC_PASS
    CRC_PASS=$(crc console --credentials 2>&1 | grep kubeadmin | grep -oP '(?<=-p )\S+')
    oc login -u kubeadmin -p "$CRC_PASS" https://api.crc.testing:6443 --insecure-skip-tls-verify 2>/dev/null

    log "Checking KubeVirt"
    if ! oc get kv -A -o jsonpath='{.items[0].status.phase}' 2>/dev/null | grep -q Deployed; then
        log "KubeVirt not found — CRC must have the OpenShift Virtualization operator installed"
        exit 1
    fi

    log "Pushing images to CRC internal registry"
    oc patch configs.imageregistry.operator.openshift.io/cluster \
        --patch '{"spec":{"defaultRoute":true}}' --type=merge 2>/dev/null
    sleep 3
    local REGISTRY
    REGISTRY=$(oc get route default-route -n openshift-image-registry \
        -o jsonpath='{.spec.host}' 2>/dev/null)
    oc whoami -t | podman login --tls-verify=false -u kubeadmin --password-stdin "$REGISTRY"

    podman tag "$VBMC_IMAGE" "${REGISTRY}/${NAMESPACE}/vbmc-rs-sidecar:test"
    podman push --tls-verify=false "${REGISTRY}/${NAMESPACE}/vbmc-rs-sidecar:test"
    VBMC_IMAGE="image-registry.openshift-image-registry.svc:5000/${NAMESPACE}/vbmc-rs-sidecar:test"

    local WEBHOOK_LOCAL="${WEBHOOK_IMAGE:-localhost/vbmc-rs-webhook:test}"
    podman tag "$WEBHOOK_LOCAL" "${REGISTRY}/${NAMESPACE}/vbmc-rs-webhook:test"
    podman push --tls-verify=false "${REGISTRY}/${NAMESPACE}/vbmc-rs-webhook:test"
    WEBHOOK_IMAGE="image-registry.openshift-image-registry.svc:5000/${NAMESPACE}/vbmc-rs-webhook:test"

    KUBECTL="oc"
}

setup_kind() {
    local CLUSTER_NAME="${CLUSTER_NAME:-vbmc-test}"
    log "Using Kind cluster: $CLUSTER_NAME"
    kind create cluster --name "$CLUSTER_NAME" --wait 60s

    local KUBEVIRT_VERSION
    KUBEVIRT_VERSION="${KUBEVIRT_VERSION:-$(curl -sL https://api.github.com/repos/kubevirt/kubevirt/releases/latest | grep tag_name | cut -d'"' -f4)}"

    log "Installing KubeVirt $KUBEVIRT_VERSION"
    kubectl create -f "https://github.com/kubevirt/kubevirt/releases/download/${KUBEVIRT_VERSION}/kubevirt-operator.yaml"
    wait_for "virt-operator" "kubectl -n kubevirt get deployment virt-operator -o jsonpath='{.status.readyReplicas}' | grep -q '[1-9]'" 120

    log "Creating KubeVirt CR with emulation enabled"
    curl -sL "https://github.com/kubevirt/kubevirt/releases/download/${KUBEVIRT_VERSION}/kubevirt-cr.yaml" \
        | kubectl apply -f - --dry-run=client -o json \
        | jq '.spec.configuration.developerConfiguration.useEmulation = true' \
        | kubectl apply -f -

    log "Waiting for KubeVirt to be ready"
    wait_for "KubeVirt" "kubectl -n kubevirt get kv kubevirt -o jsonpath='{.status.phase}' | grep -q Deployed" 600

    log "Loading sidecar image into Kind"
    if command -v docker &>/dev/null && docker info &>/dev/null; then
        kind load docker-image "$VBMC_IMAGE" --name "$CLUSTER_NAME"
    else
        podman save "$VBMC_IMAGE" -o /tmp/vbmc-sidecar.tar
        kind load image-archive /tmp/vbmc-sidecar.tar --name "$CLUSTER_NAME"
        rm -f /tmp/vbmc-sidecar.tar
    fi

    KUBECTL="kubectl"
}

case "$PROVIDER" in
    crc) setup_crc ;;
    kind) setup_kind ;;
    *) echo "Unknown provider: $PROVIDER (use 'crc' or 'kind')" >&2; exit 1 ;;
esac

# --- Deploy test VM with vbmc-rs sidecar ---

log "Creating vbmc-rs ConfigMap"
$KUBECTL create configmap "vbmc-rs-${VM_NAME}" -n "$NAMESPACE" --from-literal=config.toml="
backend = \"libvirt\"

[server]
bind_address = \"0.0.0.0\"
port = 8000

[systems.${SYSTEM_ID}]
name = \"Test VM\"
connection_uri = \"qemu:///session\"
domain_name = \"${NAMESPACE}_${VM_NAME}\"

[systems.${SYSTEM_ID}.hardware]
cpu_count = 1
memory_mib = 1024
" --dry-run=client -o yaml | $KUBECTL apply -f -

log "Deploying vbmc-rs webhook"
WEBHOOK_IMAGE="${WEBHOOK_IMAGE:-localhost/vbmc-rs-webhook:test}"
# Generate self-signed cert for the webhook
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout /tmp/webhook-key.pem -out /tmp/webhook-cert.pem -days 1 -nodes \
    -subj "/CN=vbmc-rs-webhook.${NAMESPACE}.svc" \
    -addext "subjectAltName=DNS:vbmc-rs-webhook.${NAMESPACE}.svc" 2>/dev/null
CA_BUNDLE=$(base64 -w0 < /tmp/webhook-cert.pem)

$KUBECTL create secret tls vbmc-rs-webhook-tls -n "$NAMESPACE" \
    --cert=/tmp/webhook-cert.pem --key=/tmp/webhook-key.pem \
    --dry-run=client -o yaml | $KUBECTL apply -f -
rm -f /tmp/webhook-key.pem /tmp/webhook-cert.pem

$KUBECTL apply -n "$NAMESPACE" -f - <<EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: vbmc-rs-webhook
spec:
  replicas: 1
  selector:
    matchLabels:
      app: vbmc-rs-webhook
  template:
    metadata:
      labels:
        app: vbmc-rs-webhook
    spec:
      containers:
        - name: webhook
          image: ${WEBHOOK_IMAGE}
          command: ["/vbmc-rs-webhook"]
          args:
            - --cert=/etc/webhook/tls/tls.crt
            - --key=/etc/webhook/tls/tls.key
            - --sidecar-image=${VBMC_IMAGE}
          ports:
            - containerPort: 8443
          volumeMounts:
            - name: tls
              mountPath: /etc/webhook/tls
              readOnly: true
      volumes:
        - name: tls
          secret:
            secretName: vbmc-rs-webhook-tls
---
apiVersion: v1
kind: Service
metadata:
  name: vbmc-rs-webhook
spec:
  selector:
    app: vbmc-rs-webhook
  ports:
    - port: 443
      targetPort: 8443
EOF

wait_for "webhook ready" "$KUBECTL get deployment vbmc-rs-webhook -n ${NAMESPACE} -o jsonpath='{.status.readyReplicas}' | grep -q '[1-9]'" 120

$KUBECTL apply -f - <<EOF
apiVersion: admissionregistration.k8s.io/v1
kind: MutatingWebhookConfiguration
metadata:
  name: vbmc-rs-sidecar-injector
webhooks:
  - name: vbmc-rs-sidecar-injector.kubevirt.io
    clientConfig:
      service:
        name: vbmc-rs-webhook
        namespace: ${NAMESPACE}
        path: /mutate
      caBundle: ${CA_BUNDLE}
    rules:
      - operations: ["CREATE"]
        apiGroups: [""]
        apiVersions: ["v1"]
        resources: ["pods"]
    objectSelector:
      matchLabels:
        kubevirt.io: virt-launcher
    failurePolicy: Ignore
    sideEffects: None
    admissionReviewVersions: ["v1"]
EOF

log "Creating VirtualMachine"
$KUBECTL apply -n "$NAMESPACE" -f - <<EOF
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
EOF

log "Waiting for VM to start"
wait_for "VMI Running" "$KUBECTL get vmi ${VM_NAME} -n ${NAMESPACE} -o jsonpath='{.status.phase}' | grep -q Running" 300

log "Waiting for virt-launcher pod"
LAUNCHER_POD=""
wait_for "launcher pod" "$KUBECTL get pods -n ${NAMESPACE} -l 'kubevirt.io/domain=${VM_NAME}' -o jsonpath='{.items[0].metadata.name}' | grep -q ." 120
LAUNCHER_POD=$($KUBECTL get pods -n "$NAMESPACE" -l "kubevirt.io/domain=${VM_NAME}" -o jsonpath='{.items[0].metadata.name}')
log "Launcher pod: $LAUNCHER_POD"

log "Port-forwarding to sidecar"
$KUBECTL port-forward -n "$NAMESPACE" "pod/${LAUNCHER_POD}" 8000:8000 &
PF_PID=$!
sleep 3

# --- Test Redfish endpoints ---

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

# --- Cleanup test resources (leave cluster running) ---

log "Cleaning up test resources"
$KUBECTL delete mutatingwebhookconfiguration vbmc-rs-sidecar-injector --ignore-not-found
$KUBECTL delete vm "$VM_NAME" -n "$NAMESPACE" --ignore-not-found
$KUBECTL delete deployment vbmc-rs-webhook -n "$NAMESPACE" --ignore-not-found
$KUBECTL delete service vbmc-rs-webhook -n "$NAMESPACE" --ignore-not-found
$KUBECTL delete secret vbmc-rs-webhook-tls -n "$NAMESPACE" --ignore-not-found
$KUBECTL delete configmap "vbmc-rs-${VM_NAME}" -n "$NAMESPACE" --ignore-not-found

log "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
