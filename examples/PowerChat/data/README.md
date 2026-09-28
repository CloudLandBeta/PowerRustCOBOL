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
| `prompt-versions.idx` | Every version of each topic's system prompt, and of the main prompt (topic id `*MAIN`) |
| `prompts.idx` | The versions before 1.70.332, when a text held 1,000 characters: copied into `prompt-versions.idx` the first time the Prompt screen opens, then only kept |
| `folders.idx` | Each topic's document folders, so an empty one still shows |
| `templates.idx` | The report templates the chat offers — name, what it suits, its HTML page: the shipped ones (from `samples/report-templates.txt`) and every one changed or described in a conversation |

Set the `POWERCHAT_DATA` environment variable to keep them somewhere else.
Keys are not here: they are in the application's key store
(`settings/model-keys.dat`).
