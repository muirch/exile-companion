# poebo data-extractor

Generates poebo's campaign **data pack** (`../../data-pack/areas.json`) directly from Path of
Exile's game files, so the data is fresh per patch and independent of any third-party dataset.

**This is a standalone Node toolchain. It is NOT part of the Rust app build** — the app only reads
the committed `data-pack/*.json`. You run this only when you want to refresh the data (e.g. a new
league).

## How it works

1. [`pathofexile-dat`](https://www.npmjs.com/package/pathofexile-dat) downloads the bundle index and
   the `WorldAreas` table straight from GGG's patch CDN by version — **no PoE install required**
   (Oodle decompression runs in WASM via `ooz-wasm`). Output lands in `tables/English/WorldAreas.json`.
2. `src/transform/areas.ts` reshapes the raw rows into poebo's flat `ZoneInfo` schema, resolving the
   row-index connectivity into `Id` references.
3. `src/export.ts` writes `../../data-pack/areas.json` + `manifest.json` (with a sha256), validated
   against known anchors (The Twilight Strand → act 1; Lioneye's Watch → act 1 town waypoint).

## Regenerate

```bash
cd tools/data-extractor
npm ci                 # restore pinned deps
npm run regen          # fetch + transform + write ../../data-pack/*.json
# npm run regen -- --no-fetch   # skip the download, reuse the cached tables/ (offline transform)
```

Then review the `data-pack/areas.json` diff and commit it. Update `config.json`'s `patch` for a new
league — find the current version at
<https://raw.githubusercontent.com/poe-tool-dev/latest-patch-version/main/latest.txt>.

## Versioning

- `config.json` → `patch`: the pinned game version the facts come from (changes every league).
- `SCHEMA_VERSION` in `src/export.ts` and `PACK_SCHEMA_VERSION` in `crates/poebo-data`: the data-pack
  shape. Bump both together only when the schema changes; the Rust loader rejects a mismatch.

## Notes / gotchas

- The cold CDN fetch occasionally exits non-zero after writing valid output; `export.ts` tolerates
  that and validates the produced file instead.
- `tables/`, `.cache/`, and `node_modules/` are git-ignored — only the generated `data-pack/*.json`
  is committed.
- The generated data is derived from PoE game files, © Grinding Gear Games (see repo README).
