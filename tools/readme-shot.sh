#!/bin/sh
# The README's picture: the chat next to the compact overlay with an item card, over a fake
# game scene, from the browser mock.
#   pnpm dev --port 5199 &   then   tools/readme-shot.sh
# Needs google-chrome.
set -eu

URL=${UI_URL:-http://localhost:5199}
OUT=$(cd "$(dirname "$0")/.." && pwd)/docs/images
STAGE=$(mktemp --suffix=.html)
trap 'rm -f "$STAGE"' EXIT

cat > "$STAGE" <<EOF
<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;height:100%;overflow:hidden}
body{background:
 radial-gradient(ellipse at 22% 70%, #6b7c45 0%, transparent 38%),
 radial-gradient(ellipse at 48% 88%, #b08a4c 0%, transparent 30%),
 repeating-linear-gradient(35deg,#2b2a22 0 18px,#24231c 18px 36px)}
iframe{position:absolute;border:0;background:transparent}
.overlay{left:120px;bottom:0;width:460px;height:560px}
.chat{right:60px;bottom:24px;width:420px;height:400px}
</style></head><body>
<iframe class="overlay" allowtransparency="true" src="$URL/?view=locked&edge=1&compact&card=gear"></iframe>
<iframe class="chat" allowtransparency="true" src="$URL/?window=chat&chat=readme"></iframe>
</body></html>
EOF

google-chrome --headless=new --disable-gpu --hide-scrollbars --force-prefers-reduced-motion \
  --window-size=1100,450 --virtual-time-budget=2500 --screenshot="$OUT/companion.png" \
  "file://$STAGE" 2>/dev/null
echo "$OUT/companion.png"
