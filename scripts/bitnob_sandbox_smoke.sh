#!/usr/bin/env bash
set -euo pipefail

# HANBOVA M3B.3B — BITNOB SANDBOX SMOKE TEST SCRIPT
#
# Usage:
#   PROVIDER_MODE=sandbox BITNOB_CLIENT_ID=... BITNOB_CLIENT_SECRET=... ./scripts/bitnob_sandbox_smoke.sh
# or ensure .env contains BITNOB_CLIENT_ID and BITNOB_CLIENT_SECRET.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Load .env if present and variables not yet in environment
if [ -f "${ROOT_DIR}/.env" ]; then
  # Read variables without exporting credentials to trace logs
  set +a
  # Source .env safely
  # shellcheck disable=SC1091
  source <(grep -E '^(BITNOB_CLIENT_ID|BITNOB_CLIENT_SECRET|PROVIDER_MODE|HANBOVA_API_HOST|HANBOVA_API_PORT)=' "${ROOT_DIR}/.env" || true)
fi

PROVIDER_MODE="${PROVIDER_MODE:-sandbox}"
API_HOST="${HANBOVA_API_HOST:-127.0.0.1}"
API_PORT="${HANBOVA_API_PORT:-8080}"
CLIENT_ID="${BITNOB_CLIENT_ID:-}"
CLIENT_SECRET="${BITNOB_CLIENT_SECRET:-}"

echo "=================================================="
echo "HANBOVA M3B.3B BITNOB SANDBOX SMOKE TEST"
echo "=================================================="
echo "Provider Mode : ${PROVIDER_MODE}"
echo "Corridor      : USDT -> NGN (Market: NG)"

if [ -z "${CLIENT_ID}" ] || [ -z "${CLIENT_SECRET}" ]; then
  echo "Status        : FAILED TO CONNECT"
  echo "Reason        : Missing BITNOB_CLIENT_ID or BITNOB_CLIENT_SECRET"
  echo ""
  echo "To connect against genuine Bitnob Sandbox:"
  echo "  export PROVIDER_MODE=sandbox"
  echo "  export BITNOB_CLIENT_ID=\"your-client-id\""
  echo "  export BITNOB_CLIENT_SECRET=\"your-client-secret\""
  echo "  ./scripts/bitnob_sandbox_smoke.sh"
  echo "=================================================="
  exit 1
fi

# Check if the Hanbova API backend is already running locally on the configured port
API_ENDPOINT="http://${API_HOST}:${API_PORT}/api/v1/rates/hanbova?market=NG&asset=USDT&currency=NGN"

if curl -s -m 2 "http://${API_HOST}:${API_PORT}/health" >/dev/null 2>&1; then
  echo "Target        : Running Hanbova API (${API_ENDPOINT})"
  RESPONSE=$(curl -s -w "\n%{http_code}" "${API_ENDPOINT}")
  HTTP_STATUS=$(echo "${RESPONSE}" | tail -n 1)
  BODY=$(echo "${RESPONSE}" | sed '$d')

  if [ "${HTTP_STATUS}" -eq 200 ]; then
    ENV=$(echo "${BODY}" | grep -o '"environment":"[^"]*' | cut -d'"' -f4 || echo "unknown")
    PROV=$(echo "${BODY}" | grep -o '"provider":"[^"]*' | cut -d'"' -f4 || echo "unknown")
    IS_LIVE=$(echo "${BODY}" | grep -o '"is_live":[^,}]*' | cut -d':' -f2 || echo "true")
    RATE=$(echo "${BODY}" | grep -o '"rate":[^,}]*' | cut -d':' -f2 || echo "0")

    if [ "${ENV}" = "sandbox" ] && [ "${PROV}" = "bitnob" ] && [ "${IS_LIVE}" = "false" ]; then
      echo "=================================================="
      echo "RESULT: REAL BITNOB SANDBOX RESPONSE"
      echo "Provider    : ${PROV}"
      echo "Environment : ${ENV}"
      echo "Rate        : ${RATE}"
      echo "is_live     : ${IS_LIVE}"
      echo "Raw Payload : ${BODY}"
      echo "=================================================="
      exit 0
    else
      echo "=================================================="
      echo "RESULT: FAILED TO CONNECT (Unexpected semantics: env=${ENV}, prov=${PROV}, is_live=${IS_LIVE})"
      echo "Body: ${BODY}"
      echo "=================================================="
      exit 1
    fi
  else
    echo "=================================================="
    echo "RESULT: FAILED TO CONNECT (HTTP ${HTTP_STATUS})"
    echo "Body: ${BODY}"
    echo "=================================================="
    exit 1
  fi
else
  echo "Target        : Direct BitnobRateProvider Sandbox Runner (cargo test)"
  echo "Attempting real Bitnob sandbox request..."
  
  if PROVIDER_MODE=sandbox \
     BITNOB_CLIENT_ID="${CLIENT_ID}" \
     BITNOB_CLIENT_SECRET="${CLIENT_SECRET}" \
     cargo test -p hanbova-api --test bitnob_sandbox_test -- test_real_bitnob_sandbox_connectivity --ignored --nocapture; then
    echo "=================================================="
    echo "RESULT: REAL BITNOB SANDBOX RESPONSE"
    echo "=================================================="
    exit 0
  else
    echo "=================================================="
    echo "RESULT: FAILED TO CONNECT"
    echo "=================================================="
    exit 1
  fi
fi
