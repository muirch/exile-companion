# poebo data-extractor

Generates poebo's campaign data pack (`../../data-pack/areas.json`) directly from Path of Exile's game files, so the data stays current per patch and doesn't depend on any third-party dataset.

This is a standalone Node toolchain and is not part of the Rust app build. The app only reads the committed `data-pack/*.json`. You run this when you want to refresh the data, for example after a new league.

## How it works

1. [`pathofexile-dat`](https://www.npmjs.com/package/pathofexile-dat) downloads the bundle index and the `WorldAreas` table straight from GGG's patch CDN by version. No PoE install is required (Oodle decompression runs in WASM via `ooz-wasm`). The output lands in `tables/English/WorldAreas.json`.
2. `src/transform/areas.ts` reshapes the raw rows into poebo's flat `ZoneInfo` schema, resolving the row-index connectivity into `Id` references.
3. `src/export.ts` writes `../../data-pack/areas.json` and `manifest.json` (with a sha256), and validates the result against a couple of known anchors (The Twilight Strand is act 1; Lioneye's Watch is an act 1 town with a waypoint).

## Regenerate

```bash
cd tools/data-extractor
npm ci                 # restore pinned deps
npm run regen          # fetch, transform, and write ../../data-pack/*.json
# npm run regen -- --no-fetch   # skip the download and reuse the cached tables/ (offline transform)
```

Then review the `data-pack/areas.json` diff and commit it. For a new league, update `patch` in `config.json`. The current version is listed at <https://raw.githubusercontent.com/poe-tool-dev/latest-patch-version/main/latest.txt>.

## Versioning

- `config.json` `patch`: the pinned game version the facts come from. This changes every league.
- `SCHEMA_VERSION` in `src/export.ts` and `PACK_SCHEMA_VERSION` in `crates/poebo-data`: the shape of the data pack. Bump both together only when the schema changes; the Rust loader rejects a mismatch.

## Notes and gotchas

- The cold CDN fetch occasionally exits non-zero after writing valid output, so `export.ts` tolerates that and validates the produced file instead.
- `tables/`, `.cache/`, and `node_modules/` are git-ignored. Only the generated `data-pack/*.json` is committed.
- The generated data is derived from PoE game files and is (c) Grinding Gear Games (see the repo README).
