#!/usr/bin/env bash
# Walk a scenario's tour from the page the way the operator does — apply
# nothing, serve, then click `next` once per action — screenshotting the
# board after every step. What proves the stepped demo is the pictures a
# person reads (`design/flywheel-next/scenarios/storefront.md`).
#
#   scripts/tour-walk.sh [scenario] [out-dir]
#
# Writes step-NN-desktop.png, step-NN-phone.png and step-NN.html per action
# into the out directory (default target/shots/tour-<scenario>), and
# step-00-* for the moment before the first action.
set -euo pipefail
cd "$(dirname "$0")/.."

scenario="${1:-scenarios/storefront}"
name="$(basename "$scenario")"
out="${2:-target/shots/tour-${name}}"
chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
flywheel="${FLYWHEEL:-target/debug/flywheel}"
[ -x "$flywheel" ] || cargo build -q -p flywheel
port="${PORT:-$(( 4300 + RANDOM % 500 ))}"
inst="$(mktemp -d "${TMPDIR:-/tmp}/flywheel-tour-XXXXXX")"
url="http://127.0.0.1:$port/"

mkdir -p "$out"
"$flywheel" scenario apply "$scenario" --into "$inst" --through 0 > "$out/apply.log" 2>&1 \
  || { cat "$out/apply.log"; exit 1; }
"$flywheel" host --manifest "$inst/flywheel.yaml" --serve "$port" --operator chuck > "$out/host.log" 2>&1 &
host=$!
trap 'kill $host 2>/dev/null || true; rm -rf "$inst"' EXIT
for _ in $(seq 1 40); do curl -sf "$url" -o /dev/null && break; sleep 0.25; done

actions="$(grep -c '^  - ' "$scenario/scenario.yaml" 2>/dev/null || echo 0)"
shot() { "$chrome" --headless=new --disable-gpu --hide-scrollbars --window-size="$1" --screenshot="$out/$2" "$url" 2>/dev/null; }
snap() {
  local n; n="$(printf '%02d' "$1")"
  curl -s "$url" -o "$out/step-$n.html"
  shot 1440,900 "step-$n-desktop.png"
  shot 390,844  "step-$n-phone.png"
}
snap 0
step=0
while true; do
  page="$(curl -s "$url")"
  grep -q 'action="/tour/next"' <<<"$page" || break
  step=$((step + 1))
  curl -s -X POST "$url"tour/next -H "Referer: $url" -o /dev/null
  # The beat, then the cascade: wait until the strip says this step is shown
  # and the machinery is no longer working.
  for _ in $(seq 1 240); do
    page="$(curl -s "$url")"
    if grep -q "step <b>$step</b>" <<<"$page" && ! grep -q 'class="tour working' <<<"$page"; then break; fi
    sleep 0.5
  done
  snap "$step"
  echo "step $step: $(grep -o '<span class="said">[^<]*' "$out/step-$(printf '%02d' "$step").html" | sed 's/<span class="said">//' | cut -c1-110)"
done
echo "$out · $step steps"
