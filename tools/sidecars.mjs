// Builds the programs the installer ships next to the app (Tauri externalBin) and puts them in
// app/desktop/binaries under the names Tauri wants, <name>-<target triple>[.exe].
//   node tools/sidecars.mjs [--release] [--target <triple>] [--placeholder]
// --placeholder writes empty files instead, for clippy on the app where nothing gets installed.
// Don't use it where the same target dir also runs these programs: Tauri copies the files back
// over cargo's own builds of them.
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const BINS = ['exile-pob-engine', 'exile-companion-mcp'];
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const release = args.includes('--release');
const placeholder = args.includes('--placeholder');
const targetArg = args.includes('--target') ? args[args.indexOf('--target') + 1] : undefined;

const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (\S+)/m)[1];
const triple = targetArg ?? process.env.TAURI_ENV_TARGET_TRIPLE ?? host;
const exe = triple.includes('windows') ? '.exe' : '';
const out = join(root, 'app', 'desktop', 'binaries');
mkdirSync(out, { recursive: true });

if (placeholder) {
  for (const bin of BINS) {
    const file = join(out, `${bin}-${triple}${exe}`);
    if (!existsSync(file)) writeFileSync(file, '');
  }
  process.exit(0);
}

// Building for the host without --target shares target/<profile> with the rest of the workspace.
const cross = triple !== host;
const cargoArgs = ['build', ...(release ? ['--release'] : []), ...(cross ? ['--target', triple] : [])];
for (const bin of BINS) cargoArgs.push('-p', bin);
execFileSync('cargo', cargoArgs, { cwd: root, stdio: 'inherit' });

const built = join(root, 'target', ...(cross ? [triple] : []), release ? 'release' : 'debug');
for (const bin of BINS) {
  copyFileSync(join(built, `${bin}${exe}`), join(out, `${bin}-${triple}${exe}`));
}
