# poebo — Path of Exile 1 Campaign Assistant

A **passive, read-only** desktop helper for the Path of Exile 1 campaign. It tails the game's
`Client.txt` log, works out where you are (act / zone / level), and shows the relevant info — and,
in later phases, the matching Path of Building passive-tree section and gem setup — in an
always-on-top overlay, so you rarely have to Alt+Tab into Path of Building while leveling.

> **Not affiliated with or endorsed by Grinding Gear Games.** Path of Exile is a trademark of
> Grinding Gear Games. This is a free, non-commercial fan tool.

## What it does _not_ do

It is strictly a tail-and-display tool. It **never** injects into the game, reads game memory, hooks
graphics APIs, automates gameplay, or simulates input. Its only inputs are the `Client.txt` log,
optional user hotkeys, optional clipboard, and optional local Path of Building files.

## Architecture (short version)

The log, replay files, and (later) Path of Building imports all become one `LineStream` consumed by a
single source-agnostic pipeline (`parse → campaign state → UI`). Live tailing and replay are two
implementations of one `EventSource`, so they are behaviorally identical.

```
crates/poebo-core   Pure, Tauri-free campaign logic (unit-testable headless)
crates/poebo-data   The committed, versioned campaign data pack (embedded)
src-tauri           Thin Tauri v2 shell: overlay window, IPC, persistence
src/                React + TypeScript + Tailwind overlay UI
data-pack/          Generated campaign data (areas.json) — see tools/data-extractor
tools/data-extractor  Node pipeline that generates the data pack from PoE game files
```

## Development

Requires Rust (stable), Node 20+, and pnpm.

```bash
# Rust core: build + test (no GUI deps — runs anywhere)
cargo build
cargo test -p poebo-core

# Regenerate the campaign data pack from PoE's game files (see tools/data-extractor/README.md)
cd tools/data-extractor && npm ci && npm run regen

# Run the overlay app in dev (added once the Tauri shell lands)
pnpm install
pnpm tauri dev
```

## Data & licensing

- Application code: `MIT OR Apache-2.0`.
- The generated campaign data pack is **derived from Path of Exile game files, © Grinding Gear
  Games**, redistributed under GGG's fan-content policy. It is not covered by the code license.
