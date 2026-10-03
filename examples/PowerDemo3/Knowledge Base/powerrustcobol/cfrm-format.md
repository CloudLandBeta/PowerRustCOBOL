<!-- powerrustcobol-kit: 1.80.100 -->
# The `.cfrm` form file

A form is one XML file in `forms/`, UTF-8, that the IDE's Form Designer reads and writes and
from which PowerRustCOBOL generates the form's COBOL program. You may write it directly; then
call `validate` (it must load), `add_to_project` (a new form), `regenerate` and `check`.

## The `Form` element

The root element. Its attributes:

- `name` — the form's id: a COBOL word, which becomes the generated `PROGRAM-ID`.
- `title` — the window title. `width`, `height` — the form's size in pixels.
- `background` — the background colour, `RRGGBBAA` hex. `transparency` — 0 (opaque) to 100.
- `background-gradient-enabled`, `background-gradient-start`, `background-gradient-end`,
  `background-gradient-direction` — an optional background gradient.
- `background-image`, `bg-image-mode` — an optional background image and how it is scaled.
- `grid-size`, `snap-to-grid` — the designer's grid. `target` — the designer's target device.
- `theme`, `use-theme-background`, `glass-style`, `control-style` — the form's look.
- `main-form` — `true` on exactly **one** form of the project: the form a built application
  starts. Never put it on a second form.
- `taskbar-icon`, `can-minimize`, `can-maximize`, `window-state`, `full-screen`,
  `title-visible`, `start-position`, `x`, `y`, `window-effects`, `modal-overlay-style`,
  `form-format`, `responsive` — window behaviour; the properties of the same names in
  `form-layout-and-events.md` and `controls.md` say what each value means.

An attribute that is left out takes its default; the IDE writes many of them only when they
differ from it.

## Blocks of COBOL the form owns

Each holds COBOL source in a CDATA section and is woven into the generated program:

- `working-storage` — the form's own data items. A handler sees one only when it is
  declared `GLOBAL`.
- `special-names`, `repository`, `file-control`, `file-section` — the form's
  SPECIAL-NAMES, REPOSITORY, FILE-CONTROL (`SELECT`) and FILE SECTION (`FD`, declared
  `IS GLOBAL` when a handler uses the file).
- `user-procedures` — procedures you write for the form, each an `Event` element whose
  `name` and `paragraph` are the procedure's name; callable with `CALL "<name>"`.
- `form-events` — the form's own events (`onLoad`, `onClose`, …), each an `Event` element.

## The `Control` element

One per control, in the order they are painted. Attributes:

- `id` — the control's id: a COBOL word, unique in the form.
- `type` — the control type, exactly as `controls.md` names it (`Button`, `TextBox`, …).
- `x`, `y`, `w`, `h` — position and size in pixels, in **form** coordinates — also for a
  control inside a container, whose rectangle lies inside the container's.
- `tab-order` — keyboard order. `z-order` — stacking. `visible`, `enabled` — `true`/`false`.
- `parent` — the id of the container (GroupBox, Panel, TabControl, Splitter, …) the control
  is inside. **Membership is this attribute, never the geometry.**
- `tab` — inside a TabControl, the 0-based page the control sits on.

Inside a `Control`:

- `Property` — one property: its `name` attribute and its value as text. Use only the names
  `controls.md` lists for that type; a property left out takes its default.
- `Event` — one bound handler: `name` is the event (`onClick`) and `paragraph` the handler's
  program name, `<CONTROL-ID>--<EVENT>` upper-cased (`BTN-SAVE--ONCLICK`). Its CDATA body is the
  handler's source: `ENVIRONMENT DIVISION.`, `DATA DIVISION.` (with the handler's own
  `WORKING-STORAGE SECTION.` when it needs one) and `PROCEDURE DIVISION.` with the statements.
  An event with no `Event` element runs nothing.
- `Animation` — a designer-made animation (`name`, `trigger`, `kind`, `duration`, `delay`,
  `easing`, `repeat`, `repeat-count`, `repeat-delay`, `slide-dx`, `slide-dy`).
- `Children` — an older nesting form the loader still reads; the IDE writes every control
  flat, with `parent`.

## Other elements the IDE writes

- `DataBindings` (attribute `schema-version`) — data bindings, as JSON in CDATA. Edit them in
  the IDE.
- `MenuPaneBackground` (`color`, `gradient-enabled`, `gradient-start`, `gradient-end`,
  `gradient-direction`, `transparency`, `image`, `image-mode`) — the shell sidebar's background.
- `FormLayout` and `Breakpoints` / `Breakpoint` (`name`, `min-width`, `font-factor`) /
  `Override` (`control`, `property`) — responsive layout.
- `deleted-controls` / `DeletedControl` (`id`, `deleted-at`) — handler code of deleted controls,
  kept and never compiled. Leave it alone.

## A live example

Serialised by the same code the IDE saves forms with (a Panel holding a Label and a TextBox, and
a Button whose `onClick` is bound):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Form name="CUSTOMER-FORM" title="Customers" width="480" height="280" background="#2E3138FF" transparency="0" grid-size="8" snap-to-grid="true" target="Custom">
  <working-storage><![CDATA[       01 WS-NAME         GLOBAL PIC X(40).
]]></working-storage>
  <repository><![CDATA[           CLASS RUST-BOOL IS "Rust.bool"
           CLASS RUST-CHAR IS "Rust.char"
           CLASS RUST-I8 IS "Rust.i8"
           CLASS RUST-I16 IS "Rust.i16"
           CLASS RUST-I32 IS "Rust.i32"
           CLASS RUST-I64 IS "Rust.i64"
           CLASS RUST-I128 IS "Rust.i128"
           CLASS RUST-ISIZE IS "Rust.isize"
           CLASS RUST-U8 IS "Rust.u8"
           CLASS RUST-U16 IS "Rust.u16"
           CLASS RUST-U32 IS "Rust.u32"
           CLASS RUST-U64 IS "Rust.u64"
           CLASS RUST-U128 IS "Rust.u128"
           CLASS RUST-USIZE IS "Rust.usize"
           CLASS RUST-F32 IS "Rust.f32"
           CLASS RUST-F64 IS "Rust.f64"
           CLASS RUST-STR IS "Rust.str"
           CLASS RUST-UNIT IS "Rust.unit"
           CLASS RUST-STRING IS "Rust.String"
           CLASS RUST-OSSTRING IS "Rust.OsString"
           CLASS RUST-OSSTR IS "Rust.OsStr"
           CLASS RUST-CSTRING IS "Rust.CString"
           CLASS RUST-CSTR IS "Rust.CStr"
           CLASS RUST-PATH IS "Rust.Path"
           CLASS RUST-PATHBUF IS "Rust.PathBuf"
           CLASS RUST-VEC IS "Rust.Vec"
           CLASS RUST-VECDEQUE IS "Rust.VecDeque"
           CLASS RUST-LINKEDLIST IS "Rust.LinkedList"
           CLASS RUST-HASHMAP IS "Rust.HashMap"
           CLASS RUST-BTREEMAP IS "Rust.BTreeMap"
           CLASS RUST-HASHSET IS "Rust.HashSet"
           CLASS RUST-BTREESET IS "Rust.BTreeSet"
           CLASS RUST-BINARYHEAP IS "Rust.BinaryHeap"
           CLASS RUST-OPTION IS "Rust.Option"
           CLASS RUST-RESULT IS "Rust.Result"
           CLASS RUST-BOX IS "Rust.Box"
           CLASS RUST-RC IS "Rust.Rc"
           CLASS RUST-ARC IS "Rust.Arc"
           CLASS RUST-WEAK IS "Rust.Weak"
           CLASS RUST-CELL IS "Rust.Cell"
           CLASS RUST-REFCELL IS "Rust.RefCell"
           CLASS RUST-MUTEX IS "Rust.Mutex"
           CLASS RUST-RWLOCK IS "Rust.RwLock"
           CLASS RUST-COW IS "Rust.Cow"
           CLASS RUST-DURATION IS "Rust.Duration"
           CLASS RUST-INSTANT IS "Rust.Instant"
           CLASS RUST-SYSTEMTIME IS "Rust.SystemTime"
           CLASS RUST-RANGE IS "Rust.Range"]]></repository>
  <form-events>
    <Event name="onLoad" paragraph="CUSTOMER-FORM--ONLOAD"><![CDATA[       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE SPACES TO WS-NAME
]]></Event>
    <Event name="onClose" paragraph="CUSTOMER-FORM--ONCLOSE">
    </Event>
  </form-events>
  <Control id="PNL-DETAILS" type="Panel" x="16" y="16" w="448" h="176" tab-order="0" z-order="0" visible="true" enabled="true">
  </Control>
  <Control id="LBL-NAME" type="Label" x="32" y="40" w="120" h="24" tab-order="0" z-order="0" visible="true" enabled="true" parent="PNL-DETAILS">
    <Property name="Caption">Name</Property>
  </Control>
  <Control id="TXT-NAME" type="TextBox" x="160" y="40" w="280" h="28" tab-order="0" z-order="0" visible="true" enabled="true" parent="PNL-DETAILS">
    <Property name="Text"></Property>
  </Control>
  <Control id="BTN-SAVE" type="Button" x="352" y="216" w="112" h="32" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Save</Property>
    <Event name="onClick" paragraph="BTN-SAVE--ONCLICK"><![CDATA[       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE TXT-NAME::Text TO WS-NAME
           SET LBL-NAME::Caption TO "Saved"
]]></Event>
  </Control>
</Form>
```
