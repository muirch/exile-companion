// Vendors exile-leveling's hand-written leveling route into data-pack/routes/exile-leveling/.
// Run with `npm run routes`. Bump COMMIT to pick up a newer route, then review the diff.

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = 'HeartofPhos/exile-leveling';
const COMMIT = 'b7b2dd0ed62ae25cf55c74085fa64a1f4d7cf4ba';
const ACTS = 10;

const here = dirname(fileURLToPath(import.meta.url));
const outDir = resolve(here, '..', '..', '..', 'data-pack', 'routes', 'exile-leveling');

async function fetchText(path: string): Promise<string> {
  const url = `https://raw.githubusercontent.com/${REPO}/${COMMIT}/${path}`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  return res.text();
}

mkdirSync(outDir, { recursive: true });

for (let act = 1; act <= ACTS; act++) {
  const name = `act-${act}.txt`;
  writeFileSync(resolve(outDir, name), await fetchText(`common/data/routes/${name}`));
}
writeFileSync(resolve(outDir, 'LICENSE'), await fetchText('LICENSE'));
writeFileSync(
  resolve(outDir, 'SOURCE.md'),
  `Leveling route from https://github.com/${REPO} at commit ${COMMIT}, MIT licensed (see LICENSE).\n` +
    'Regenerate with `npm run routes` in tools/data-extractor. Do not edit these files by hand.\n',
);
console.log(`[routes] wrote ${ACTS} acts from ${REPO}@${COMMIT.slice(0, 7)}`);
