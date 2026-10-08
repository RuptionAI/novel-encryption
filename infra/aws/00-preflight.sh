#!/usr/bin/env bash
# Refuse to run against any account but Hottub's.
source "$(dirname "$0")/lib.sh"
say "preflight"
acct=$(awsr sts get-caller-identity --query Account --output text)
[[ "$acct" == "$AWS_ACCOUNT_ID" ]] || { echo "wrong AWS account: $acct (want $AWS_ACCOUNT_ID, profile $AWS_PROFILE)" >&2; exit 1; }
for t in aws python3 cargo wasm-bindgen; do command -v "$t" >/dev/null || { echo "missing $t" >&2; exit 1; }; done
echo "account $acct ok"
