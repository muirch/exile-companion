# Ask an AI agent about your build

The build assistant is an MCP server. Your AI agent (Claude Desktop, Claude Code, Cursor, Codex and others) starts it on your PC, then asks your own Path of Building about the build you imported in Exile Companion, plus the [Path of Exile Wiki](https://www.poewiki.net) for mechanics and crafting. It runs on your agent's plan, Exile Companion needs no API key.

## What it needs

Exile Companion with a build imported (settings, Path of Building, Paste code or link) and a PoB folder it found or you picked. The installer puts the assistant's two programs, `exile-companion-mcp` and `exile-pob-engine`, next to the app.

## Connecting your agent

Open Exile Companion's settings and find Assistant. It lists the agents it knows (Claude Desktop, Claude Code, Cursor, VS Code with Copilot, Codex) with a Connect button for each one installed. Connect adds Exile Companion's server to that agent's config, keeps the rest of the file, and saves a copy of the old file next to it. Restart the agent after that.

For any other agent, Copy config puts this on the clipboard, in the format most agents read:

```json
{
  "mcpServers": {
    "exile-companion": {
      "command": "C:\\Users\\you\\AppData\\Local\\Exile Companion\\exile-companion-mcp.exe",
      "args": []
    }
  }
}
```

The path is where the installer put Exile Companion. Agents with their own add command take the same path, for example:

```bash
claude mcp add --transport stdio --scope user exile-companion -- "C:\Users\you\AppData\Local\Exile Companion\exile-companion-mcp.exe"
codex mcp add exile-companion -- "C:\Users\you\AppData\Local\Exile Companion\exile-companion-mcp.exe"
```

Web chats only connect to servers on the internet, so they can't use this one. It runs your Path of Building on your PC.

Then ask things like "Why is my damage low?", "Is this helmet an upgrade?" with the item pasted from the game, or "What should I take with my next three passive points?".

## Other builds

Paste a PoB code or a build link into the chat and ask the agent to load it. That build is used until the conversation ends, the one in Exile Companion stays as it is.

The server can also be pointed at files directly:

```bash
exile-companion-mcp --pob <Path of Building folder> --build <build.xml> --engine <exile-pob-engine>
```

## What the agent can do with it

Read the build as PoB's sidebar shows it, with its gear, gems, tree and Configuration. Check defences and max hits. Measure what each gem and notable is worth, compare a copied item against your gear, try changes together (nodes, items, gems, config options, mod lines, level) without touching your build, export the build as a PoB code. Search and read the wiki and its item and mod tables.

Numbers are Path of Building's own, with the build's Configuration. If the build assumes a boss, buffs or custom modifiers, so do the numbers.

## Building it

```bash
node tools/sidecars.mjs --release
```

builds both programs and puts them in `app/desktop/binaries`, where the installer build picks them up. `pnpm tauri build` runs it on its own.
