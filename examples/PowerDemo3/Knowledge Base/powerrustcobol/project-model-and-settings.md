<!-- powerrustcobol-kit: 1.80.100 -->
# PowerRustCOBOL Project Model and Settings

## What a project is on disk

A project is one **TOML manifest** plus the files it lists. The manifest is
named after the project — `PowerDemo3.project.toml` — **not** a fixed file name,
so tooling finds it by scanning a directory and its ancestors for
`*.project.toml`. A legacy `cobolt.toml` is still accepted and wins when both
are present.

A `.cfrm` written into `forms/` is **not** in the project until it is listed
under `[files] forms`. Writing a file to disk is half the job; the tree shows
only what the manifest tracks.

The standard sub-folders are `forms/`, `indexed/`, `src/`, `generated/`,
`Assets/`, `Knowledge Base/`, `data/`, `bin/` and `dist/`. Missing ones are
back-filled when an older project is opened.

## The manifest sections

```toml
[project]
name    = "MyApp"
version = "1.0.0"
main    = "src/main.cbl"

[files]
sources = ["src/main.cbl", "src/helpers.cbl"]
forms   = ["forms/main-form.cfrm", "forms/login.cfrm"]
assets  = ["Assets/logo.png"]

[runtime]
fixed_format = false
```

Beyond these there are `[ide]` (per-project IDE appearance), `[forms]` (form
defaults, window effects, the main-form designation), `[ai]` (model profiles and
agent settings) and the External Crates pins. Every section is optional: a
missing section takes its defaults, which is what keeps older projects loading
unchanged.

## `[files]` — the five tracked lists

- **`sources`** — hand-written COBOL (`.cbl`).
- **`forms`** — form definitions (`.cfrm`).
- **`generated`** — RAD output, one `.cbl` per form. Moved out of `sources`
  when a form generates it.
- **`indexed`** — indexed-file definitions (`.cidx`).
- **`assets`** — images and other resources.
- **`documentation`** — the project's own Knowledge Base documents.

## The project tree's seven categories

The IDE owns these top-level nodes; developers add entries *within* a category,
never a category of their own. In display order, with the folder each one owns:

| Category | Folder | Notes |
|---|---|---|
| Forms | `forms/` | `.cfrm` |
| Indexed Files | `indexed/` | `.cidx` |
| Common Code | `src/` | hand-written COBOL only |
| Generated Code | `generated/` | **read-only**, populated by the designer |
| External Crates | vendor folder | one node per registered crate pin |
| Assets | `Assets/` | |
| Documentation | `Knowledge Base/` | the project's own KB |

**Generated Code cannot be added to** and its files cannot be edited: they are
opened read-only and drawn in blue, and they are regenerated from the form. A
change belongs in the form or in Common Code, never in `generated/`.

## `[project]` — metadata and build settings

`name`, `version`, `main` (the entry program), `copyright`, `license_model` and
`license_text`, `destination_folder` (where a build installs the deliverable —
`dist/` when unset), `debug_compilation`, and `built_with_version`, which
records the PowerRustCOBOL that last **fully** built the project. Opening a
project last fully built by an older PowerRustCOBOL — or never fully built —
makes the next **Build** a full one.

## `[forms]` — form defaults and the main form

- **`theme`** — the project's default **form** theme id. Empty means Liquid
  Glass. A form's own `Theme` overrides it.
- **Window effects** — `entrance-effect`, `entrance-ms`, `entrance-easing`,
  `exit-effect`, `exit-ms`, `exit-easing`, `entrance-on-restore`. Effects are a
  **project-level** choice: a form only decides *whether* to play them, through
  its `WindowEffects` property. Absent settings mean no effect, so a project
  written before effects existed is unchanged.
- **`main-form`** and its seal — exactly one form per project is the main form.
  Only the main form starts an application: `rcrun` and a built binary open the
  form the project designates, never one a caller names. The designation is
  recorded in both the manifest and the `.cfrm` itself.

## `[ide]` — per-project IDE appearance

The look travels with the project, not with the developer: `theme` (the IDE
colour theme id), `background_image`, `background_opacity` (0-100),
`project_icon`, and `hide_ai_setup_prompt`.

These theme the **IDE chrome**. They are a different setting from `[forms]
theme`, which themes the developer's designed forms. Do not confuse the two: no
`[ide]` setting changes how a form looks at run time.

Machine-local developer aids are deliberately **not** here — the IDE debug
switches live in the IDE's own settings (Help → Debug Settings), so they are not
project data and are not shared with a colleague who opens the project.

## `[runtime]` — source format

`fixed_format` selects fixed-form COBOL (columns 7-72) rather than free form.
Free form is the default for new projects.

## Where things that are NOT project files live

- **The System Knowledge Base** — platform documentation, regenerated from the
  running binary at machine level in `~/PowerRustCOBOL/Knowledge Base/`, with
  its index in `~/PowerRustCOBOL/data/`. It is never copied into a project, and
  a build never publishes it into one.
- **The Project Knowledge Base** — the developer's own `<project>/Knowledge
  Base/`, indexed under `<project>/data/`. Empty is a normal state.
- **API keys** — never in the manifest. They resolve from the machine-local
  store by model-profile id, so a manifest can be committed and shared safely.
