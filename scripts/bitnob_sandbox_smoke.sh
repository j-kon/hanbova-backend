#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# HANBOVA M3B.3B — BITNOB REAL CONNECTIVITY DIAGNOSTIC SCRIPT
#
# Diagnoses real Bitnob connectivity through 3 explicit steps:
#   STEP 1: GET /api/whoami (authentication & IP whitelist check)
#   STEP 2: GET /api/exchange-rates?from=USDT&to=NGN (dedicated rate discovery)
#   STEP 3: POST /api/payouts/quotes (executable rate quote creation)
#
# Never exposes raw secrets, HMAC signatures, or authorization headers.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT_DIR}"

# Load .env if present in hanbova-backend and variables not yet in environment
if [ -f "${ROOT_DIR}/.env" ]; then
  set +a
  # shellcheck disable=SC1091
  source <(grep -E '^(BITNOB_CLIENT_ID|BITNOB_CLIENT_SECRET|PROVIDER_MODE|HANBOVA_API_HOST|HANBOVA_API_PORT)=' "${ROOT_DIR}/.env" || true)
  set -a
fi

PROVIDER_MODE="${PROVIDER_MODE:-sandbox}"

# Normalize credentials: trim whitespace and CRLF
CLIENT_ID="$(echo -n "${BITNOB_CLIENT_ID:-}" | tr -d '\r\n' | awk '{$1=$1};1')"
CLIENT_SECRET="$(echo -n "${BITNOB_CLIENT_SECRET:-}" | tr -d '\r\n' | awk '{$1=$1};1')"

CLIENT_ID_LEN=${#CLIENT_ID}
CLIENT_SECRET_LEN=${#CLIENT_SECRET}

echo "=================================================="
echo "HANBOVA BITNOB REAL CONNECTIVITY DIAGNOSTIC"
echo "=================================================="
echo ""

# Section 9: Safe Credential Preflight
echo "REAL CREDENTIAL PREFLIGHT"
echo "Provider mode: ${PROVIDER_MODE}"
if [ "${CLIENT_ID_LEN}" -gt 0 ]; then
  echo "Client ID present: YES"
  echo "Client ID length: ${CLIENT_ID_LEN}"
else
  echo "Client ID present: NO"
fi

if [ "${CLIENT_SECRET_LEN}" -gt 0 ]; then
  echo "Client Secret present: YES"
  echo "Client Secret length: ${CLIENT_SECRET_LEN}"
else
  echo "Client Secret present: NO"
fi
echo ""

# Section 10: System Clock Diagnostic
LOCAL_UTC="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
LOCAL_EPOCH="$(date -u +"%s")"
echo "SYSTEM CLOCK DIAGNOSTIC"
echo "Local UTC timestamp: ${LOCAL_UTC}"
echo "Unix epoch seconds: ${LOCAL_EPOCH}"

if [ "${LOCAL_EPOCH}" -lt 1700000000 ]; then
  echo "Warning: System epoch is abnormally low. Clock skew suspected."
  echo "FAILURE CLASSIFICATION: CLOCK_SKEW_SUSPECTED"
  echo ""
  echo "FINAL RESULT:"
  echo "FAILED TO CONNECT"
  echo "=================================================="
  exit 1
fi
echo ""

# Validate environment mode
if [ "${PROVIDER_MODE}" != "sandbox" ]; then
  echo "Error: PROVIDER_MODE must be 'sandbox' for sandbox verification (found: '${PROVIDER_MODE}')."
  echo "FAILURE CLASSIFICATION: INVALID_ENVIRONMENT"
  echo ""
  echo "FINAL RESULT:"
  echo "FAILED TO CONNECT"
  echo "=================================================="
  exit 1
fi

# Validate credentials presence
if [ "${CLIENT_ID_LEN}" -eq 0 ] || [ "${CLIENT_SECRET_LEN}" -eq 0 ]; then
  echo "Status: FAILED TO CONNECT"
  echo "FAILURE CLASSIFICATION: MISSING_CREDENTIALS"
  echo "Reason: Missing BITNOB_CLIENT_ID or BITNOB_CLIENT_SECRET"
  echo ""
  echo "FINAL RESULT:"
  echo "FAILED TO CONNECT"
  echo "=================================================="
  exit 1
fi

TMP_OUTPUT="$(mktemp)"
trap 'rm -f "${TMP_OUTPUT}"' EXIT

# ------------------------------------------------------------------------------
# STEP 1: AUTHENTICATION (GET /api/whoami)
# ------------------------------------------------------------------------------
echo "STEP 1 — AUTHENTICATION"
echo "GET /api/whoami"

set +e
PROVIDER_MODE=sandbox \
BITNOB_CLIENT_ID="${CLIENT_ID}" \
BITNOB_CLIENT_SECRET="${CLIENT_SECRET}" \
cargo test -p hanbova-api --test bitnob_sandbox_test -- test_real_bitnob_whoami --ignored --nocapture > "${TMP_OUTPUT}" 2>&1
WHOAMI_EXIT=$?
set -e

if [ ${WHOAMI_EXIT} -eq 0 ] && grep -q "Result: PASS" "${TMP_OUTPUT}"; then
  echo "Status: PASS"
  echo ""
else
  echo "Status: FAIL"
  echo ""
  
  # Extract safe diagnostic details
  if grep -q "IP_NOT_WHITELISTED" "${TMP_OUTPUT}" || grep -qi "IP address not whitelisted" "${TMP_OUTPUT}"; then
    CLASSIFICATION="IP_NOT_WHITELISTED"
    HTTP_STATUS=403
    SAFE_DETAIL="IP address not whitelisted"
  elif grep -q "AUTHENTICATION_FAILED" "${TMP_OUTPUT}" || grep -qi "authentication failed" "${TMP_OUTPUT}"; then
    CLASSIFICATION="AUTHENTICATION_FAILED"
    HTTP_STATUS=401
    SAFE_DETAIL="Authentication failed"
  elif grep -q "RATE_LIMITED" "${TMP_OUTPUT}" || grep -qi "rate limit" "${TMP_OUTPUT}"; then
    CLASSIFICATION="RATE_LIMITED"
    HTTP_STATUS=429
    SAFE_DETAIL="Bitnob rate limit exceeded"
  elif grep -q "PROVIDER_FORBIDDEN" "${TMP_OUTPUT}" || grep -qi "access forbidden" "${TMP_OUTPUT}"; then
    CLASSIFICATION="PROVIDER_FORBIDDEN"
    HTTP_STATUS=403
    SAFE_DETAIL="Bitnob access forbidden"
  elif grep -q "NETWORK_ERROR" "${TMP_OUTPUT}" || grep -qi "network request failed" "${TMP_OUTPUT}"; then
    CLASSIFICATION="NETWORK_ERROR"
    HTTP_STATUS="N/A"
    SAFE_DETAIL="Network connection to api.bitnob.com failed"
  else
    CLASSIFICATION="PROVIDER_UNAVAILABLE"
    HTTP_STATUS=503
    SAFE_DETAIL="Bitnob provider unavailable"
  fi

  echo "HTTP status: ${HTTP_STATUS}"
  echo "Classification: ${CLASSIFICATION}"
  echo "Safe detail: ${SAFE_DETAIL}"

  if [ "${CLASSIFICATION}" = "IP_NOT_WHITELISTED" ]; then
    PUB_IP=$(curl -s -m 3 https://api.ipify.org 2>/dev/null || echo "unknown")
    echo ""
    echo "Current outbound IP:"
    echo "${PUB_IP}"
    echo ""
    echo "Action required: Add ${PUB_IP} to the Bitnob dashboard IP whitelist for this API key and rerun."
  fi

  echo ""
  echo "FINAL RESULT:"
  echo "FAILED TO CONNECT"
  echo "=================================================="
  exit 1
fi

# ------------------------------------------------------------------------------
# STEP 2: EXCHANGE RATE (GET /api/exchange-rates?from=USDT&to=NGN)
# ------------------------------------------------------------------------------
echo "STEP 2 — EXCHANGE RATE"
echo "USDT -> NGN"

set +e
PROVIDER_MODE=sandbox \
BITNOB_CLIENT_ID="${CLIENT_ID}" \
BITNOB_CLIENT_SECRET="${CLIENT_SECRET}" \
cargo test -p hanbova-api --test bitnob_sandbox_test -- test_real_bitnob_exchange_rate --ignored --nocapture > "${TMP_OUTPUT}" 2>&1
RATE_EXIT=$?
set -e

if [ ${RATE_EXIT} -eq 0 ] && grep -q "Status: PASS" "${TMP_OUTPUT}"; then
  RATE_VAL=$(grep -E '^Parsed rate:' "${TMP_OUTPUT}" | head -n1 | awk '{print $3}')
  echo "Status: PASS"
  echo "Rate received: YES"
  if [ -n "${RATE_VAL}" ]; then
    echo "Rate: ${RATE_VAL}"
  fi
  echo ""
else
  echo "Status: FAIL"
  echo "Rate received: NO"
  echo ""
fi

# ------------------------------------------------------------------------------
# STEP 3: PAYOUT QUOTE (POST /api/payouts/quotes)
# ------------------------------------------------------------------------------
echo "STEP 3 — PAYOUT QUOTE"
echo "POST /api/payouts/quotes"
echo "Reference generated: YES"

set +e
PROVIDER_MODE=sandbox \
BITNOB_CLIENT_ID="${CLIENT_ID}" \
BITNOB_CLIENT_SECRET="${CLIENT_SECRET}" \
cargo test -p hanbova-api --test bitnob_sandbox_test -- test_real_bitnob_sandbox_connectivity --ignored --nocapture > "${TMP_OUTPUT}" 2>&1
QUOTE_EXIT=$?
set -e

if [ ${QUOTE_EXIT} -eq 0 ] && grep -q "RESULT: REAL BITNOB SANDBOX RESPONSE" "${TMP_OUTPUT}"; then
  QUOTE_RATE=$(grep -E '^Rate:' "${TMP_OUTPUT}" | head -n1 | awk '{print $2}')
  echo "Status: PASS"
  echo "Rate received: YES"
  echo "Rate: ${QUOTE_RATE}"
  echo ""
  echo "FINAL RESULT:"
  echo "REAL BITNOB SANDBOX CONNECTIVITY VERIFIED"
  echo ""
  echo "Provider: bitnob"
  echo "Environment: sandbox"
  echo "is_live: false"
  echo "=================================================="
  exit 0
else
  echo "Status: FAIL"
  echo "Rate received: NO"
  
  if grep -q "REQUEST_VALIDATION_FAILED" "${TMP_OUTPUT}" || grep -qi "validation failed" "${TMP_OUTPUT}"; then
    QUOTE_CLASS="REQUEST_VALIDATION_FAILED"
  elif grep -q "UNSUPPORTED_CORRIDOR" "${TMP_OUTPUT}"; then
    QUOTE_CLASS="UNSUPPORTED_CORRIDOR"
  elif grep -q "RATE_LIMITED" "${TMP_OUTPUT}"; then
    QUOTE_CLASS="RATE_LIMITED"
  else
    QUOTE_CLASS="PROVIDER_UNAVAILABLE"
  fi
  echo "Classification: ${QUOTE_CLASS}"
  echo ""
  echo "FINAL RESULT:"
  echo "FAILED TO CONNECT"
  echo "=================================================="
  exit 1
fi
