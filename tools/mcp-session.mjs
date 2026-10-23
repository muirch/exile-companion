// Keeps exile-companion-mcp running and calls its tools from the command line, the way an agent
// would over MCP. For trying the assistant by hand or from a test agent.
//   node tools/mcp-session.mjs start <name> [server args, e.g. --pob <dir> --build <file.xml>]
//   node tools/mcp-session.mjs info <name>                      instructions and tools
//   node tools/mcp-session.mjs call <name> <tool> ['<json args>']
//   node tools/mcp-session.mjs stop <name>
import { spawn } from 'node:child_process';
import { existsSync, unlinkSync } from 'node:fs';
import { createConnection, createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const [cmd, name, ...rest] = process.argv.slice(2);
if (!cmd || !name) {
  console.error('usage: mcp-session.mjs start|info|call|stop <name> ...');
  process.exit(2);
}
const sock =
  process.platform === 'win32' ? `\\\\.\\pipe\\exile-companion-mcp-${name}` : join(tmpdir(), `exile-companion-mcp-${name}.sock`);

function ask(request) {
  return new Promise((resolve, reject) => {
    const c = createConnection(sock, () => c.write(JSON.stringify(request) + '\n'));
    let data = '';
    c.on('data', (d) => (data += d));
    c.on('end', () => resolve(data));
    c.on('error', reject);
  });
}

if (cmd === 'start') {
  const child = spawn(process.execPath, [fileURLToPath(import.meta.url), 'serve', name, ...rest], {
    detached: true,
    stdio: 'ignore',
  });
  child.unref();
  // The server answers initialize at once. The engine starts with the first build tool call.
  for (let i = 0; i < 100 && !existsSync(sock) && process.platform !== 'win32'; i++) {
    await new Promise((r) => setTimeout(r, 100));
  }
  console.log(`started ${name}`);
} else if (cmd === 'serve') {
  const server = join(root, 'target', 'debug', `exile-companion-mcp${exe}`);
  const engine = join(root, 'target', 'debug', `exile-pob-engine${exe}`);
  const mcp = spawn(server, ['--engine', engine, ...rest], { stdio: ['pipe', 'pipe', 'ignore'] });
  const lines = createInterface({ input: mcp.stdout });
  const waiting = new Map();
  lines.on('line', (line) => {
    const msg = JSON.parse(line);
    waiting.get(msg.id)?.(msg);
    waiting.delete(msg.id);
  });
  let id = 0;
  const rpc = (method, params) =>
    new Promise((resolve) => {
      id += 1;
      waiting.set(id, resolve);
      mcp.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
    });
  const init = await rpc('initialize', {
    protocolVersion: '2025-06-18',
    capabilities: {},
    clientInfo: { name: 'mcp-session', version: '0' },
  });
  mcp.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
  const tools = await rpc('tools/list', {});
  if (existsSync(sock)) unlinkSync(sock);
  // One request at a time, like an agent waiting for each tool result.
  let queue = Promise.resolve();
  const srv = createServer((c) => {
    createInterface({ input: c }).once('line', (line) => {
      queue = queue.then(async () => {
        const req = JSON.parse(line);
        if (req.stop) {
          c.end('stopped\n');
          mcp.kill();
          srv.close();
          process.exit(0);
        }
        if (req.info) {
          const out = { instructions: init.result.instructions, tools: tools.result.tools };
          c.end(JSON.stringify(out, null, 1) + '\n');
          return;
        }
        const r = await rpc('tools/call', { name: req.tool, arguments: req.args ?? {} });
        const text = (r.result?.content ?? []).map((b) => b.text).join('\n');
        c.end((r.result?.isError ? 'TOOL ERROR: ' : '') + (text || JSON.stringify(r.error)) + '\n');
      });
    });
  });
  srv.listen(sock);
} else if (cmd === 'info') {
  process.stdout.write(await ask({ info: true }));
} else if (cmd === 'call') {
  const [tool, json] = rest;
  process.stdout.write(await ask({ tool, args: json ? JSON.parse(json) : {} }));
} else if (cmd === 'stop') {
  process.stdout.write(await ask({ stop: true }));
}
