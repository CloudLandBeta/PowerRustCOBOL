<!-- powerrustcobol-kit: 1.80.100 -->
# PowerRustCOBOL Form Layout, Events and Hosting

## The layout model — position, nesting, order

A form holds a **flat list** of controls; nesting is derived from each control's
`parent` link rather than stored as a tree. Four pieces of state place a control:

- **`X` / `Y` / `Width` / `Height`** — position and size in points. A control's
  `X`/`Y` are relative to its container (the form, or the parent control).
- **`parent`** — the id of the enclosing container, or none for a direct child
  of the form. A control whose parent is a **TabControl** also carries `tab`,
  the 0-based page it belongs to.
- **`z_order`** — higher is drawn on top; 0 is bottommost; negatives are legal.
- **`tab_order`** — the keyboard traversal sequence.

A form that is **not responsive** (the default for every form that existed
before 1.80) keeps exactly this: a control does not resize with its container.
Geometry is what the designer recorded, and COBOL changes it by writing `X`,
`Y`, `Width` or `Height` at run time.

## Responsive design: anchoring, docking, flex, grid and flow

A form whose **`Responsive design`** switch is on (form property `Responsive`,
`.cfrm` attribute `responsive="true"`) lays its controls out for the surface it
is drawn on — the window, a shell's ContentPane, the preview or the designer
canvas — and reflows when that size changes. The designed rectangles stay the
source of truth; layout is a view of them. Every layout number is a property
with a default, and each is set in the Properties pane's **Layout** section or
from COBOL.

- **`Anchor`** — the parent edges a control follows: any of `Top`, `Bottom`,
  `Left`, `Right` as comma-separated text (default `Top,Left`; a MenuBar
  defaults to `Top,Left,Right`, a StatusBar to `Bottom,Left,Right`). One edge on
  an axis keeps that distance; both stretch the control with its parent; neither
  keeps its centre at the same proportion.
- **`Dock`** — `Left`, `Top`, `Right`, `Bottom` or `Fill`, taken in z-order from
  what remains of the parent; a docked control ignores `Anchor`.
- **`MinWidth`/`MinHeight`/`MaxWidth`/`MaxHeight`** — limits on any stretched,
  docked, grown or grid-sized dimension (0 = none).
- **`LayoutMode`** on the form, a Panel, a GroupBox or a TabControl — `Absolute`
  (anchors and docking, default), `Flex`, `Grid` or `Flow`, with the container
  properties `FlexDirection`, `FlexWrap`, `JustifyContent`, `AlignItems`,
  `AlignContent`, `Gap`, `RowGap`, `ColumnGap`, `GridColumns`, `GridRows`,
  `JustifyItems`, `FlowDirection`, `WrapContents`, and `Padding` with
  `PaddingLeft/Top/Right/Bottom`. Items of such a container use `FlexGrow`,
  `FlexShrink`, `FlexBasis`, `AlignSelf`, `Order`, `GridColumn`, `GridRow`,
  `ColumnSpan`, `RowSpan`, `JustifySelf` and `FlowBreak`.
- **Breakpoints** — named ranges of available width (default `Compact` < 600 ≤
  `Medium` < 1024 ≤ `Expanded`); each non-design breakpoint may override layout,
  visibility, size and `FontSize` per control.
- **Font scaling** — the form's `FontScaling` (`None`, `Fluid`, `Stepped`) with
  `MinFontScale`/`MaxFontScale`; per control `ScaleFont`, `MinFontSize`,
  `MaxFontSize`.
- **Obsolete scaling style** — the form's `ObsoleteScalingStyle` (0–7, default
  0), a compatibility mode for forms migrated from PowerCOBOL. It is a sum of
  flags: 1 resizes each control by the window ratio (`w·rx`, `h·ry`), 2 moves
  it (`x·rx`, `y·ry` from its parent's client), 4 scales fonts by `min(rx, ry)`
  between `MinFontScale` and `MaxFontScale`. Any value other than 0 lays the
  form out even when `Responsive` is off. Breakpoint overrides apply first,
  then scaling; `MinWidth`/`MaxWidth`/`MinHeight`/`MaxHeight` still bound the
  sizes, and a control's children scale within its scaled client. A control
  anchored to other edges than its type's default, or docked, keeps anchoring
  or docking. A program may set it at run time (`me::ObsoleteScalingStyle`);
  a value outside 0–7 is refused and the style is kept. Prefer anchors and
  containers for new forms.

## `Locked` is the design-time lock

`Locked` (Boolean, default false) locks a control's position **against mouse
dragging on the designer canvas**. Keyboard nudges and property-pane entry still
move it, and it has no run-time effect. Before 1.80 this lock was stored as a
boolean `Anchor`; a form saved that way is migrated on load (`Anchor` true or
false becomes `Locked`, and `Anchor` then carries edges). A legacy string such as
`"Top,Left"` is kept as the edge set it always described.

## The form's own geometry and `StartPosition`

A FORM's `X`/`Y` are its window's position on screen — not to be confused with a
CONTROL's `X`/`Y`, which are inside the form.

`StartPosition` decides where the window opens. Accepted values, exactly:
`"System"`, `"Custom"`, `"TopLeft"`, `"TopCenter"`, `"TopRight"`, `"MiddleLeft"`,
`"Center"`, `"MiddleRight"`, `"BottomLeft"`, `"BottomCenter"`, `"BottomRight"`.

- `"System"` (the default) leaves placement to the window manager and **ignores
  `X`/`Y`**.
- `"Custom"` applies the form's `X`/`Y` at launch.
- Every other value computes a position from the real screen and window size at
  launch and ignores `X`/`Y`.

Setting `X`/`Y` alone moves nothing unless `StartPosition` is also `"Custom"`.

## Form events — the complete catalogue

A form supports **57** events, all `on`-prefixed, in these groups. Bind them the
way control events are bound.

Eleven were **retired on 2026-09-06** and are no longer offered: `onPaint`,
`onRepaint` and `onLayout` are per-frame, so a COBOL handler on one would run
about sixty times a second; `onFontChanged`, `onDisplayChanged`,
`onPowerSuspend`, `onPowerResume`, `onSessionLock` and `onSessionUnlock` have no
platform source behind them; and `onClosing` / `onClosed` never reached a form
at all — the ones the host raises are **Snackbar** events carrying a control id,
and the Snackbar keeps them. A form's close path is `onClose` (called after the
event loop) and `onDestroy`. A form saved with a handler on any of these keeps
its code — the inspector lists it under **Retired** so it stays editable — but
nothing will ever call it.

- **Lifecycle** — `onCreate`, `onInitialize`, `onLoad`, `onOpened`, `onShow`,
  `onHide`, `onClose`, `onCloseRejected`, `onDestroy`
- **Activation & Focus** — `onActivate`, `onActivated`, `onDeactivate`,
  `onDeactivated`, `onGotFocus`, `onLostFocus`
- **Window State** — `onResize`, `onResizing`, `onMove`, `onMoving`,
  `onMinimize`, `onMaximize`, `onRestore`, `onFullscreen`, `onExitFullscreen`,
  `onFullScreenChanged`, `onBreakpointChanged`. `onResizing` repeats while the
  window is dragged and `onResize` fires once when it settles — the form's
  counterpart of a control's `onResized`. Both find the new size already in the
  form's `Width` and `Height`, so a handler lays the controls out from those.
  On a responsive form they also find every control's laid-out `X`, `Y`,
  `Width` and `Height`, and `onBreakpointChanged` (read `me::Breakpoint`) fires
  before `onResize` when the size crossed into another breakpoint.
- **Appearance** — `onThemeChanged`, `onDpiChanged`
- **Mouse** — `onClick`, `onDoubleClick`, `onMouseDown`, `onMouseUp`,
  `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`
- **Touch & Pointer** — `onPointerDown`, `onPointerUp`, `onPointerMove`,
  `onPointerEnter`, `onPointerLeave`, `onPointerCancel`, `onGesture`
- **Scrolling** — `onScroll`, `onScrollStart`, `onScrollEnd`,
  `onHorizontalScroll`, `onVerticalScroll`
- **Drag & Drop** — `onDragEnter`, `onDragLeave`, `onDragOver`, `onDrop`
- **Clipboard** — `onCut`, `onCopy`, `onPaste`
- **System / OS** — `onSystemColorChanged`
- **Error Handling** — `onUnhandledException`

`onCloseRejected` fires when a close attempt was refused because the form (or a
synchronous child of it) is `Waiting`. `onFullScreenChanged` fires when the
actual fullscreen state changed in either direction — read `me`'s `FullScreen`
for the new value.

## `onUnhandledException` — the form keeps running

A COBOL failure inside an event handler does **not** end the form. The failing
handler is abandoned, and the event loop carries on with the next event.

- Bind **`onUnhandledException`** and it is called, with the details published
  on the form as **`LastException`** — read `me::LastException`.
- Bind nothing and the operator sees a **critical notification** instead:
  *"A critical exception has occurred: &lt;details&gt;. Implement the event handler
  onUnhandledException to get better control over the exception."* It never
  expires and carries the built-in ✕, and it needs no Snackbar on the form —
  a form that has not thought about errors is exactly the one without one.

**An unguarded size error is an exception.** `COMPUTE` / `ADD` / `SUBTRACT` /
`MULTIPLY` / `DIVIDE` raise the SIZE ERROR condition when the result will not
fit, division by zero included. With `ON SIZE ERROR` declared it is the
developer's to handle and nothing else happens. With no phrase, nobody is
handling it, so the statement raises an exception — catchable by an enclosing
`TRY … CATCH`, and otherwise delivered to `onUnhandledException`.

An exception raised **inside** `onUnhandledException` is not handed back to it;
it is reported like any other failure.

This is forms only. A console program with no window still fails to its caller.

## Which teardown event to use: `onDeactivate` or `onDestroy`

These two are constantly confused, and using the wrong one closes files that are
still in use — or leaks the ones that are not.

- **`onDeactivate`** — the form's body left the ContentPane but the form is
  **still resident**: it became an ancestor in the navigation chain, or it was
  parked by *Preserve previous form*. Its storage and its handlers stay live.
  **Do not close files here.**
- **`onDestroy`** — the form's storage is about to be released. This is the
  teardown point: close files, `COMMIT`, free resources. It is **never** fired
  for a mere swap-out.

## Form hosting — `Standalone`, `Embedded`, `Both`

The form-level `FormFormat` property decides how a form may be loaded:

- **`Standalone`** (default, and what every older `.cfrm` reads as) — its own OS
  window, reached by `OpenFormSync` / `OpenFormAsync`.
- **`Embedded`** — loaded into the application shell's ContentPane by a menu
  item.
- **`Both`** — valid on either path: a reusable lookup screen that is a modal
  dialog in one place and a pane occupant in another.

A menu item may load only a form that allows `Embedded`; `OpenFormSync` /
`OpenFormAsync` may open only one that allows `Standalone`. Anything
unrecognised in the file reads as `Standalone`, so a hand-edited form never
fails to load over this field.

## The application shell — one window instead of many

Placing a **SideMenu** control on the **main form** is the entire switch that
turns an application into a shell: one window divided into a menu pane, a
breadcrumb, and a ContentPane where forms load in place.

- Main form with a SideMenu → shell mode.
- No SideMenu — including a form with a classic `MenuBar` → every form opens in
  its own window, exactly as before.

An existing project cannot become a shell application by accident. The sidebar
is filled with the **same menu editor a `MenuBar` uses** (select it, then *Edit
Menu…*), because the menu is stored in a sidecar keyed by the control, not by
the kind of control.

The **breadcrumb is the navigation chain**: every form on it is still resident
and its handlers still fire while its body is not displayed. Clicking a segment
destroys everything below it, deepest first, and shows that form again. Per
menu item, *Preserve previous form* decides whether a sibling switch destroys
the form being left or keeps it resident for an instant return.

## `me` and `super` — addressing a form from COBOL

`me` addresses the current form. **`super`** addresses the form that loaded or
opened it, on both paths — a menu load and `OpenFormSync` / `OpenFormAsync`.

```cobol
           MOVE super::Title TO WS-T.
           MOVE "Processing..." TO super::Title.
           INVOKE super::"SetWindowState"("Minimized").
           MOVE super::super::Title TO WS-T.
           super::SIDE-1::Collapse().
           super::SIDE-1::Open().
```

- **Bare properties are checked at build time** against the universal form
  surface: `Name`, `Title`, `Width`, `Height`, `X`, `Y`, `WindowState`,
  `FullScreen`, `TitleVisible`, `CanMinimize`, `CanMaximize`, `FormState`,
  `FormFormat`, `BackgroundColor`, `Transparency`. A typo such as `super::Widht`
  fails the build at any depth.
- **Writing them changes the window**: `MOVE "Processing..." TO super::Title`
  retitles the opener's window; `BackgroundColor`/`Transparency` repaint its
  backdrop, `Width`/`Height` resize it and `X`/`Y` move it (a shell
  application's window belongs to the shell: there only the backdrop changes).
- **Form-specific procedures use parentheses** — `INVOKE super::"RecalcTotals"()` —
  and run one of the PARENT's own procedures (a paragraph or user procedure of
  its program, as `CALL "RecalcTotals"` would there). The call is answered at
  once and the procedure runs when the parent next waits for an event — so it
  returns nothing, and its effects are seen by the parent, not by the caller.
  Use it to tell the form that loaded you that something changed (a settings
  pane asking the main form to re-check its menu). A name the parent has no
  procedure for is reported in its program output and nothing runs.
- **`super` can be NULL**: in the main form, and in an async-opened form whose
  opener has closed (a child never keeps its opener alive). Referencing a NULL
  `super` raises the standard runtime error.
- Each opened form runs as its **own program with its own WORKING-STORAGE**.
  Forms never read each other's data items; they talk through published form
  properties, `super::X`, and windowHandler methods.
