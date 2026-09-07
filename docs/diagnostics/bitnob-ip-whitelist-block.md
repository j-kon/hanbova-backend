# Bitnob IP Whitelist Diagnostic Report

> [!WARNING]
> This is a diagnostic document for upstream troubleshooting with Bitnob Support.
> It is **NOT** a verification artifact and does not certify sandbox connectivity.

## Summary

- **Date / Time (UTC)**: 2026-09-07T05:20:20Z
- **Hanbova Commit SHA**: `97fdab1c1f118e347721faeee0f223a98a0c0086`
- **Provider**: Bitnob
- **Environment**: Sandbox
- **Endpoint**: `GET /api/whoami`
- **Base URL**: `https://api.bitnob.com`
- **Result**: **BLOCKED**

---

## Network & Egress Profile

- **Observed Public IPv4 Egress**: `102.91.132.170`
- **Public IPv6 Presence**: `NO` (`UNAVAILABLE`)
- **DNS Resolution for api.bitnob.com**:
  - IPv4 A records: `YES` (`35.179.228.33`, `18.170.251.252`)
  - IPv6 AAAA records: `NO` (None present)
- **Environment Proxies**: None configured (`HTTP_PROXY=NO`, `HTTPS_PROXY=NO`, `ALL_PROXY=NO`)
- **Dynamic IP Changes**: `NO` (Egress IP remained static across all attempts)

---

## Upstream Diagnostic Execution

- **HTTP Status**: `403 Forbidden`
- **Safe Provider Detail**: `IP address not whitelisted`
- **Root Cause Classification**: `BITNOB_WHITELIST_PERSISTENT_REJECTION`
- **Number of Retry Attempts**: 3

### Attempt Records

| Attempt | Timestamp (UTC) | HTTP Status | Classification | Correlation / Request ID |
|---|---|---|---|---|
| 1 | `2026-09-07T05:20:21Z` | 403 | `IP_NOT_WHITELISTED` | `req_01a07a4f-86f7-7caf-8c7d-4b9215fec6f8` |
| 2 | `2026-09-07T05:20:33Z` | 403 | `IP_NOT_WHITELISTED` | `req_01a07a4f-a3f2-7147-a331-58291fee0111` |
| 3 | `2026-09-07T05:20:41Z` | 403 | `IP_NOT_WHITELISTED` | `req_01a07a4f-c234-7a35-800c-ad8a63dbcd02` |

---

## Local Socket Binding Diagnostic

- **IPv4-Only Socket Binding (`BITNOB_DIAGNOSTIC_FORCE_IPV4=true`)**: `FAIL` (HTTP 403 `IP_NOT_WHITELISTED`)
- **Deduction**: Outbound traffic is confirmed exiting through IPv4 `102.91.132.170`. The issue is strictly located in upstream whitelist recognition.

---

## Downstream Provider State

- **GET /api/whoami**: `BLOCKED`
- **GET /api/exchange-rates?from=USDT&to=NGN**: `NOT ATTEMPTED` (Halted per fail-closed policy)
- **POST /api/payouts/quotes**: `NOT ATTEMPTED` (Halted per fail-closed policy)

---

## Support Message

```text
Hello Bitnob Support,

I'm testing the Sandbox API using HMAC authentication against
https://api.bitnob.com.

GET /api/whoami consistently returns:
HTTP 403 Forbidden
Detail: IP address not whitelisted

The current public outbound IPv4 has already been added to the Bitnob
dashboard whitelist.

Environment: Sandbox
Outbound IPv4: 102.91.132.170
IPv6 egress present: NO
UTC timestamp: 2026-09-07T05:20:20Z
Correlation/request ID: req_01a07a4f-86f7-7caf-8c7d-4b9215fec6f8
Hanbova commit: 97fdab1c1f118e347721faeee0f223a98a0c0086

Could you confirm whether the IP whitelist is active for this workspace/API
configuration and whether Sandbox requests use the same whitelist?
```
