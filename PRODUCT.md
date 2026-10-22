# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The UI is a web page rendered inside a Tauri v2 window on Windows (WebView2). Linux runs the same page but without a reliable overlay.

## Users

Path of Exile 1 players leveling a character through the ten-act campaign, usually at a league start. Two kinds, served by one route:

- regular players who run the campaign once or twice a league and don't want to open guides or alt-tab;
- experienced players and speedrunners who know the campaign and want the shortest route with as little text as possible.

The first user is the author, who also tests it live in the game. The tool is meant to be shared with the community later.

## Product Purpose

poebo is a passive overlay that sits over the game and tells the player what to do in the area they are in: where to go next, which quest to hand in, where to place a portal, which boss to kill, where the trial is. Success means the player levels through the campaign without alt-tabbing to a guide or to Path of Building.

## Positioning

poebo reads only the game's `Client.txt` log, knows which area the player is in from the log's internal area ids, and shows the matching steps of a speedrunning route (exile-leveling's, MIT). It is a native Windows app with the route engine in Rust, built to grow into a multi-tool overlay: build guidance from Path of Building and an AI build assistant that answers from real PoB calculations.

## Operating Context

- Used while playing, in windowed or windowed-fullscreen mode. The overlay is always on top, transparent, click-through by default.
- Read in short glances during combat and loading screens, over a busy, dark game scene.
- Holding the tilde key (left of 1) in the game opens a quick menu at the cursor (feature toggles, settings, move, quit). Settings live in their own frameless window, styled like the overlay and moved by its title bar, also reachable from the tray. Alt+M unlocks the overlay for dragging.
- It shows only while the game (or the overlay itself) is the foreground window. At startup without the game it shows a short "running in the background" notice.

## Capabilities and Constraints

- Inputs allowed: `Client.txt`, user hotkeys, the clipboard on request, local PoB files, and a PoB build link the user pastes (downloaded once from the build site, the way PoB itself imports it). It must never inject into the game, read its memory, hook rendering, or automate play. This follows GGG's third-party tool policy and is non-negotiable.
- One opt-in exception to "sends nothing to the game": the relog key types `/exit` in chat, one command per key press, only while the game is in front, off by default (decided by the user on 2026-10-21, modelled on Awakened PoE Trade's F9).
- All hotkeys are configurable. Defaults: hold the tilde key for the quick menu (Tab is the other choice; Esc isn't offered because the game acts on it first), Alt+M to move the overlay, Alt+Left / Alt+Right to step the route, F9 to relog when enabled.
- Route steps come typed: kill, arena, enter area, waypoint, portal, logout, quest hand-in, quest item, trial, ascend, crafting recipe, direction (8-way compass), copy text.
- Campaign data (areas, quests, rewards, trials) is extracted from the game files per patch.
- The overlay window is fitted to its content and anchored by its bottom center to a fraction of the monitor, so it survives resolution changes and the route stays put when the move bar opens above it.
- UI language: English, matching the game client and the route text.
- With a Path of Building build imported, the route lists its gems after the quest hand-in where to take them as the reward or buy them from a vendor. A gem is placed at the point the build needs it: preset titles ("Act 3", "lvl 28", "Bossing"), preset order and the gem's required level decide that. Gems the campaign never gives are listed apart. Settings choose whether the route shows these gem steps at all, only the quest rewards, or rewards and vendor buys (the default).
- Planned features that share the overlay: gem and passive-tree guidance from a PoB build, an AI build assistant (chat), and Path of Exile 2 support after its 1.0 release.

## Brand Commitments

- Name: poebo, lower case.
- Not affiliated with or endorsed by Grinding Gear Games; Path of Exile is their trademark. The app must not imitate GGG branding or logos.
- Third-party content keeps its attribution: the leveling route is exile-leveling's (MIT).

## Evidence on Hand

- Real route data: `data-pack/routes/exile-leveling/act-*.txt` (233 areas with league-start steps).
- Browser mock with real route views: `src/ipc/mock-route.json`, used by `pnpm dev`.
- No users, reviews, or usage numbers yet. Don't invent any.

## Product Principles

1. Glanceable over complete: the player reads it mid-fight, so the next action must be clear in one look.
2. Invisible until needed: no clicks stolen from the game, no presence outside the game.
3. Feels like part of the game, not a web app placed on top of it.
4. Honest about what it knows: when the log or route position is unknown, say so and say how to fix it.
5. Read-only toward the game, always.

## Accessibility & Inclusion

Text must stay readable at small sizes over bright and dark game scenes, and color must not be the only way to tell step types apart.
