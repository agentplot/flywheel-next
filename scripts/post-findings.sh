#!/usr/bin/env bash
# Post three synthetic monitor findings to the capture tool on a running page's
# host, as a monitor's webhook would, so the signals tray shows a third source
# and curation has something to drop. The capture endpoint for a caller that
# cannot reach any host is dispatch's and is not built; this posts to the
# page's own host (216, S227).
#
#   scripts/post-findings.sh <page address>
#
# The address is the page's, as `flywheel host --serve` prints it
# (http://127.0.0.1:4391/fwscratch). Run it once: each finding is its own
# delivery, and the capture tool keys a capture by the delivery it arrived in,
# so a second run captures the three again.
set -euo pipefail

address="${1:?usage: scripts/post-findings.sh <page address>}"
origin="$(printf '%s' "$address" | sed -E 's#^(https?://[^/]+).*#\1#')"

findings=(
  "datadog-4417|synthetic monitor: the gateway's 5xx rate held at 3.1% for 10 minutes on prod"
  "datadog-4420|synthetic monitor: the tile export job ran 42 minutes against its 30-minute budget"
  "datadog-3981|synthetic monitor: disk on search-indexer-2 is at 91% and rising 2% an hour"
)

for finding in "${findings[@]}"; do
  delivery="${finding%%|*}"
  text="${finding#*|}"
  body="$(printf '{"args":{"text":%s,"source":"datadog"},"delivery_id":"%s"}' \
    "$(printf '%s' "$text" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')" "$delivery")"
  printf '%s\t' "$delivery"
  curl -sS --fail-with-body -X POST "$origin/api/tools/capture" \
    -H 'Content-Type: application/json' \
    -H "Referer: $address" \
    -d "$body"
  printf '\n'
done
