#!/usr/bin/env bash
# Route 53 hosted zone. The domain is registered at Spaceship; point its
# nameservers at the four printed here (one-time, in the Spaceship dashboard).
source "$(dirname "$0")/lib.sh"
say "hosted zone $NE_DOMAIN"
zone=$(awsr route53 list-hosted-zones-by-name --dns-name "$NE_DOMAIN." --query "HostedZones[?Name=='$NE_DOMAIN.'].Id | [0]" --output text)
if [[ -z "$zone" || "$zone" == None ]]; then
  if dry; then
    awsm route53 create-hosted-zone --name "$NE_DOMAIN" --caller-reference "ne-<ts>"
    zone="/hostedzone/<new>"
  else
    zone=$(awsm route53 create-hosted-zone --name "$NE_DOMAIN" --caller-reference "ne-$(date +%s)" \
      --hosted-zone-config Comment="Novel Encryption" --query HostedZone.Id --output text)
    awsm route53 change-tags-for-resource --resource-type hostedzone --resource-id "${zone##*/}" \
      --add-tags Key="$NE_TAG_KEY",Value="$NE_TAG_VALUE"
  fi
fi
zone="${zone##*/}"
save zone_id "$zone"
echo "zone $zone"
if ! dry; then
  echo "nameservers (set these at Spaceship):"
  awsr route53 get-hosted-zone --id "$zone" --query DelegationSet.NameServers --output text | tr '\t' '\n' | sed 's/^/  /'
  echo "currently delegated to:"; dig +short NS "$NE_DOMAIN" | sed 's/^/  /'
fi
