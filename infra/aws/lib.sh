# Shared helpers, sourced by every step. DRY_RUN=1 prints each mutating AWS
# call instead of running it; read-only lookups still run.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=/dev/null
source "$HERE/config.sh"
STATE="$HERE/.state"            # git-ignored outputs (zone id, cert arn, distribution id…)
mkdir -p "$STATE"

say() { printf '\n▶ %s\n' "$*"; }
dry() { [[ "${DRY_RUN:-0}" == 1 ]]; }
awsr() { aws --profile "$AWS_PROFILE" --region "$AWS_REGION" "$@"; }
awsm() {
  if dry; then { printf '[dry-run] aws'; printf ' %q' "$@"; printf '\n'; } >&2; else aws --profile "$AWS_PROFILE" --region "$AWS_REGION" "$@"; fi
}
# ACM certificates for CloudFront must live in us-east-1.
awsr_east() { aws --profile "$AWS_PROFILE" --region us-east-1 "$@"; }
awsm_east() {
  if dry; then { printf '[dry-run] aws --region us-east-1'; printf ' %q' "$@"; printf '\n'; } >&2; else aws --profile "$AWS_PROFILE" --region us-east-1 "$@"; fi
}
save() { printf '%s' "$2" > "$STATE/$1"; }
load() { cat "$STATE/$1" 2>/dev/null || echo "${2:-}"; }
json() { python3 -c "import json,sys; d=json.load(sys.stdin); print($1)"; }
