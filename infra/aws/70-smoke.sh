#!/usr/bin/env bash
# Check the live site: pages, types and security headers.
source "$(dirname "$0")/lib.sh"
dry && exit 0
base="https://$(load dist_domain)"
say "smoke $base"
fail=0
check() {  # check <path> <expected content-type prefix>
  local h; h=$(curl -sSI "$base$1")
  local code; code=$(echo "$h" | head -1 | awk '{print $2}')
  local ct; ct=$(echo "$h" | grep -i '^content-type:' | cut -d' ' -f2- | tr -d '\r')
  if [[ "$code" == 200 && "$ct" == "$2"* ]]; then echo "  ok   $1 ($ct)"; else echo "  FAIL $1 → $code $ct"; fail=1; fi
}
check / text/html
check /whitepaper.html text/html
check /pkg/novel_encryption_wasm_bg.wasm application/wasm
check /novels/catalog.json application/json
check /novels/king-james-bible.txt text/plain
check /fonts/fonts.css text/css
h=$(curl -sSI "$base/")
for hdr in content-security-policy strict-transport-security x-frame-options x-content-type-options referrer-policy permissions-policy; do
  if echo "$h" | grep -qi "^$hdr:"; then echo "  ok   $hdr"; else echo "  FAIL missing $hdr"; fail=1; fi
done
code=$(curl -s -o /dev/null -w '%{http_code}' "http://$(load dist_domain)/")
[[ "$code" == 301 ]] && echo "  ok   http → https" || { echo "  FAIL http gave $code"; fail=1; }
exit $fail
