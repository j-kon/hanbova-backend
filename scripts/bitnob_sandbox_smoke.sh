#!/usr/bin/env bash
set -euo pipefail

# HANBOVA M3B.3B — BITNOB SANDBOX SMOKE TEST SCRIPT
#
# Usage:
#   PROVIDER_MODE=sandbox BITNOB_CLIENT_ID=... BITNOB_CLIENT_SECRET=... ./scripts/bitnob_sandbox_smoke.sh
# or ensure hanbova-backend/.env contains BITNOB_CLIENT_ID and BITNOB_CLIENT_SECRET.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Load .env if present in hanbova-backend and variables not yet in environment
if [ -f "${ROOT_DIR}/.env" ]; then
  set +a
  # Source only the relevant variables safely
  # shellcheck disable=SC1091
  source <(grep -E '^(BITNOB_CLIENT_ID|BITNOB_CLIENT_SECRET|PROVIDER_MODE|HANBOVA_API_HOST|HANBOVA_API_PORT)=' "${ROOT_DIR}/.env" || true)
fi

PROVIDER_MODE="${PROVIDER_MODE:-sandbox}"
CLIENT_ID="${BITNOB_CLIENT_ID:-}"
CLIENT_SECRET="${BITNOB_CLIENT_SECRET:-}"

if [ -z "${CLIENT_ID}" ] || [ -z "${CLIENT_SECRET}" ]; then
  echo "=================================================="
  echo "HANBOVA BITNOB SANDBOX VERIFICATION"
  echo ""
  echo "Status: FAILED TO CONNECT"
  echo "FAILURE CLASSIFICATION: MISSING_CREDENTIALS"
  echo "Reason: Missing BITNOB_CLIENT_ID or BITNOB_CLIENT_SECRET"
  echo ""
  echo "To connect against genuine Bitnob Sandbox:"
  echo "  export PROVIDER_MODE=sandbox"
  echo "  export BITNOB_CLIENT_ID=\"your-client-id\""
  echo "  export BITNOB_CLIENT_SECRET=\"your-client-secret\""
  echo "  ./scripts/bitnob_sandbox_smoke.sh"
  echo "=================================================="
  exit 1
fi

# Run the real Bitnob connectivity verification test
TMP_OUTPUT=$(mktemp)
trap 'rm -f "${TMP_OUTPUT}"' EXIT

set +e
PROVIDER_MODE=sandbox \
BITNOB_CLIENT_ID="${CLIENT_ID}" \
BITNOB_CLIENT_SECRET="${CLIENT_SECRET}" \
cargo test -p hanbova-api --test bitnob_sandbox_test -- test_real_bitnob_sandbox_connectivity --ignored --nocapture > "${TMP_OUTPUT}" 2>&1
TEST_EXIT_CODE=$?
set -e

if [ ${TEST_EXIT_CODE} -eq 0 ] && grep -q "RESULT: REAL BITNOB SANDBOX RESPONSE" "${TMP_OUTPUT}"; then
  RATE_VAL=$(grep -E '^Rate:' "${TMP_OUTPUT}" | head -n1 | awk '{print $2}')
  
  echo "=================================================="
  echo "HANBOVA BITNOB SANDBOX VERIFICATION"
  echo ""
  echo "Authentication: PASS"
  echo "Quote request: PASS"
  echo "Provider: bitnob"
  echo "Environment: sandbox"
  echo "Market: NG"
  echo "Pair: USDT -> NGN"
  echo "Rate: ${RATE_VAL}"
  echo "is_live: false"
  echo "is_stale: false"
  echo ""
  echo "RESULT: REAL BITNOB SANDBOX RESPONSE"
  echo "=================================================="
  exit 0
else
  # Classify error from output
  if grep -q "IP_NOT_WHITELISTED" "${TMP_OUTPUT}" || grep -qi "IP address not whitelisted" "${TMP_OUTPUT}"; then
    CLASSIFICATION="IP_NOT_WHITELISTED"
  elif grep -q "AUTHENTICATION_FAILED" "${TMP_OUTPUT}" || grep -qi "authentication failed" "${TMP_OUTPUT}"; then
    CLASSIFICATION="AUTHENTICATION_FAILED"
  elif grep -q "RATE_LIMITED" "${TMP_OUTPUT}" || grep -qi "rate limit" "${TMP_OUTPUT}"; then
    CLASSIFICATION="RATE_LIMITED"
  elif grep -q "UNSUPPORTED_CORRIDOR" "${TMP_OUTPUT}" || grep -qi "unexpected from_asset\|unexpected to_currency" "${TMP_OUTPUT}"; then
    CLASSIFICATION="UNSUPPORTED_CORRIDOR"
  elif grep -q "QUOTE_VALIDATION_FAILED" "${TMP_OUTPUT}" || grep -qi "Missing or invalid exchange rate" "${TMP_OUTPUT}"; then
    CLASSIFICATION="QUOTE_VALIDATION_FAILED"
  elif grep -q "MALFORMED_RESPONSE" "${TMP_OUTPUT}" || grep -qi "Failed to parse" "${TMP_OUTPUT}"; then
    CLASSIFICATION="MALFORMED_RESPONSE"
  elif grep -q "PROVIDER_FORBIDDEN" "${TMP_OUTPUT}" || grep -qi "access forbidden" "${TMP_OUTPUT}"; then
    CLASSIFICATION="PROVIDER_FORBIDDEN"
  elif grep -q "NETWORK_ERROR" "${TMP_OUTPUT}" || grep -qi "network request failed" "${TMP_OUTPUT}"; then
    CLASSIFICATION="NETWORK_ERROR"
  else
    CLASSIFICATION="PROVIDER_UNAVAILABLE"
  fi

  echo "=================================================="
  echo "HANBOVA BITNOB SANDBOX VERIFICATION"
  echo ""
  echo "FAILURE CLASSIFICATION: ${CLASSIFICATION}"

  if [ "${CLASSIFICATION}" = "IP_NOT_WHITELISTED" ]; then
    PUB_IP=$(curl -s -m 2 https://api.ipify.org 2>/dev/null || echo "unknown")
    echo ""
    echo "Bitnob rejected this request because the outbound IP is not whitelisted."
    echo ""
    echo "Current outbound IP:"
    echo "${PUB_IP}"
    echo ""
    echo "Add this IP to the Bitnob API key whitelist and rerun the verification."
  elif [ "${CLASSIFICATION}" = "AUTHENTICATION_FAILED" ]; then
    echo "Reason: Bitnob rejected credentials or request signature."
  elif [ "${CLASSIFICATION}" = "RATE_LIMITED" ]; then
    echo "Reason: Bitnob API rate limit exceeded."
  elif [ "${CLASSIFICATION}" = "UNSUPPORTED_CORRIDOR" ]; then
    echo "Reason: The requested corridor (USDT -> NGN) was not matched by Bitnob."
  else
    echo "Reason: Provider request failed."
  fi

  echo ""
  echo "RESULT: FAILED TO CONNECT"
  echo "=================================================="
  exit 1
fi
