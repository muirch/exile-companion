# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The UI is a web page rendered inside a Tauri v2 window on Windows (WebView2). Linux runs the same page but without a reliable overlay.

## Users

The author first, a Path of Exile player who wants one companion next to the game instead of a guide, Path of Building and a chat window open on the side. The tool is meant to be shared with the community later.

The campaign guide, the first feature, serves Path of Exile 1 players leveling through the ten acts, usually at a league start. Two kinds, served by one route:

- regular players who run the campaign once or twice a league and don't want to open guides or alt-tab,
- experienced players and speedrunners who know the campaign and want the shortest route with as little text as possible.

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

- Inputs allowed: `Client.txt`, user hotkeys, the clipboard on request, local PoB files, the player's installed Path of Building (found in the usual install folders or picked in settings, and run headless for build calculations), and a PoB build link the user pastes (downloaded once from the build site, the way PoB itself imports it). It must never inject into the game, read its memory, hook rendering, or automate play. This follows GGG's third-party tool policy and is non-negotiable.
- One opt-in exception to "sends nothing to the game": the relog key types `/exit` in chat, one command per key press, only while the game is in front, off by default (decided by the user on 2026-10-21, modelled on Awakened PoE Trade's F9).
- All hotkeys are configurable. By default you hold the tilde key for the quick menu, press Alt+M to move the overlay, Alt+Left and Alt+Right to step the route, and F9 to relog when it's on. Tab can replace tilde. Esc isn't offered because the game acts on it first.
- Every route step has a type, such as kill, arena, enter area, waypoint, portal, logout, quest hand-in, quest item, trial, ascend, crafting recipe, direction (8-way compass) or copy text.
- Campaign data (areas, quests, rewards, trials) is extracted from the game files per patch.
- The overlay window is fitted to its content and anchored by its bottom center to a fraction of the monitor (by default in the gap between the flask bar and the skill bar, clear of the play field and the stash and vendor windows), so it survives resolution changes and the route stays put when the move bar opens above it.
- UI language: English, matching the game client and the route text.
- With a Path of Building build imported, the route lists its gems after the quest hand-in where to take them as the reward or buy them from a vendor. A gem is placed at the point the build needs it. Preset titles ("Act 3", "lvl 28", "Bossing"), preset order and the gem's required level decide that. Gems the campaign never gives are listed apart. Exile Companion keeps the last imported build until it is cleared, also across restarts. The settings list the build's presets with what Exile Companion reads from each, and the player can set the act a preset is for, mark it endgame or leave it out. Settings choose whether the route shows these gem steps at all, only the quest rewards, or rewards and vendor buys (the default).
- The compact layout is the default. It has the zone, act and step on one grey line, then the actions and the way out. The full layout adds the header band, the class level and the route hints, for players new to the campaign. It can be switched from the quick menu or the settings. Either layout can fade out after a few seconds without a new step and comes back with the next step or area, or with a tap of the menu key. In compact the same tap shows the step's hints for a few seconds.
- Planned next are passive tree guidance from a PoB build, an AI build assistant to chat with, and Path of Exile 2 after its 1.0 release.

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
