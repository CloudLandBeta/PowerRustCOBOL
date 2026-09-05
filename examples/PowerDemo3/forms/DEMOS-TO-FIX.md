# Demo catalogue — what did not work, to fix later

Written while building one demo per toolbox control (2026-09-02). Every demo in
this folder passes the same gate the IDE applies to a form — generate → tokenize
(free form) → parse → semantic analysis — plus a member check that refuses any
property or method the runtime does not actually have.

The items below are the ones that did **not** work. Each was verified against
the source, not assumed.

---

## 1. 23 control methods run, but do not parse as an inline call

`is_known_method` in `crates/cobolt-runtime/src/interpreter.rs` is the closed
vocabulary the parser uses to decide whether `Ctrl::Name(...)` is a **call** or a
**collection subscript**. These 23 names are dispatched by `exec_method` /
`exec_member_method` but are missing from that list, so the inline form parses
the parentheses as a subscript and **the call silently does nothing**:

```
ADDMARKER  ADDREGION  ADDROUTE  CANCEL  CLEARREGIONS  CLEARROUTES  DIRECTIONS
DISTANCEMATRIX  GEOCODE  ISBUSY  ISSELECTED  PLACESSEARCH  REMOVEMARKER
REMOVEREGION  REMOVEROUTE  RESULTCOUNT  REVERSEGEOCODE  SEARCH  SETSELECTED
TOPLINK  TOPSNIPPET  TOPTITLE  TRACEROAD
```

**This is already live in a shipped demo.** `Non-Visual/restapi-form.cfrm` writes
`RestClient-1::Cancel(...)` on its Cancel button, so that button is a no-op today.
`Inner-Forms/maps-demo.cfrm` avoids the problem only because it was written with
`INVOKE MAP-1 "AddMarker" USING …` throughout.

**Workaround used in the new demos.** `Non-Visual/websearch-form.cfrm` calls
`INVOKE Web-Find "Search" USING …` and `INVOKE Web-Find "Cancel"` instead of the
inline form, with a comment saying why. Nothing else needed it.

**Fix.** Add the 23 names to `is_known_method`. It is a vocabulary list, not
behaviour: the dispatch arms already exist and are already correct.

**Radio buttons, specifically.** `ISSELECTED` / `SETSELECTED` are in that list,
so `Rad-1::IsSelected()` does not work. Use `Rad-1::IsChecked()` /
`Rad-1::SetChecked()` — `canonical_prop_name` maps `Checked` to `Selected` on a
RadioButton — or read the `Selected` property directly, which is what
`Common/radiobuttons-form.cfrm` does.

---

## 2. Gauge has no events

`ControlType::Gauge.supported_events()` is empty. That reads as deliberate — a
gauge is a reading, and nothing about a reading is a user gesture — so
`Common/gauge-form.cfrm` drives it entirely by writing `Value` from buttons.
Recorded here only so the omission is not mistaken for an oversight later.

`Line`, `Shape`, `Splitter`, `ToolBar` and `StatusBar` are the same: no events of
their own. Their demos drive them from neighbouring buttons or a Timer.

---

## 3. No animated asset exists in this project

`Graphics/animator-form.cfrm` needs an animated GIF, WebP or APNG. Every image
under `Assets/` is a still — the three `.webp` files carry no `ANIM` chunk — so
the demo loads one of them and it plays as a single frame. **Drop a real animated
file into `Assets/` and point `Anm-Hero::Source` at it** to see the control do
what it is for. The handler wiring (`Play` / `Pause` / `StopAnimation`, and the
`onStarted` / `onLooped` / `onEnded` counters) is complete and correct.

---

## 4. `Common/checkboxes-form.cfrm` was left untouched, on purpose

That file holds **no controls and 22 `<DeletedControl>` entries** — it is the
recycle bin of a form whose controls were deleted, and it still carries their
COBOL. Overwriting it would have destroyed that code, so the CheckBox demo was
written to a new file, **`Common/checkbox-form.cfrm`** (singular). Decide what to
do with the old file separately; nothing in this pass depends on it.

---

## Not a defect, but worth knowing

- A `.menu.yaml` is keyed by **control id** and lives beside the `.cfrm`, so two
  forms in the same folder cannot both use `MenuBar-1`. The demos use
  `MenuBar-Demo` and `SideMenu-Demo` for that reason.
- Every demo is `form-format = "Both"`, so each one loads either into its own
  window or into the application shell's ContentPane.
- None of them sets `main_form`; the project's single main form is unchanged.
