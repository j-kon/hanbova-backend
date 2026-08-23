# Security Policy

Hanbova treats financial and cryptographic software with strict security standards.

---

## 1. Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

---

## 2. Reporting a Vulnerability

If you discover a security vulnerability or cryptographic flaw in Hanbova:

1. **Do NOT open a public GitHub issue.**
2. Send a detailed report to `security@hanbova.org` (or directly to repository maintainers).
3. Include:
   * Description of the vulnerability.
   * Steps to reproduce or proof-of-concept.
   * Potential impact on user funds or key material.

We will acknowledge receipt within 48 hours and work with you to coordinate responsible disclosure.

---

## 3. Key Security Directives in Hanbova

* Never log seed phrases, private keys, or preimages.
* Hardware-backed storage (`flutter_secure_storage`) for all local secrets.
* Deterministic cryptographic timelocks with monotonic time verification.
* Mandatory request body limits (2MB) and rate-limiting on API surfaces.
