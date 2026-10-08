#!/usr/bin/env bash
# Build the site and upload it with correct types and cache lifetimes, then
# invalidate. Pinned texts and fonts never change: cache a year. Everything
# else: a minute, so a deploy shows up quickly.
source "$(dirname "$0")/lib.sh"
say "build"
dry || "$ROOT/scripts/build_site.sh"
DIST="$ROOT/dist"
SHORT="public, max-age=60"
LONG="public, max-age=31536000, immutable"
up() {  # up <include-glob> <content-type> <cache-control>
  awsm s3 cp "$DIST" "s3://$NE_BUCKET" --recursive --only-show-errors --exclude "*" --include "$1" \
    --content-type "$2" --cache-control "$3"
}
say "upload to s3://$NE_BUCKET"
up "*.html" "text/html; charset=utf-8" "$SHORT"
up "*.css" "text/css; charset=utf-8" "$SHORT"
up "fonts/*.css" "text/css; charset=utf-8" "$LONG"
up "*.js" "text/javascript; charset=utf-8" "$SHORT"
up "*.wasm" "application/wasm" "$SHORT"
up "*.json" "application/json; charset=utf-8" "$SHORT"
up "*.md" "text/markdown; charset=utf-8" "$SHORT"
up "novels/*.txt" "text/plain; charset=utf-8" "$LONG"
up "*.woff2" "font/woff2" "$LONG"
# Remove files that are no longer part of the site (everything current was
# just uploaded above, so this only deletes).
awsm s3 sync "$DIST" "s3://$NE_BUCKET" --delete --size-only --only-show-errors
dist=$(load dist_id)
if [[ -n "$dist" ]]; then
  awsm cloudfront create-invalidation --distribution-id "$dist" --paths "/*" --query Invalidation.Id --output text
fi
echo "uploaded"
