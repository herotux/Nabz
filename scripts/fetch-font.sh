#!/usr/bin/env bash
set -euo pipefail
mkdir -p assets
curl --fail --location --retry 3 \
  'https://raw.githubusercontent.com/google/fonts/main/ofl/notosansarabic/NotoSansArabic%5Bwdth%2Cwght%5D.ttf' \
  --output assets/NotoSansArabic-Regular.ttf
test -s assets/NotoSansArabic-Regular.ttf
echo "Downloaded Noto Sans Arabic variable font."
