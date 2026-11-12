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

shot() { # name page width height [virtual ms]
  google-chrome --headless=new --disable-gpu --hide-scrollbars --force-prefers-reduced-motion \
    --window-size="$3,$4" --virtual-time-budget="${5:-2500}" --screenshot="$OUT/$1.png" "$2" 2>/dev/null
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
overlay overlay-compact "view=locked&edge=1&compact"
overlay overlay-compact-boss "view=locked&edge=9&compact"
overlay notice "view=notice"
stage menu "window=menu" 236 399
shot menu "file://$OUT/.stage-menu.html" 560 420
for c in nokey empty answered history saved; do
  stage "chat-$c" "window=chat&chat=$c" 420 560
  shot "chat-$c" "file://$OUT/.stage-chat-$c.html" 560 600
done
for g in gems:window=gems gems-early:window=gems\&set=0 gems-hover:window=gems\&hover gems-notes:window=gems\&notes gems-items:window=gems\&items\&hover; do
  stage "${g%%:*}" "${g#*:}" 340 540
  shot "${g%%:*}" "file://$OUT/.stage-${g%%:*}.html" 440 580
done
# The dev server builds the tree's mock data on the first request, so ask for it once first.
shot tree-warmup "$URL/?window=tree" 632 540 8000
for t in tree:window=tree tree-new:window=tree\&mode=new tree-character:window=tree\&mode=character\&hover \
  tree-nochar:window=tree\&mode=character\&nochar tree-mastery:window=tree\&stage=4\&hover=mastery \
  tree-ascendancy:window=tree\&mode=new\&hover=ascendancy tree-early:window=tree\&stage=0 \
  tree-bloodline:window=tree\&stage=11 tree-list:window=tree\&stage=11\&list \
  tree-folded:window=tree\&stage=11\&folded=Guardian; do
  stage "${t%%:*}" "${t#*:}" 632 540
  # Headless Chrome sometimes shoots before the big mock tree has loaded, and an empty window
  # compresses smaller than a drawn one. Keep the biggest of a few tries, stop at a full tree.
  best=0
  for _ in 1 2 3 4; do
    shot try "file://$OUT/.stage-${t%%:*}.html" 700 580 15000
    size=$(stat -c %s "$OUT/try.png")
    if [ "$size" -gt "$best" ]; then
      best=$size
      mv "$OUT/try.png" "$OUT/${t%%:*}.png"
    fi
    [ "$best" -gt 120000 ] && break
  done
  rm -f "$OUT/try.png"
done
shot settings "$URL/?window=settings" 560 2300

cd "$OUT"
ffmpeg -loglevel error -y \
  -i overlay-first-zone.png -i overlay-town.png -i overlay-hints.png \
  -i overlay-passage.png -i overlay-boss.png -i overlay-unlocked.png \
  -i notice.png -i menu.png -i overlay-compact.png \
  -filter_complex "[0][1][2]hstack=3[r1];[3][4][5]hstack=3[r2];[6][7][8]hstack=3[r3];\
[r1][r2][r3]vstack=3" overview.png
ffmpeg -loglevel error -y -i chat-nokey.png -i chat-empty.png -i chat-answered.png \
  -i chat-history.png -i chat-saved.png \
  -filter_complex "[0][1][2][3][4]hstack=5" chat.png
ffmpeg -loglevel error -y -i gems.png -i gems-early.png -i gems-hover.png -i gems-notes.png -i gems-items.png \
  -filter_complex "[0][1][2][3][4]hstack=5" gems-both.png
ffmpeg -loglevel error -y -i tree.png -i tree-new.png -i tree-character.png -i tree-nochar.png \
  -i tree-mastery.png -i tree-ascendancy.png -i tree-early.png -i tree-bloodline.png -i tree-list.png \
  -i tree-folded.png -filter_complex "[0][1][2][3][4]hstack=5[r1];[5][6][7][8][9]hstack=5[r2];[r1][r2]vstack=2" \
  tree-all.png
rm -f .stage-*.html tree-warmup.png
echo "$OUT/overview.png"
echo "$OUT/chat.png"
echo "$OUT/gems-both.png"
echo "$OUT/tree-all.png"
