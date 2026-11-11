#!/bin/sh
# The README's picture over a fake game scene, from the browser mock: the full overlay with an
# item card, the compact overlay with a level-up card and the chat.
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
.full{left:16px;bottom:16px;width:460px;height:640px}
.compact{left:476px;bottom:16px;width:460px;height:640px}
.chat{right:24px;bottom:24px;width:420px;height:400px}
</style></head><body>
<iframe class="full" allowtransparency="true" src="$URL/?view=locked&edge=1&card=gear"></iframe>
<iframe class="compact" allowtransparency="true" src="$URL/?view=locked&edge=9&compact&card=levelup"></iframe>
<iframe class="chat" allowtransparency="true" src="$URL/?window=chat&chat=readme"></iframe>
</body></html>
EOF

google-chrome --headless=new --disable-gpu --hide-scrollbars --force-prefers-reduced-motion \
  --window-size=1400,440 --virtual-time-budget=2500 --screenshot="$OUT/companion.png" \
  "file://$STAGE" 2>/dev/null
echo "$OUT/companion.png"
