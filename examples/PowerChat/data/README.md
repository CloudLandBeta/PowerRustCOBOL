# PowerChat data

PowerChat keeps its own indexed files here (`STORAGE MODE IS DISK`), creating
each one the first time it is needed:

| File | Holds |
|---|---|
| `settings.idx` | The Knowledge Base folder, the open topic, the model the chat uses |
| `topics.idx` | Each topic: name and system prompt |
| `convs.idx` | Each conversation: its topic, title, and token counts |
| `turns.idx` | Every question and answer, in order |
| `models.idx` | The model list (names, APIs, endpoints, models, tools, rank — never keys) |
| `topic-files.idx` | The indexed files each topic registers, by path |
| `prompts.idx` | Every version of each topic's system prompt |

Set the `POWERCHAT_DATA` environment variable to keep them somewhere else.
Keys are not here: they are in the application's key store
(`settings/model-keys.dat`).
