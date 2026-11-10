#!/bin/sh
# Answer 4's survey (#2324): which "keep this out" signals public bank, health and other pages
# send, read from one fetch each. Signed-in pages need accounts, which this never uses.
# Usage: survey.sh urls.txt > results.txt
set -e
ua='Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0 Safari/537.36'
body=$(mktemp)
while read -r kind url; do
  head=$(curl -s -m 15 -L -A "$ua" -D - -o "$body" "$url" || true)
  code=$(echo "$head" | grep -i '^HTTP/' | tail -1 | awk '{print $2}')
  cache=$(echo "$head" | grep -i '^cache-control:' | tail -1 | tr -d '\r' | cut -d: -f2- | tr -d ' ')
  robots=$( (echo "$head" | grep -i '^x-robots-tag:' | grep -iv 'bot:'; grep -io '<meta[^>]*name="robots"[^>]*>' "$body") | grep -io 'noarchive\|noindex' | head -1 || true)
  types=$(grep -o '"@type" *: *"[A-Za-z]*"' "$body" | sed 's/.*"\([A-Za-z]*\)"$/\1/' | sort -u |
    grep -E 'Medical|Hospital|Physician|Health|Bank|Financial|Insurance|Drug|Clinic|Patient|Dentist|Pharmacy' | tr '\n' ' ' || true)
  case "$cache" in *no-store*) store=no-store ;; *) store=- ;; esac
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$kind" "${code:--}" "$store" "${robots:--}" "${types:--}" "$url"
done < "$1"
rm -f "$body"
