#!/usr/bin/env bash
#
# Run the DMTF Redfish Service Validator against a local vbmc-rs instance.
#
# Usage:
#   ./scripts/dmtf-validate.sh              # build, start, validate, report
#   ./scripts/dmtf-validate.sh --no-build   # skip cargo build
#
# Prerequisites:
#   - python3 with venv support
#   - cargo / rustc
#
# The script will:
#   1. Build vbmc-rs (unless --no-build)
#   2. Set up a Python venv and install the DMTF validator
#   3. Start vbmc-rs on port 18000 with a test config
#   4. Run the validator against it
#   5. Print results and exit with the validator's exit code

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
VENV_DIR="$PROJECT_DIR/.venv-dmtf"
PORT=18000
HOST="http://127.0.0.1:$PORT"
CONFIG="$PROJECT_DIR/examples/config-test.toml"
LOG_DIR="$PROJECT_DIR/dmtf-reports"
VBMC_PID=""
DO_BUILD=true

for arg in "$@"; do
    case "$arg" in
        --no-build) DO_BUILD=false ;;
        *) echo "Unknown argument: $arg"; exit 1 ;;
    esac
done

cleanup() {
    if [ -n "$VBMC_PID" ] && kill -0 "$VBMC_PID" 2>/dev/null; then
        echo "Stopping vbmc-rs (PID $VBMC_PID)..."
        kill "$VBMC_PID" 2>/dev/null || true
        wait "$VBMC_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# ── Step 1: Build ──────────────────────────────────────────────────────

if [ "$DO_BUILD" = true ]; then
    echo "==> Building vbmc-rs..."
    cargo build --manifest-path "$PROJECT_DIR/Cargo.toml" --release 2>&1
    echo ""
fi

BINARY="$PROJECT_DIR/target/release/vbmc-rs"
if [ ! -x "$BINARY" ]; then
    echo "Error: binary not found at $BINARY"
    echo "Run without --no-build or build manually first."
    exit 1
fi

# ── Step 2: Python venv ────────────────────────────────────────────────

if [ ! -d "$VENV_DIR" ]; then
    echo "==> Creating Python venv at $VENV_DIR..."
    python3 -m venv "$VENV_DIR"
fi

echo "==> Installing/upgrading DMTF Redfish Service Validator..."
"$VENV_DIR/bin/pip" install --quiet --upgrade redfish_service_validator
echo ""

# ── Step 3: Generate temp config with state dir ────────────────────────

STATE_DIR=$(mktemp -d)
TEMP_CONFIG=$(mktemp --suffix=.toml)

# Copy test config and override state_directory
sed "s|^state_directory.*||" "$CONFIG" > "$TEMP_CONFIG"
echo "state_directory = \"$STATE_DIR\"" >> "$TEMP_CONFIG"

# ── Step 4: Start vbmc-rs ─────────────────────────────────────────────

echo "==> Starting vbmc-rs on $HOST..."
"$BINARY" -c "$TEMP_CONFIG" &
VBMC_PID=$!

# Wait for the server to be ready
MAX_WAIT=10
for i in $(seq 1 $MAX_WAIT); do
    if curl -sf "$HOST/redfish" > /dev/null 2>&1; then
        echo "    Server ready (attempt $i/$MAX_WAIT)"
        break
    fi
    if ! kill -0 "$VBMC_PID" 2>/dev/null; then
        echo "Error: vbmc-rs exited unexpectedly"
        wait "$VBMC_PID" || true
        exit 1
    fi
    sleep 1
done

if ! curl -sf "$HOST/redfish" > /dev/null 2>&1; then
    echo "Error: vbmc-rs did not become ready within ${MAX_WAIT}s"
    exit 1
fi

# Quick sanity check
echo "    Service root:"
curl -sf "$HOST/redfish/v1" | python3 -m json.tool | head -5
echo "    ..."
echo ""

# ── Step 5: Run DMTF Validator ─────────────────────────────────────────

mkdir -p "$LOG_DIR"

echo "==> Running DMTF Redfish Service Validator..."
echo "    Target: $HOST"
echo "    Reports: $LOG_DIR"
echo ""

set +e
"$VENV_DIR/bin/rf_service_validator" \
    --rhost "$HOST" \
    --user test \
    --password test \
    --authtype Basic \
    --nooemcheck \
    --logdir "$LOG_DIR" \
    2>&1 | tee "$LOG_DIR/validator-stdout.txt"
VALIDATOR_EXIT=$?
set -e

echo ""
echo "================================================================"

# ── Step 6: Parse results ─────────────────────────────────────────────

# Find reports (validator may put them in a timestamped subdirectory)
TEXT_LOG=$(find "$LOG_DIR" -name 'ConformanceLog_*.txt' -type f 2>/dev/null | sort -r | head -1)
HTML_REPORT=$(find "$LOG_DIR" -name 'ConformanceHtmlLog_*.html' -type f 2>/dev/null | sort -r | head -1)

if [ -n "$TEXT_LOG" ] && [ -f "$TEXT_LOG" ]; then
    echo ""
    echo "==> Validator Summary (from $TEXT_LOG):"
    echo ""
    tail -30 "$TEXT_LOG"
fi

# Extract actual fail count from the summary table in stdout (strip ANSI codes first)
ACTUAL_FAILS=$(sed 's/\x1b\[[0-9;]*m//g' "$LOG_DIR/validator-stdout.txt" 2>/dev/null \
    | grep -oP 'FAIL\s*\|\s*\K[0-9]+' 2>/dev/null \
    | head -1 || true)

echo ""
echo "================================================================"
echo "Validator exit code: $VALIDATOR_EXIT"
echo "Actual failures:     ${ACTUAL_FAILS:-unknown}"
echo "HTML report: $HTML_REPORT"
echo "Text report: $TEXT_LOG"
echo "================================================================"

# Clean up temp files
rm -f "$TEMP_CONFIG"
rm -rf "$STATE_DIR"

# Exit code 2 with 0 actual failures means informational issues only
if [ "$VALIDATOR_EXIT" -eq 2 ] && [ "${ACTUAL_FAILS:-1}" -eq 0 ]; then
    echo "Validator exit code 2 with 0 failures — treating as success."
    exit 0
fi

exit $VALIDATOR_EXIT
