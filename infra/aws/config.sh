# Novel Encryption on AWS (Hottub account). No secrets here.
export AWS_PROFILE="${AWS_PROFILE:-hottub-new}"
export AWS_REGION="${AWS_REGION:-us-west-2}"
export AWS_ACCOUNT_ID="509083161733"          # deploy refuses any other account

export NE_DOMAIN="novelencryption.com"
export NE_ALIASES="novelencryption.com,www.novelencryption.com"
export NE_BUCKET="novelencryption-site-${AWS_ACCOUNT_ID}"
export NE_DIST_COMMENT="novelencryption"      # how 40-cloudfront.sh finds its distribution
export NE_OAC_NAME="novelencryption-site"
export NE_HEADERS_POLICY="novelencryption-security-headers"
export NE_TAG_KEY="project"
export NE_TAG_VALUE="novelencryption"

# Matches the <meta> CSP in site/*.html, plus frame-ancestors (header-only).
export NE_CSP="default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; connect-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
