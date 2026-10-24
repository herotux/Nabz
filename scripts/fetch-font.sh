#!/usr/bin/env bash
set -euo pipefail
mkdir -p assets
curl --fail --location --retry 3 \
  'https://raw.githubusercontent.com/google/fonts/main/ofl/vazirmatn/Vazirmatn%5Bwght%5D.ttf' \
  --output assets/Vazirmatn.ttf
curl --fail --location --retry 3 \
  'https://raw.githubusercontent.com/google/fonts/main/ofl/vazirmatn/OFL.txt' \
  --output assets/OFL.txt
test -s assets/Vazirmatn.ttf
test -s assets/OFL.txt
echo "Downloaded the Vazirmatn Persian font and its OFL license."
