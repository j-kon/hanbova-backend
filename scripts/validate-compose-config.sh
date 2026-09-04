#!/usr/bin/env bash
set -euo pipefail

compose_file="${1:?usage: validate-compose-config.sh <rendered-compose-file>}"

test -s "$compose_file" || {
  echo "compose file is empty: $compose_file" >&2
  exit 1
}

if grep -Eq 'CASHU_MINT_URL|JWT_SECRET:.*(hanbova|replace-with|change-me)|MINT_PRIVATE_KEY:.*(hanbova|replace-with|change-me)' "$compose_file"; then
  echo "compose file contains legacy or hardcoded secret configuration" >&2
  exit 1
fi

for required in HANBOVA_ENV HANBOVA_API_HOST HANBOVA_API_PORT DATABASE_URL JWT_SECRET MINT_URL PROVIDER_MODE CORS_ALLOWED_ORIGINS; do
  grep -q "$required" "$compose_file" || {
    echo "compose file is missing required variable: $required" >&2
    exit 1
  }
done

grep -q 'HANBOVA_ENV=development' "$compose_file" || {
  echo "only the explicit development compose profile may use this validator" >&2
  exit 1
}

echo "compose configuration passed development safety checks"
