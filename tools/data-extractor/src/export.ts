// Orchestrates a full data-pack regeneration:
//   1. run pathofexile-dat (fetch the tables in config.json from GGG's patch CDN -> tables/English/)
//   2. reshape rows -> Exile Companion's flat schemas (see ./transform/*.ts)
//   3. write ../../data-pack/{areas,quests,trials}.json + manifest.json (with sha256), each
//      validated against anchors so a bad extraction never overwrites a good pack
//
// Run with `npm run regen`. Pass `--no-fetch` to skip the download and reuse the cached tables
// (useful in CI once the bundle cache is warm, or for offline transform iteration).

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { transformAreas, type RawWorldArea, type ZoneInfo } from './transform/areas.js';
import {
  transformQuests,
  type QuestInfo,
  type RawBaseItemType,
  type RawCharacter,
  type RawNPC,
  type RawNPCTalk,
  type RawQuest,
  type RawQuestReward,
  type RawQuestRewardOffer,
} from './transform/quests.js';
import { transformTrials, type RawLabyrinthTrial, type TrialInfo } from './transform/trials.js';

// Each data file carries its own schema_version. The Rust loaders validate per file.
const AREAS_SCHEMA = 1;
const QUESTS_SCHEMA = 1;
const TRIALS_SCHEMA = 1;

const here = dirname(fileURLToPath(import.meta.url)); // tools/data-extractor/src
const extractorDir = resolve(here, '..'); // tools/data-extractor
const repoRoot = resolve(extractorDir, '..', '..'); // repo root
const dataPackDir = resolve(repoRoot, 'data-pack');
const tablesDir = resolve(extractorDir, 'tables/English');
const cliPath = resolve(extractorDir, 'node_modules/pathofexile-dat/dist/cli/run.js');

const SOURCE = 'PoE game files (GGG patch CDN) via pathofexile-dat';

function readPatch(): string {
  const cfg = JSON.parse(readFileSync(resolve(extractorDir, 'config.json'), 'utf8')) as {
    patch?: string;
  };
  return cfg.patch ?? 'unknown';
}

function runFetch(): void {
  console.log('[regen] fetching game data via pathofexile-dat ...');
  try {
    execFileSync(process.execPath, [cliPath], { cwd: extractorDir, stdio: 'inherit' });
  } catch {
    // The cold CDN fetch occasionally exits non-zero after writing valid output. Tolerate it here
    // and let the output validation below be the real gate.
    console.warn('[regen] pathofexile-dat exited non-zero - validating its output before failing.');
  }
}

function loadTable<T>(name: string): T[] {
  const path = resolve(tablesDir, `${name}.json`);
  if (!existsSync(path)) {
    throw new Error(`${name}.json missing at ${path}; the fetch step produced no output.`);
  }
  return JSON.parse(readFileSync(path, 'utf8')) as T[];
}

function writeJson(path: string, value: unknown): string {
  const text = JSON.stringify(value, null, 2) + '\n';
  writeFileSync(path, text);
  return text;
}

function sha256(text: string): string {
  return createHash('sha256').update(text).digest('hex');
}

function validateAreaAnchors(areas: ZoneInfo[]): void {
  // NB: zone names are NOT unique across acts - Acts 6-10 revisit Acts 1-5 with the same names
  // (e.g. "Lioneye's Watch" exists in both Act 1 and Act 6). So we validate that at least one
  // candidate per name matches the expected Act-1 facts, not that a name maps 1:1.
  const strandActs = areas.filter((a) => a.name === 'The Twilight Strand').map((a) => a.act);
  const watchAct1 = areas.find((a) => a.name === "Lioneye's Watch" && a.act === 1);
  if (!strandActs.includes(1)) {
    throw new Error('Anchor failed: no Act-1 "The Twilight Strand" - extraction looks wrong; not writing.');
  }
  if (!watchAct1 || !watchAct1.is_town || !watchAct1.has_waypoint) {
    throw new Error('Anchor failed: Act-1 "Lioneye\'s Watch" is not a waypoint town - not writing.');
  }
}

function validateQuestAnchors(quests: QuestInfo[]): void {
  // "Enemy at the Gate" (a1q1) is the first quest and rewards a starter skill gem per class.
  const first = quests.find((q) => q.id === 'a1q1');
  if (!first || first.gem_rewards.length === 0) {
    throw new Error('Anchor failed: a1q1 "Enemy at the Gate" has no gem rewards - not writing.');
  }
  const witchStarter = first.gem_rewards.some((g) => g.classes.includes('Witch'));
  if (!witchStarter) {
    throw new Error('Anchor failed: a1q1 has no Witch-class starter gem - class join looks wrong; not writing.');
  }
  if (first.npc !== 'Tarkleigh') {
    throw new Error(`Anchor failed: a1q1 reward NPC is "${first.npc}" (expected Tarkleigh) - NPC join looks wrong; not writing.`);
  }
  const withGems = quests.filter((q) => q.gem_rewards.length > 0).length;
  if (withGems !== 12) {
    throw new Error(`Anchor failed: ${withGems} campaign gem-reward quests (expected exactly 12; Tencent-only offers must be excluded) - not writing.`);
  }
  // "The Way Forward" (a1q9) grants a Book of Skill (a passive point) and no gem.
  const a1q9 = quests.find((q) => q.id === 'a1q9');
  if (!a1q9 || a1q9.passive_points !== 1 || a1q9.gem_rewards.length !== 0) {
    throw new Error('Anchor failed: a1q9 "The Way Forward" should grant 1 passive point and no gem - not writing.');
  }
  const totalPoints = quests.reduce((sum, q) => sum + q.passive_points, 0);
  if (totalPoints !== 22) {
    throw new Error(`Anchor failed: ${totalPoints} passive points from quests (expected 22 Book-of-Skill grants) - not writing.`);
  }
  const withNpc = quests.filter((q) => q.npc).length;
  console.log(`[regen] ${quests.length} campaign quests: ${withGems} with gems, ${totalPoints} passive points (${withNpc}/${quests.length} with a reward NPC).`);
}

function validateTrialAnchors(trials: TrialInfo[]): void {
  const lowerPrisonAct1 = trials.find((t) => t.area_name === 'The Lower Prison' && t.act === 1);
  if (!lowerPrisonAct1) {
    throw new Error('Anchor failed: no Act-1 "The Lower Prison" trial - not writing.');
  }
  // 6 Normal Labyrinth trials live in acts 1-3. The campaign has 12 in total (acts 1-10).
  const normalLab = trials.filter((t) => t.act <= 3).length;
  if (normalLab !== 6 || trials.length !== 12) {
    throw new Error(`Anchor failed: expected 6 acts-1-3 trials and 12 total, got ${normalLab} and ${trials.length} - not writing.`);
  }
  console.log(`[regen] ${trials.length} campaign lab trials (${normalLab} in acts 1-3).`);
}

function main(): void {
  if (!process.argv.includes('--no-fetch')) runFetch();

  const patch = readPatch();
  const generatedAt = new Date().toISOString();

  const areas = transformAreas(loadTable<RawWorldArea>('WorldAreas'));
  const quests = transformQuests(
    loadTable<RawQuest>('Quest'),
    loadTable<RawQuestReward>('QuestRewards'),
    loadTable<RawQuestRewardOffer>('QuestRewardOffers'),
    loadTable<RawCharacter>('Characters'),
    loadTable<RawBaseItemType>('BaseItemTypes'),
    loadTable<RawNPCTalk>('NPCTalk'),
    loadTable<RawNPC>('NPCs'),
  );
  const trials = transformTrials(
    loadTable<RawLabyrinthTrial>('LabyrinthTrials'),
    loadTable<RawWorldArea>('WorldAreas'),
  );

  if (areas.length === 0) throw new Error('No campaign areas produced - aborting.');
  if (quests.length === 0) throw new Error('No campaign quests produced - aborting.');
  if (trials.length === 0) throw new Error('No campaign trials produced - aborting.');
  validateAreaAnchors(areas);
  validateQuestAnchors(quests);
  validateTrialAnchors(trials);

  const files: Array<{ name: string; text: string }> = [
    {
      name: 'areas.json',
      text: writeJson(resolve(dataPackDir, 'areas.json'), {
        schema_version: AREAS_SCHEMA,
        patch,
        generated_at: generatedAt,
        source: SOURCE,
        areas,
      }),
    },
    {
      name: 'quests.json',
      text: writeJson(resolve(dataPackDir, 'quests.json'), {
        schema_version: QUESTS_SCHEMA,
        patch,
        generated_at: generatedAt,
        source: SOURCE,
        quests,
      }),
    },
    {
      name: 'trials.json',
      text: writeJson(resolve(dataPackDir, 'trials.json'), {
        schema_version: TRIALS_SCHEMA,
        patch,
        generated_at: generatedAt,
        source: SOURCE,
        trials,
      }),
    },
  ];

  writeJson(resolve(dataPackDir, 'manifest.json'), {
    patch,
    generated_at: generatedAt,
    files: files.map((f) => ({ name: f.name, sha256: sha256(f.text) })),
    note: 'Generated from PoE game files via tools/data-extractor (npm run regen).',
  });

  console.log(`[regen] wrote ${areas.length} areas, ${quests.length} quests, ${trials.length} trials (patch ${patch}).`);
  for (const f of files) console.log(`[regen] ${f.name} sha256 ${sha256(f.text)}`);
}

main();
