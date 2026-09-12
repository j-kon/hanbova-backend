#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# HANBOVA M3B.3B — BITNOB 403 WHITELIST ROOT-CAUSE DIAGNOSTIC SCRIPT
#
# Diagnoses Bitnob connectivity and IP whitelist rejections:
#   - Full local egress diagnostics (IPv4, IPv6, proxies)
#   - DNS address family resolution (A vs AAAA)
#   - Network stability detection (egress before vs after)
#   - Correlation / request ID tracking
#   - Controlled retry evaluation
#   - Safe error classification
#
# Never exposes raw secrets, HMAC signatures, or authorization headers.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT_DIR}"

# Load .env if present in hanbova-backend
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
echo "HANBOVA BITNOB WHITELIST DIAGNOSTIC"
echo "=================================================="
echo ""

# ------------------------------------------------------------------------------
# 1. NETWORK & EGRESS PREFLIGHT
# ------------------------------------------------------------------------------
echo "NETWORK PREFLIGHT"

# IPv4 Egress
IPV4_INITIAL=$(curl -4 -s -m 5 https://api.ipify.org 2>/dev/null || echo "UNAVAILABLE")
echo "Public IPv4 egress: ${IPV4_INITIAL}"

# IPv6 Egress
RAW_V6=$(curl -6 -s -m 3 https://api64.ipify.org 2>/dev/null || true)
if [[ "${RAW_V6}" == *:* ]]; then
  IPV6_EGRESS="${RAW_V6}"
  IPV6_AVAILABLE="YES"
else
  IPV6_EGRESS="UNAVAILABLE"
  IPV6_AVAILABLE="NO"
fi
echo "Public IPv6 egress: ${IPV6_EGRESS}"
echo "Public IPv6 available: ${IPV6_AVAILABLE}"

# Proxy Environment Variables (never print URLs)
[ -n "${HTTP_PROXY:-}" ] && HTTP_PROXY_PRESENT="YES" || HTTP_PROXY_PRESENT="NO"
[ -n "${HTTPS_PROXY:-}" ] && HTTPS_PROXY_PRESENT="YES" || HTTPS_PROXY_PRESENT="NO"
[ -n "${ALL_PROXY:-}" ] && ALL_PROXY_PRESENT="YES" || ALL_PROXY_PRESENT="NO"
[ -n "${NO_PROXY:-}" ] && NO_PROXY_PRESENT="YES" || NO_PROXY_PRESENT="NO"

echo "HTTP_PROXY present: ${HTTP_PROXY_PRESENT}"
echo "HTTPS_PROXY present: ${HTTPS_PROXY_PRESENT}"
echo "ALL_PROXY present: ${ALL_PROXY_PRESENT}"
echo "NO_PROXY present: ${NO_PROXY_PRESENT}"

# DNS Resolution for api.bitnob.com
DNS_A_FOUND="NO"
if dig +short A api.bitnob.com 2>/dev/null | grep -E '^[0-9]' >/dev/null; then
  DNS_A_FOUND="YES"
elif host -t A api.bitnob.com 2>/dev/null | grep -E 'has address' >/dev/null; then
  DNS_A_FOUND="YES"
elif nslookup -type=A api.bitnob.com 2>/dev/null | grep -E 'Address: [0-9]' >/dev/null; then
  DNS_A_FOUND="YES"
fi

DNS_AAAA_FOUND="NO"
if dig +short AAAA api.bitnob.com 2>/dev/null | grep -E '^[0-9a-fA-F:]' >/dev/null; then
  DNS_AAAA_FOUND="YES"
elif host -t AAAA api.bitnob.com 2>/dev/null | grep -E 'has IPv6' >/dev/null; then
  DNS_AAAA_FOUND="YES"
elif nslookup -type=AAAA api.bitnob.com 2>/dev/null | grep -E 'Address: [0-9a-fA-F:]' >/dev/null; then
  DNS_AAAA_FOUND="YES"
fi

echo "Bitnob DNS IPv4 available: ${DNS_A_FOUND}"
echo "Bitnob DNS IPv6 available: ${DNS_AAAA_FOUND}"
echo ""

# ------------------------------------------------------------------------------
# 2. BITNOB CREDENTIAL PREFLIGHT
# ------------------------------------------------------------------------------
echo "BITNOB PREFLIGHT"
echo "Base URL: https://api.bitnob.com"
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

LOCAL_UTC="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
LOCAL_EPOCH="$(date -u +"%s")"
echo "System UTC timestamp: ${LOCAL_UTC}"
echo "System epoch seconds: ${LOCAL_EPOCH}"
echo ""

# Validations
if [ "${PROVIDER_MODE}" != "sandbox" ]; then
  echo "Error: PROVIDER_MODE must be 'sandbox' (found: '${PROVIDER_MODE}')."
  echo "FAILURE CLASSIFICATION: INVALID_ENVIRONMENT"
  exit 1
fi

if [ "${CLIENT_ID_LEN}" -eq 0 ] || [ "${CLIENT_SECRET_LEN}" -eq 0 ]; then
  echo "FAILURE CLASSIFICATION: MISSING_CREDENTIALS"
  echo "Reason: Missing BITNOB_CLIENT_ID or BITNOB_CLIENT_SECRET"
  exit 1
fi

TMP_OUTPUT="$(mktemp)"
trap 'rm -f "${TMP_OUTPUT}"' EXIT

# Function to perform one whoami invocation
run_whoami_call() {
  local force_ipv4="${1:-false}"
  set +e
  PROVIDER_MODE=sandbox \
  BITNOB_CLIENT_ID="${CLIENT_ID}" \
  BITNOB_CLIENT_SECRET="${CLIENT_SECRET}" \
  BITNOB_DIAGNOSTIC_FORCE_IPV4="${force_ipv4}" \
  cargo test -p hanbova-api --test bitnob_sandbox_test -- test_real_bitnob_whoami --ignored --nocapture > "${TMP_OUTPUT}" 2>&1
  local exit_code=$?
  set -e
  return ${exit_code}
}

# ------------------------------------------------------------------------------
# 3. CONTROLLED RETRY DIAGNOSTIC LOOP (3 ATTEMPTS)
# ------------------------------------------------------------------------------
echo "STEP 1 — AUTHENTICATION WHOAMI (RETRY EVALUATION)"
echo "GET /api/whoami"

declare -a ATTEMPT_STATUSES
declare -a ATTEMPT_CLASSES
declare -a ATTEMPT_TIMES
declare -a ATTEMPT_CORRS
declare -a ATTEMPT_IPS

WHOAMI_PASSED="NO"

for attempt in 1 2 3; do
  IPV4_BEFORE=$(curl -4 -s -m 5 https://api.ipify.org 2>/dev/null || echo "UNAVAILABLE")
  ATTEMPT_TIME="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  
  run_whoami_call "false" || true
  
  IPV4_AFTER=$(curl -4 -s -m 5 https://api.ipify.org 2>/dev/null || echo "UNAVAILABLE")
  
  CORR_ID=$(grep -E 'Correlation-ID-Inline:' "${TMP_OUTPUT}" | head -n1 | awk '{print $2}')
  if [ -z "${CORR_ID}" ]; then
    CORR_ID=$(grep -A1 '^Correlation ID:' "${TMP_OUTPUT}" | tail -n1 | tr -d '[:space:]')
  fi
  [ -z "${CORR_ID}" ] && CORR_ID="unavailable"

  if grep -q "Result: PASS" "${TMP_OUTPUT}" || grep -q "HTTP status: 200" "${TMP_OUTPUT}"; then
    STATUS_CODE="200"
    CLASS="SUCCESS"
    WHOAMI_PASSED="YES"
  elif grep -q "IP_NOT_WHITELISTED" "${TMP_OUTPUT}" || grep -qi "IP address not whitelisted" "${TMP_OUTPUT}"; then
    STATUS_CODE="403"
    CLASS="IP_NOT_WHITELISTED"
  elif grep -q "AUTHENTICATION_FAILED" "${TMP_OUTPUT}" || grep -qi "authentication failed" "${TMP_OUTPUT}"; then
    STATUS_CODE="401"
    CLASS="AUTHENTICATION_FAILED"
  elif grep -q "RATE_LIMITED" "${TMP_OUTPUT}"; then
    STATUS_CODE="429"
    CLASS="RATE_LIMITED"
  elif grep -q "PROVIDER_FORBIDDEN" "${TMP_OUTPUT}"; then
    STATUS_CODE="403"
    CLASS="PROVIDER_FORBIDDEN"
  elif grep -q "NETWORK_ERROR" "${TMP_OUTPUT}"; then
    STATUS_CODE="N/A"
    CLASS="NETWORK_ERROR"
  else
    STATUS_CODE="500"
    CLASS="PROVIDER_UNAVAILABLE"
  fi

  ATTEMPT_STATUSES+=("${STATUS_CODE}")
  ATTEMPT_CLASSES+=("${CLASS}")
  ATTEMPT_TIMES+=("${ATTEMPT_TIME}")
  ATTEMPT_CORRS+=("${CORR_ID}")
  ATTEMPT_IPS+=("${IPV4_BEFORE}")

  echo "  Attempt ${attempt} (${ATTEMPT_TIME}): HTTP ${STATUS_CODE} | ${CLASS} | Egress: ${IPV4_BEFORE} | Correlation: ${CORR_ID}"

  if [ "${WHOAMI_PASSED}" = "YES" ]; then
    break
  fi

  # Sleep between retry attempts if not final attempt
  if [ "${attempt}" -lt 3 ]; then
    sleep 5
  fi
done

echo ""

# Check for dynamic IP change during attempts
IP_CHANGED="NO"
if [ "${ATTEMPT_IPS[0]}" != "${ATTEMPT_IPS[-1]}" ] || [ "${IPV4_BEFORE}" != "${IPV4_AFTER}" ]; then
  IP_CHANGED="YES"
fi

# ------------------------------------------------------------------------------
# 4. OPTIONAL IPV4-ONLY DIAGNOSTIC
# ------------------------------------------------------------------------------
IPV4_TEST_ATTEMPTED="YES"
IPV4_TEST_RESULT="NOT ATTEMPTED"

if [ "${WHOAMI_PASSED}" = "NO" ]; then
  echo "RUNNING LOCAL IPV4-ONLY DIAGNOSTIC BINDING..."
  run_whoami_call "true" || true
  if grep -q "HTTP status: 200" "${TMP_OUTPUT}" || grep -q "Result: PASS" "${TMP_OUTPUT}"; then
    IPV4_TEST_RESULT="PASS"
  else
    IPV4_TEST_RESULT="FAIL"
  fi
  echo "IPv4-only result: ${IPV4_TEST_RESULT}"
  echo ""
fi

# ------------------------------------------------------------------------------
# 5. ROOT CAUSE CLASSIFICATION
# ------------------------------------------------------------------------------
if [ "${WHOAMI_PASSED}" = "YES" ]; then
  FINAL_ROOT_CAUSE="NONE"
elif [ "${IP_CHANGED}" = "YES" ]; then
  FINAL_ROOT_CAUSE="DYNAMIC_EGRESS_IP_CHANGED"
elif [ "${IPV4_TEST_RESULT}" = "PASS" ]; then
  FINAL_ROOT_CAUSE="IPV6_EGRESS_WHITELIST_MISMATCH"
elif [ "${HTTP_PROXY_PRESENT}" = "YES" ] || [ "${HTTPS_PROXY_PRESENT}" = "YES" ] || [ "${ALL_PROXY_PRESENT}" = "YES" ]; then
  FINAL_ROOT_CAUSE="PROXY_EGRESS_SUSPECTED"
elif [ "${ATTEMPT_CLASSES[0]}" = "AUTHENTICATION_FAILED" ]; then
  FINAL_ROOT_CAUSE="AUTHENTICATION_FAILED"
elif [ "${ATTEMPT_CLASSES[0]}" = "NETWORK_ERROR" ]; then
  FINAL_ROOT_CAUSE="NETWORK_ERROR"
elif [ "${ATTEMPT_CLASSES[0]}" = "IP_NOT_WHITELISTED" ]; then
  FINAL_ROOT_CAUSE="BITNOB_WHITELIST_PERSISTENT_REJECTION"
else
  FINAL_ROOT_CAUSE="UNKNOWN"
fi

echo "ROOT CAUSE CLASSIFICATION: ${FINAL_ROOT_CAUSE}"
echo ""

# ------------------------------------------------------------------------------
# 6. DOWNSTREAM STEP HANDLING
# ------------------------------------------------------------------------------
if [ "${WHOAMI_PASSED}" = "NO" ]; then
  echo "DOWNSTREAM EXECUTION POLICY:"
  echo "  WHOAMI: BLOCKED"
  echo "  EXCHANGE RATE: NOT ATTEMPTED"
  echo "  PAYOUT QUOTE: NOT ATTEMPTED"
  echo ""
  echo "Reason: /api/whoami was rejected with HTTP ${ATTEMPT_STATUSES[0]} (${ATTEMPT_CLASSES[0]})."
  echo "Per milestone requirements, downstream rate and quote calls are halted."
  echo ""

  echo "=================================================="
  echo "MANUAL VERIFICATION CHECKLIST FOR DEVELOPER"
  echo "=================================================="
  echo "1. Verify in Bitnob Dashboard (https://app.bitnob.com -> Settings > API Keys):"
  echo "   - Confirm the public IP '${IPV4_INITIAL}' is present in the IP Whitelist."
  echo "   - Ensure you pressed [Enter] / [Add] so the IP became a distinct chip/tag."
  echo "   - Ensure you clicked [Save Changes] or [Apply]."
  echo "2. Confirm Environment & Key Identity:"
  echo "   - Verify the whitelist was edited under the SANDBOX tab (not Production)."
  echo "   - Verify the key corresponds to the Client ID in .env (length: ${CLIENT_ID_LEN})."
  echo "3. Subnet formatting:"
  echo "   - If plain '${IPV4_INITIAL}' is not matching, check if dashboard requires '${IPV4_INITIAL}/32'."
  echo "=================================================="
  echo ""

  echo "=================================================="
  echo "READY-TO-SEND BITNOB SUPPORT MESSAGE"
  echo "=================================================="
  echo "Hello Bitnob Support,"
  echo ""
  echo "I'm testing the Sandbox API using HMAC authentication against"
  echo "https://api.bitnob.com."
  echo ""
  echo "GET /api/whoami consistently returns:"
  echo "HTTP 403 Forbidden"
  echo "Detail: IP address not whitelisted"
  echo ""
  echo "The current public outbound IPv4 has already been added to the Bitnob"
  echo "dashboard whitelist."
  echo ""
  echo "Environment: Sandbox"
  echo "Outbound IPv4: ${IPV4_INITIAL}"
  echo "IPv6 egress present: ${IPV6_AVAILABLE}"
  echo "UTC timestamp: ${LOCAL_UTC}"
  echo "Correlation/request ID: ${ATTEMPT_CORRS[0]}"
  echo "Hanbova commit: $(git rev-parse HEAD 2>/dev/null || echo 'unknown')"
  echo ""
  echo "Could you confirm whether the IP whitelist is active for this workspace/API"
  echo "configuration and whether Sandbox requests use the same whitelist?"
  echo "=================================================="
  echo ""
  exit 1
fi

# If whoami PASSED, continue to Steps 2 and 3!
echo "STEP 2 — EXCHANGE RATE (GET /api/exchange-rates?from=USDT&to=NGN)"
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
  echo "Rate: ${RATE_VAL}"
else
  echo "Status: FAIL"
fi
echo ""

echo "STEP 3 — PAYOUT QUOTE (POST /api/payouts/quotes)"
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
  echo "Rate: ${QUOTE_RATE}"
  echo ""
  echo "FINAL RESULT: REAL BITNOB SANDBOX CONNECTIVITY VERIFIED"
  exit 0
else
  echo "Status: FAIL"
  exit 1
fi
