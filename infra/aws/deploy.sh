#!/usr/bin/env bash
# novelencryption.com on AWS (Hottub account), in order. Every step is
# idempotent. BILLABLE: Route 53 zone ($0.50/month), S3 and CloudFront (pennies).
#
#   ./deploy.sh --dry-run        # print every mutating command, change nothing
#   ./deploy.sh                  # everything (domain attaches once the certificate is issued)
#   ./deploy.sh 50               # rebuild the site and upload it
#   ./deploy.sh 20 40 60         # after the nameserver switch: certificate, aliases, DNS records
set -euo pipefail
cd "$(dirname "$0")"
steps=()
for a in "$@"; do
  case "$a" in
    --dry-run) export DRY_RUN=1 ;;
    [0-9][0-9]) steps+=("$a") ;;
    *) echo "usage: ./deploy.sh [--dry-run] [step…]" >&2; exit 2 ;;
  esac
done
[[ ${#steps[@]} -gt 0 ]] || steps=(00 10 20 30 40 50 60 70)
for s in "${steps[@]}"; do
  script=$(ls "${s}"-*.sh 2>/dev/null | head -1)
  [[ -n "$script" ]] || { echo "no step $s" >&2; exit 2; }
  bash "./$script"
done
