#!/usr/bin/env bash
# Apply a scenario into a scratch instance, serve it, and screenshot the page
# at the desktop and the phone: what proves the page is a picture a person
# reads, not a test that finds an id (AGENTS.md, Gates; task 17).
#
#   scripts/page-shot.sh [scenario] [through] [out-dir]
#
# Defaults: scenarios/storefront, every action, target/shots/<scenario>-<n>.
# Writes desktop.png (1440×900), desktop-tall.png (1440×2400), phone.png
# (390×844) and page.html into the out directory. Needs a Chrome at $CHROME
# (default: the Mac's Google Chrome) and the flywheel binary at $FLYWHEEL
# (default: target/debug/flywheel, built if absent).
set -euo pipefail
cd "$(dirname "$0")/.."

scenario="${1:-scenarios/storefront}"
through="${2:-}"
name="$(basename "$scenario")"
out="${3:-target/shots/${name}-${through:-all}}"
chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
flywheel="${FLYWHEEL:-target/debug/flywheel}"
[ -x "$flywheel" ] || cargo build -q -p flywheel
port="${PORT:-$(( 4300 + RANDOM % 500 ))}"
inst="$(mktemp -d "${TMPDIR:-/tmp}/flywheel-shot-XXXXXX")"

mkdir -p "$out"
"$flywheel" scenario apply "$scenario" --into "$inst" ${through:+--through "$through"} > "$out/apply.log" 2>&1 \
  || { cat "$out/apply.log"; exit 1; }
"$flywheel" host --manifest "$inst/flywheel.yaml" --serve "$port" --operator chuck > "$out/host.log" 2>&1 &
host=$!
trap 'kill $host 2>/dev/null || true; rm -rf "$inst"' EXIT
for _ in $(seq 1 40); do
  curl -sf "http://127.0.0.1:$port/" -o "$out/page.html" && break
  sleep 0.25
done
[ -s "$out/page.html" ] || { echo "the page did not answer on :$port"; cat "$out/host.log"; exit 1; }
shot() { "$chrome" --headless=new --disable-gpu --hide-scrollbars --window-size="$1" --screenshot="$out/$2" "http://127.0.0.1:$port/$3" 2>/dev/null; }
shot 1440,900  desktop.png ""
shot 1440,2400 desktop-tall.png ""
shot 390,844   phone.png ""
echo "$out"
ls "$out"
