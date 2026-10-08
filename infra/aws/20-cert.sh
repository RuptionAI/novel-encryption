#!/usr/bin/env bash
# ACM certificate (us-east-1, for CloudFront) for the apex and www, validated
# by DNS records in our zone. It issues once Spaceship delegates to Route 53.
source "$(dirname "$0")/lib.sh"
zone=$(load zone_id)
[[ -n "$zone" ]] || { echo "run step 10 first" >&2; exit 1; }
say "certificate for $NE_ALIASES"
arn=$(awsr_east acm list-certificates --certificate-statuses PENDING_VALIDATION ISSUED \
  --query "CertificateSummaryList[?DomainName=='$NE_DOMAIN'].CertificateArn | [0]" --output text)
if [[ -z "$arn" || "$arn" == None ]]; then
  if dry; then
    awsm_east acm request-certificate --domain-name "$NE_DOMAIN" --subject-alternative-names "www.$NE_DOMAIN" --validation-method DNS
    exit 0
  fi
  arn=$(awsm_east acm request-certificate --domain-name "$NE_DOMAIN" --subject-alternative-names "www.$NE_DOMAIN" \
    --validation-method DNS --tags Key="$NE_TAG_KEY",Value="$NE_TAG_VALUE" --query CertificateArn --output text)
  sleep 8   # validation records appear a few seconds after the request
fi
save cert_arn "$arn"
echo "certificate $arn"
records=$(awsr_east acm describe-certificate --certificate-arn "$arn" \
  --query "Certificate.DomainValidationOptions[].ResourceRecord" --output json)
changes=$(echo "$records" | python3 -c '
import json, sys
seen, out = set(), []
for r in json.load(sys.stdin) or []:
    if r and r["Name"] not in seen:
        seen.add(r["Name"])
        out.append({"Action": "UPSERT", "ResourceRecordSet": {"Name": r["Name"], "Type": r["Type"], "TTL": 300, "ResourceRecords": [{"Value": r["Value"]}]}})
print(json.dumps({"Changes": out}))')
if [[ "$(echo "$changes" | json 'len(d["Changes"])')" -gt 0 ]]; then
  awsm route53 change-resource-record-sets --hosted-zone-id "$zone" --change-batch "$changes" >/dev/null
  echo "validation records upserted"
fi
status=$(awsr_east acm describe-certificate --certificate-arn "$arn" --query Certificate.Status --output text)
echo "status $status"
