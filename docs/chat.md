# Ask the assistant in the game

Press Alt+A, or hold Tilde and pick Ask the assistant, and a chat window opens over the game. The assistant answers about the build you imported in Exile Companion. It measures with your own Path of Building, knows where you are in the campaign from the game's `Client.txt`, and reads the [Path of Exile Wiki](https://www.poewiki.net) for mechanics.

## What's next, without a model

Before your first question the window shows what to change next to get closer to the stage of the guide you follow. It lists support gems, passives with the points they take, gear slots and ascendancy notables, ranked by what Path of Building says they give. When your level has reached the guide's next stage, it says so. This list needs no AI model and no key, so it costs nothing and works right away. It needs a guide to compare with, which the assistant saves when you ask it to follow a guide's Path of Building link.

## Pick a provider

Everything else is answered by an AI model, which you pay for with your own key. Open the settings and go to the Assistant group.

- Anthropic. Make a key at console.anthropic.com. Pick Haiku 5.5, Sonnet 5.5 (the default) or Opus 5.5.
- OpenRouter. Make a key at openrouter.ai and type the id of any model that supports tools, as openrouter.ai/models shows it under the model's name.
- Other server. Any server with OpenAI's chat completions, such as DeepSeek's API, LM Studio or Ollama. Give its address, for example `http://127.0.0.1:11434/v1`, and the model's name. Local servers usually need no key.

Keys stay in Windows Credential Manager, or the Secret Service keyring on Linux, one per provider. Exile Companion sends a key only to its provider.

Effort sets how long the model thinks before it answers. Low is the default, because thinking is most of what an answer costs.

## What it costs

These are estimates for a typical question that takes one or two tool calls. Every answer in the chat shows the model that gave it and what it cost, when the provider says.

| Model | One question | 20 questions in an evening |
|---|---|---|
| Haiku 5.5 | about $0.001 | about $0.02 to $0.04 |
| DeepSeek V4.1 Flash on OpenRouter | about $0.001 | about $0.02 to $0.04 |
| Sonnet 5.5 | about $0.015 to $0.035 | about $0.30 to $0.75 |

The first question after an hour without one costs a little more, since the provider caches the assistant's instructions for an hour.

## Local models

A model on your own PC costs nothing per question. With Ollama, Exile Companion loads the model when the chat opens and gives the video memory back to the game a minute after it closes. Give Ollama a context of at least 32k tokens (`OLLAMA_CONTEXT_LENGTH=32768`), or it cuts off the assistant's instructions without saying so. In LM Studio set the context length when you load the model.

I tried a 7.5B model on an RTX 5080. It answered in 3 to 9 seconds, but it got mechanics wrong, mangled pasted items and sometimes gave no answer. I don't recommend small local models for advice yet.

## Using the chat

Enter asks, Shift+Enter starts a new line. Esc closes the window and gives the keyboard back to the game, and an answer that is still coming goes on. The first line of an answer is the point, and Details opens the rest. Stop ends an answer, New chat starts over. A conversation lasts until New chat or until you quit the app, and switching the provider starts a new one.

## What leaves your PC

Your question goes to the provider you picked, together with the assistant's instructions and what its tools read for that question. That is your build's gear, gems, passives and stats, where you are in the campaign, the guide you follow and wiki text. Nothing goes anywhere while you don't ask.

## When something goes wrong

The chat says what happened and what to do. The usual cases are a key the provider refused (check it in the settings), an account out of credit, a model name the provider doesn't know, and a server that doesn't answer (check its address and that it runs). "The model gave no answer" means the model stopped without a word. Ask again, or pick a stronger model.

You can also ask through an AI agent you already use, such as Claude Desktop or Cursor, on its own subscription. See [mcp.md](mcp.md).
