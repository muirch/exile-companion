# Ask Claude about your build

The build assistant starts as an MCP server. Claude Desktop or Claude Code runs it on your machine, and Claude asks your own Path of Building about the build you imported in Exile Companion, plus the [Path of Exile Wiki](https://www.poewiki.net) for mechanics and crafting. It runs on your Claude plan, no API key needed.

The installer doesn't include it yet. For now it's built from source, see below.

## What it needs

- Exile Companion with a build imported (settings, Path of Building, Paste code or link) and a PoB folder it found or you picked.
- `exile-companion-mcp` and `exile-pob-engine` in the same folder.

## Claude Desktop

Open Settings, Developer, Edit Config, and add the server to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "exile-companion": {
      "command": "C:\\path\\to\\exile-companion-mcp.exe"
    }
  }
}
```

Restart Claude Desktop. Then ask things like "Why is my damage low?", "Is this helmet an upgrade?" with the item pasted from the game, or "What should I take with my next three passive points?".

## Claude Code

```bash
claude mcp add exile-companion -- /path/to/exile-companion-mcp
```

## Other builds

Paste a PoB code or a build link into the chat and ask Claude to load it. That build is used until the conversation ends, the one in Exile Companion stays as it is.

The server can also be pointed at files directly:

```bash
exile-companion-mcp --pob <Path of Building folder> --build <build.xml> --engine <exile-pob-engine>
```

## Building it

```bash
cargo build --release -p exile-companion-mcp -p exile-pob-engine
```

Both binaries land in `target/release`.

## What Claude can do with it

Read the build as PoB's sidebar shows it, with its gear, gems, tree and Configuration. Check defences and max hits. Measure what each gem and notable is worth, compare a copied item against your gear, try changes together (nodes, items, gems, config options, mod lines, level) without touching your build, export the build as a PoB code. Search and read the wiki and its item and mod tables.

Numbers are Path of Building's own, with the build's Configuration. If the build assumes a boss, buffs or custom modifiers, so do the numbers.
