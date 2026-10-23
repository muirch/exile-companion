#!/bin/sh
# Fetches the parts of Path of Building the engine needs, for its tests: tools/fetch-pob.sh [dir]
# then EXILE_POB_DIR=<dir> cargo test -p exile-pob-engine. About 220 MB, no images and only the
# current passive tree (PoB also loads every tree's Assets.lua and the non-version folders).
set -eu

TAG=${POB_TAG:-v2.67.2}
TREE=${POB_TREE:-3_29}
DIR=${1:-.pob}

if [ ! -d "$DIR/.git" ]; then
  git clone --quiet --depth 1 --branch "$TAG" --filter=blob:none --no-checkout \
    https://github.com/PathOfBuildingCommunity/PathOfBuilding.git "$DIR"
fi
cd "$DIR"
git sparse-checkout set --no-cone '/manifest.xml' '/runtime/lua/' '/src/' \
  '!/src/TreeData/[0-9]*/' "/src/TreeData/$TREE/" '/src/TreeData/*/Assets.lua' \
  '!*.png' '!*.jpg' '!*.jpeg' '!*.webp'
git checkout --quiet
echo "$(pwd) at $TAG"
