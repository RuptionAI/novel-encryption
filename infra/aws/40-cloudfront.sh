#!/usr/bin/env bash
# CloudFront in front of the private bucket (OAC), with strict security
# headers. The custom domain and certificate are attached only once ACM has
# issued the certificate; until then the site is served on *.cloudfront.net.
source "$(dirname "$0")/lib.sh"
CACHING_OPTIMIZED=658327ea-f89d-4fab-a63d-7e88639e58f6

say "origin access control"
oac=$(awsr cloudfront list-origin-access-controls --query "OriginAccessControlList.Items[?Name=='$NE_OAC_NAME'].Id | [0]" --output text)
if [[ -z "$oac" || "$oac" == None ]]; then
  if dry; then awsm cloudfront create-origin-access-control --origin-access-control-config "Name=$NE_OAC_NAME,..."; oac="<new-oac>"
  else
    oac=$(awsm cloudfront create-origin-access-control --origin-access-control-config \
      Name="$NE_OAC_NAME",SigningProtocol=sigv4,SigningBehavior=always,OriginAccessControlOriginType=s3 \
      --query OriginAccessControl.Id --output text)
  fi
fi
echo "oac $oac"

say "security headers policy"
hp_cfg=$(python3 - "$NE_HEADERS_POLICY" "$NE_CSP" <<'PY'
import json, sys
name, csp = sys.argv[1:]
print(json.dumps({
  "Name": name, "Comment": "Novel Encryption: CSP, HSTS and friends",
  "SecurityHeadersConfig": {
    "ContentSecurityPolicy": {"Override": True, "ContentSecurityPolicy": csp},
    "StrictTransportSecurity": {"Override": True, "AccessControlMaxAgeSec": 63072000, "IncludeSubdomains": True, "Preload": False},
    "ContentTypeOptions": {"Override": True},
    "FrameOptions": {"Override": True, "FrameOption": "DENY"},
    "ReferrerPolicy": {"Override": True, "ReferrerPolicy": "no-referrer"},
  },
  "CustomHeadersConfig": {"Quantity": 3, "Items": [
    {"Header": "Permissions-Policy", "Value": "camera=(), microphone=(), geolocation=(), payment=(), usb=(), interest-cohort=()", "Override": True},
    {"Header": "Cross-Origin-Opener-Policy", "Value": "same-origin", "Override": True},
    {"Header": "Cross-Origin-Resource-Policy", "Value": "same-origin", "Override": True},
  ]},
}))
PY
)
hp=$(awsr cloudfront list-response-headers-policies --type custom --query "ResponseHeadersPolicyList.Items[?ResponseHeadersPolicy.ResponseHeadersPolicyConfig.Name=='$NE_HEADERS_POLICY'].ResponseHeadersPolicy.Id | [0]" --output text)
if [[ -z "$hp" || "$hp" == None ]]; then
  if dry; then awsm cloudfront create-response-headers-policy --response-headers-policy-config "<CSP/HSTS JSON>"; hp="<new-policy>"
  else hp=$(awsm cloudfront create-response-headers-policy --response-headers-policy-config "$hp_cfg" --query ResponseHeadersPolicy.Id --output text); fi
else
  etag=$(awsr cloudfront get-response-headers-policy --id "$hp" --query ETag --output text)
  awsm cloudfront update-response-headers-policy --id "$hp" --if-match "$etag" --response-headers-policy-config "$hp_cfg" >/dev/null
fi
echo "headers policy $hp"

# Attach the domain only when the certificate is issued.
aliases=""; cert=""
arn=$(load cert_arn)
if [[ -n "$arn" ]] && [[ "$(awsr_east acm describe-certificate --certificate-arn "$arn" --query Certificate.Status --output text 2>/dev/null)" == ISSUED ]]; then
  aliases="$NE_ALIASES"; cert="$arn"
  echo "certificate issued: attaching $aliases"
else
  echo "certificate not issued yet: serving on *.cloudfront.net only"
fi

existing=$(awsr cloudfront list-distributions --query "DistributionList.Items[?Comment=='$NE_DIST_COMMENT'].Id | [0]" --output text)
[[ "$existing" == None ]] && existing=""
caller_ref="novelencryption-$(date +%s)"; etag=""
if [[ -n "$existing" ]]; then
  live=$(awsr cloudfront get-distribution-config --id "$existing")
  etag=$(echo "$live" | json 'd["ETag"]')
  caller_ref=$(echo "$live" | json 'd["DistributionConfig"]["CallerReference"]')
fi
config=$(python3 - "$caller_ref" "$NE_BUCKET" "$AWS_REGION" "$oac" "$hp" "$aliases" "$cert" "$NE_DIST_COMMENT" "$CACHING_OPTIMIZED" <<'PY'
import json, sys
ref, bucket, region, oac, hp, aliases, cert, comment, cache = sys.argv[1:]
al = [a for a in aliases.split(",") if a]
print(json.dumps({
  "CallerReference": ref, "Comment": comment, "Enabled": True, "PriceClass": "PriceClass_100",
  "HttpVersion": "http2and3", "IsIPV6Enabled": True, "DefaultRootObject": "index.html",
  "Aliases": {"Quantity": len(al), "Items": al} if al else {"Quantity": 0},
  "ViewerCertificate": {"ACMCertificateArn": cert, "SSLSupportMethod": "sni-only", "MinimumProtocolVersion": "TLSv1.2_2021"} if cert else {"CloudFrontDefaultCertificate": True},
  "Origins": {"Quantity": 1, "Items": [{
    "Id": "site", "DomainName": f"{bucket}.s3.{region}.amazonaws.com", "OriginPath": "",
    "S3OriginConfig": {"OriginAccessIdentity": ""}, "OriginAccessControlId": oac, "CustomHeaders": {"Quantity": 0}}]},
  "DefaultCacheBehavior": {
    "TargetOriginId": "site", "ViewerProtocolPolicy": "redirect-to-https", "Compress": True,
    "AllowedMethods": {"Quantity": 2, "Items": ["GET", "HEAD"], "CachedMethods": {"Quantity": 2, "Items": ["GET", "HEAD"]}},
    "CachePolicyId": cache, "ResponseHeadersPolicyId": hp},
  "CustomErrorResponses": {"Quantity": 1, "Items": [
    {"ErrorCode": 403, "ResponsePagePath": "/index.html", "ResponseCode": "404", "ErrorCachingMinTTL": 60}]},
}))
PY
)
if [[ -z "$existing" ]]; then
  say "create distribution"
  body=$(python3 -c 'import json,sys; print(json.dumps({"DistributionConfig": json.loads(sys.argv[1]), "Tags": {"Items": [{"Key": sys.argv[2], "Value": sys.argv[3]}]}}))' "$config" "$NE_TAG_KEY" "$NE_TAG_VALUE")
  if dry; then echo "[dry-run] aws cloudfront create-distribution-with-tags <generated JSON>" >&2; dist="<new-dist>"
  else dist=$(awsm cloudfront create-distribution-with-tags --distribution-config-with-tags "$body" --query Distribution.Id --output text); fi
else
  say "update distribution $existing"
  # Lay our config over the live one so fields we never set survive.
  config=$(python3 - "$live" "$config" <<'PY'
import json, sys
REPLACE = {"ViewerCertificate", "Aliases"}  # members are mutually exclusive: replace whole
def key(d):
    return d.get("Id") or d.get("PathPattern") if isinstance(d, dict) else None
def merge(base, ours):
    if isinstance(base, dict) and isinstance(ours, dict):
        out = dict(base)
        for k, v in ours.items():
            out[k] = merge(base[k], v) if k in base and k not in REPLACE else v
        return out
    if isinstance(base, list) and isinstance(ours, list) and ours and all(key(x) for x in ours):
        live = {key(x): x for x in base if key(x)}
        return [merge(live[key(x)], x) if key(x) in live else x for x in ours]
    return ours
print(json.dumps(merge(json.loads(sys.argv[1])["DistributionConfig"], json.loads(sys.argv[2]))))
PY
)
  awsm cloudfront update-distribution --id "$existing" --if-match "$etag" --distribution-config "$config" >/dev/null
  dist="$existing"
fi
save dist_id "$dist"
if ! dry; then
  domain=$(awsr cloudfront get-distribution --id "$dist" --query Distribution.DomainName --output text)
  save dist_domain "$domain"
  echo "distribution $dist  https://$domain"
fi

say "bucket policy: CloudFront only"
policy=$(python3 - "$NE_BUCKET" "$AWS_ACCOUNT_ID" "$dist" <<'PY'
import json, sys
bucket, acct, dist = sys.argv[1:]
print(json.dumps({"Version": "2012-10-17", "Statement": [{
  "Sid": "CloudFrontRead", "Effect": "Allow", "Principal": {"Service": "cloudfront.amazonaws.com"},
  "Action": "s3:GetObject", "Resource": f"arn:aws:s3:::{bucket}/*",
  "Condition": {"StringEquals": {"AWS:SourceArn": f"arn:aws:cloudfront::{acct}:distribution/{dist}"}}}]}))
PY
)
awsm s3api put-bucket-policy --bucket "$NE_BUCKET" --policy "$policy"
echo "done"
