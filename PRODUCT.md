# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The UI is a web page rendered inside a Tauri v2 window on Windows (WebView2). Linux runs the same page in WebKitGTK. There the app runs as an X11 window (the game under Proton is one too), so the overlay, the quick menu and the chat work on GNOME with Wayland through XWayland. The Linux app is built from source, installers are Windows only.

## Users

The author first, a Path of Exile player who wants one companion next to the game instead of a guide, Path of Building and a chat window open on the side. The tool is meant to be shared with the community later.

The campaign guide, the first feature, serves Path of Exile 1 players leveling through the ten acts, usually at a league start. Two kinds, served by one route:

- regular players who run the campaign once or twice a league and don't want to open guides or alt-tab,
- experienced players and speedrunners who know the campaign and want the shortest route with as little text as possible.

The build assistant serves newer players and players who follow a build guide without understanding Path of Building. Players who already read PoB fluently need it least. A new player shouldn't design a build. They pick a proven guide, and the assistant brings their character toward the guide's stage: what differs, what that costs, what to change or buy first. Advice is the difference to a reference build (the guide's PoB, or several guides for the same skill), ranked by what PoB says each change is worth. It doesn't suggest passives, keystones or items the reference builds don't take, unless the player asks to explore. Without a guide, it helps choose one: playstyle, budget, league start or not, then links to popular guides for that class and skill, and the player loads the guide's PoB.

The author also tests it live in the game.

## Product Purpose

Exile Companion is a companion app for Path of Exile that sits over the game. The goal is an assistant that knows the player's build and answers questions about it from real Path of Building calculations. The campaign guide is the first feature and the one that works today. It tells the player what to do in the area they are in, where to go next, which quest to hand in, which boss to kill and which gems the build wants from each quest. Success for the guide means the player levels through the campaign without alt-tabbing to a guide or to Path of Building.

## Positioning

Exile Companion is a native Windows app with its engine in Rust and one overlay that every feature shares. The campaign guide reads only the game's `Client.txt` log, knows which area the player is in from the log's internal area ids, and shows the matching steps of a speedrunning route (exile-leveling's, MIT). The build assistant comes next and will work from the same imported Path of Building build.

## Operating Context

- Used while playing, in windowed or windowed-fullscreen mode. The overlay is always on top, transparent, click-through by default.
- Read in short glances during combat and loading screens, over a busy, dark game scene.
- Holding the tilde key (left of 1) in the game opens a quick menu at the cursor (route and layout toggles, previous and next step, settings, move, quit). Settings live in their own frameless window, styled like the overlay and moved by its title bar, also reachable from the tray. Alt+M unlocks the overlay for dragging.
- It shows only while the game (or the overlay itself) is the foreground window. At startup without the game it shows a short "running in the background" notice that also names the menu key, the step keys and, in compact, where to turn on the full layout. In a hideout, a map or an area off the route it says it is showing the last step. It marks each finished act on the first step of the next one and says when the campaign is done.

## Capabilities and Constraints

- Inputs allowed: `Client.txt`, user hotkeys, the clipboard on request, local PoB files, the player's installed Path of Building (found in the usual install folders or picked in settings, and run headless for build calculations), one build file in PoB's Builds folder when the player turns on sync (Exile Companion's own or one the player picks, kept the same as the app's build in both directions), and a PoB build link the user pastes (downloaded once from the build site, the way PoB itself imports it). It must never inject into the game, read its memory, hook rendering, or automate play. This follows GGG's third-party tool policy and is non-negotiable.
- Four opt-in exceptions to "sends nothing to the game", all off by default, one action per key press and only while the game is in front. The relog key types `/exit` in chat (decided by the user on 2026-10-21). The hideout key types `/hideout` the same way (decided on 2026-10-25). Both wait until the player lets go of the shortcut's modifiers. The item check key presses Ctrl+C over the item under the cursor, the game's own text copy (decided on 2026-10-25). The overlay then shows a card above the route for 12 seconds: for gear the verdict for the best slot, the DPS, EHP and life change and resistance changes, measured with the player's PoB through compare_item and no model, for a gem which socket groups of the build use it with their links and where the campaign gives it. When a checked upgrade needs a higher level than the character has, the card says so and the item is remembered until the app quits. On a level-up the same place shows what became usable: the build's gems from the level the build starts using them, and the remembered upgrades that came due. Entering Kitava's zone (The Cathedral Rooftop in act 5, The Feeding Trough in act 10) measures the build's elemental resistances with the penalty he leaves (-30% and -60%) and, if any falls below 75%, shows them there before the fight. No model, nothing when they stay capped. Reaching a new act with a followed guide shows the chat's What's next there too, cut to three changes, and a line when the character's level has reached the guide's next stage. Nothing without a guide or when the build is on track. The gem search key types a search string for the gems to buy here into the game's search field, so the open vendor window highlights them (decided by the user on 2026-10-25). The game draws the highlight, Exile Companion doesn't read the screen. On Linux these keys go to the game through X11's XTest, the game under Proton being an X11 window.
- All hotkeys are configurable. By default you hold the tilde key for the quick menu, press Alt+A to ask the assistant, Alt+M to move the overlay, Alt+Left and Alt+Right to step the route, Alt+D to check an item, F5 for the hideout, F9 to relog and Alt+G to search a vendor for the route's gems when they're on. Tab can replace tilde. Esc isn't offered because the game acts on it first.
- Every route step has a type, such as kill, arena, enter area, waypoint, portal, logout, quest hand-in, quest item, trial, ascend, crafting recipe, direction (8-way compass) or copy text.
- Campaign data (areas, quests, rewards, trials) is extracted from the game files per patch.
- The overlay window is fitted to its content and anchored by its bottom center to a fraction of the monitor (by default in the gap between the flask bar and the skill bar, clear of the play field and the stash and vendor windows), so it survives resolution changes and the route stays put when the move bar opens above it.
- UI language: English, matching the game client and the route text.
- With a Path of Building build imported, the route lists its gems after the quest hand-in where to take them as the reward or buy them from a vendor. A gem is placed at the point the build needs it. Preset titles ("Act 3", "lvl 28", "Bossing"), preset order and the gem's required level decide that. Gems the campaign never gives are listed apart. When the player's class gets a gem only after the act the build needs it, or never, and another class gets it for the first quest, the route's first step and the settings suggest a mule: a character of that class kills Hillock and leaves the gem in the stash before the real one starts. Exile Companion keeps the last imported build until it is cleared, also across restarts. The settings list the build's presets with what Exile Companion reads from each, and the player can set the act a preset is for, mark it endgame or leave it out. A "Gems" row in the settings lists every gem of the build with where the plan takes it, and the player can move one to the act they want it in or mark it as one they already have (from a mule or bought before the run), which leaves it out of the route. These choices reset when another build is imported. Settings choose whether the route shows these gem steps at all, only the quest rewards, or rewards and vendor buys (the default).
- When the area costs experience, the overlay's zone line (and the full layout's subtitle) ends with the share the character gets there, "XP 64%", from PoE 1's penalty formula, the area level and the character level. Nothing at full experience or in towns.
- The compact layout is the default. It has the zone, act and step on one grey line, then the actions and the way out. The full layout adds the header band, the class level and the route hints, for players new to the campaign. It can be switched from the quick menu or the settings. Either layout can fade out after a few seconds without a new step and comes back with the next step or area, or with a tap of the menu key. In compact the same tap shows the step's hints for a few seconds.
- The player asks the build assistant from the game. Alt+A or the quick menu opens a chat window in the overlay's style, on the right of the screen and where the player last put it. It takes the keyboard while open, and Esc hands it back to the game while an answer keeps coming. Before the first question it shows what to change next. Those are the gems, passives (with points), gear and ascendancy notables that bring the build closest to the followed guide's stage, measured with Path of Building and ranked by what they give, plus a note when the character's level has reached the guide's next stage. That card uses no model and no key, so the most common question costs nothing. Answers stream in with the point in the first line and the rest behind Details. While it works, the window says which tool runs ("Measuring passives") and shows the model's short progress notes. Each answer names the model that gave it and what it cost, when the provider says. The player brings their own key. The provider is Anthropic (Claude Sonnet 5.5 by default, Haiku 5.5 at a twentieth of its price, or Opus 5.5), OpenRouter with any model there that supports tools, or any server with OpenAI's chat completions (DeepSeek, Kimi, LM Studio, Ollama). Keys stay in the system's credential store (Windows Credential Manager, Secret Service on Linux), one per provider, and go only to that provider. Settings also pick the effort, low by default because thinking is most of what an answer costs. A conversation lasts until New chat or quitting the app, and switching the provider starts a new one. A model that stops without a word gets a plain "no answer" message instead of an empty one. With Ollama the model loads when the chat opens and frees the video memory for the game a minute after it closes. A 7.5B local model on an RTX 5080 answered in 3 to 9 seconds but gave wrong advice, so local models aren't recommended for now. Which cloud models give good advice with our tools isn't tested yet. The setup and costs for players are in docs/chat.md. The chat uses the same tools as the MCP server, which the app starts for it. The MCP server also stays for players who'd rather use their own AI agent and subscription (Claude Desktop, Claude Code, Cursor, VS Code with Copilot, Codex and others), and for testing. The assistant answers from Path of Building's calculations on the imported build and from the Path of Exile Wiki. For a respec it can list the branches of the taken tree that give the least per point, with the points each frees and what it costs. It knows where the player is from `Client.txt`, remembers the guide they follow between conversations, and with their OK makes a guide their build, so the route shows that guide's gems. Settings connect a known agent with one click by adding the server to its config (the old file is kept as a copy), or copy a config for other agents.
- Planned next are a table of models tested on our scenarios, passive tree guidance from a PoB build, and Path of Exile 2 after its 1.0 release.

## Brand Commitments

- Name: Exile Companion, two words, both capitalised. Not shortened to "EC" in the UI.
- Not affiliated with or endorsed by Grinding Gear Games. Path of Exile is their trademark. The app must not imitate GGG branding or logos.
- Third-party content keeps its attribution. The leveling route is exile-leveling's (MIT).

## Evidence on Hand

- Real route data: `crates/data/data/routes/exile-leveling/act-*.txt` (233 areas with league-start steps).
- Browser mock with real route views: `app/ui/src/ipc/mock-route.json`, used by `pnpm dev`.
- No users, reviews, or usage numbers yet. Don't invent any.

## Product Principles

1. Glanceable over complete. The player reads it mid-fight, so the next action must be clear in one look.
2. Invisible until needed. No clicks stolen from the game, nothing on screen outside the game.
3. Feels like part of the game, not a web app placed on top of it.
4. Honest about what it knows. When the log or route position is unknown, say so and say how to fix it.
5. Read-only toward the game, always.

## Accessibility & Inclusion

Text must stay readable at small sizes over bright and dark game scenes, and color must not be the only way to tell step types apart.
