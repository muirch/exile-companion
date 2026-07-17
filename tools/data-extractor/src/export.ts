// Orchestrates a full data-pack regeneration:
//   1. run pathofexile-dat (fetch WorldAreas from GGG's patch CDN -> tables/English/WorldAreas.json)
//   2. reshape rows -> poebo's flat schema (see ./transform/areas.ts)
//   3. write ../../data-pack/areas.json + manifest.json (with sha256), validated against anchors
//
// Run with `npm run regen`. Pass `--no-fetch` to skip the download and reuse the cached table
// (useful in CI once the bundle cache is warm, or for offline transform iteration).

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { transformAreas, type RawWorldArea, type ZoneInfo } from './transform/areas.js';

const SCHEMA_VERSION = 1;

const here = dirname(fileURLToPath(import.meta.url)); // tools/data-extractor/src
const extractorDir = resolve(here, '..'); // tools/data-extractor
const repoRoot = resolve(extractorDir, '..', '..'); // repo root
const dataPackDir = resolve(repoRoot, 'data-pack');
const worldAreasPath = resolve(extractorDir, 'tables/English/WorldAreas.json');
const cliPath = resolve(extractorDir, 'node_modules/pathofexile-dat/dist/cli/run.js');

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
    // The cold CDN fetch occasionally exits non-zero after writing valid output; tolerate it here
    // and let the output validation below be the real gate.
    console.warn('[regen] pathofexile-dat exited non-zero - validating its output before failing.');
  }
}

function loadWorldAreas(): RawWorldArea[] {
  if (!existsSync(worldAreasPath)) {
    throw new Error(`WorldAreas.json missing at ${worldAreasPath}; the fetch step produced no output.`);
  }
  return JSON.parse(readFileSync(worldAreasPath, 'utf8')) as RawWorldArea[];
}

function writeJson(path: string, value: unknown): string {
  const text = JSON.stringify(value, null, 2) + '\n';
  writeFileSync(path, text);
  return text;
}

function validateAnchors(areas: ZoneInfo[]): void {
  // NB: zone names are NOT unique across acts - Acts 6-10 revisit Acts 1-5 with the same names
  // (e.g. "Lioneye's Watch" exists in both Act 1 and Act 6). So we validate that at least one
  // candidate per name matches the expected Act-1 facts, not that a name maps 1:1.
  const strandActs = areas.filter((a) => a.name === 'The Twilight Strand').map((a) => a.act);
  const watches = areas.filter((a) => a.name === "Lioneye's Watch");
  const watchAct1 = watches.find((a) => a.act === 1);
  console.log(`[regen] "The Twilight Strand" appears in acts: [${strandActs.sort().join(', ')}]`);
  console.log(`[regen] "Lioneye's Watch" appears in acts: [${watches.map((a) => a.act).sort().join(', ')}]`);
  if (!strandActs.includes(1)) {
    throw new Error('Anchor failed: no Act-1 "The Twilight Strand" - extraction looks wrong; not writing.');
  }
  if (!watchAct1 || !watchAct1.is_town || !watchAct1.has_waypoint) {
    throw new Error('Anchor failed: Act-1 "Lioneye\'s Watch" is not a waypoint town - not writing.');
  }
}

function main(): void {
  if (!process.argv.includes('--no-fetch')) runFetch();

  const patch = readPatch();
  const areas = transformAreas(loadWorldAreas());
  if (areas.length === 0) {
    throw new Error('No campaign areas produced - aborting to avoid writing an empty pack.');
  }
  validateAnchors(areas);

  const generatedAt = new Date().toISOString();
  const areasText = writeJson(resolve(dataPackDir, 'areas.json'), {
    schema_version: SCHEMA_VERSION,
    patch,
    generated_at: generatedAt,
    source: 'PoE game files (GGG patch CDN) via pathofexile-dat',
    areas,
  });

  const sha256 = createHash('sha256').update(areasText).digest('hex');
  writeJson(resolve(dataPackDir, 'manifest.json'), {
    schema_version: SCHEMA_VERSION,
    patch,
    generated_at: generatedAt,
    files: [{ name: 'areas.json', sha256 }],
    note: 'Generated from PoE game files via tools/data-extractor (npm run regen).',
  });

  console.log(`[regen] wrote ${areas.length} campaign areas -> data-pack/areas.json (patch ${patch})`);
  console.log(`[regen] areas.json sha256 ${sha256}`);
}

main();
