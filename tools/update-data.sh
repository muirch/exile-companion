#!/bin/sh
# Points the game data, the leveling route and the PoB the engine tests use at new versions, then
# regenerates the data. An empty variable takes the latest:
#   PATCH=3.30.0.1 ROUTE_COMMIT=<sha> POB_TAG=v2.68.0 tools/update-data.sh
# Needs node, npm and an authenticated gh. Writes a summary for the pull request to $SUMMARY
# (update-data.md in the current folder by default).
set -eu
cd "$(dirname "$0")/.."

CONFIG=tools/data-extractor/config.json
ROUTES=tools/data-extractor/src/routes.ts
POB=tools/fetch-pob.sh
SUMMARY=${SUMMARY:-update-data.md}

old_patch=$(sed -n 's/.*"patch": "\([^"]*\)".*/\1/p' "$CONFIG")
old_route=$(sed -n "s/^const COMMIT = '\([0-9a-f]*\)';/\1/p" "$ROUTES")
old_pob=$(sed -n 's/^TAG=${POB_TAG:-\(.*\)}$/\1/p' "$POB")

PATCH=${PATCH:-$(curl -fsSL https://raw.githubusercontent.com/poe-tool-dev/latest-patch-version/main/latest.txt | tr -d '[:space:]')}
ROUTE_COMMIT=${ROUTE_COMMIT:-$(gh api repos/HeartofPhos/exile-leveling/commits/main --jq .sha)}
POB_TAG=${POB_TAG:-$(gh api repos/PathOfBuildingCommunity/PathOfBuilding/releases/latest --jq .tag_name)}

echo "$PATCH" | grep -Eq '^[0-9]+(\.[0-9]+){3}$' || { echo "update-data: not a game patch: $PATCH" >&2; exit 1; }
echo "$ROUTE_COMMIT" | grep -Eq '^[0-9a-f]{40}$' || { echo "update-data: not a commit: $ROUTE_COMMIT" >&2; exit 1; }
echo "$POB_TAG" | grep -Eq '^v[0-9]+(\.[0-9]+)+$' || { echo "update-data: not a PoB release: $POB_TAG" >&2; exit 1; }
# PoB keeps one passive tree folder per league, 3_30 for 3.30.x.
TREE=$(echo "$PATCH" | cut -d. -f1-2 | tr . _)

sed -i "s/\"patch\": \"[^\"]*\"/\"patch\": \"$PATCH\"/" "$CONFIG"
sed -i "s/^const COMMIT = '[0-9a-f]*';/const COMMIT = '$ROUTE_COMMIT';/" "$ROUTES"
sed -i "s/^TAG=\${POB_TAG:-.*}\$/TAG=\${POB_TAG:-$POB_TAG}/; s/^TREE=\${POB_TREE:-.*}\$/TREE=\${POB_TREE:-$TREE}/" "$POB"

# The extractor goes on with old tables when the download fails, so start without them.
rm -rf tools/data-extractor/tables
(cd tools/data-extractor && npm ci --no-audit --no-fund && npm run regen && npm run routes)

{
  echo "Game data from patch $PATCH (was $old_patch)."
  echo
  if [ "$ROUTE_COMMIT" = "$old_route" ]; then
    echo "exile-leveling route unchanged."
  else
    echo "exile-leveling route: https://github.com/HeartofPhos/exile-leveling/compare/$old_route...$ROUTE_COMMIT"
  fi
  echo
  echo "Path of Building for the engine tests: $POB_TAG with tree $TREE (was $old_pob)."
} > "$SUMMARY"

if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "patch=$PATCH" >> "$GITHUB_OUTPUT"
fi
cat "$SUMMARY"
