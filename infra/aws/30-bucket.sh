#!/usr/bin/env bash
# Private S3 bucket for the site; only CloudFront (via OAC, step 40) reads it.
source "$(dirname "$0")/lib.sh"
say "bucket $NE_BUCKET"
if ! awsr s3api head-bucket --bucket "$NE_BUCKET" 2>/dev/null; then
  awsm s3api create-bucket --bucket "$NE_BUCKET" --create-bucket-configuration LocationConstraint="$AWS_REGION" >/dev/null
fi
awsm s3api put-public-access-block --bucket "$NE_BUCKET" --public-access-block-configuration \
  BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=true,RestrictPublicBuckets=true
awsm s3api put-bucket-ownership-controls --bucket "$NE_BUCKET" --ownership-controls 'Rules=[{ObjectOwnership=BucketOwnerEnforced}]'
awsm s3api put-bucket-encryption --bucket "$NE_BUCKET" --server-side-encryption-configuration \
  '{"Rules":[{"ApplyServerSideEncryptionByDefault":{"SSEAlgorithm":"AES256"},"BucketKeyEnabled":true}]}'
awsm s3api put-bucket-tagging --bucket "$NE_BUCKET" --tagging "TagSet=[{Key=$NE_TAG_KEY,Value=$NE_TAG_VALUE}]"
echo "bucket ready (private, encrypted)"
