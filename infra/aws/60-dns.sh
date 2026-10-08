#!/usr/bin/env bash
# Apex and www → CloudFront (A + AAAA aliases), once the domain is attached.
source "$(dirname "$0")/lib.sh"
zone=$(load zone_id); dist=$(load dist_id)
aliases=$(awsr cloudfront get-distribution-config --id "$dist" --query "DistributionConfig.Aliases.Items" --output text 2>/dev/null || true)
if [[ -z "$aliases" || "$aliases" == None ]]; then
  echo "domain not attached to CloudFront yet (certificate pending); skipping DNS records"
  exit 0
fi
say "alias records → $(load dist_domain)"
changes=$(python3 - "$(load dist_domain)" "$NE_ALIASES" <<'PY'
import json, sys
target, aliases = sys.argv[1:]
out = []
for name in aliases.split(","):
    for t in ("A", "AAAA"):
        out.append({"Action": "UPSERT", "ResourceRecordSet": {"Name": name, "Type": t,
          "AliasTarget": {"HostedZoneId": "Z2FDTNDATAQYW2", "DNSName": target, "EvaluateTargetHealth": False}}})
print(json.dumps({"Changes": out}))
PY
)
awsm route53 change-resource-record-sets --hosted-zone-id "$zone" --change-batch "$changes" >/dev/null
echo "records upserted"
