<div align="center">

# Exile Companion

A companion app for Path of Exile that sits over the game. I'm building it for myself first. The goal is an assistant that knows your build and answers questions about it. Today it guides you through the campaign, and the first version of the assistant answers in a chat over the game.

[![CI](https://github.com/muirch/exile-companion/actions/workflows/ci.yml/badge.svg)](https://github.com/muirch/exile-companion/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Path of Exile 1](https://img.shields.io/badge/Path%20of%20Exile-1-a68a64.svg)

![The full overlay in three areas. A boss to kill, a town step with the gems to take and buy, and a passage with only the way out.](docs/images/overlay.png)

</div>

## What it does today

The first thing it does is the campaign. Exile Companion sits over the game and looks like one of its item tooltips. The header is the area you're in, the body is what to do there, and the last line is the way out. Bosses, quests, waypoints and trials use the game's own text colours, so you can read a step in one glance between fights.

It follows you on its own. The game writes every area it loads to its `Client.txt` log, and Exile Companion reads that log to know where you are, including the Act 6-10 towns that share names with Act 1-5. Your level tells repeat visits apart. The route is the speedrunning route from [exile-leveling](https://github.com/HeartofPhos/exile-leveling), and you can step through it by hand when you go off it.

Import your Path of Building build, from a file, a PoB code or a pobb.in or poe.ninja link, and the route also says which gem to take from each quest and what to buy from which vendor, at the point the build needs it.

The overlay shows only while the game is in front, and clicks go through it to the game. It starts as a few compact lines between the flask bar and the skill bar. The full layout adds the area header and the route's hints.

Press Alt+A and ask about your build. The assistant measures with your own Path of Building, knows where you are in the campaign and reads the Path of Exile Wiki. Before you ask anything, the chat lists what to change next to get closer to the guide you follow, and that list needs no AI model and costs nothing. Answers come from Anthropic, OpenRouter or a model server you name, with your own key, at about $0.001 to $0.03 a question depending on the model. See [docs/chat.md](docs/chat.md).

## Install

Exile Companion runs on Windows, and on Linux from source (the overlay needs X11 or XWayland, which is how the game runs under Proton). On Windows, download the setup file from the [dev release](https://github.com/muirch/exile-companion/releases/tag/dev) and run it. A newer build installs over the old one. Builds aren't signed yet, so Windows SmartScreen may ask you to confirm.

The app used to be called poebo. If you have poebo installed, uninstall it. Exile Companion picks up its settings and saved build on the first start.

Start Exile Companion before or after the game. It finds `Client.txt` in the usual Steam and standalone install folders. If it doesn't, pick the file in the settings.

## Keys

All of them can be changed in the settings.

| Key | What it does |
|---|---|
| Hold Tilde (left of 1) | Opens the quick menu with route and layout, previous and next step, the assistant, settings, moving the overlay and quit |
| Tap Tilde | Show the overlay again after it faded out, and the step's hints in the compact layout |
| Alt+A | Open or close the assistant's chat |
| Alt+M | Unlock the overlay to drag it, press again to lock |
| Alt+Left, Alt+Right | Previous or next step of the route |
| Alt+D | Check the item under the cursor against your build, off until you turn it on |
| F5 | Go to your hideout, off until you turn it on |
| F9 | Relog to character select, off until you turn it on |
| Alt+G | At a vendor, search for the gems the route says to buy there, off until you turn it on |

## How it treats the game

Exile Companion reads `Client.txt`, a text log the game writes to disk anyway, plus the keys you bind and, when you ask, the clipboard. If you paste a build link (pobb.in, poe.ninja, pastebin and the other sites Path of Building imports from), it downloads that one build. If you turn on sync with Path of Building, it reads and writes one build file in PoB's Builds folder. When you ask the assistant, your question and what its tools read about your build and your progress go to the AI provider you picked, and nothing goes while you don't ask. It leaves the game process alone. It doesn't read the game's memory or hook its rendering, and it doesn't play for you.

Four keys send something to the game, and all are off until you turn them on. F5 types `/hideout` and F9 types `/exit` in chat, the way Awakened PoE Trade does. Alt+D presses Ctrl+C over the item under the cursor to copy its text, like Awakened PoE Trade's Ctrl+D. Alt+G types a search into the open vendor window, the way Exile-UI does, and the game highlights the gems. Each is one action per key press, and only while the game is in front.

Exile Companion isn't affiliated with or endorsed by Grinding Gear Games. Path of Exile is their trademark. It's a free, non-commercial fan tool.

## Where it's going

The campaign guide is the first feature, not the whole app. What I want most is an assistant next to the game that knows my build. I want to ask it why my damage dropped or what to change next, and get an answer from real Path of Building numbers instead of a guess.

- A build assistant that answers from Path of Building's own calculations. The chat over the game is new and still being tested with different models. You can also ask through an AI agent you already use, such as Claude Desktop, Claude Code, Cursor or Codex, see [docs/mcp.md](docs/mcp.md).
- Passive tree guidance from your build while you level.
- Path of Exile 2, after its 1.0 release.

## For developers

### How it works

The live log and a replayed log both become one stream of lines. A single pipeline reads it, parses each line, folds it into the campaign state and updates the overlay. Replay behaves exactly like a live session, and the whole engine is tested without a GUI.

```
app/ui                  the React and TypeScript UI
app/desktop             the Tauri v2 shell with windows, hotkeys, IPC and settings
app/mcp                 the build assistant's MCP server for Claude Desktop and Claude Code
crates/core             campaign logic, no Tauri, covered by tests
crates/pob              Path of Building import
crates/pob-engine       Path of Building's calculations run headless, with LuaJIT built in
crates/fetch            HTTP with a disk cache for the assistant's outside sources
crates/wiki             the Path of Exile Wiki through its API, for the build assistant
crates/guides           build guides with their PoB links from poebuilds.cc
crates/assistant        the assistant's chat, a model with the MCP server's tools
crates/data             the campaign data and route, embedded at build time
tools/data-extractor    builds the campaign data from the game's files
docs                    images and a sample Client.txt
```

The backend is Rust, Tokio and Tauri v2. The UI is React, TypeScript, Vite, Tailwind CSS and Zustand.

### Getting started

You need Rust (stable), Node 22+ and [pnpm](https://pnpm.io/).

```bash
# engine tests, no GUI needed
cargo test -p exile-core

# watch the engine follow a sample campaign log
cargo run -p exile-core --example replay_log

# the UI in a browser with mock data, no Tauri or Windows needed
pnpm install
pnpm dev

# UI tests and lint
pnpm test
pnpm lint

# the PoB engine tests run on a copy of Path of Building, about 220 MB
tools/fetch-pob.sh .pob
EXILE_POB_DIR=$PWD/.pob cargo test -p exile-pob-engine
```

The desktop app is built for Windows, by the manual Release workflow or `pnpm tauri build` on Windows. On Linux, work on the UI with `pnpm dev` and on the logic with `cargo test`. There's a sample log at [docs/sample-client.txt](docs/sample-client.txt) for the replay mode.

### Regenerating the campaign data

The data pack is committed, so you only need this when a league changes the campaign. It pulls the tables straight from GGG's patch CDN, no game install required:

```bash
cd tools/data-extractor
npm ci
npm run regen
```

See [tools/data-extractor/README.md](tools/data-extractor/README.md) for details.

### How it's built

This is a side project, and I build it with an AI coding agent (Claude Code) on purpose. I set the architecture and the constraints, review every change and test it in the game. The agent does most of the typing. It's a faster way to get a hobby tool done, and an experiment in how far that workflow goes when you keep it on a short leash.

The leash is in the repo. [AGENTS.md](AGENTS.md) has the project rules. It says what Exile Companion may and may not do to the game, what to run before committing and how to write comments and commits. [PRODUCT.md](PRODUCT.md) says who the tool is for and what it must never do. [DESIGN.md](DESIGN.md) and `.impeccable/` hold the design system, and UI work goes through the impeccable skill. `.claude/settings.json` has the hooks and permission rules every session shares. Every change goes through a pull request and CI before it lands on `main`.

### Contributing

Issues and pull requests are welcome. Logic lives in `exile-core` and comes with tests. UI behaviour is tested with Vitest next to the component. CI runs rustfmt, Clippy with warnings as errors, the Rust and UI tests, the UI lint, a check that the TypeScript bindings are up to date, and a Windows build. A pull request with the `smoke` label also installs the app on a Windows runner and starts it a few times.

If you open the project in Claude Code, the hooks in `.claude/settings.json` run the impeccable design checks after UI edits. They download the impeccable binary the first time. If you don't want that, turn them off with `/impeccable hooks off`. Nothing else depends on Claude Code.

## Credits and licenses

The leveling route in `crates/data/data/routes/exile-leveling` is written by the exile-leveling authors and kept under its MIT license (see the `LICENSE` file there).

`crates/pob-engine/lua/headless.lua` is adapted from Path of Building Community's headless wrapper, under its MIT license (see `headless.LICENSE` next to it). The engine runs Path of Building itself from a folder on your machine. PoB isn't part of this repo.

The build assistant reads the [Path of Exile Wiki](https://www.poewiki.net) through its API, on your machine. Wiki content is under CC BY-NC-SA 3.0 and isn't part of this repo.

The overlay uses Alegreya Sans and Alegreya Sans SC by Huerta Tipográfica, under the SIL Open Font License (`app/ui/src/assets/fonts/OFL.txt`).

The design skill in `.claude/skills/impeccable` is [impeccable](https://github.com/pbakaus/impeccable) by Paul Bakaus, under the Apache License 2.0.

The campaign data pack comes from Path of Exile's game files and is (c) Grinding Gear Games, shared under their fan content policy, so the code license doesn't cover it.

The application code is under the MIT license, see [LICENSE](LICENSE).
