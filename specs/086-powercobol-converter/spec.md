<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — PowerCOBOL project converter (PowerCOBOL 5.x and later)

- **Status:** draft
- **Folder:** specs/086-powercobol-converter/
- **Author:** Claude (for Emerson Lopes)   **Date:** 2026-10-03

## 1. Overview

A developer with a working Fujitsu PowerCOBOL application has, today, no way
into PowerRustCOBOL AI except rebuilding every form by hand. The release
candidate post (2026-08-24) told the community a converter was in
development. No spec, crate or prototype exists yet: the 2026-08-23 RC
session and spec 081 §2 both confirm it.

This feature adds the converter: an IDE menu action that reads a PowerCOBOL
5.x-or-later project (`.ppj`) and writes a new PowerRustCOBOL project. The
new project contains:
- the forms, controls and properties;
- the event scripts and form procedures, translated to RustCOBOL;
- the images, moved into the project's assets;
- an `.cidx` definition for every indexed file the source declares, plus a
  generated COBOL program that loads that file's legacy data.

A conversion report lists everything that could not be carried over exactly.
The aim is a project that opens, passes **Check**, and runs. Where something
cannot be carried over, the report says so: the converter never silently
drops anything.

### What the source looks like (measured on the operator's samples)

The operator's sample application (`~/Documents/Legacy/CBL`) has 16 `.ppj`
projects, 50 forms and 723 script streams. That is the evidence base for this
spec. What it shows:

- **A `.ppj` is an OLE compound document.** Its storages nest as
  project → module (e.g. `M-FAMILIAS`) → form (e.g. `F-FAMILIAS`).
- **Each form has a script storage (`$Staff…`).** It holds one stream per
  script, named for what the script is:
  - `<CONTROL>-<Event>` streams (`BTSALIR-Click`, `CAMPO1-Return`,
    `F-FAMILIAS-Opened`);
  - the form's own procedures (`PARASALIR`, `VACIAR`, `GRABAR`);
  - the data sections (`WORKING-STORAGE`, `FILE-CONTROL`, `FILE`,
    `REPOSITORY`).
- **Scripts are COBOL text in Windows-1252.** An event script is a small
  nested program (`ENVIRONMENT DIVISION … PROCEDURE DIVISION`). Scripts use:
  - `POW-SELF` and `POW-SUPER`;
  - `INVOKE ctl "Method" USING …`;
  - `"Prop" OF ctl` property references and `POW-…` constants;
  - `#INCLUDE "C:\…\X.cop"` with absolute Windows paths;
  - `GLOBAL` and `EXTERNAL` data items;
  - **embedded SQL** (`EXEC SQL … END-EXEC`), heavily.
- **Each object's properties are in a binary `$Recipe` stream.** It is a
  typed table: numeric property ids, OLE VARIANT type codes, offsets into a
  value area. Strings are UTF-16, so control names, captions, tooltips, edit
  masks (`ZZ9`, `B-.---.---.--9,9999`), font names and image resource names
  can all be read. Bitmaps and toolbar/menu item lists (`CObArray`,
  `CButtonItem`) are embedded in the value area. **Property names are not
  stored.** A property id means something only in the context of its control
  class, which is identified by a class GUID.
- **Not every control is a PowerCOBOL control.** Besides the native ones
  (`CmStatic`, `CmCommand`, `CmCheck`, `CmFrame`, `CmImage`, `CmShape`,
  edit and list controls, Timer), the samples use third-party ActiveX
  controls (`ctToolBar`, `FramePlus`, `MSCOMCT2.OCX`).
- **Build outputs sit beside the project** (`M-*/Debug|Release`). The
  generated `.cob` lists each form's controls in order. The `.rc` maps image
  resource names to `.bmp` paths, also absolute Windows paths. The
  converter's source is the `.ppj`; these outputs can serve only as a
  cross-check.
- **Indexed files are rare in these samples:** one `ORGANIZATION IS
  INDEXED` file, in a copybook (`Copys/EFAM.cop`). Every form-level SELECT is
  `LINE SEQUENTIAL` or `SEQUENTIAL`; the application's data lives mostly in
  SQL. The indexed-file importer (rule d) must still work wherever an indexed
  SELECT appears: in a form's `FILE-CONTROL`, or in a copybook it includes.

## 2. Goals / Non-goals

**Goals**
- One IDE menu action converts a PowerCOBOL 5.x+ project into a new
  PowerRustCOBOL project that opens in the IDE.
- Full scope (operator, 2026-10-03): project structure, forms, controls,
  properties, event scripts, form procedures, data sections, copybooks and
  images.
- The four operator rules:
  - **(a)** images are moved into the project's assets;
  - **(b)** each converted form's responsiveness follows
    `ObsoleteScalingStyle` (spec 081);
  - **(c)** forms get the **Elegance** theme by default, and the developer
    may choose another;
  - **(d)** every indexed file found through SELECT/FD gets an equivalent
    PowerRustCOBOL definition and generated import code, and all of it
    becomes part of the project.
- Nothing is lost silently. Whatever is not translated exactly is kept and
  listed in a conversion report that the developer can act on.
- Converting the same input twice produces the same project, byte for byte.

**Non-goals**
- A command-line converter (`rcrun`). The operator chose the IDE menu only
  (2026-10-03).
- PowerCOBOL versions before 5.x. Their project format is different and
  unsampled; such a file is refused with a message (R3).
- Reading Fujitsu's binary indexed **data** files directly. CLAUDE.md forbids
  any claim of Fujitsu binary compatibility. How legacy data gets in is
  settled in Q2.
- Converting the third-party ActiveX controls' own behaviour. A control with
  no native equivalent becomes a placeholder (R16).
- Round-tripping back to PowerCOBOL.
- Re-designing forms for the modern responsive tools (anchors, containers).
  The migrated form keeps its PowerCOBOL geometry under
  `ObsoleteScalingStyle`, and the developer modernises it later if they want
  to.

## 3. User stories

- As a developer maintaining a PowerCOBOL application, I want to pick my
  `.ppj` and get a PowerRustCOBOL project, so that I start from my own forms
  and code instead of a blank designer.
- As that developer, I want my bitmaps and icons in the new project's assets,
  so that the application no longer depends on `C:\` paths.
- As that developer, I want my forms to resize the way they did in
  PowerCOBOL, so that my users see what they are used to.
- As that developer, I want my indexed files defined in the new project, with
  a program that loads my existing data, so that the converted application
  starts with my records, not empty files.
- As that developer, I want a precise list of what the converter could not
  translate, with the original code kept, so that I can finish the migration
  by hand without hunting for it.

## 4. Requirements (EARS)

### Entry point and project

- **R1 (event):** When the developer chooses the converter's menu item
  (**File → Import PowerCOBOL Project…**), the IDE shall open an import
  dialog with these fields:
  - the source `.ppj`;
  - the destination folder for the new project;
  - the form theme (default **Elegance**, any installed theme selectable);
  - an optional legacy root folder used to find images and copybooks.
- **R2 (constraint):** The converter shall never modify, move or delete
  anything under the source folder. It only reads.
- **R3 (event):** When the selected file is not an OLE compound document, or
  lacks the PowerCOBOL project structure (a root `$Recipe` and module
  storages), the converter shall refuse it with a message naming the reason,
  and write nothing.
- **R4 (constraint):** The converter shall refuse a destination that already
  holds a PowerRustCOBOL project or any non-empty folder. It shall not
  overwrite the developer's work.
- **R5 (ubiquitous):** The converter shall write a complete standard
  PowerRustCOBOL project at the destination: the manifest, `forms/`,
  `indexed/`, `src/`, `generated/`, `COPYBOOKS/`, `Assets/` and `data/`. The
  project shall open in the IDE immediately afterwards.
- **R6 (ubiquitous):** Every PowerCOBOL form in every module of the `.ppj`
  shall become one `.cfrm`. Form names (and therefore program names) are kept
  as written in the source. The module a form came from is recorded in the
  report.
- **R7 (ubiquitous):** Exactly one converted form shall be the main form
  (spec 037). It is the form the source project designates as its start form
  when the `.ppj` records one. Otherwise it is the first form of the first
  module, and the report says so.
- **R8 (ubiquitous):** Converting the same `.ppj` with the same options
  twice shall produce identical files.

### Forms, controls, properties

- **R9 (ubiquitous):** For each form, the converter shall carry over the
  form's caption, size and position, and each control's:
  - name, type, position and size;
  - z-order and parent container;
  - caption or text, font, colours, enabled/visible state, tab order,
    tooltip and edit mask, where the target control has that property.
- **R10 (ubiquitous):** The converter shall identify each control's class by
  its class GUID, not by its name. It shall map the class through one
  documented mapping table, *PowerCOBOL class → PowerRustCOBOL
  `ControlType`*, with a property map per class. The table lives in the
  plan, and its reference copy goes in the Developer's Guide appendix.
- **R11 (event):** When a source property has no mapping for its class, or a
  target that cannot hold its value, the converter shall leave it out of the
  `.cfrm` and list it in the report with:
  - the control's name;
  - the property id or name;
  - the original value.
- **R12 (ubiquitous) — rule (b):** Every converted form shall be responsive
  through `ObsoleteScalingStyle` (spec 081). Its value comes from the source
  form's own scaling setting, using the default settled in Q4. A form shall
  not carry anchors or containers the source did not have.
- **R13 (ubiquitous) — rule (c):** Every converted form shall carry the theme
  chosen in the dialog, **Elegance** by default. The theme's defaults are
  applied the way the designer applies them (`apply_theme_defaults`), except
  for explicit source colours and fonts: those are preserved. The developer
  can change the theme afterwards like on any form.
- **R14 (ubiquitous):** Toolbar and menu item lists embedded in a control's
  recipe shall become the target control's items, with:
  - captions;
  - tooltips;
  - images;
  - enabled state.
- **R15 (ubiquitous):** Picture and edit masks (`ZZ9`, `B-.---.---.--9,9999`)
  shall be carried to the target's input-mask or numeric-edit property. The
  source's decimal-point convention (a comma here) is kept.
- **R16 (event):** When a control's class has no native equivalent (a
  third-party ActiveX control), the converter shall place a placeholder of
  the same name and rectangle. Every script referring to the control is
  still converted, so the project compiles, and the report names the original
  class and its ActiveX file when known. Q5 settles which placeholder type
  is used.

### Scripts and code

- **R17 (ubiquitous):** Every `<CONTROL>-<Event>` script shall become the
  matching PowerRustCOBOL event handler on that control (or on the form).
  The event-name mapping (e.g. `Click → onClick`, `Opened → onLoad`) is the
  documented table in the plan. A script whose event has no equivalent
  becomes a form procedure under its original name, and the report lists it
  as unbound.
- **R18 (ubiquitous):** A form's non-event scripts (`PARASALIR`, `VACIAR` …)
  shall become that form's procedures, keeping their names. So
  `CALL "PARASALIR"` in a handler still reaches them.
- **R19 (ubiquitous):** The form's `WORKING-STORAGE`, `FILE-CONTROL`, `FILE`
  and `REPOSITORY` scripts shall become the form's COBOL structure blocks
  (spec 005).
- **R20 (ubiquitous):** Script text shall be decoded from Windows-1252 and
  written as UTF-8, so accented literals and comments (`¡ Atención … !`,
  `CONTRASEÑA`) arrive intact.
- **R21 (ubiquitous):** The converter shall rewrite the PowerCOBOL object
  syntax into RustCOBOL's, using one documented table in the plan:
  - `POW-SELF` → `me`;
  - `POW-SUPER` → `super`;
  - `"Prop" OF ctl` → `ctl::Prop`;
  - `INVOKE ctl "Method" USING …` → the equivalent method, or a supported
    `INVOKE`;
  - `POW-…` constants → their values or our equivalents.

  The rewrite works on tokens, never on raw text, so string literals and
  comments are untouched.
- **R22 (ubiquitous):** Every `#INCLUDE "<path>"` shall become a `COPY` of
  the same copybook, copied into the project's `COPYBOOKS/`. The converter
  locates it by file name under the `.ppj`'s folder, then under the legacy
  root (R1). The original absolute path is listed in the report.
- **R23 (event):** When the converter cannot translate a statement (an
  unknown method, an unmapped constant, embedded SQL as settled in Q1), it
  shall keep the original statement in the code as a clearly marked comment
  block (`*> POWERCOBOL:`), followed by the original lines. It shall also
  list the statement in the report with its form, script and line. Developer
  code is never deleted (GOLDEN RULE *user code is sacred*).
- **R24 (ubiquitous):** `GLOBAL` and `EXTERNAL` data items shared between
  forms shall be kept with the sharing semantics settled in Q6. They are
  never silently made local.

### Images — rule (a)

- **R25 (ubiquitous):** Every image the forms use shall be written into the
  project's `Assets/`, named after its resource name, and referenced from the
  `.cfrm` by its project-relative path. This covers control pictures, toolbar
  images and form backgrounds. A `.cfrm` never references a path outside the
  project.
- **R26 (ubiquitous):** The converter shall take an image from the bitmap
  embedded in the `.ppj` when one is there. Otherwise it locates the file
  named by the module's resource script by file name, under the `.ppj`'s
  folder and then the legacy root.
- **R27 (ubiquitous):** Identical images used under several names or by
  several forms shall be stored once.
- **R28 (event):** When an image cannot be found, the converter shall leave
  the property empty and list the resource name and original path in the
  report.

### Indexed files — rule (d)

- **R29 (ubiquitous):** The converter shall find every `SELECT … ORGANIZATION
  IS INDEXED` and its `FD` in each form's `FILE-CONTROL`/`FILE` scripts and
  in every copybook those scripts or the `WORKING-STORAGE` include.
- **R30 (ubiquitous):** For each distinct indexed file, the converter shall
  write one `.cidx` definition in `indexed/` and add it to the project, with:
  - the record layout from the FD;
  - the `RECORD KEY`;
  - every `ALTERNATE RECORD KEY`, with its `WITH DUPLICATES` flag;
  - the access mode;
  - the assignment.
- **R31 (state):** While the same file (same SELECT name and same record
  layout) appears in several forms, the converter shall define it once. When
  the same SELECT name appears with different layouts, it shall define each
  under a distinguishable name and report the conflict.
- **R32 (ubiquitous):** For each indexed file, the converter shall generate a
  COBOL import program in the project's Common Code (`src/`), added to the
  project. It reads the legacy data in the form settled in Q2 and writes
  every record into the new indexed file. Records rejected by the new file
  (a duplicate key, for example) are counted, not fatal.
- **R33 (ubiquitous):** Each import program shall print one result block at
  its end (GOLDEN RULE #7):
  - the file it imported;
  - records read, written and rejected, with the reason;
  - elapsed time;
  - records per second.
- **R34 (ubiquitous):** The converter shall also generate one driver program
  that runs every import program in turn, so a whole application's data
  loads in one step.
- **R35 (ubiquitous):** A dynamic assignment (`ASSIGN TO data-name`, as in
  `ASSIGN TO FILE12-CLASI`) shall keep its data-name in the converted code.
  The `.cidx` and the import program then default the file to the project's
  `data/` folder, and the report says where the original program built its
  path.

### Report and completion

- **R36 (ubiquitous):** The converter shall write a conversion report
  (Markdown) into the project's Documentation category. It lists:
  - per form: the controls converted, placeholders, unmapped properties,
    unbound events, untranslated statements and missing images;
  - per indexed file: its definition, its import program, and any layout
    conflict;
  - totals.
- **R37 (event):** When conversion finishes, the converter shall regenerate
  every form and run **Check**. The IDE then shows a summary with:
  - forms, controls, scripts, images and indexed files converted;
  - the report's item count;
  - the Check result.

  A project with Check errors still opens, and the summary points at the
  report.
- **R38 (state):** While a conversion runs, the IDE shall stay responsive
  and show progress (which form is being converted). The developer can
  cancel, and a cancelled conversion leaves no partial project behind.

### Product constraints

- **R39 (constraint):** Every new user-facing string (menu item, dialog,
  summary, errors) shall be a `Tr` field in all six languages. The report
  is written in the IDE's language.
- **R40 (constraint):** Generated COBOL, data-item names, paragraph names
  and the comment markers the converter adds shall be in English. The
  developer's own identifiers, literals and comments (often Spanish) are
  kept exactly as written.
- **R41 (constraint):** The converter is implemented in Rust only (PRIME
  DIRECTIVE). The OLE container reader is a pure-Rust dependency, recorded in
  `docs/DEPENDENCIES-en.md` and `THIRD_PARTY_NOTICES.md`.

## 5. Acceptance criteria

Measured against the operator's samples (`~/Documents/Legacy/CBL`), which
are never copied into the repository (they are the developer's code). Tests
use small, original `.ppj` fixtures written for this purpose.

- [ ] AC1 (R1, R5, R6): Converting `P-FAMILIAS.ppj` gives a project that
  opens in the IDE, with exactly the three forms `F-FAMILIAS`,
  `F-CON-FAMILIAS` and `F-LIS-FAMILIAS`.
- [ ] AC2 (R2, R4): The source folder's file listing and checksums are
  identical before and after a conversion. A non-empty destination is
  refused.
- [ ] AC3 (R3): A non-OLE file and an OLE file without the PowerCOBOL
  structure are each refused with a message, and nothing is written.
- [ ] AC4 (R8): Two conversions of the same `.ppj` into two folders produce
  byte-identical trees.
- [ ] AC5 (R9, R10): Every control the source form's generated `.cob` lists
  in `POW-FORM` appears in the `.cfrm` with the same name. Its rectangle
  matches the source within one pixel after unit conversion.
- [ ] AC6 (R12, R13): Every converted form carries `ObsoleteScalingStyle`
  from its source and the chosen theme. With the default options, that
  theme is `elegance`.
- [ ] AC7 (R17, R18, R19): Every script stream of `F-FAMILIAS` maps to a
  handler, procedure or structure block. The report accounts for every
  stream not mapped, and the count of streams equals mapped plus reported.
- [ ] AC8 (R20): The literal `" ¡ Atención ... !"` and the comment
  containing `CONTRASEÑA` survive conversion byte-correct in UTF-8.
- [ ] AC9 (R21, R23): `MOVE 1 TO "Enabled" OF BTLISTAR` becomes
  `MOVE 1 TO BTLISTAR::Enabled`. An untranslatable statement appears in a
  `*> POWERCOBOL:` block with its original lines, and in the report.
- [ ] AC10 (R22): `#INCLUDE "C:\CBL\Copys\EXTERNOS.cop"` becomes a `COPY` of
  `COPYBOOKS/EXTERNOS.cop`, and that copybook is in the project.
- [ ] AC11 (R25–R28): Every image resource named in the module's `.rc` is
  in `Assets/` once, or listed in the report as missing. No `.cfrm` contains
  an absolute path.
- [ ] AC12 (R29, R30): The indexed file declared in `Copys/EFAM.cop` yields a
  `.cidx` whose keys and record layout match its SELECT/FD.
- [ ] AC13 (R32–R34): The generated import program for that file, run on a
  legacy-data fixture in the Q2 form, loads every valid record, counts
  rejected ones, and prints the R33 block with real measured numbers. The
  driver runs all import programs.
- [ ] AC14 (R36, R37): The report exists in the Documentation category, and
  its counts equal the summary's. Check has run, and its result is shown.
- [ ] AC15 (R38): Cancelling midway leaves the destination as it was.
- [ ] AC16 (R39): `i18n_tests` pass with every new `Tr` field present in all
  six languages.

## 6. Constraints & steering check

- **i18n (6 languages):** yes. These need `Tr` fields:
  - the menu item;
  - the dialog, its fields and its errors;
  - the progress and summary text;
  - the report's headings.
- **Generated-code contract:** unchanged. Converted forms are ordinary
  `.cfrm` files; their `.cbl` is generated and regenerated like any other.
  The import programs and the driver are **Common Code** (`src/`), not
  generated code: the developer owns them and may edit them.
- **Docs:** the English Developer's Guide gains a migration section. Appendix
  A's "do not expect compatibility" caveat is rewritten to describe what the
  converter carries over and what it does not. This invalidates the five
  translations (GOLDEN RULE #8: delete them in the same change). The System
  KB is untouched unless the work adds a property, method or event.
- **Fix vs feature:** **feature** (a new IDE capability). It goes on
  `features`, through the spec pipeline. The version bump is `z` only.
- **Dependencies:** one new pure-Rust crate for OLE compound files (e.g.
  `cfb`), license-checked. No C code.
- **Legal:** the `.ppj` format is read for interoperability with the
  developer's own files. The mapping tables and documentation are original
  work. Nothing from Fujitsu's manuals is copied.
- **Risk — undocumented property ids.** `$Recipe` names no properties. The
  per-class property map (R10) must be derived by comparing the samples:
  - change one property in a known form, and see which id moved;
  - cross-check against the generated `.cob` and `.rc`.

  This is the largest piece of work. The plan should open with a spike that
  decodes the classes the samples actually use, and treat the rest as
  placeholders (R16) until a sample exists.

## 7. Open questions

- **Q1 — Embedded SQL.** The samples use `EXEC SQL … END-EXEC` heavily:
  cursors, `FETCH … INTO :host`, `BEGIN/END DECLARE SECTION`. RustCOBOL has
  no `EXEC SQL`; SQL goes through `COBOL::"OPEN-DB"` / `"EXEC-SQL"` /
  `"FETCH-ROW"`. Which should the converter do?
  - (a) Translate `EXEC SQL` blocks into those built-ins. This is feasible
    for simple statements, but cursors and host-variable lists make it
    large.
  - (b) Keep every `EXEC SQL` block as a `*> POWERCOBOL:` comment and report
    it (R23). The project compiles, and SQL is redone by hand.
  - (c) (b) now, and a separate spec that adds `EXEC SQL` to the language.

  **Recommendation: (c).** A language extension must not be invented inside
  a converter (GOLDEN RULE #2's spirit), and done properly it serves every
  migrated application, not only converted ones.

- **Q2 — Where the legacy data comes from.** We cannot read Fujitsu's binary
  indexed files, and must never claim to. Which input should the generated
  import program read?
  - (a) A record-sequential file with the FD's exact layout, exported on the
    developer's Windows machine with Fujitsu's own file utility. The
    import program reads it record by record.
  - (b) (a), plus a line-sequential (text) variant.
  - (c) Something else you already use to export.

  **Recommendation: (a).** The export is lossless for binary fields
  (`COMP`, `COMP-3`), which a text export is not.

- **Q3 — One `.ppj` or the whole application?** Your sample is one
  application split into 16 `.ppj` files (`TyC`, `P-FAMILIAS`,
  `P-ARTICULOS`…) sharing copybooks and `EXTERNAL` data. Should the converter:
  - (a) convert one `.ppj` into one project (what this draft specifies); or
  - (b) take a folder and merge every `.ppj` in it into one project, with
    the forms of all of them?

  (b) is what a real migration needs, and it makes `EXTERNAL` sharing
  (Q6) and cross-project `CALL`s work.

- **Q4 — `ObsoleteScalingStyle` when the source form has no scaling.** Rule
  (b) says responsiveness follows `ObsoleteScalingStyle`. When the source
  form declares a scaling mode, its value is carried over. When it declares
  none (a fixed-size PowerCOBOL form), which value should the converted form
  get?
  - **0**: fixed, faithful to the source.
  - **7**: everything scales, so every converted form is responsive.

  **Recommendation: 7.** It matches the "every form is responsive" golden
  rule for applications.

- **Q5 — Placeholder for unknown ActiveX controls.** Should the placeholder
  be:
  - a **Label** showing the original class name;
  - a **Panel** with the same name and rectangle; or
  - the generic plugin `Custom` control?

  **Recommendation: a Panel.** It keeps the space, can hold the original
  children, and any `::` property a script sets on it compiles. Known ones
  are mapped to native controls: toolbar → `ToolBar`, progress bar →
  `ProgressBar`, date picker → `DateTimePicker`.

- **Q6 — `EXTERNAL` and `GLOBAL` data across forms.** The samples share
  state through `GLOBAL EXTERNAL` items (`RUTA`, `WEMP`, `WNIV`…).
  PowerRustCOBOL forms run as separate programs and never share data items.
  The plan will first measure whether the runtime honours `EXTERNAL` across
  forms. If it does not, which should the converter do?
  - (a) Keep `EXTERNAL` and leave the gap for a fix spec (a COBOL-85
    construct that should work is technical debt).
  - (b) Rewrite the items into published properties of the main form.

  **Recommendation: (a).** It is standard COBOL and the faithful
  translation.
