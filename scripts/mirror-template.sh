#!/usr/bin/env bash
# mirror-template.sh — push the release-baked template to the public template
# repository so "Use this template" and browsability are covered (ADR-0008 §6).
# The target repo and auth token come from workflow variables (MIRROR_REPO /
# MIRROR_TOKEN), so the destination is swappable without editing the workflow.
set -euo pipefail

: "${MIRROR_REPO:?MIRROR_REPO not set (workflow variable)}"
: "${MIRROR_TOKEN:?MIRROR_TOKEN not set (workflow secret)}"

# The template, already baked to the release train version by create-torimi's build.
SRC="Torimi/create-torimi/dist/template"
if [ ! -d "$SRC" ]; then
  echo "mirror-template: $SRC missing — build create-torimi first" >&2
  exit 1
fi

REPO_NAME="${MIRROR_REPO##*/}"
WORK="$(mktemp -d)"
cp -R "$SRC/." "$WORK/"

# Ship a real .gitignore and a concrete project name (the mirror repo's name) so
# the template is a usable "Use this template" repo, not a placeholder skeleton.
if [ -f "$WORK/gitignore" ]; then mv "$WORK/gitignore" "$WORK/.gitignore"; fi
grep -rl '__PROJECT_NAME__' "$WORK" | while read -r f; do
  sed -i "s/__PROJECT_NAME__/${REPO_NAME}/g" "$f"
done

cd "$WORK"
git init -q
git checkout -q -b main
git add -A
git -c user.name='torimi-release' -c user.email='noreply@users.noreply.github.com' \
  commit -q -m "Sync template from release ${GITHUB_SHA:-local}"
git push -q --force "https://x-access-token:${MIRROR_TOKEN}@github.com/${MIRROR_REPO}.git" main
echo "mirror-template: pushed baked template to ${MIRROR_REPO}"
