# Agent rules for Exile Companion

The project language is English: code, comments, docs, commits, issues.

## Project essentials

- Exile Companion is a passive Path of Exile tool. It must never inject into the game, read its memory, hook rendering, or automate play. Inputs are `Client.txt`, user hotkeys (global shortcuts and a low-level keyboard hook that only watches and passes every key on), the clipboard, local Path of Building files and build links the user pastes (pobb.in, poe.ninja and the other sites PoB imports from). The assistant's chat sends a question and what its tools read to the AI provider the player picked, with the player's own key, and only when they ask.
- Sending input to the game is allowed only for an explicit, opt-in user key press that performs exactly one game action, and only while the game is in front (GGG's one-action-per-keypress rule). Today that is the relog and hideout keys (`/exit` and `/hideout` in chat) and the gem search key (one search typed into the open vendor window). Anything that cuts the game's network connection or chains actions is out.
- `crates/core` (the `exile-core` crate) and `crates/assistant` (the chat's model clients and tool loop) stay free of Tauri and are covered by tests. Logic goes there, and `app/desktop` is a thin shell. The UI lives in `app/ui`.
- Logic changes come with tests: Rust logic in `exile-core` and `exile-assistant` with `cargo test`, UI behaviour (labels, key handling, what the overlay shows in each state) with Vitest next to the component (`*.test.tsx`).
- Before committing: `cargo fmt --all`, `cargo clippy --workspace --exclude exile-companion --all-targets -- -D warnings`, `cargo test --workspace --exclude exile-companion`, `pnpm lint`, `pnpm test`, `pnpm build`. The Tauri app is checked for Windows locally with `node tools/sidecars.mjs --placeholder --target x86_64-pc-windows-msvc` once, then `RC=$PWD/tools/fake-llvm-rc cargo clippy -p exile-companion --target x86_64-pc-windows-msvc --all-targets -- -D warnings`. Installers come from the manual Release workflow.
- Data under `crates/data/data/` is generated. Regenerate it with `tools/data-extractor` and don't edit it by hand. Third-party data (routes from exile-leveling) keeps its license and attribution.
- UI and visual work goes through the impeccable skill (`.claude/skills/impeccable`). `PRODUCT.md` holds the product facts and `DESIGN.md` the design system. Keep both current when the UI changes.
- Shared controls (buttons, toggles, choice rows, select, slider, settings sections and rows, notes) live in `app/ui/src/kit`. Use them instead of raw `<button>`, `<select>` or `<input>`, and add a new control there when two places need it.
- Code goes to main through pull requests: one per feature, and one per round of fixes after an in-game test, with a commit per change inside. Rebase-merge after CI passes so those commits stay. Changes that only touch docs (\*.md) can be committed to main directly.
- After any UI change, run `tools/ui-shots.sh` (needs `pnpm dev --port 5199`) and give the user the paths to `.ui-shots/overview.png` and `.ui-shots/settings.png`.

## Writing style: code comments and prose

These rules are strict and apply to every text you write that someone else will read: code comments, doc comments, README and docs, commit messages, PR titles and descriptions, PR review comments, issues and issue comments, release notes, chat messages posted to a tracker. Write like an experienced engineer who is a bit short on time: plain, specific, no filler. If a reviewer could tell the text came from an AI, rewrite it before sending.

### Writing as the user

Commits, PRs, issues, review comments and release notes go out under the user's name, so write them in the user's voice: first person ("I moved...", "I couldn't reproduce..."), the way a developer writes to teammates.

- Lead with what changed and why. Skip "This PR...", "Summary", "Overview" and "In this change we...".
- Keep the body as long as the change needs: a small fix gets one or two sentences, and a larger change gets a few short paragraphs. Use lists only for real lists, like steps to reproduce or a set of files.
- Mention a problem or open question plainly ("Not tested on Proton yet."), not as a caveats section.
- No sign-offs, no "Let me know if...", no "Happy to adjust". No emoji, no checkmark lists, no generated-by footer unless the user asks for one.
- Use the language the project uses for the text (English for Exile Companion). Don't translate the user's wording into corporate English.
- Say only what the reader needs to review or use the change. Leave out things you didn't change, local-only files and folders, and reasons nobody asked for.
- Short sentences and everyday words; contractions are fine. Read it back: if it sounds like a report or a press release, cut it in half.

### Comments

- Comment rarely. The default is no comment. Add one only for something a reader can't get from the code: why it is done this way, a trap, a required order, a workaround (link the issue), a unit or invariant that isn't obvious.
- Never restate the code, the name or the signature. `// create the client`, `/// Returns the level.` and `// loop over zones` are noise. If the code needs a comment to say what it does, rename or simplify it first.
- Doc comments on public items: one or two lines on purpose or contract, and only if the name doesn't already say it. Don't list parameters that just repeat their types. Don't write module-level essays; architecture belongs in docs.
- No section banners, divider lines, `Step 1:` numbering, or headers inside function bodies.
- No history in comments ("now uses", "changed from", "new:", "fixed bug where"). That is what git is for.
- No marketing or reassurance: robust, seamless, comprehensive, elegant, powerful, crucial, ensure, leverage, simply, just, "note that", "it's worth noting", "this ensures that".
- Short and concrete. A sentence fragment is fine. Match the tone, casing and density of the comments around it.
- `TODO:` only with a concrete next action and, if there is one, the blocker: `TODO: drop once PoB #9748 is merged`.
- When you change code, fix or delete the comments it made wrong. Don't add comments to code you didn't touch.

Bad:

```rust
// Parse the line and return the event.
// This ensures robust handling of all log formats.
let event = parse(line)?;
```

Good (only if it's true and not obvious):

```rust
// Acts 6-10 reuse zone names from acts 1-5, so the name alone can't give us the act.
let candidates = zones.zones_named(name);
```

### Prose (README, docs, commits, PRs)

- Say the thing. Use "is" and "has", not "serves as", "stands as", "boasts", "features".
- Avoid the usual AI vocabulary: delve, robust, seamless, leverage, comprehensive, crucial, pivotal, showcase, underscore, testament, streamline, empower, intricate, meticulous, vibrant, landscape, realm, navigate (figurative), "in today's...".
- No "not just X, but Y" or "it's not X, it's Y" setups. Don't group things in threes by reflex. Don't end sections with a summary line ("Overall, ...", "In short, ...").
- No vague authorities ("experts say", "many users"). Name the source or drop the claim.
- Formatting: sentence-case headings, no emoji, no bold scattered through sentences, no bullet lists where every item starts with a bold label, no tables holding sentences. Use lists for real lists, not to chop up an explanation.
- No chat tone in files: no "I hope this helps", "Let me know", "Great question", and no notes about how the text was produced.
- Commit messages: Conventional Commits. The subject says what changed, the body (only if needed) says why. No adjectives selling the change.

### Sources

Adapted, not copied verbatim:

- Google Engineering Practices, "What to look for in a code review", section Comments: https://google.github.io/eng-practices/review/reviewer/looking-for.html
- Linux kernel coding style, section 8 "Commenting": https://www.kernel.org/doc/html/latest/process/coding-style.html
- Wikipedia, "Signs of AI writing" (WikiProject AI Cleanup): https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing
