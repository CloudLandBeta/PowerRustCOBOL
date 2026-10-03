<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — PowerCOBOL application converter (PowerCOBOL 5.x and later)

- **Status:** approved (operator, 2026-10-03; Q1–Q6 settled in §7)
- **Folder:** specs/086-powercobol-converter/
- **Author:** Claude (for Emerson Lopes)   **Date:** 2026-10-03

## 1. Overview

A developer with a working Fujitsu PowerCOBOL application has, today, no way
into PowerRustCOBOL AI except rebuilding every form by hand. The release
candidate post (2026-08-24) told the community a converter was in
development. No spec, crate or prototype exists yet: the 2026-08-23 RC
session and spec 081 §2 both confirm it.

This feature adds the converter. It is an IDE menu action that reads a
PowerCOBOL 5.x-or-later **application** and writes one new PowerRustCOBOL
project. The application is a folder of `.ppj` projects that share
copybooks, images and `EXTERNAL` data (§7 Q3). The new project contains:
- every form, with its controls and properties;
- the event scripts and form procedures, translated to RustCOBOL;
- the copybooks;
- the images, moved into the project's assets;
- an `.cidx` definition for every indexed file the source declares, plus a
  generated COBOL program that loads that file's legacy data.

A conversion report lists everything that could not be carried over exactly.
The aim is a project that opens, passes **Check**, and runs. Where something
cannot be carried over, the report says so: the converter never silently
drops anything.

### What the source looks like (measured on the operator's samples)

The operator's sample application (`~/Documents/Legacy/CBL`) has 16 `.ppj`
projects, 34 forms (26 distinct names) and 723 script streams. That is the
evidence base for this spec. What it shows:

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
- **The same form appears in several `.ppj`, and the copies differ.** Six
  form names occur in more than one project. For example, `F-CON-ART` is in
  `P-CON-ART`, `P-LIST-ART` and `P-LIST-ART_1.0.0`, and the scripts differ
  between copies. `P-LIST-ART_1.0.0.ppj` is an older copy of
  `P-LIST-ART.ppj`. Merging a folder (Q3) therefore needs a rule for which
  copy wins (R7).
- **Build outputs sit beside each project** (`M-*/Debug|Release`). The
  generated `.cob` lists each form's controls in order. The `.rc` maps image
  resource names to `.bmp` paths, also absolute Windows paths. The
  converter's source is the `.ppj`; these outputs can serve only as a
  cross-check. `Backup/*.ppj~` are editor backups.
- **Indexed files are rare in these samples:** one `ORGANIZATION IS
  INDEXED` file, in a copybook (`Copys/EFAM.cop`). Every form-level SELECT is
  `LINE SEQUENTIAL` or `SEQUENTIAL`; the application's data lives mostly in
  SQL. The indexed-file importer (rule d) must still work wherever an indexed
  SELECT appears: in a form's `FILE-CONTROL`, or in a copybook it includes.

## 2. Goals / Non-goals

**Goals**
- One IDE menu action converts a PowerCOBOL 5.x+ application (a folder of
  `.ppj`) into one PowerRustCOBOL project that opens in the IDE.
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
- Controls become **native PowerRustCOBOL controls** wherever any reasonable
  equivalent exists, including third-party ActiveX controls (Q5).
- Nothing is lost silently. Whatever is not translated exactly is kept and
  listed in a conversion report that the developer can act on.
- Converting the same input twice produces the same project, byte for byte.

**Non-goals**
- A command-line converter (`rcrun`). The operator chose the IDE menu only
  (2026-10-03).
- **Translating embedded SQL.** `EXEC SQL` blocks are kept as marked comments
  (Q1). Adding `EXEC SQL` to RustCOBOL is a **separate spec**, to be written
  after this one; it serves every migrated application.
- PowerCOBOL versions before 5.x. Their project format is different and
  unsampled; such a file is skipped with a message (R3).
- Reading Fujitsu's binary indexed **data** files directly. CLAUDE.md forbids
  any claim of Fujitsu binary compatibility. Legacy data comes in through a
  record-sequential export (Q2).
- Reproducing third-party ActiveX controls' behaviour beyond what the native
  control they are mapped to offers.
- Round-tripping back to PowerCOBOL.
- Re-designing forms for the modern responsive tools (anchors, containers).
  The migrated form keeps its PowerCOBOL geometry under
  `ObsoleteScalingStyle`, and the developer modernises it later if they want
  to.

## 3. User stories

- As a developer maintaining a PowerCOBOL application, I want to point at my
  application's folder and get one PowerRustCOBOL project, so that I start
  from my own forms and code instead of a blank designer.
- As that developer, I want my bitmaps and icons in the new project's assets,
  so that the application no longer depends on `C:\` paths.
- As that developer, I want my forms to resize the way they did in
  PowerCOBOL, so that my users see what they are used to.
- As that developer, I want my grids, toolbars and other third-party controls
  to become real PowerRustCOBOL controls, so that the converted forms work
  and are not full of empty boxes.
- As that developer, I want my indexed files defined in the new project, with
  a program that loads my existing data, so that the converted application
  starts with my records, not empty files.
- As that developer, I want a precise list of what the converter could not
  translate, with the original code kept, so that I can finish the migration
  by hand without hunting for it.

## 4. Requirements (EARS)

### Entry point and project

- **R1 (event):** When the developer chooses **File → Import PowerCOBOL
  Application…**, the IDE shall open an import dialog with these fields:
  - the source folder;
  - the destination folder for the new project;
  - the form theme (default **Elegance**, any installed theme selectable);
  - an optional legacy root folder used to find images and copybooks that
    are not under the source folder.
- **R2 (event):** When a source folder is chosen, the dialog shall list
  every `.ppj` directly in that folder, all selected by default, each with
  its modules and forms. Editor backups (`*.ppj~`) and subfolders are not
  scanned. The developer may deselect any project.
- **R3 (event):** When a listed file is not an OLE compound document, or
  lacks the PowerCOBOL project structure (a root `$Recipe` and module
  storages), the dialog shall mark it as not convertible, with the reason,
  and exclude it. When no convertible project remains selected, the converter
  shall not start.
- **R4 (constraint):** The converter shall never modify, move or delete
  anything under the source folder or the legacy root. It only reads.
- **R5 (constraint):** The converter shall refuse a destination that already
  holds a PowerRustCOBOL project or any non-empty folder. It shall not
  overwrite the developer's work.
- **R6 (ubiquitous):** The converter shall write one complete standard
  PowerRustCOBOL project at the destination: the manifest, `forms/`,
  `indexed/`, `src/`, `generated/`, `COPYBOOKS/`, `Assets/` and `data/`. It
  contains every form of every selected `.ppj`, and opens in the IDE
  immediately afterwards.
- **R7 (state):** While two selected projects contain a form of the same
  name:
  - If the copies are identical, the converter shall convert the form once.
  - If they differ, the dialog shall show the conflict and propose the copy
    from the most recently modified `.ppj`. The developer may pick another
    copy. Only the chosen copy is converted, and the report names the copies
    left out.

  The same applies to copybooks and images of the same name with different
  content.
- **R8 (ubiquitous):** Form names (and therefore program names) shall be
  kept as written in the source. The report records the `.ppj` and module
  each form came from.
- **R9 (ubiquitous):** Exactly one converted form shall be the main form
  (spec 037). The dialog proposes it from the source; the plan defines how
  the source marks a start form (e.g. the `Main.MainForm` class seen in
  `TyC.ppj`). The developer can pick another form before converting.
- **R10 (event):** When a script opens or calls a form or module that, in
  PowerCOBOL, lived in another `.ppj`, the converted code shall reach that
  form inside the merged project the way PowerRustCOBOL opens a form. Where
  the call cannot be recognised, R26 applies.
- **R11 (ubiquitous):** Converting the same selection with the same options
  twice shall produce identical files.

### Forms, controls, properties

- **R12 (ubiquitous):** For each form, the converter shall carry over the
  form's caption, size and position, and each control's:
  - name, type, position and size;
  - z-order and parent container;
  - caption or text, font, colours, enabled/visible state, tab order,
    tooltip and edit mask, where the target control has that property.
- **R13 (ubiquitous):** The converter shall choose each control's target
  type in this order, using one documented mapping table in the plan:
  1. **By class:** the source class GUID, for every PowerCOBOL class and
     every third-party class the table knows.
  2. **By role, best effort (Q5):** an unknown class is mapped from what it
     evidently is, judged by its class name, its ActiveX file name and its
     control name. Anything named or behaving as a table or grid becomes
     `DataGrid`. Toolbar → `ToolBar`, progress bar → `ProgressBar`, date or
     calendar picker → `DateTimePicker`, tree → `TreeView`, tab strip →
     `TabControl`, frame or group → `GroupBox` or `Panel`, and so on.
  3. **Otherwise:** a `Panel` with the original control's name and
     rectangle.

  The report states, for every control mapped by role or turned into a
  Panel, its original class, its ActiveX file when known, and the reason
  for the choice.
- **R14 (ubiquitous):** Every script that refers to a control shall still
  be converted whatever its target type, so the project compiles. A
  property or method the target type lacks falls under R26.
- **R15 (event):** When a source property has no mapping for its class, or
  a target that cannot hold its value, the converter shall leave it out of
  the `.cfrm` and list it in the report with:
  - the control's name;
  - the property id or name;
  - the original value.
- **R16 (ubiquitous) — rule (b):** Every converted form shall be responsive
  through `ObsoleteScalingStyle` (spec 081):
  - When the source form declares a scaling mode, its value is carried
    over.
  - When it declares none, the value is **7**: resize, move and scale fonts
    (Q4).

  A form shall not carry anchors or containers the source did not have.
- **R17 (ubiquitous) — rule (c):** Every converted form shall carry the
  theme chosen in the dialog, **Elegance** by default. The theme's defaults
  are applied the way the designer applies them (`apply_theme_defaults`),
  except for explicit source colours and fonts: those are preserved. The
  developer can change the theme afterwards like on any form.
- **R18 (ubiquitous):** Toolbar, menu and popup-menu item lists embedded in
  a control's or form's recipe shall become the target control's items,
  with:
  - captions;
  - tooltips;
  - images;
  - enabled state.
- **R19 (ubiquitous):** Picture and edit masks (`ZZ9`, `B-.---.---.--9,9999`)
  shall be carried to the target's input-mask or numeric-edit property. The
  source's decimal-point convention (a comma here) is kept.

### Scripts and code

- **R20 (ubiquitous):** Every `<CONTROL>-<Event>` script shall become the
  matching PowerRustCOBOL event handler on that control (or on the form).
  The event-name mapping (e.g. `Click → onClick`, `Opened → onLoad`) is the
  documented table in the plan. A script whose event has no equivalent
  becomes a form procedure under its original name, and the report lists it
  as unbound.
- **R21 (ubiquitous):** A form's non-event scripts (`PARASALIR`, `VACIAR` …)
  shall become that form's procedures, keeping their names. So
  `CALL "PARASALIR"` in a handler still reaches them.
- **R22 (ubiquitous):** The form's `WORKING-STORAGE`, `FILE-CONTROL`, `FILE`
  and `REPOSITORY` scripts shall become the form's COBOL structure blocks
  (spec 005).
- **R23 (ubiquitous):** Script and copybook text shall be decoded from
  Windows-1252 and written as UTF-8, so accented literals and comments
  (`¡ Atención … !`, `CONTRASEÑA`) arrive intact.
- **R24 (ubiquitous):** The converter shall rewrite the PowerCOBOL object
  syntax into RustCOBOL's, using one documented table in the plan:
  - `POW-SELF` → `me`;
  - `POW-SUPER` → `super`;
  - `"Prop" OF ctl` → `ctl::Prop`;
  - `INVOKE ctl "Method" USING …` → the equivalent method, or a supported
    `INVOKE`;
  - `POW-…` constants → their values or our equivalents.

  The rewrite works on tokens, never on raw text, so string literals and
  comments are untouched.
- **R25 (ubiquitous):** Every `#INCLUDE "<path>"` shall become a `COPY` of
  the same copybook, copied into the project's `COPYBOOKS/`. The converter
  locates it by file name under the source folder, then under the legacy
  root (R1). The original absolute path is listed in the report.
- **R26 (event):** When the converter cannot translate a statement (an
  unknown method, an unmapped constant, or a property the target type
  lacks), it shall keep the original statement in the code as a clearly
  marked comment block (`*> POWERCOBOL:`), followed by the original lines.
  It shall also list the statement in the report with its form, script and
  line. Developer code is never deleted (GOLDEN RULE *user code is sacred*).
- **R27 (ubiquitous) — embedded SQL (Q1):** Every `EXEC SQL … END-EXEC`
  block, including `BEGIN/END DECLARE SECTION`, shall be kept verbatim
  under R26, with these exceptions:
  - The host variables declared inside a `DECLARE SECTION` stay as ordinary,
    live data items, so the rest of the code that uses them still compiles.
  - Statements depending on an SQL result (`SQLSTATE`, `SQLCODE` tests) stay
    live.

  The report gives the count of SQL blocks per form, so the follow-up spec
  can size its work.
- **R28 (ubiquitous) — `EXTERNAL`/`GLOBAL` (Q6):** `GLOBAL` and `EXTERNAL`
  data items shall be kept exactly as declared, with standard COBOL
  semantics. They are never made local or rewritten into properties. If the
  plan's measurement shows the runtime does not share `EXTERNAL` items
  between forms, that gap is technical debt (GOLDEN RULE #5 precedent),
  handled as a separate **fix**. The converter's output stays standard
  COBOL either way.

### Images — rule (a)

- **R29 (ubiquitous):** Every image the forms use shall be written into the
  project's `Assets/`, named after its resource name, and referenced from the
  `.cfrm` by its project-relative path. This covers control pictures, toolbar
  and menu images, and form backgrounds. A `.cfrm` never references a path
  outside the project.
- **R30 (ubiquitous):** The converter shall take an image from the bitmap
  embedded in the `.ppj` when one is there. Otherwise it locates the file
  named by the module's resource script by file name, under the source
  folder and then the legacy root.
- **R31 (ubiquitous):** Identical images used under several names, forms or
  projects shall be stored once.
- **R32 (event):** When an image cannot be found, the converter shall leave
  the property empty and list the resource name and original path in the
  report.

### Indexed files — rule (d)

- **R33 (ubiquitous):** The converter shall find every `SELECT … ORGANIZATION
  IS INDEXED` and its `FD` in each form's `FILE-CONTROL`/`FILE` scripts and
  in every copybook those scripts or the `WORKING-STORAGE` include.
- **R34 (ubiquitous):** For each distinct indexed file, the converter shall
  write one `.cidx` definition in `indexed/` and add it to the project, with:
  - the record layout from the FD;
  - the `RECORD KEY`;
  - every `ALTERNATE RECORD KEY`, with its `WITH DUPLICATES` flag;
  - the access mode;
  - the assignment.
- **R35 (state):** While the same file (same SELECT name and same record
  layout) appears in several forms or projects, the converter shall define
  it once. When the same SELECT name appears with different layouts, it
  shall define each under a distinguishable name and report the conflict.
- **R36 (ubiquitous) — legacy data (Q2):** For each indexed file, the
  converter shall generate a COBOL import program in the project's Common
  Code (`src/`), added to the project. It reads a **record-sequential** file
  whose records have exactly the FD's layout, as written by Fujitsu's own
  file utility on the developer's Windows machine. It writes every record
  into the new indexed file. Records rejected by the new file (a duplicate
  key, for example) are counted, not fatal.
- **R37 (ubiquitous):** Each import program shall print one result block at
  its end (GOLDEN RULE #7):
  - the file it imported;
  - records read, written and rejected, with the reason;
  - elapsed time;
  - records per second.
- **R38 (ubiquitous):** The converter shall also generate one driver program
  that runs every import program in turn, so a whole application's data
  loads in one step.
- **R39 (ubiquitous):** The Developer's Guide section on the converter, and
  the comment heading each import program, shall say which export file the
  program expects, where it looks for it by default, and what layout it
  must have. They shall not reproduce the vendor's manual.
- **R40 (ubiquitous):** A dynamic assignment (`ASSIGN TO data-name`, as in
  `ASSIGN TO FILE12-CLASI`) shall keep its data-name in the converted code.
  The `.cidx` and the import program then default the file to the project's
  `data/` folder, and the report says where the original program built its
  path.

### Report and completion

- **R41 (ubiquitous):** The converter shall write a conversion report
  (Markdown) into the project's Documentation category. It lists:
  - per form: its source `.ppj`, the controls converted (by class, by role,
    or as a Panel), unmapped properties, unbound events, untranslated
    statements, SQL block count and missing images;
  - conflicts resolved under R7;
  - per indexed file: its definition, its import program, and any layout
    conflict;
  - totals.
- **R42 (event):** When conversion finishes, the converter shall regenerate
  every form and run **Check**. The IDE then shows a summary with:
  - forms, controls, scripts, images and indexed files converted;
  - the report's item count;
  - the Check result.

  A project with Check errors still opens, and the summary points at the
  report.
- **R43 (state):** While a conversion runs, the IDE shall stay responsive
  and show progress (which project and form is being converted). The
  developer can cancel, and a cancelled conversion leaves no partial project
  behind.

### Product constraints

- **R44 (constraint):** Every new user-facing string (menu item, dialog,
  summary, errors) shall be a `Tr` field in all six languages. The report
  is written in the IDE's language.
- **R45 (constraint):** Generated COBOL, data-item names, paragraph names
  and the comment markers the converter adds shall be in English. The
  developer's own identifiers, literals and comments (often Spanish) are
  kept exactly as written.
- **R46 (constraint):** The converter is implemented in Rust only (PRIME
  DIRECTIVE). The OLE container reader is a pure-Rust dependency, recorded in
  `docs/DEPENDENCIES-en.md` and `THIRD_PARTY_NOTICES.md`.

## 5. Acceptance criteria

Measured against the operator's samples (`~/Documents/Legacy/CBL`), which
are never copied into the repository (they are the developer's code). Tests
use small, original `.ppj` fixtures written for this purpose.

- [ ] AC1 (R1, R2, R6): Choosing the samples folder lists all 16 `.ppj` and
  no `.ppj~`. Converting them gives one project that opens in the IDE, with
  one `.cfrm` per distinct form name (26).
- [ ] AC2 (R4, R5): The source folder's file listing and checksums are
  identical before and after a conversion. A non-empty destination is
  refused.
- [ ] AC3 (R3): A non-OLE file and an OLE file without the PowerCOBOL
  structure, placed in the folder, are each marked not convertible with a
  reason. The remaining projects still convert.
- [ ] AC4 (R7): The dialog shows the six conflicting form names (e.g.
  `F-CON-ART`: `P-CON-ART`, `P-LIST-ART`, `P-LIST-ART_1.0.0`) and proposes
  the copy from the most recently modified `.ppj`. Choosing another copy
  converts that one, and the report names the copies left out. The two
  identical copies of `F-AYUDA` cause no conflict.
- [ ] AC5 (R9): Exactly one form is the main form, and it is the one the
  dialog showed or the developer picked.
- [ ] AC6 (R11): Two conversions of the same selection into two folders
  produce byte-identical trees.
- [ ] AC7 (R12, R13): Every control that a source form's generated `.cob`
  lists in `POW-FORM` appears in the `.cfrm` with the same name, and its
  rectangle matches the source within one pixel after unit conversion.
  `TABLA` (a grid) is a `DataGrid`, `ctToolBar1` is a `ToolBar`, and every
  control left as a Panel is in the report with its class and reason.
- [ ] AC8 (R16, R17): Every converted form carries `ObsoleteScalingStyle`:
  its source value, or 7 where the source has none. Every form also carries
  the chosen theme, which is `elegance` with the default options.
- [ ] AC9 (R20, R21, R22): Every script stream of `F-FAMILIAS` maps to a
  handler, procedure or structure block. The report accounts for every
  stream not mapped, and the count of streams equals mapped plus reported.
- [ ] AC10 (R23): The literal `" ¡ Atención ... !"` and the comment
  containing `CONTRASEÑA` survive conversion byte-correct in UTF-8.
- [ ] AC11 (R24, R26): `MOVE 1 TO "Enabled" OF BTLISTAR` becomes
  `MOVE 1 TO BTLISTAR::Enabled`. An untranslatable statement appears in a
  `*> POWERCOBOL:` block with its original lines, and in the report.
- [ ] AC12 (R27): In `F-CON-FAMILIAS-Opened`, every `EXEC SQL` block is a
  `*> POWERCOBOL:` comment with its original lines. `SQLSTATE`, `SQLCODE` and
  `CONTASQL` remain declared data items. The form passes Check, and the
  report gives its SQL block count.
- [ ] AC13 (R25): `#INCLUDE "C:\CBL\Copys\EXTERNOS.cop"` becomes a `COPY` of
  `COPYBOOKS/EXTERNOS.cop`, and that copybook is in the project.
- [ ] AC14 (R28): `01 RUTA PIC X(255) GLOBAL EXTERNAL.` is in the converted
  code exactly as declared.
- [ ] AC15 (R29–R32): Every image resource named in the modules' `.rc` files
  is in `Assets/` once, or listed in the report as missing. No `.cfrm`
  contains an absolute path.
- [ ] AC16 (R33, R34): The indexed file declared in `Copys/EFAM.cop` yields a
  `.cidx` whose keys and record layout match its SELECT/FD.
- [ ] AC17 (R36–R38): Run on a record-sequential fixture with that FD's
  layout, the generated import program loads every valid record, counts
  rejected ones (including a planted duplicate key), and prints the R37
  block with real measured numbers. The driver runs every import program.
- [ ] AC18 (R41, R42): The report exists in the Documentation category, and
  its counts equal the summary's. Check has run, and its result is shown.
- [ ] AC19 (R43): Cancelling midway leaves the destination as it was.
- [ ] AC20 (R44): `i18n_tests` pass with every new `Tr` field present in all
  six languages.

## 6. Constraints & steering check

- **i18n (6 languages):** yes. These need `Tr` fields:
  - the menu item;
  - the dialog, its project list, conflict choice and main-form picker;
  - the errors;
  - the progress and summary text;
  - the report's headings.
- **Generated-code contract:** unchanged. Converted forms are ordinary
  `.cfrm` files; their `.cbl` is generated and regenerated like any other.
  The import programs and the driver are **Common Code** (`src/`), not
  generated code: the developer owns them and may edit them.
- **Docs:** the English Developer's Guide gains a migration section. It
  covers:
  - the dialog;
  - what is carried over and what is not;
  - the control-mapping table;
  - the legacy-data export (R39);
  - the report.

  Appendix A's "do not expect compatibility" caveat is rewritten to match.
  This invalidates the five translations (GOLDEN RULE #8: delete them in the
  same change). The System KB is untouched unless the work adds a property,
  method or event.
- **Fix vs feature:** **feature** (a new IDE capability), on `features`
  through the spec pipeline. The version bump is `z` only. Two related
  pieces of work are **not** part of it:
  - `EXEC SQL` support, which is a separate feature spec;
  - `EXTERNAL` sharing across forms, a separate fix if the R28 measurement
    finds a gap.
- **Dependencies:** one new pure-Rust crate for OLE compound files (e.g.
  `cfb`), license-checked. No C code.
- **Legal:** the `.ppj` format is read for interoperability with the
  developer's own files. The mapping tables and documentation are original
  work. Nothing from Fujitsu's manuals is copied.
- **Risk — undocumented property ids.** `$Recipe` names no properties. The
  per-class property map (R13) must be derived by comparing the samples:
  - change one property in a known form, and see which id moved;
  - cross-check against the generated `.cob` and `.rc`.

  This is the largest piece of work. The plan opens with a spike that
  decodes the classes the samples actually use. The role-based mapping
  (R13 step 2) covers the rest until a sample exists.

## 7. Open questions — settled (operator, 2026-10-03)

- **Q1 — Embedded SQL → keep as marked comments now, and give `EXEC SQL`
  its own spec.** The converter keeps every block verbatim (R27). Adding
  `EXEC SQL` to RustCOBOL is a separate spec, so that a language extension
  is not invented inside a converter.
- **Q2 — Legacy data → a record-sequential export with the FD's exact
  layout,** made with Fujitsu's file utility on the developer's machine.
  It is lossless for `COMP`/`COMP-3` fields (R36, R39).
- **Q3 — Scope of one conversion → the whole application.** A folder of
  `.ppj` merges into one project (R1, R2, R6), with a rule for same-named
  forms that differ (R7).
- **Q4 — Default `ObsoleteScalingStyle` → 7** when the source form declares
  no scaling (R16).
- **Q5 — Controls with no PowerCOBOL equivalent → best-effort native
  mapping.** Anything named or behaving as a table or grid becomes
  `DataGrid`, and so on. A `Panel` with the original name is used only when
  nothing fits (R13).
- **Q6 — `EXTERNAL`/`GLOBAL` data → keep as standard COBOL.** A runtime gap,
  if measured, is a separate fix (R28).
