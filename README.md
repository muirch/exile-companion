<div align="center">

# poebo

A passive, read-only campaign assistant for Path of Exile 1.

It follows the game's `Client.txt` log, works out where you are in the campaign (act, zone, level), and shows what you need in an always-on-top overlay, so you can stop Alt-Tabbing into Path of Building every few zones while leveling.

[![CI](https://github.com/muirch/poebo/actions/workflows/ci.yml/badge.svg)](https://github.com/muirch/poebo/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)
![Tauri](https://img.shields.io/badge/Tauri-v2-24C8DB.svg)
![Path of Exile 1](https://img.shields.io/badge/Path%20of%20Exile-1-a68a64.svg)

</div>

---

## What it is, and what it isn't

poebo only reads a text log the game already writes to disk, and shows you things you could read yourself. That is the whole idea. It does not:

- inject into the game process,
- read game memory,
- hook DirectX, Vulkan or OpenGL,
- automate anything,
- touch the game process in any way.

The only inputs it looks at are the `Client.txt` log, a few optional hotkeys, an optional clipboard read, and (later) your local Path of Building files.

It sends nothing to the game, with one opt-in exception: the relog key (off by default, F9 when on) types `/exit` into the game chat, the same way Awakened PoE Trade does. That is one chat command per key press, only while the game is in front, which GGG's third-party policy allows.

This project is not affiliated with or endorsed by Grinding Gear Games. Path of Exile is a trademark of Grinding Gear Games. It is a free, non-commercial fan tool.

## What works today

- **Live tracking.** It tails `Client.txt` and keeps your act, zone, level and death count up to date in real time, without ever touching the game.
- **Sensible progress detection.** Logouts, portals, town and hideout visits, and re-entering the same zone all behave the way you'd expect. It also handles the fact that Acts 6-10 reuse the zone names from Acts 1-5 (there is a "Lioneye's Watch" in both Act 1 and Act 6), telling them apart from where you are in the run and from the game's internal area id.
- **Replay.** Feed a recorded log back through the same pipeline, with pause, single-step, and 1x / 2x / 5x / 20x / 100x speeds. Handy for testing and for reproducing bugs.
- **Recording.** A recording is just another `Client.txt`, so it plays back exactly.
- **Leveling route.** The overlay shows what to do in the current area and what comes next, using the route from [exile-leveling](https://github.com/HeartofPhos/exile-leveling) (MIT). It follows the area the game loads, and your level tells repeat town visits apart; Alt+Right and Alt+Left step it by hand (all hotkeys can be changed in settings). The position is saved between sessions.
- **Quick menu and settings.** Hold Esc in the game for a menu under the cursor: turn features on or off, open the settings window, move the overlay, quit. poebo only watches the key; the game still gets it.
- **The overlay.** A borderless, transparent, always-on-top window that is click-through by default, so the game keeps the mouse. Press Alt+M to unlock it and drag it, and again to lock it.
- **Data straight from the game.** The 155-area campaign map is generated from Path of Exile's own game files for the current patch, so it does not depend on someone else's dataset staying current.

## Roadmap

It is built in phases, and the groundwork for the later ones is already in place.

- **Phase 1 (done):** the `Client.txt` watcher, parser, campaign state machine, replay, and the overlay.
- **Phase 2:** import a Path of Building build (passive trees and gems) and fill out the campaign knowledge base (quests, gem rewards, passive points, lab trials).
- **Phase 3:** switch the shown tree section and gem setup automatically as you level.
- **Phase 4:** polish, settings, packaging, and some leveling analytics.

## How it works

The live log, replayed logs, and (later) Path of Building imports all turn into one stream of lines that a single pipeline consumes: read a line, fold it into the campaign state, update the overlay. Live tailing and replay are just two sources feeding that same pipeline, which is why replay behaves exactly like a live session, and why the whole engine can be tested without a GUI.

## Project layout

```
crates/poebo-core       pure campaign logic, no Tauri, fully unit-tested
crates/poebo-data       the committed campaign data pack, embedded at build time
src-tauri               the Tauri v2 shell: overlay window, IPC, persistence
src                     the React + TypeScript overlay UI
data-pack               generated campaign data (areas.json)
tools/data-extractor    Node pipeline that builds the data pack from PoE's game files
```

## Tech stack

- Backend: Rust, Tokio, Serde, Tauri v2.
- Frontend: React, TypeScript, Vite, Tailwind CSS, Zustand, TanStack Query.
- Tooling: pnpm, plus a small Node extraction pipeline built on [pathofexile-dat](https://github.com/SnosMe/poe-dat-viewer).

## Getting started

You will need Rust (stable), Node 22+, and [pnpm](https://pnpm.io/).

```bash
# run the headless core test suite (no GUI dependencies, runs anywhere)
cargo test -p poebo-core

# watch the engine follow a sample campaign log, no GUI needed
cargo run -p poebo-core --example replay_log

# develop the overlay UI in a browser with mock data (no Tauri or webkit needed)
pnpm install
pnpm dev

# UI component tests and lint
pnpm test
pnpm lint

# or build it for production
pnpm build
```

There is a sample log at [examples/sample-client.txt](examples/sample-client.txt) you can also load in the app's replay mode.

### Running the overlay

The desktop overlay needs the usual WebView libraries. On Debian or Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
                 librsvg2-dev libayatana-appindicator3-dev
pnpm tauri dev
```

The overlay is a Windows target: the desktop app is only built on Windows (via the manual "Release" workflow, or `pnpm tauri build` on Windows). On Linux, develop the UI in the browser with `pnpm dev` and the logic with `cargo test` / the `replay_log` example.

### Regenerating the campaign data

The data pack is committed, so you only need this when a new league changes the campaign. It pulls the tables it needs straight from GGG's patch CDN, with no game install required:

```bash
cd tools/data-extractor
npm ci
npm run regen
```

See [tools/data-extractor/README.md](tools/data-extractor/README.md) for the details.

## Data and licensing

The overlay uses Alegreya Sans and Alegreya Sans SC by Huerta Tipográfica, bundled in `src/assets/fonts` under the SIL Open Font License (`OFL.txt` there).

The leveling route in `data-pack/routes/exile-leveling` is written by the exile-leveling authors and kept under its MIT license (see the `LICENSE` file there). `npm run routes` in `tools/data-extractor` refreshes it from a pinned commit.

The design skill in `.claude/skills/impeccable` is [impeccable](https://github.com/pbakaus/impeccable) by Paul Bakaus, under the Apache License 2.0 (`LICENSE` there).

The application code is MIT; see [LICENSE](LICENSE). The campaign data pack is derived from Path of Exile's game files and is still (c) Grinding Gear Games, redistributed under their fan content policy, so it is not covered by the code license.

## How it's built

This is a side project, and I build it with an AI coding agent (Claude Code) on purpose. I set the architecture and the constraints, review every change, and test it in the game; the agent does most of the typing. It's a faster way to get a hobby tool done, and an experiment in how far that workflow goes when you keep it on a short leash.

The leash is in the repo:

- [AGENTS.md](AGENTS.md) has the project rules: what poebo may and may not do to the game, what to run before committing, how to write comments and commit messages. `CLAUDE.md` only points Claude Code at it.
- [PRODUCT.md](PRODUCT.md) says who the tool is for and what it must never do.
- [DESIGN.md](DESIGN.md) and `.impeccable/` hold the design system. UI work goes through the impeccable skill and gets an independent review before it ships.
- `.claude/settings.json` has the hooks and the permission rules every session shares.

Every change goes through a pull request, CI and my review before it lands on `main`.

## Contributing

Issues and pull requests are welcome. `poebo-core` is the heart of the project and it is covered by tests, so please keep it that way (`cargo test -p poebo-core`). The UI has its own tests (`pnpm test`). CI runs formatting, Clippy with warnings treated as errors, the Rust and UI tests, the UI lint, and a check that the TypeScript bindings are up to date.

## License

MIT. See [LICENSE](LICENSE).
