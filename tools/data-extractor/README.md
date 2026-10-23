# Exile Companion data-extractor

Generates Exile Companion's campaign data pack (`../../crates/data/data/{areas,quests,trials}.json`) directly from Path of Exile's game files, so the data stays current per patch and doesn't depend on any third-party dataset.

This is a standalone Node toolchain and is not part of the Rust app build. The app only reads the committed `crates/data/data/*.json`. You run this when you want to refresh the data, for example after a new league.

## How it works

1. [`pathofexile-dat`](https://www.npmjs.com/package/pathofexile-dat) downloads the bundle index and the tables listed in `config.json` straight from GGG's patch CDN by version. No PoE install is required (Oodle decompression runs in WASM via `ooz-wasm`). The output lands in `tables/English/*.json`. Foreign keys export as row indices into the referenced table.
2. `src/transform/*.ts` reshape the raw rows into Exile Companion's flat schemas, resolving the row-index references into ids:
   - `areas.ts` -> `ZoneInfo` (zone -> act table + connectivity).
   - `quests.ts` -> `QuestInfo` (every campaign quest, with per-class gem rewards, passive points and the reward NPC). Join: `QuestRewards.RewardOffer -> QuestRewardOffers.QuestKey -> Quest`, plus `Reward -> BaseItemTypes` (gem id/name, or a Book of Skill for a passive point) and `Characters -> Characters` (class names). Routes reference quests without rewards too, so all of them are kept.
   - `trials.ts` -> `TrialInfo` (campaign Trials of Ascendancy). Join: `LabyrinthTrials.WorldAreas -> WorldAreas`. Endgame (Uber Lab, act 11) trials are dropped.
3. `src/export.ts` writes `../../crates/data/data/{areas,quests,trials}.json` and `manifest.json` (each file with a sha256), and validates each against known anchors (The Twilight Strand is act 1, a1q1 rewards a Witch starter gem, The Lower Prison is an act 1 trial, there are 6 trials in acts 1-3 and 12 in total).

### Data not extracted here (known gaps)

- **Vendor (purchasable) gems** have no clean `.dat` table (only quest gem rewards do). The app takes them from exile-leveling's data, which comes from the PoE Wiki `vendor_rewards` table.
- The bandit choice in act 2 (kill all for +2 passives) is a branch, not a Book of Skill, so it isn't counted.

## Regenerate

```bash
cd tools/data-extractor
npm ci                 # restore pinned deps
npm run regen          # fetch, transform, and write ../../crates/data/data/*.json
# npm run regen -- --no-fetch   # skip the download and reuse the cached tables/ (offline transform)
```

Then review the `crates/data/data/*.json` diff and commit it. For a new league, update `patch` in `config.json`. The current version is listed at <https://raw.githubusercontent.com/poe-tool-dev/latest-patch-version/main/latest.txt>.

## Versioning

- `config.json` `patch`: the pinned game version the facts come from. This changes every league.
- Each data file carries its own `schema_version` (`AREAS_SCHEMA`/`QUESTS_SCHEMA`/`TRIALS_SCHEMA` in `src/export.ts`, mirrored by the `*_SCHEMA_VERSION` consts in `crates/exile-data`). Bump a file's version and its Rust const together only when that file's shape changes. The Rust loader rejects a mismatch.

## Notes and gotchas

- The cold CDN fetch occasionally exits non-zero after writing valid output, so `export.ts` tolerates that and validates the produced file instead.
- `tables/`, `.cache/`, and `node_modules/` are git-ignored. Only the generated `crates/data/data/*.json` is committed.
- The generated data is derived from PoE game files and is (c) Grinding Gear Games (see the repo README).
