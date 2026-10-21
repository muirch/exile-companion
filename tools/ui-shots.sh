#!/bin/sh
# Screenshots of every UI state from the browser mock, plus overview.png with all of them.
#   pnpm dev --port 5199 &   then   tools/ui-shots.sh [out-dir]
# Needs google-chrome and ffmpeg. The backdrop is a fake game scene so the see-through
# tooltip body can be judged.
set -eu

OUT=${1:-.ui-shots}
URL=${UI_URL:-http://localhost:5199}
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)

stage() { # name query width height
  cat > "$OUT/.stage-$1.html" <<EOF
<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;height:100%;overflow:hidden}
body{background:
 radial-gradient(ellipse at 30% 75%, #6b7c45 0%, transparent 40%),
 radial-gradient(ellipse at 70% 85%, #b08a4c 0%, transparent 35%),
 radial-gradient(ellipse at 55% 60%, #d8d0b0 0%, transparent 18%),
 repeating-linear-gradient(35deg,#2b2a22 0 18px,#24231c 18px 36px)}
iframe{position:absolute;left:50%;bottom:0;transform:translateX(-50%);width:$3px;height:$4px;border:0;background:transparent}
</style></head><body><iframe allowtransparency="true" src="$URL/?$2"></iframe></body></html>
EOF
}

shot() { # name page width height
  google-chrome --headless=new --disable-gpu --hide-scrollbars --force-prefers-reduced-motion \
    --window-size="$3,$4" --virtual-time-budget=2500 --screenshot="$OUT/$1.png" "$2" 2>/dev/null
}

overlay() { # name query
  stage "$1" "$2" 460 520
  shot "$1" "file://$OUT/.stage-$1.html" 560 420
}

overlay overlay-first-zone "view=locked&edge=0"
overlay overlay-town "view=locked&edge=1"
overlay overlay-hints "view=locked&edge=3"
overlay overlay-passage "view=locked&edge=4"
overlay overlay-boss "view=locked&edge=9"
overlay overlay-unlocked "view=unlocked&edge=7"
overlay notice "view=notice"
stage menu "window=menu" 236 220
shot menu "file://$OUT/.stage-menu.html" 560 420
shot settings "$URL/?window=settings" 560 1200

cd "$OUT"
ffmpeg -loglevel error -y \
  -i overlay-first-zone.png -i overlay-town.png -i overlay-hints.png \
  -i overlay-passage.png -i overlay-boss.png -i overlay-unlocked.png \
  -i notice.png -i menu.png \
  -filter_complex "[0][1][2]hstack=3[r1];[3][4][5]hstack=3[r2];\
[6][7]hstack=2,pad=1680:420:0:0:color=#24231c[r3];\
[r1][r2][r3]vstack=3" overview.png
rm -f .stage-*.html
echo "$OUT/overview.png"
