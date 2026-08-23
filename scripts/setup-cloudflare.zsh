#!/bin/zsh
# One-time provisioning for the Pages projects, release bucket, and Live
# database. Releases apply migrations and deploy the Worker.
# Idempotent — re-running skips whatever already exists.
set -euo pipefail

root=${0:A:h:h}
site="$root/site"
live="$root/live"
bucket=superiority-releases
pages_project=superiority-sc2-updates
docs_project=superiority-sc2docs
updates_wrangler="$site/node_modules/.bin/wrangler"
live_wrangler="$live/node_modules/.bin/wrangler"
db=superiority-live

if [[ ! -x "$updates_wrangler" ]]; then
  print -u2 "Site dependencies are missing. Run npm install in $site."
  exit 1
fi
if [[ ! -x "$live_wrangler" ]]; then
  print -u2 "Live dependencies are missing. Run npm install in $live."
  exit 1
fi

"$updates_wrangler" whoami >/dev/null

if ! NO_COLOR=1 "$updates_wrangler" pages project list | /usr/bin/grep -q "$pages_project"; then
  "$updates_wrangler" pages project create "$pages_project" --production-branch main
fi
if ! NO_COLOR=1 "$updates_wrangler" pages project list | /usr/bin/grep -q "$docs_project"; then
  "$updates_wrangler" pages project create "$docs_project" --production-branch main
fi

if ! "$updates_wrangler" r2 bucket info "$bucket" >/dev/null 2>&1; then
  "$updates_wrangler" r2 bucket create "$bucket"
fi
"$updates_wrangler" r2 bucket dev-url enable "$bucket" --force

print "Cloudflare update hosting is ready."
NO_COLOR=1 "$updates_wrangler" r2 bucket dev-url get "$bucket"
print "Feed: https://${pages_project}.pages.dev/appcast.xml"
print "SC2Docs: https://${docs_project}.pages.dev/"

if ! NO_COLOR=1 "$live_wrangler" d1 list | /usr/bin/grep -q "$db"; then
  "$live_wrangler" d1 create "$db"
fi

database_id=$(NO_COLOR=1 "$live_wrangler" d1 info "$db" --json | /usr/bin/python3 -c 'import json,sys; print(json.load(sys.stdin)["uuid"])')
if ! /usr/bin/grep -q "$database_id" "$live/wrangler.jsonc"; then
  print -u2 ""
  print -u2 "Paste this database_id into $live/wrangler.jsonc and re-run:"
  print -u2 "  \"database_id\": \"$database_id\""
  exit 1
fi

print ""
print "Cloudflare infrastructure is ready."
