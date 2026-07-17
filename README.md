<div align="center">

# poebo

**A passive, read-only campaign assistant for Path of Exile 1.**

It quietly follows your `Client.txt` log, works out where you are in the campaign
(act · zone · level), and shows just what you need in an always-on-top overlay — so you can
stop Alt-Tabbing into Path of Building while leveling.

[![CI](https://github.com/muirch/poebo/actions/workflows/ci.yml/badge.svg)](https://github.com/muirch/poebo/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)
![Tauri](https://img.shields.io/badge/Tauri-v2-24C8DB.svg)
![Path of Exile 1](https://img.shields.io/badge/Path%20of%20Exile-1-a68a64.svg)

</div>

---

## Not a bot. Not a cheat. Not an automation tool.

poebo is **strictly a tail-and-display helper**. It reads a text log the game already writes and
shows you information you could read yourself — nothing more. It **never**:

- injects into the game process,
- reads game memory,
- hooks DirectX / Vulkan / OpenGL,
- automates gameplay or simulates keyboard/mouse input,
- interacts with the game process in any way.

Its only inputs are the `Client.txt` log, optional user hotkeys, an optional clipboard read, and
optional local Path of Building files.

> ⚠️ **Not affiliated with or endorsed by Grinding Gear Games.** Path of Exile is a trademark of
> Grinding Gear Games. This is a free, non-commercial fan project.

## What it does today

- 🛰️ **Live tracking** — tails `Client.txt` and updates your **act, zone, level, and death count** in
  real time, without touching the game itself.
- 🧭 **Robust progress detection** — tolerates logouts, portals, town/hideout visits, and repeated
  zones. It even handles the fact that **Acts 6–10 reuse Act 1–5 zone names** (there's a "Lioneye's
  Watch" in both Act 1 and Act 6), disambiguating by your progression and the game's internal area id.
- ⏪ **Replay mode** — feed a recorded log back through the exact same pipeline with
  **pause / step / 1× · 2× · 5× · 20× · 100×** speed. Perfect for testing and debugging.
- ⏺️ **Recording** — a recording is itself a valid `Client.txt`, so it replays byte-for-byte.
- 🪟 **Overlay UI** — borderless, transparent, always-on-top, and **click-through** so the game keeps
  the mouse. A global hotkey (**Ctrl+Alt+Space**) "grabs" it when you want to drag it or tweak
  settings, then releases it.
- 📦 **Fresh data, from the source** — the campaign data (155 areas) is generated **directly from Path
  of Exile's game files** per patch, so it never depends on a third-party dataset going stale.

## Roadmap

poebo is built in phases; the seams for what's next are already in place.

| Phase | Scope | Status |
|------:|-------|--------|
| **1** | Client.txt watcher → parser → campaign state machine → replay → overlay | ✅ Implemented |
| **2** | Path of Building import (passive trees + gems) and a full campaign knowledge base (quests, gem rewards, passive points, lab trials) | ⏳ Planned |
| **3** | Automatic passive-tree and gem-setup switching as you progress | ⏳ Planned |
| **4** | Polished overlay, settings, packaging, and analytics | ⏳ Planned |

## How it works

The game log, replay files, and (later) Path of Building imports all become one **`LineStream`** that
a single, source-agnostic pipeline consumes:

```
Client.txt ─▶ live tail ┐
                        ├─▶ parse ─▶ campaign state machine ─▶ overlay
recorded log ─▶ replay ─┘
```

Because live tailing and replay are just two implementations of one `EventSource`, replay is
**behaviorally identical** to a live session — which is exactly why the whole engine is testable
headless.

## Project layout

```
crates/poebo-core/     Pure, Tauri-free campaign logic (fully unit-tested; no GUI deps)
crates/poebo-data/     The committed, versioned campaign data pack (embedded at build time)
src-tauri/             Thin Tauri v2 shell: overlay window, IPC, persistence
src/                   React + TypeScript overlay UI (Tailwind, Zustand, TanStack Query)
data-pack/             Generated campaign data (areas.json)
tools/data-extractor/  Node pipeline that generates the data pack from PoE's game files
```

## Tech stack

- **Backend:** Rust · Tokio · Serde · Tauri v2
- **Frontend:** React 19 · TypeScript · Vite · TailwindCSS · Zustand · TanStack Query
- **Tooling:** pnpm · a Node data-extraction pipeline built on [`pathofexile-dat`](https://github.com/SnosMe/poe-dat-viewer)

## Getting started

**Prerequisites:** Rust (stable), Node 22+, and [pnpm](https://pnpm.io/).

```bash
# Run the fast, headless core test suite (no GUI dependencies — runs anywhere)
cargo test -p poebo-core

# Build the overlay UI
pnpm install
pnpm build
```

### Running the overlay (desktop)

The Tauri overlay needs the standard WebView system libraries. On Debian/Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
                 librsvg2-dev libayatana-appindicator3-dev
pnpm tauri dev
```

The Windows installer is produced by CI (`.github/workflows/ci.yml`); you can also build it locally on
Windows with `pnpm tauri build`.

### Regenerating the campaign data

The data pack is committed, so you only need this when a new league changes the campaign. It downloads
the required tables straight from GGG's patch CDN — **no game install required**:

```bash
cd tools/data-extractor
npm ci
npm run regen
```

See [`tools/data-extractor/README.md`](tools/data-extractor/README.md) for details.

## Data & licensing

- **Application code:** [MIT](LICENSE).
- **Campaign data pack:** derived from Path of Exile game files and **© Grinding Gear Games**,
  redistributed under GGG's fan-content policy. It is not covered by the code license.

## Contributing

Issues and pull requests are welcome. The `poebo-core` crate is the heart of the project and is
covered by tests — please keep it that way (`cargo test -p poebo-core`). CI runs formatting, Clippy
(with warnings denied), the test suite, and a TypeScript-bindings drift check.

## License

Released under the [MIT License](LICENSE).
