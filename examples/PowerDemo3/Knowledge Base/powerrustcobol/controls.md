<!-- powerrustcobol-kit: 1.80.100 -->
# PowerRustCOBOL Form Controls Reference

Complete reference for every control the RAD Form Designer supports: purpose, properties with value types / defaults / allowed domains, events, and inline methods. Properties are read as `<control>::<Property>` and written with `SET <control>::<Property> TO <value>` (or the designer op `set_property`). All properties are OPTIONAL — a control works with its defaults; set only what the request needs. Property names are case-insensitive at runtime but SHOULD be written exactly as listed. Setting a misspelled property silently creates a new, ignored property — it is never an error, so spelling matters.

**Control ids are COBOL words.** Each id becomes part of the generated program's data-names and paragraph names (`WS-<id>-TEXT`, `<id>-OPEN`), so it may contain ONLY letters, digits and hyphens, and may neither begin nor end with a hyphen. Write `TEXTBOX-1`, never `TEXTBOX_1`: an underscore is not a COBOL character, and a name carrying one is discarded by the compiler along with the control's whole storage. The same rule governs every WORKING-STORAGE item and paragraph name a handler declares.

## Universal properties (every control)

Layout fields (settable like any property):

- `Name` (String — control identifier) — The control id (assigned by the designer; treat as read-only). It becomes a COBOL word in the generated program (`WS-<id>-TEXT`, `<id>-OPEN`), so it may hold ONLY letters, digits and hyphens — `TEXTBOX-1`, never `TEXTBOX_1`.
- `Visible` (Boolean — `1`/`0`) — Whether the control is drawn. Hiding a CONTAINER hides everything inside it: a GroupBox, Panel, TabControl or Splitter pane that is not drawn has no inside to draw into, so its children go with it and come back with it. The children's own `Visible` is untouched — showing the container again restores exactly what was showing before.
- `Enabled` (Boolean — `1`/`0`) — Whether the control accepts input. Disabling a CONTAINER disables everything inside it: a GroupBox, Panel, Splitter or TabControl set to 0 stops its children taking clicks and keystrokes, and switching it back to 1 brings them back. The children's own `Enabled` is never written, so a control you disabled in its own right stays disabled when the container returns — enabling a group restores what it was, not everything in it.
- `X` (Integer — pixels from the form's left edge) — Horizontal position.
- `Y` (Integer — pixels from the form's top edge) — Vertical position.
- `Width` (Integer — pixels > 0) — Control width.
- `Height` (Integer — pixels > 0) — Control height.
- `TabOrder` (Integer ≥ 0) — Keyboard Tab traversal order: Tab goes up the numbers, Shift+Tab down, and both wrap; equal numbers go in the order the form is painted. Only visible, enabled controls that can take the keyboard are visited. A Label holds a place too but never keeps the focus: when Tab, Enter or a click reaches it, it raises `onGotFocus` (for a screen reader, say) and the focus walks straight on to the next control.
- `Parent` (String — container control id or empty) — The container that owns this control.
- `Tab` (Integer — 0-based tab page index) — Which TabControl page the control sits on (only inside a TabControl).

Appearance and behaviour properties shared by every control:

- `BackgroundColor` (String, default "#F0F0F0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Fill color behind the control's content.
- `BackgroundGradientEnabled` (Boolean, default 0; `1` (true) or `0` (false)) — Enables the two-color background gradient.
- `BackgroundGradientStartColor` (String, default "#F0F0F0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Gradient start color.
- `BackgroundGradientEndColor` (String, default "#C8D0DC"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Gradient end color.
- `BackgroundGradientDirection` (String, default "South"; one of: `North` | `NorthEast` | `East` | `SouthEast` | `South` | `SouthWest` | `West` | `NorthWest`) — Direction the gradient flows toward.
- `ForegroundColor` (String, default "#FFFFFF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Text / foreground drawing color. On a CheckBox, RadioButton or DateTimePicker it is kept only while it reads on the surface the text lands on, and otherwise flips to black or white — measured against the control's FRAME (its BackgroundColor), never against the tick box or circle, which the caption sits beside rather than on. Above Transparency 70 the frame paints too little to measure, so the color is used exactly as set; a CheckBox is 100 % transparent by default, so its caption color is always the one you gave it.
- `FontName` (String, default "Arial"; installed font family name, e.g. `"Arial"`) — Font family for the control's text.
- `FontSize` (Integer, default 14; points, > 0 (typical 8-72)) — Font size in points.
- `Bold` (Boolean, default 0; `1` (true) or `0` (false)) — Bold text: the caption, list items, grid cells and GroupBox legend; a TextBox being edited uses the font's real bold face when the system has one.
- `Italic` (Boolean, default 0; `1` (true) or `0` (false)) — Italic text.
- `Underline` (Boolean, default 0; `1` (true) or `0` (false)) — Underlined text.
- `Strikethrough` (Boolean, default 0; `1` (true) or `0` (false)) — Struck-through text.
- `Tooltip` (String, default (empty); free text) — Text shown in a small pop-up while the pointer rests on the control in the running form, after its HoverDelayMs (empty = no tooltip). Every visual control shows it.
- `Cursor` (String, default "Default"; one of: `Default` | `Hand` | `Text` | `Wait` | `Crosshair` | `No` | `SizeAll` | `SizeNS` | `SizeWE`) — Mouse cursor shown while hovering the control.
- `HoverDelayMs` (Integer, default 200; milliseconds ≥ 0) — How long the pointer must rest before `onHoverEnter` fires.
- `Locked` (Boolean, default 0; `1` (true) or `0` (false)) — Locks the control against mouse dragging on the design canvas; keyboard and Properties-pane entry still move it. No run-time effect. (Stored as a boolean `Anchor` before 1.80; migrated on load.)
- `Anchor` (String, default "Top,Left"; comma-separated edges from `Top` | `Bottom` | `Left` | `Right` (default `Top,Left`; MenuBar `Top,Left,Right`, StatusBar `Bottom,Left,Right`)) — Responsive design: the parent edges the control follows. One edge on an axis keeps that distance, both stretch the control with its parent, neither keeps its centre at the same proportion. Ignored while the form is not responsive, and ignored by a docked control or an item of a Flex/Grid/Flow container.
- `Dock` (String, default "None"; one of: `None` | `Left` | `Top` | `Right` | `Bottom` | `Fill` (default `None`)) — Responsive design: claims one edge of the parent's remaining area, in z-order, at the control's designed thickness; `Fill` takes what remains. A docked control ignores `Anchor`.
- `MinWidth` (Integer, default 0; form pixels ≥ 0 (0 = no limit, default)) — Responsive design: the smallest width a stretched, docked, grown or grid-sized control may take.
- `MinHeight` (Integer, default 0; form pixels ≥ 0 (0 = no limit, default)) — Responsive design: the smallest height a stretched, docked, grown or grid-sized control may take.
- `MaxWidth` (Integer, default 0; form pixels ≥ 0 (0 = no limit, default)) — Responsive design: the largest width a stretched, docked, grown or grid-sized control may take. A `Left,Right` control at its MaxWidth stays attached to Left.
- `MaxHeight` (Integer, default 0; form pixels ≥ 0 (0 = no limit, default)) — Responsive design: the largest height a stretched, docked, grown or grid-sized control may take.
- `FlexGrow` (String, default "0"; decimal ≥ 0 (default 0)) — Flex item: its share of the container's free space on the main axis (CSS `flex-grow`).
- `FlexShrink` (String, default "1"; decimal ≥ 0 (default 1)) — Flex item: how much it gives up, in proportion to its basis, when the items overflow (CSS `flex-shrink`).
- `FlexBasis` (String, default "Auto"; `Auto` (default, the designed size), pixels, or a percentage) — Flex item: its starting size on the main axis (CSS `flex-basis`).
- `AlignSelf` (String, default "Auto"; one of: `Auto` | `Stretch` | `Start` | `Center` | `End` (default `Auto`)) — Flex or grid item: overrides the container's `AlignItems` for this item.
- `Order` (Integer, default 0; integer (default 0)) — Flex or flow item: its position among its siblings; ties keep the designed reading order.
- `GridColumn` (Integer, default 0; integer ≥ 0, 1-based (0 = auto-placed, default)) — Grid item: the column it starts in.
- `GridRow` (Integer, default 0; integer ≥ 0, 1-based (0 = auto-placed, default)) — Grid item: the row it starts in.
- `ColumnSpan` (Integer, default 1; integer ≥ 1 (default 1)) — Grid item: how many columns it covers.
- `RowSpan` (Integer, default 1; integer ≥ 1 (default 1)) — Grid item: how many rows it covers.
- `JustifySelf` (String, default "Auto"; one of: `Auto` | `Stretch` | `Start` | `Center` | `End` (default `Auto`)) — Grid item: overrides the container's `JustifyItems` for this item.
- `FlowBreak` (Boolean, default 0; `1` (true) or `0` (false)) — Flow item: ends the line after this control (default 0).
- `ScaleFont` (Boolean, default 1; `1` (true) or `0` (false)) — Whether the form's font scaling resizes this control's text (default 1). A COBOL read of `FontSize` always returns the designed size.
- `MinFontSize` (Integer, default 0; points ≥ 0 (0 = no limit, default)) — The smallest size font scaling may shrink this control's text to.
- `MaxFontSize` (Integer, default 0; points ≥ 0 (0 = no limit, default)) — The largest size font scaling may grow this control's text to.
- `Padding` (Integer, default 0; points, 0-128) — Extra space between the control's frame and its content: a caption moves away from the edges (Button, Label, CheckBox, RadioButton and other captioned controls), and a TextBox adds it to its InnerPadding.
- `PaddingLeft` (String, default (empty); form pixels, or empty (default) to use `Padding`) — One side's padding inside a container's client area, when it should differ from `Padding`.
- `PaddingTop` (String, default (empty); form pixels, or empty (default) to use `Padding`) — One side's padding inside a container's client area, when it should differ from `Padding`.
- `PaddingRight` (String, default (empty); form pixels, or empty (default) to use `Padding`) — One side's padding inside a container's client area, when it should differ from `Padding`.
- `PaddingBottom` (String, default (empty); form pixels, or empty (default) to use `Padding`) — One side's padding inside a container's client area, when it should differ from `Padding`.
- `Transparency` (Integer, default 0; 0-100 (percent)) — How much of what is behind the control shows through; 0 = opaque, 100 = the control's own face is not painted and the form (or the control underneath) shows in full. Replaces the former Opacity, which ran the other way round. A CheckBox defaults to 100.
- `ShadowEnabled` (Boolean, default 0; `1` (true) or `0` (false)) — Enables the drop shadow.
- `ShadowOpacity` (Integer, default 6; 0-100 (percent)) — Drop-shadow opacity.
- `ShadowColor` (String, default "#000000"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Dark shadow color.
- `ShadowLightColor` (String, default "#FFFFFFFF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Light (highlight) shadow color for neumorphic styles.
- `ShadowDirection` (String, default "SouthEast"; one of: `North` | `NorthEast` | `East` | `SouthEast` | `South` | `SouthWest` | `West` | `NorthWest`) — Direction the shadow is cast toward.
- `ShadowDistance` (Integer, default 7; pixels ≥ 0) — Shadow offset distance.
- `ShadowBlur` (Boolean, default 1; `1` (true) or `0` (false)) — Enables soft blur falloff on the shadow.
- `ShadowBlurStrength` (Integer, default 8; -20..20, default 8) — Blur radius of the shadow, in layers. A negative value draws the shadow INSIDE the frame (sunken / inset) instead of outside it.
- `ZOrder` (Integer, default 0; any integer; higher paints in front) — Stacking order among siblings. Changed at run time by SET ctl::ZOrder, BringToFront (10000) or SendToBack (-10000), and the form redraws in the new order.

## Universal events (visual controls)

Most visual controls support this shared input/lifecycle set (each control section lists which of them apply):

- `onClick` — left click released on the control
- `onDblClick / onDoubleClick` — double click (aliases)
- `onRightClick` — right click
- `onMiddleClick` — middle click
- `onMouseEnter / onMouseLeave` — pointer entered / left the control
- `onMouseDown / onMouseUp` — button pressed / released
- `onMouseMove` — pointer moved over the control
- `onMouseWheel` — wheel scrolled over the control
- `onContextMenu` — context-menu request (right click)
- `onGotFocus / onLostFocus` — keyboard focus gained / lost
- `onKeyDown / onKeyUp / onKeyPress` — keyboard input while focused
- `onEnterPressed / onEscapePressed` — Enter / Escape pressed while focused
- `onHoverEnter / onHoverLeave` — pointer rested ≥ `HoverDelayMs` / left after hovering
- `onResize / onResized / onMove / onMoved` — geometry changed
- `onVisibleChanged / onEnabledChanged` — Visible / Enabled flipped
- `onLoad` — control initialised when the form opens

Event handlers carry NO parameters. A handler is a nested program, and the dispatcher calls it as `CALL "<handler-program>"` with no arguments, so its `LINKAGE SECTION` is empty and its header is a plain `PROCEDURE DIVISION.`. The single exception is a control inside a REPEATING GROUP, which is called `USING CONTROL-ARRAY-INDEX` (`PIC S9(4) COMP-5`, the 1-based index of the card that fired) and writes `PROCEDURE DIVISION USING CONTROL-ARRAY-INDEX.`.

The OTHER is a TreeView NODE event, called `USING CONTROL-NODE-DATA` — a group of `CONTROL-NODE` (the label), `CONTROL-NODE-INDEX` and `CONTROL-NODE-LEVEL` (both 1-based) and `CONTROL-NODE-CHECKED` (`1`/`0`). It is how a handler knows which node was clicked, checked, folded or unfolded. `onNodeDrop` (a tree with `AllowDrag`) carries the same group with two more items after them: `05 CONTROL-TARGET-INDEX PIC S9(4) COMP-5` (the node it was dropped on, 0 for empty space) and `05 CONTROL-TARGET-NODE PIC X(256)` (that node's label).

In particular **no event delivers a key code**: never declare `KEY-CODE` or write `PROCEDURE DIVISION USING KEY-CODE.` — nothing populates it. `onKeyDown`, `onKeyUp` and `onKeyPress` fire for ANY key and say nothing about which one, so a specific key has its own event: bind `onEnterPressed` for ENTER and `onEscapePressed` for ESC. To see what was typed, read the control's own text (`MOVE MY-BOX::Text TO WS-VALUE`).

## The Form (window)

Form-level designer attributes: `title` (String), `width`/`height` (Integer px), `background_color` (hex color), optional background gradient (enabled/start/end/direction as in the universal gradient properties), `transparency` (0-100, 0 = opaque), `background_image` (path) with scale mode, and `GlassStyle` (exactly one of `"Classic"`, `"Enhanced"`, `"Neumorphic Light"`, `"Neumorphic Dark"`).

A running window the user maximizes or drags BIGGER keeps its controls at the designed size and stretches only the BACKGROUND — the gradient, or the background image, covers the whole window instead of stopping at the form's edge. A window dragged SMALLER than the form keeps a form-sized background (the form scrolls inside it) rather than cropping it to the window. The designer canvas and the preview always show the backdrop at the form's own size.

Form events (bind a handler in the designer; `onLoad` / `onClose` are pre-stubbed):

- Lifecycle: `onCreate`, `onInitialize`, `onLoad`, `onOpened`, `onShow`, `onHide`, `onClose`, `onCloseRejected`, `onDestroy`
- Activation & Focus: `onActivate`, `onActivated`, `onDeactivate`, `onDeactivated`, `onGotFocus`, `onLostFocus`
- Window State: `onResize`, `onResizing`, `onMove`, `onMoving`, `onMinimize`, `onMaximize`, `onRestore`, `onFullscreen`, `onExitFullscreen`, `onFullScreenChanged`, `onBreakpointChanged`
- Appearance: `onThemeChanged`, `onDpiChanged`
- Mouse: `onClick`, `onDoubleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`
- Touch & Pointer: `onPointerDown`, `onPointerUp`, `onPointerMove`, `onPointerEnter`, `onPointerLeave`, `onPointerCancel`, `onGesture`
- Scrolling: `onScroll`, `onScrollStart`, `onScrollEnd`, `onHorizontalScroll`, `onVerticalScroll`
- Drag & Drop: `onDragEnter`, `onDragLeave`, `onDragOver`, `onDrop`
- Clipboard: `onCut`, `onCopy`, `onPaste`
- System / OS: `onSystemColorChanged`
- Error Handling: `onUnhandledException`

Not every form event is wired into the runtime yet — prefer `onLoad` and `onClose` for lifecycle logic. Wired since spec 037: `onCloseRejected` (a close attempt was refused while `FormState` is `Waiting`, or a Sync child is Waiting) and `onFullScreenChanged` (the ACTUAL fullscreen state changed — read `me`'s `FullScreen` for the new value; fires once per real transition).

### Main form & window lifecycle (spec 037)

Window-lifecycle designer attributes on every form: `MainForm` (Boolean — exactly ONE form per project holds it; the first form created is the default; the Forms tree marks it with a crown. It is also the only form a RUNTIME starts: a built binary and `rcrun run-form` open the main form and nothing else, so a sign-on main form cannot be stepped over. The IDE's Run Form still runs any form. A project whose designation has been edited by hand — the mark moved, `[forms] main-form` pointed elsewhere, the seal removed — reports a corrupted application and exits without opening a window), `TaskbarIcon` (image path — main form only: the single taskbar/dock entry uses it; non-main windows create no taskbar entries), `CanMinimize` / `CanMaximize` (Boolean, default true — native title-bar buttons), `WindowState` (`"Normal"` | `"Minimized"` | `"Maximized"` — the state the window opens in, settable at runtime), `FullScreen` (Boolean — orthogonal to WindowState; leaving fullscreen returns to the previous state), and `TitleVisible` (Boolean, default true — false renders a chromeless window).

Where the window properties apply: the main window, a SHELL application's window (a main form carrying a SideMenu — CanMinimize, CanMaximize, TitleVisible, FullScreen, WindowState, StartPosition/X/Y and TaskbarIcon are all honoured there too), and a CHILD window opened with OpenFormSync/OpenFormAsync or a Stand Alone menu action (CanMinimize, CanMaximize, FullScreen, WindowState, and StartPosition/X/Y when the caller passes no position). Entrance/exit effects and a see-through `Transparency` are the main window's only. `TaskbarIcon` is resolved against the application's folder, like every asset, so a project-relative icon works however the program is launched. `Transparency` (0-100) on the main window shows the DESKTOP through the form: the window is created carrying alpha whenever it is above 0. `BackgroundColor` `#000000FF` paints black (only `#00000000`, a 6-digit black or empty mean 'unset', the default dark blue), and a colour's own alpha multiplies with `Transparency`.

Writing the form's own properties at run time — `me::X`, or `super::X` from a form it opened — changes the running window: `Title` retitles it, `BackgroundColor` and `Transparency` repaint the backdrop, `Width`/`Height` resize it (64 to 8192) and `X`/`Y` move it. In a shell application the shell owns the window, so there only the backdrop changes.

`FormState` (`"Ready"` | `"Waiting"`, runtime-only, default Ready) guards unsaved work: while `Waiting`, EVERY close attempt on the form is refused (title-bar close, windowHandler Close, cascades) and `onCloseRejected` fires; set it back to `Ready` to allow closing. A Sync caller is also blocked while any of its Sync children is Waiting. Set it with `INVOKE me "SetProperty" USING "FormState" "Waiting"`.

Opening forms from COBOL — two methods on `me`, two syntaxes:

- Comma form (trailing parameters OPTIONAL, defaulted from the target form's design): `INVOKE me::"OpenFormSync"("FORM-ID", [windowState], [x], [y], [width], [height], [modal]) RETURNING H` — `modal` defaults to TRUE. `INVOKE me::"OpenFormAsync"("FORM-ID", [windowState], [x], [y], [width], [height]) RETURNING H` — Async is never modal.
- COBOL-standard space form (ALL parameters required — a mismatch is a compile error): `INVOKE me "OpenFormSync" USING form-id windowState x y width height modal RETURNING H`.

`H` is a `windowHandler` (USAGE OBJECT). Methods on the handle: `Close`, `Focus` (restores a minimized window first), `SetWindowState(state)`, `SetFullScreen(bool)`, `SetTitleVisible(bool)`; read `H::FormState` for the child's state. When a form closes, every windowHandler referring to it becomes NULL automatically; invoking through a NULL handle is a runtime error.

Lifecycle rules: the MAIN form is a singleton — opening it again focuses the running instance and returns its existing handle; other forms can run any number of concurrent instances. Sync children close together with their caller (a Waiting form anywhere in the chain vetoes the whole close). Async children survive their caller — except when the MAIN form closes, which closes every form and exits the application. A modal Sync child blocks the caller's input AND its COBOL flow until the child closes (the RETURNING handle is then already NULL). `me` window methods: `SetWindowState`, `SetFullScreen`, `SetTitleVisible`, `Focus`, `Close`. `me` also carries `SetBreadcrumbDetail(text)` / `ClearBreadcrumbDetail()`, which name the record the form is holding in the shell's breadcrumb — not window methods: an embedded form has no window of its own and still owns the crumb after its own name.

### Application shell & the `super` receiver (spec 049)

`FormFormat` (`"Standalone"` | `"Embedded"` | `"Both"`, default Standalone) declares how a form may be loaded: Standalone opens as its own window (`OpenFormSync`/`OpenFormAsync`); Embedded is loaded into the shell's ContentPane by a sidebar-menu item; Both allows either path. The build REJECTS a menu item that targets a Standalone form and an OpenForm* call that targets an Embedded one. The MAIN form is always Standalone (it owns the window). While a form is Embedded, the window-only properties (WindowState, FullScreen, TitleVisible, CanMinimize, CanMaximize) are inert and its Width/Height report the DESIGNED values.

`ModalOverlayStyle` (`"None"` | `"SemiTransparent"` | `"Greyed"`, default None) controls how THIS form's own face looks while a Sync-opened (modal) child of its own blocks it: a real child window, or — since a ContentPane occupant has no window of its own — a modal child that occupant opened, which blocks the shell (rail, breadcrumb and pane) underneath it the same way. The BEHAVIOUR of the block never depends on this value: input is refused, a click on the blocked form reaches no handler, and the modal child keeps the focus. The style only chooses the layer drawn over the blocked face. `None` (the default) draws nothing — the form keeps exactly the look it was designed with; `SemiTransparent` is a light grey layer (25 % opaque); `Greyed` the same layer heavier (~60 %, the classic dimmed modal backdrop). The form keeps its own designed Transparency under any of them. An Async-opened child never blocks its opener — the two windows are independent, so for an Async open every style reads as `None`; the child can still publish to its opener through `super`, and closing the opener closes its children of either kind unless one of them is `Waiting` (its `FormState`), which vetoes the whole close and raises `onCloseRejected`.

SHELL mode starts when the main form carries a `SideMenu` control: ONE window with a MenuPane (root menu slot — mounted once — plus the current subsystem's contextual slot; Open/Collapsed, a narrow icon rail when collapsed, with the ☰ toggle drawn on the pane itself in BOTH states and whether or not any menu item exists; its own background from the main form's MenuPaneBackground group, never repainted by a loaded form), a breadcrumb FRAME (shell chrome — one segment per navigation-chain entry, each naming its form by its designed Title; clicking a segment destroys everything below it, deepest first), and a ContentPane hosting the loaded form top-left at its designed size. The loaded form's background paints the WHOLE pane (image/gradient modes evaluate against the PANE rect) and stays fixed while the form scrolls; a fully transparent form shows the desktop through the pane region only. Embedded forms play no window entrance/exit effects.

The BREADCRUMB FRAME always runs from the sidebar's right edge to the window's right edge — there is no width or position property — and the SideMenu owns its three: `BreadcrumbHeight` (16..200, default 28), `BreadcrumbBackgroundColor` (empty = follow the ContentPane's backdrop; a chosen colour may carry alpha but the frame is always painted opaque, being chrome) and `BreadcrumbTextAlign` (Top | Middle | Bottom, default Middle). The HEIGHT is independent of the breadcrumb's font: a larger FontSize never grows the frame and a smaller one never shrinks it, and text too big for the frame is CLIPPED by it rather than drawn outside. That is what makes the alignment meaningful — on a frame taller than its text, `BreadcrumbTextAlign` says whether the chain sits against the top, in the middle, or against the bottom. The chain and the toggle move as ONE GROUP: the alignment places the pair, and the text then centres on the toggle's own line, so a tall icon and a small font stay on one line at Top and at Bottom exactly as they do at Middle. While the sidebar's `FullHeight` is on (the default) the frame is the top BAND of the content area — it OVERLAYS the SHELL form's own coordinate space exactly as the designer canvas draws it, so the window opens at the form's designed height and THE SHELL FORM'S CONTROLS MAY BE PLACED OVER THE FRAME: such a control is an ordinary form control that merely overlaps (NOT a child — it is not clipped by the frame, does not scroll with it, keeps all its properties and events) and it paints on top of the frame and takes the click. A form LOADED INTO THE CONTENTPANE is the exception, because it is a different form with a coordinate space of its own: an embedded form starts BELOW the band, never over it, so its first row of controls can never land on the navigation chain. With FullHeight off the frame is a strip above the WHOLE window (above the sidebar too), the window pays its height, and nothing can be placed over it.

BREADCRUMB DETAIL LEVEL: the displayed form names the record it is holding with `INVOKE me "SetBreadcrumbDetail" USING <text>` (empty text or `INVOKE me "ClearBreadcrumbDetail"` removes it), shown as one more step after that form's own name (`Main Menu > Customer Data > John Smith`). It is one level, not a stack — setting it again replaces it. Only the DISPLAYED form may set one (a call from an off-pane form is ignored), and any navigation (another screen, a breadcrumb click, Home) drops it. With a detail showing, the form's OWN segment becomes a RESET link: clicking it starts that form over. The form has the last word through the universal property `PreventReset` (default 0): set it while holding unsaved data and the reset is refused and `onResetRejected` fires instead. Allowed, a pane occupant is REBUILT — onDestroy on the old instance, then a brand-new instance with blank WORKING-STORAGE in the same chain position (a reset is NOT a navigation) — while the shell's own main form, which has no second instance to swap in, receives `onReset`. Either way the detail level is cleared.

Navigation lifecycle: forms in the chain stay RESIDENT (storage alive, menu handlers callable) while not displayed. Each menu item carries `PreservePreviousForm` (default false): false destroys the outgoing sibling on a switch; true keeps it resident for an instant return. Two distinct form events: `onDeactivate` — the body left the ContentPane, the form is STILL resident (never a teardown point) — and `onDestroy` — fired immediately before storage is released (close files / COMMIT here).

`super` is the form that LOADED or OPENED this one, bound at load time on both paths (menu load and OpenForm*): `super::Title` reads/assigns the parent's properties, `super::"SetWindowState"("Minimized")` drives its window (the whole windowHandler method surface), and `super::super::…` walks one loader per step. Bare properties on `me`/`super` are checked at build time against the universal form surface (Name, Title, Width, Height, X, Y, WindowState, FullScreen, TitleVisible, CanMinimize, CanMaximize, FormState, FormFormat, BackgroundColor, Transparency, PreventReset) at any depth; form-specific procedures use parentheses and dispatch at run time. In the MAIN form — or after an async opener closed — `super` is NULL and referencing it raises the standard error. `super::<menu-id>::Collapse()` / `Open()` drive the MenuPane (pane-wide; the state persists per application). `me::<property>` works the same way on the form's OWN surface — including `me::PreventReset`, the guard that refuses a breadcrumb reset while the form is holding unsaved data.

### Multi-form host (spec 051)

An application holds MANY live forms at once, each running as its OWN program instance: own WORKING-STORAGE, own interpreter, own event loop. Forms never read each other's data items — they communicate through the supervisor surface only (published form properties, `super::X`, `handle::"SetProperty"`/`"GetProperty"`, windowHandler methods). The compiled binary embeds one program per openable form beside the main program.

THREE doors open a form: (1) a sidebar item's `Open form` action loads it into the ContentPane (FormFormat Embedded/Both); (2) `INVOKE me "OpenFormSync"/"OpenFormAsync"` opens it as a child WINDOW parented to the calling form (Standalone/Both); (3) a sidebar item's `Open Stand Alone Form (Sync)/(Async)` action — or the SideMenu control's `OpenStandAloneFormSync`/`OpenStandAloneFormAsync` methods — opens a child window parented to the SHELL (Standalone/Both). Sync is IMPLICITLY MODAL everywhere: the parent's whole face (shell chrome included, when the parent is the shell) takes no input until the child closes; Async is never modal. A close cascades per spec 037: Sync children close with their caller, Async children survive detached, the main form's close takes everything, and a Waiting form vetoes the whole close.

The way BACK from door (1) is the sidebar's `Home (main content pane)` action, which takes no target: it restores the shell form's OWN ContentPane content, so a 'main screen' needs no form of its own. Home PARKS the outgoing occupant — onDeactivate, never onDestroy — so its WORKING-STORAGE survives and loading it again revives that instance; no other live form is affected. The breadcrumb collapses to the shell form. SideMenu only: a MenuBar form has no ContentPane.

A PRESERVED pane occupant (`PreservePreviousForm`) keeps its interpreter and storage parked off-pane, and its enabled Timer controls KEEP TICKING (handlers run while parked; ticks coalesce against a busy queue). An open that cannot be satisfied — unknown form id, a form whose generated program was missing at build time — raises a visible runtime error and the handle is NULL; it is never silently dropped. EXEC RUST blocks share ONE object bridge per PROCESS: a handle created by any form's block resolves in every other form's blocks (values stored through the bridge must be `Send`); each form's COBOL storage and control registry stay its own.

### Window effects (spec 038)

Window entrance/exit effects are configured ONCE PER PROJECT (project settings → Appearance) and apply to every form: an entrance effect, an exit effect, each with a duration (100–3000 ms; `matrix-rain` uses its own 1500–4000 ms band, and `transporter-ii` is fixed at exactly 4000 ms) and an easing (`linear` | `ease-in` | `ease-out` | `ease-in-out`). The effect catalogue: `none`, `fade`, `zoom` (dBASE-style box zoom), `slide-left/right/top/bottom`, `expand-title-bar`, `radar-wipe`, `iris-wipe`, `blinds`, `checkerboard`, `matrix-rain` (katakana/digit glyph lines falling in from above the top edge over a see-through window; each line's END OF TRAIL — the faint top glyph — walks down its band and progressively uncovers what stands behind it, so the form is complete exactly when the last character leaves; lines arrive 25 ms apart at first, then 10-25 ms behind each other; this effect ignores the easing setting and runs on linear time), `genie` (squash-and-bend approximation), `transporter-ii` (a two-phase cinematic materialisation over a see-through window, fixed at 4000 ms and running on linear time whatever the easing says: PHASE 1 — two thin horizontal beams, each about half the form's width and horizontally centred, start overlapped on the vertical centre line and separate to the top and bottom edges, while the gap opening between them fills with a dense cloud of flickering white-and-yellow particles; PHASE 2 — the horizontal beams fade out as they land, two FULL-HEIGHT vertical beams fade in at the horizontal centre and sweep outward to the left and right edges, revealing the form in the band widening between them while the cloud dissolves wherever a beam has passed. Through the last stretch particles, glow and beams ease to nothing, so the light is gone exactly as the beams reach the borders. Every beam is a layered translucent gradient with a bloom — never a solid bar. An exit runs the sequence backwards and dematerialises the form). New projects default to a `matrix-rain` entrance and no exit effect.

While an effect runs the window wears NO title bar (nothing stands still during the animation); it arrives with the finished form, and only if that form shows one. The face-only effects (`fade`, `zoom`, the slides, `expand-title-bar`, `genie`), `matrix-rain` and `transporter-ii` also open a SEE-THROUGH window, so the form animates over the desktop — on those windows the form's `transparency` reaches the desktop for real. Only the masked reveals (`radar-wipe`, `iris-wipe`, `blinds`, `checkerboard`) keep an opaque window: they hide the form by painting covers over it, which nothing transparent can undo.

Per-form control: the Boolean designer attribute `WindowEffects` (default true) — false opens/closes that form instantly while the rest of the project animates. Forms never choose WHICH effect; only the project does. The entrance plays on the window's first opening; the project option `entrance-on-restore` additionally replays it when a window is restored after being minimized (no form events fire on a restore replay). Control load-time animations start immediately AFTER the entrance effect finishes, and the controls they animate are HELD BACK until then: a control with an `OnFormLoad`/`OnShow` animation is not painted into the entrance at all, so it arrives under its own power instead of materialising with the window and then jumping back to fly in a second time (1.61.5+; before that it did exactly that). Controls with no load animation appear with the window as always. The COBOL `onLoad` event timing is unchanged. An exit effect delays the actual close until the animation completes — `FormState` vetoes fire BEFORE the animation, so a refused close plays nothing, and `onClose` still fires exactly once. Machine-wide kill-switch: Help → Debug Settings → "Disable window effects" (`PRC_NO_WINDOW_FX=1` for a bare `rcrun run-form` or a built application — both honour it).

Effects play in EVERY host of a form (spec 042): Run Form and the BUILT application run the same shared window host, so the entrance/exit behaviour is identical in both. The settings are baked into the executable at build time — a shipped binary reads no project file. The same shared host gives the built application the full designed window behaviour: the form's own `Title` (falling back to "AppName vVersion" only when the designed title is blank), `TitleVisible`, minimize/maximize buttons, `FullScreen`, the opening `WindowState` and `StartPosition`, window close at program end (through the exit effect when configured), the `me::` window methods, `FormState` close vetoes, and the `onShow`/`onActivate`/`onClose`/`onCloseRejected`/`onFullScreenChanged` events.

---

## Control: Button

Clickable push button. Default size 80×28 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Caption` (String, default "_"; free text) — Visible label text (Button/Label/CheckBox/RadioButton/GroupBox).
- `IsDefault` (Boolean, default 0; `1` (true) or `0` (false)) — Form's default button (activated by Enter).
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `IconPath` (String, default (empty); image path or empty) — Icon drawn next to the caption.
- `IconAlignment` (String, default "Left"; `Left` | `Right` | `Top` | `Bottom`) — Side of the caption the icon sits on.
- `IconPadding` (Integer, default 10; pixels ≥ 0) — Gap between icon and caption.
- `IconSize` (String, default "32"; pixels, one of: `16` `32` `48` `64` `80` `96` `128` (default 32)) — Edge length of the Button's square icon slot: the IconPath image is drawn at exactly IconSize x IconSize, so supply an image of that proportion (pad a non-square one with transparency) or it is distorted. A Button with an icon and an empty Caption is an icon button: the icon is centred and nothing else is drawn on the face.
- `TextAlignment` (String, default "MiddleCenter"; `Left` | `Center` | `Right` | `Justified` on Label/TextBox (Button also accepts anchored forms like `MiddleCenter`)) — Horizontal alignment of the text. `Justified` stretches wrapped lines to the full width (static text; a TextBox being edited shows it left-aligned).
- `CornerRadius` (Integer, default 3; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseEnter`, `onMouseLeave`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetCaption(text: String)` — Replace the caption.
- `GetCaption() → String` — Read the caption.

---

## Control: TextBox

Single- or multi-line text input. Default size 160×24 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `EnterAsTab` (Boolean, default 1; `1` (true) or `0` (false)) — Enter moves the focus to the next control in the tab order, as Tab does; `onEnterPressed` still fires. On by default for TextBox, ComboBox, NumericUpDown, DateTimePicker, CheckBox and RadioButton. Ignored by a multi-line TextBox (Enter is its new line), and an open ComboBox list keeps Enter to pick its item. An Enter that moves the focus does not also press the form's default button.
- `Text` (String, default (empty); free text) — Current text content.
- `AutoEnter` (Boolean, default 0; `1` (true) or `0` (false)) — TextBox: the keystroke that fills the box to its length counts as Enter — `onEnterPressed` fires and, with `EnterAsTab` on, the focus moves to the next control. The length is the one the box enforces: an explicit `Picture`'s width, or `MaximumLength`; with neither the box is never full and the property does nothing. Off by default.
- `HintText` (String, default (empty); free text) — Placeholder shown while the box is empty.
- `TextAlignment` (String, default "Left"; `Left` | `Center` | `Right` | `Justified` on Label/TextBox (Button also accepts anchored forms like `MiddleCenter`)) — Horizontal alignment of the text. `Justified` stretches wrapped lines to the full width (static text; a TextBox being edited shows it left-aligned).
- `VerticalAlignment` (String, default "Middle"; `Top` | `Middle` | `Bottom`) — Vertical alignment of the text (Label and single-line TextBox; a multiline TextBox stays top-anchored).
- `InnerPadding` (Integer, default 3; pixels ≥ 0) — Padding between the border and the text.
- `MaximumLength` (Integer, default 0; characters ≥ 0; 0 = unlimited) — Maximum text length accepted.
- `Picture` (String, default (empty); a COBOL PICTURE template (`X(30)`, `A(20)`, `S9(4)V99`, `ZZZ,ZZ9.99-`); empty = derived from MaximumLength) — The COBOL PICTURE the box's contents obey. It is both validator and mask: each keystroke is checked against what is legal at that character position (`A` letters and space, `9` digits, `X` any byte), and the box shows the edited form when it is not focused and the plain stored value when it is. The generated `-TEXT` and `-VALUE` items are declared with this same picture, so a comparison against them follows COBOL's rules by construction. The decimal separator is the form's: under DECIMAL-POINT IS COMMA a comma is the decimal point and a period is the grouping character. A sign may be typed at either end and is normalised to where the picture puts it. Left empty, the picture is `X(n)` sized from MaximumLength, or `X(256)` single-line / `X(2048)` multiline when that is 0; set explicitly, its own width is authoritative and MaximumLength no longer bounds the field.
- `Multiline` (Boolean, default 0; `1` (true) or `0` (false)) — Multi-line editing.
- `PasswordCharacter` (String, default (empty); single character or empty) — Masks input with this character when set.
- `ReadOnly` (Boolean, default 0; `1` (true) or `0` (false)) — Blocks user editing (value still settable from COBOL).
- `ScrollBars` (String, default "None"; one of: `None` | `Horizontal` | `Vertical` | `Both`) — Which scrollbars a multiline box shows. None still scrolls, it just draws no bars. Horizontal and Both stop the text wrapping.
- `WordWrap` (Boolean, default 1; `1` (true) or `0` (false)) — Wraps long lines at the control's width. Off: a Label keeps its own lines and shrinks its font to fit; a multiline TextBox scrolls sideways instead of wrapping.
- `BorderStyle` (String, default "Fixed3D"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#AAAAAA"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onChange` — value or text changed
- `onTextChanged` — text content changed
- `onEnter` — focus entered the box (alias of onGotFocus)
- `onLeave` — focus left the box (alias of onLostFocus)
- Plus the universal events: `onKeyPress`, `onKeyDown`, `onKeyUp`, `onEnterPressed`, `onEscapePressed`, `onGotFocus`, `onLostFocus`, `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetText(text: String)` — Replace the text content.
- `GetText() → String` — Read the text content.
- `AppendText(text: String)` — Append to the text content.
- `Clear()` — Empty the text/items.

---

## Control: Label

Static text display. At RUN TIME its caption is SELECTABLE TEXT: drag across it to select, and Cmd/Ctrl+C copies the selection to the clipboard — a drag that starts on one Label and ends on another takes in both, so a reading can be copied along with the caption naming it. There is no property to switch on and nothing to write in COBOL; every Label behaves this way, in the running form and in Preview alike. A Label with a bound onClick still fires it, TAB still walks past labels to the form's own controls, and on the DESIGNER CANVAS a drag still positions the control. Do NOT tell a developer to use a ReadOnly TextBox to make text copyable — that was the workaround before labels could be selected. Default size 120×20 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Caption` (String, default "_"; free text) — Visible label text (Button/Label/CheckBox/RadioButton/GroupBox).
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `TextAlignment` (String, default "Left"; `Left` | `Center` | `Right` | `Justified` on Label/TextBox (Button also accepts anchored forms like `MiddleCenter`)) — Horizontal alignment of the text. `Justified` stretches wrapped lines to the full width (static text; a TextBox being edited shows it left-aligned).
- `VerticalAlignment` (String, default "Middle"; `Top` | `Middle` | `Bottom`) — Vertical alignment of the text (Label and single-line TextBox; a multiline TextBox stays top-anchored).
- `WordWrap` (Boolean, default 0; `1` (true) or `0` (false)) — Wraps long lines at the control's width. Off: a Label keeps its own lines and shrinks its font to fit; a multiline TextBox scrolls sideways instead of wrapping.
- `AutoSize` (Boolean, default 0; `1` (true) or `0` (false)) — Label: the control takes its caption's size (anchored at its top-left): wider as the text grows, or — with WordWrap on — the same width and taller. The designer resizes it as you edit, and a caption set from COBOL resizes it at run time.
- `BorderStyle` (String, default "None"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Supported Events
- Plus the universal events: `onClick`, `onGotFocus`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetCaption(text: String)` — Replace the caption.
- `GetCaption() → String` — Read the caption.

---

## Control: CheckBox

Boolean on/off box with caption. It has TWO surfaces, each with its own properties. The FRAME is the card behind caption and box: BackgroundColor fills it and BorderStyle/BorderColor/BorderWidth rim it, exactly as on every other control — it is 100 % transparent by default, so nothing shows until one of those is set. The BOX is the tick square: CheckBoxColor fills it, CheckBoxBorderStyle/Color/Width rim it, CheckColor draws the tick inside it and CheckSize scales that tick. Never tell a user to set BackgroundColor to colour the box, or CheckBoxColor to colour the frame. Default size 120×22 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Caption` (String, default "_"; free text) — Visible label text (Button/Label/CheckBox/RadioButton/GroupBox).
- `EnterAsTab` (Boolean, default 1; `1` (true) or `0` (false)) — Enter moves the focus to the next control in the tab order, as Tab does; `onEnterPressed` still fires. On by default for TextBox, ComboBox, NumericUpDown, DateTimePicker, CheckBox and RadioButton. Ignored by a multi-line TextBox (Enter is its new line), and an open ComboBox list keeps Enter to pick its item. An Enter that moves the focus does not also press the form's default button.
- `Checked` (Boolean, default 0; `1` (true) or `0` (false)) — Checked state of a CheckBox or a Switch. A **RadioButton** uses `Selected` instead — a radio is selected, not checked. `Checked` is still accepted on a radio so programs written before the rename keep working, and it resolves to `Selected` on the way in.
- `CheckAlignment` (String, default "Left"; `Left` | `Right`) — Side the check box or the radio circle sits on; the caption takes the other side.
- `CheckColor` (String, default "#0078D7"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Color of the check/radio mark — the tick itself, drawn inside the box.
- `CheckSize` (Integer, default 70; 0-100) — CheckBox: percentage of its box the checkmark fills. A RadioButton has none: its circle is filled whole when selected.
- `CheckSpacing` (Integer, default 6; points 0-64 (default 6)) — CheckBox / RadioButton: the distance between the box (or circle) and the caption, on whichever side `CheckAlignment` puts the box.
- `BorderStyle` (String, default "None"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#8C8CA0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `CheckBoxColor` (String, default (empty); `#RRGGBB`, or empty for the theme's own) — Fill of the TICK BOX (a RadioButton's circle) — the box only, never the frame. BackgroundColor is the frame's, as on every other control. Left EMPTY (the default) the active theme paints the box; naming a color makes it lead over whatever the theme would have used.
- `CheckBoxBorderStyle` (String, default "None"; `None` | `Single` | `Fixed3D` | `Raised` | `Sunken`) — Border drawn around the TICK BOX, separate from the frame's BorderStyle. `None` (the default) keeps whatever rim the theme draws.
- `CheckBoxBorderColor` (String, default "#8C8CA0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Color of the tick box's own border.
- `CheckBoxBorderWidth` (Integer, default 1; pixels 0-10) — Width of the tick box's own border. 0 draws none.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onCheck` — the toggle went ON (fires only in that direction)
- `onUncheck` — the toggle went OFF (fires only in that direction)
- `onCheckedChanged` — checked state flipped, either way (carries the new state)
- `onValueChanged` — value changed (the new value is delivered)
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetCaption(text: String)` — Replace the caption.
- `GetCaption() → String` — Read the caption.
- `IsChecked() → Boolean (0/1)` — Read the checked state.
- `SetChecked(value: Boolean)` — Set the checked state (`1`/`0`, also accepts true/false/yes/on).
- `Select()` — Check it (radio: also unchecks the group siblings).
- `Toggle()` — Flip the checked state.

---

## Control: RadioButton

Mutually-exclusive choice within a GroupName. Its indicator is a real drawn CIRCLE on every theme — filled when chosen, an empty rim when not — never a character in the caption. A theme that describes a toggle surface colours it (Elegance paints its green); on every other theme the circle takes the control's own `CheckColor`, and `CheckBoxColor` sets the circle's face where the developer wants one. An unchosen circle's rim is picked for CONTRAST against whatever the control was dropped on, so it is visible on a dark form and on a pale card alike. Default size 140×22 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Caption` (String, default "_"; free text) — Visible label text (Button/Label/CheckBox/RadioButton/GroupBox).
- `EnterAsTab` (Boolean, default 1; `1` (true) or `0` (false)) — Enter moves the focus to the next control in the tab order, as Tab does; `onEnterPressed` still fires. On by default for TextBox, ComboBox, NumericUpDown, DateTimePicker, CheckBox and RadioButton. Ignored by a multi-line TextBox (Enter is its new line), and an open ComboBox list keeps Enter to pick its item. An Enter that moves the focus does not also press the form's default button.
- `Selected` (Boolean, default 0; `1` (true) or `0` (false)) — Selected state of a **RadioButton** — the radio's own name for what a CheckBox calls `Checked`. Only one RadioButton in a `GroupName` is selected at a time: selecting one clears its siblings and each cleared button raises `onUncheck`. Forms saved before this name existed store `Checked`, and are migrated to `Selected` when they load.
- `GroupName` (String, default (empty); free text) — RadioButton: radios sharing a GroupName are mutually exclusive. A CheckBox has none — check boxes are independent.
- `CheckAlignment` (String, default "Left"; `Left` | `Right`) — Side the check box or the radio circle sits on; the caption takes the other side.
- `CheckColor` (String, default "#0078D7"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Color of the check/radio mark — the tick itself, drawn inside the box.
- `CheckSpacing` (Integer, default 6; points 0-64 (default 6)) — CheckBox / RadioButton: the distance between the box (or circle) and the caption, on whichever side `CheckAlignment` puts the box.
- `BorderStyle` (String, default "None"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#8C8CA0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `CheckBoxColor` (String, default (empty); `#RRGGBB`, or empty for the theme's own) — Fill of the TICK BOX (a RadioButton's circle) — the box only, never the frame. BackgroundColor is the frame's, as on every other control. Left EMPTY (the default) the active theme paints the box; naming a color makes it lead over whatever the theme would have used.
- `CheckBoxBorderStyle` (String, default "None"; `None` | `Single` | `Fixed3D` | `Raised` | `Sunken`) — Border drawn around the TICK BOX, separate from the frame's BorderStyle. `None` (the default) keeps whatever rim the theme draws.
- `CheckBoxBorderColor` (String, default "#8C8CA0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Color of the tick box's own border.
- `CheckBoxBorderWidth` (Integer, default 1; pixels 0-10) — Width of the tick box's own border. 0 draws none.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onCheck` — the toggle went ON (fires only in that direction)
- `onUncheck` — the toggle went OFF (fires only in that direction)
- `onCheckedChanged` — checked state flipped, either way (carries the new state)
- `onValueChanged` — value changed (the new value is delivered)
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetCaption(text: String)` — Replace the caption.
- `GetCaption() → String` — Read the caption.
- `IsChecked() → Boolean (0/1)` — Read the checked state.
- `SetChecked(value: Boolean)` — Set the checked state (`1`/`0`, also accepts true/false/yes/on).
- `Select()` — Check it (radio: also unchecks the group siblings).
- `Toggle()` — Flip the checked state.

---

## Control: ListBox

Scrollable list of selectable items. A click makes a row active and starts a one-row selection; a press-and-drag anchors on the row pressed and extends to the row under the pointer, in EITHER direction — reversing shrinks the range back — and holds at the first or last row when the pointer runs past an end; Up/Down arrows move the active row one line once the list has been clicked or Tabbed to, and stop at the ends. Whatever moves the active row, the list scrolls to keep it in view, on the first or last visible line. Dragging selects rather than scrolls; the wheel and the scrollbar scroll. Default size 160×100 px.

**Content structure.** The entries are the `Items` property: one entry per line, in display order. An agent reads it in the control's `CONTROLS` line and changes it with `set_property` on `Items`, sending the whole list (sorting, adding or removing an entry means sending every line). When `ItemsFile` names a text file, the list reads its entries from that file each time the form opens instead.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Items` (String, default (empty); newline-separated entries (TreeView: two-space indentation nests children)) — The list content, one item per line.
- `ItemsFile` (String, default (empty); project-relative or absolute path of a .txt, or empty) — ComboBox / ListBox: a text file the list reads its items from each time the form opens — one item per line, blank lines left out — so editing the file changes the list without touching the form. When the file cannot be read, the designed Items stay. Keep it in the project's assets/ folder so a built application carries it. In the inspector, 📂 picks the file and ✕ clears the path and the items. From COBOL, LoadFromFile(path) does the same at any time.
- `SelectedIndex` (Integer, default -1; 0-based index; -1 = no selection) — The selected item's position among the items AS SHOWN (`Sorted` applied), which is also what a pick reports. Setting it — in the designer or from COBOL, by the property or `SetSelectedIndex` — selects that item: `Value` becomes its text; -1, or a position past the end, clears the selection. ComboBox and ListBox.
- `MultiSelect` (Boolean, default 0; `1` (true) or `0` (false)) — Lets the user build a selection with Ctrl-click (Cmd on a Mac), reported in SelectedItems.
- `SelectedItems` (String, default (empty); newline-separated item text (runtime)) — The Ctrl-click selection, drawn in a dimmed highlight. Separate from Value, which is the ACTIVE row.
- `ShowCheckBoxes` (Boolean, default 0; `1` (true) or `0` (false)) — Gives every ListBox row a tick box; what they collect is CheckedItems.
- `CheckedItems` (String, default (empty); newline-separated item text (runtime)) — The ticked rows, in the order the user ticked them and with any gaps. Ticking never moves the active row.
- `Sorted` (Boolean, default 0; `1` (true) or `0` (false)) — Shows the items in alphabetical order, by TEXT and ignoring case, so 10 sorts before 9. Display order only - the stored Items keeps the order it was written in. ListBox, ComboBox and TreeView. A TreeView sorts SIBLINGS only — every child stays under the parent it was written under — and a node's handle (`CONTROL-NODE-INDEX`, the `Node…` methods) is still its line as written, so sorting never renumbers a handler.
- `ActiveItemColor` (String, default (empty); color, or empty for the control's default highlight) — Highlight behind the item Value/SelectedIndex reports: the ACTIVE row of a ListBox, or the selected item in an open ComboBox dropdown. Left empty a ListBox takes the theme's own selection color and a ComboBox its popup's built-in one.
- `SelectedItemsColor` (String, default (empty); color, or empty for ActiveItemColor dimmed to 45%) — ListBox only. Highlight behind the other rows of a MultiSelect selection, the ones SelectedItems reports. Left empty it follows ActiveItemColor, so naming that one alone restyles the whole list.
- `ActiveItemTextColor` (String, default (empty); `#RRGGBB`/`#RRGGBBAA`, or empty) — The TEXT colour of the highlighted row — the item `SelectedIndex`/`Value` reports. Empty, the default, keeps what a ListBox has always done: the row's ink is `ForegroundColor` while that clears WCAG AA on the highlight band, and pure black or white when it does not. That floor keeps a list readable no matter what `ActiveItemColor` is set to, but it left the developer no say — the band was theirs to choose and the ink on it was not. Set this and it wins outright, floor included, on the same rule every other colour on the control follows: empty means "not chosen". It applies to the ACTIVE row only; rows highlighted by `SelectedItemsColor` under `MultiSelect` keep `ForegroundColor`.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onItemChecked`
- `onChange` — value or text changed
- `onSelectedIndexChanged` — selection moved to another index
- `onItemDoubleClick` — a list item was double-clicked
- `onSelectionChanged` — the selected item/cell set changed
- `onScroll` — content scrolled
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddItem(text: String)` — Append one item.
- `RemoveItem(text: String)` — Remove the first item equal to `text`.
- `GetSelected() → String` — Read the selected item's value.
- `GetSelectedIndex() → Integer` — Read the 0-based selected index (-1 = none).
- `SetSelectedIndex(index: Integer)` — Select by 0-based index.
- `GetCount() → Integer` — Number of items.
- `Clear()` — Remove all items (a ComboBox / ListBox also clears its selection: SelectedIndex -1, Value empty).
- `LoadFromFile(path: String) → Integer` — Replace the items with a text file's lines (one item per line, blank lines left out) and clear the selection; returns the item count, or -1 when the file cannot be read (the items are then left as they were). A relative path is resolved from the project folder under Run Form and beside the executable in a built application. Example: `MOVE ComboBox-1::LoadFromFile("assets/states.txt") TO WS-COUNT`.
- `RefreshBinding() → Integer` — Bound to a COBOL table (any level, inside a GLOBAL 01): reload Items from it, one item per occurrence of the display field, blanks left out; returns the item count. Also runs by itself as the form opens, after onLoad.

---

## Control: ComboBox

Drop-down list, optionally editable. A click on the header opens the list without picking anything; a press-and-drag from the header follows the pointer item by item — in EITHER direction, reversing walks the highlight back — and holds at the first or last item when the pointer runs past an end, with the release committing that item. With the list SHUT the Up/Down arrows change the value outright; with it OPEN they move the highlight, Enter commits it and Escape closes leaving the value unchanged. The list opens scrolled to the value it holds and scrolls to keep the highlighted item in view; it is as tall as its items need up to DropDownHeight and scrolls past that, so every item is reachable. Header and dropdown both wear the control's designed background, gradient, border and corner radius, and the items are lettered in its own FontName/FontSize/ForegroundColor. Default size 160×24 px.

**Content structure.** The entries are the `Items` property: one entry per line, in display order. An agent reads it in the control's `CONTROLS` line and changes it with `set_property` on `Items`, sending the whole list (sorting, adding or removing an entry means sending every line). When `ItemsFile` names a text file, the list reads its entries from that file each time the form opens instead.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `EnterAsTab` (Boolean, default 1; `1` (true) or `0` (false)) — Enter moves the focus to the next control in the tab order, as Tab does; `onEnterPressed` still fires. On by default for TextBox, ComboBox, NumericUpDown, DateTimePicker, CheckBox and RadioButton. Ignored by a multi-line TextBox (Enter is its new line), and an open ComboBox list keeps Enter to pick its item. An Enter that moves the focus does not also press the form's default button.
- `Items` (String, default (empty); newline-separated entries (TreeView: two-space indentation nests children)) — The list content, one item per line.
- `ItemsFile` (String, default (empty); project-relative or absolute path of a .txt, or empty) — ComboBox / ListBox: a text file the list reads its items from each time the form opens — one item per line, blank lines left out — so editing the file changes the list without touching the form. When the file cannot be read, the designed Items stay. Keep it in the project's assets/ folder so a built application carries it. In the inspector, 📂 picks the file and ✕ clears the path and the items. From COBOL, LoadFromFile(path) does the same at any time.
- `SelectedIndex` (Integer, default -1; 0-based index; -1 = no selection) — The selected item's position among the items AS SHOWN (`Sorted` applied), which is also what a pick reports. Setting it — in the designer or from COBOL, by the property or `SetSelectedIndex` — selects that item: `Value` becomes its text; -1, or a position past the end, clears the selection. ComboBox and ListBox.
- `Sorted` (Boolean, default 0; `1` (true) or `0` (false)) — Shows the items in alphabetical order, by TEXT and ignoring case, so 10 sorts before 9. Display order only - the stored Items keeps the order it was written in. ListBox, ComboBox and TreeView. A TreeView sorts SIBLINGS only — every child stays under the parent it was written under — and a node's handle (`CONTROL-NODE-INDEX`, the `Node…` methods) is still its line as written, so sorting never renumbers a handler.
- `ActiveItemColor` (String, default (empty); color, or empty for the control's default highlight) — Highlight behind the item Value/SelectedIndex reports: the ACTIVE row of a ListBox, or the selected item in an open ComboBox dropdown. Left empty a ListBox takes the theme's own selection color and a ComboBox its popup's built-in one.
- `HoverItemColor` (String, default (empty); color, or empty for the popup's built-in hover highlight) — ComboBox only. Highlight behind the dropdown item the pointer, the drag or the arrow keys are on. Kept fainter than ActiveItemColor by default so hovering an item never looks like selecting it.
- `DropDownStyle` (String, default "DropDown"; one of: `DropDown` | `DropDownList` | `Simple`) — `DropDown` (the default): a text field that takes typing, with a button on the right that opens the list — a press on the text places the caret. `DropDownList`: pick-only — a press anywhere opens the list and typing is refused. `Simple`: the text field with the list always shown beneath it, inside the control, and no dropdown (no `onDropDown`).
- `DropDownHeight` (Integer, default 200; pixels > 0) — Maximum height of the opened list. The list is as tall as its items need up to this, and scrolls past it.
- `Editable` (Boolean, default 1; `1` (true) or `0` (false)) — Whether the combo's text field takes typing (the `DropDown` and `Simple` styles; a `DropDownList` never does). Typed text becomes `Value` even when it names no item — `SelectedIndex` is then -1 — and raises `onChange` and `onTextChanged`; while the list is open, typing moves its highlight to the first item that begins with the text. Off, a `DropDown` combo is pick-only. The arrow keys always walk the list.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Supported Events
- `onChange` — value or text changed
- `onSelectedIndexChanged` — selection moved to another index
- `onDropDown` — drop-down list opened
- `onDropDownClosed` — drop-down list closed
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddItem(text: String)` — Append one item.
- `RemoveItem(text: String)` — Remove the first item equal to `text`.
- `GetSelected() → String` — Read the selected item's value.
- `GetSelectedIndex() → Integer` — Read the 0-based selected index (-1 = none).
- `SetSelectedIndex(index: Integer)` — Select by 0-based index.
- `GetCount() → Integer` — Number of items.
- `Clear()` — Remove all items (a ComboBox / ListBox also clears its selection: SelectedIndex -1, Value empty).
- `LoadFromFile(path: String) → Integer` — Replace the items with a text file's lines (one item per line, blank lines left out) and clear the selection; returns the item count, or -1 when the file cannot be read (the items are then left as they were). A relative path is resolved from the project folder under Run Form and beside the executable in a built application. Example: `MOVE ComboBox-1::LoadFromFile("assets/states.txt") TO WS-COUNT`.
- `RefreshBinding() → Integer` — Bound to a COBOL table (any level, inside a GLOBAL 01): reload Items from it, one item per occurrence of the display field, blanks left out; returns the item count. Also runs by itself as the form opens, after onLoad.

---

## Control: GroupBox

Captioned container; can become a repeating card template (control array). Default size 200×120 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Caption` (String, default (empty); free text) — Visible label text (Button/Label/CheckBox/RadioButton/GroupBox).
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `HScroll` (Boolean, default 0; `1` (true) or `0` (false)) — Panel, GroupBox, TabControl: horizontal scrolling when the children reach past the content area.
- `VScroll` (Boolean, default 0; `1` (true) or `0` (false)) — Panel, GroupBox, TabControl: vertical scrolling when the children reach past the content area.
- `UserControl` (String, default (empty); User Control definition name or empty) — Marks a deployed project User Control instance.
- `HideCaption` (Boolean, default 0; `1` (true) or `0` (false)) — Hides the GroupBox caption text.
- `CaptionEnabled` (Boolean, default 1; `1` (true) or `0` (false)) — Whether the legend reads as enabled: off draws it dimmed (about 45% opacity), like a disabled caption. It reserves no space and does not change where children go.
- `CaptionBackgroundStyle` (String, default "None"; one of: `None` | `Flat` | `Gradient` (default `None`)) — GroupBox: how the caption is filled. `None` keeps the classic legend on the top border; `Flat` draws it in a box filled with `CaptionBackColor`; `Gradient` fills the box from `CaptionGradientStart` to `CaptionGradientEnd` along `CaptionGradientDirection`. The box is outlined in the GroupBox's `BorderColor` at its `BorderWidth`, and the caption text takes an ink that reads on the fill.
- `CaptionBackColor` (String, default "#2C6FD2"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — GroupBox: fill of the caption box when `CaptionBackgroundStyle` is `Flat` (default `#2C6FD2`).
- `CaptionGradientStart` (String, default "#4A8FE8"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — GroupBox: first colour of the caption box when `CaptionBackgroundStyle` is `Gradient` (default `#4A8FE8`).
- `CaptionGradientEnd` (String, default "#1F4F9A"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — GroupBox: last colour of the caption box when `CaptionBackgroundStyle` is `Gradient` (default `#1F4F9A`).
- `CaptionGradientDirection` (String, default "South"; one of: `South` | `North` | `East` | `West` | `SouthEast` | `SouthWest` | `NorthEast` | `NorthWest` | `Radial` (default `South`)) — GroupBox: direction of the caption box's gradient.
- `CaptionShape` (String, default "Rectangle"; one of: `Rectangle` | `Pill` | `AngledLeft` | `AngledRight` (default `Rectangle`)) — GroupBox: outline of the caption box — `[ caption ]`, `( caption )`, `\ caption \` (both sides leaning left) or `/ caption /` (both sides leaning right). Drawn when `CaptionBackgroundStyle` is `Flat` or `Gradient`.
- `CaptionSize` (String, default "Text"; one of: `Text` | `Full` | `Inner` (default `Text`)) — GroupBox: width of the caption — the text plus its padding (`Text`), the whole straight top edge between the rounded corners (`Full`), or the same stopping 10 px short of each corner (`Inner`). The caption, and the opening it leaves in the border, never reaches into a rounded corner: a caption padded wider than the edge is slid clear of the corner or cut to the edge.
- `CaptionPadding` (Integer, default 4; integer 0-64 (default 4)) — GroupBox: the caption's padding shorthand — the space left and right of the text inside its box, and half of it above and below. `CaptionPaddingHorizontal` / `CaptionPaddingVertical` override one axis.
- `CaptionPaddingHorizontal` (String, default (empty); integer 0-64, or empty (default) to use `CaptionPadding`) — GroupBox: space in pixels left and right of the caption text, inside its box.
- `CaptionPaddingVertical` (String, default (empty); integer 0-64, or empty (default) to use half of `CaptionPadding`) — GroupBox: space in pixels above and below the caption text, inside its box.
- `CaptionAlignment` (String, default "Auto"; one of: `Auto` | `Left` | `Center` | `Right` (default `Auto`)) — GroupBox: where the caption text sits. `Auto` is left for a `Text`-sized caption (where the classic legend has always been) and centred for `Full` and `Inner`.
- `HideBackground` (Boolean, default 0; `1` (true) or `0` (false)) — Hides the fill/border while keeping the content visible.
- `IsRepeatingGroup` (Boolean, default 0; `1` (true) or `0` (false)) — Turns the GroupBox into a repeating card template (control array).
- `ArrayName` (String, default (empty); COBOL identifier or empty (empty = control id)) — Name used to address instances: `Name(index)::Member`.
- `ItemCount` (Integer, default 0; integer 0-500) — Number of cards at run time. A bound group gets it from its data (RefreshBinding sets it); an unbound group uses it once it is above 0 — set in the designer or by the program — and shows its PreviewItemCount template cards while it is 0.
- `DataSource` (String, default (empty); COBOL table data-item name (charts / repeating GroupBox / DataGrid binding)) — Table the control binds to. On a chart, `SET-TABLE` reads each occurrence's label and value from the sub-fields named by `LabelField` and `ValueFields`; with no `LabelField` it reads the fixed `PIC X(64)` label + `PIC 9(18)V9(6)` value layout.
- `LayoutDirection` (String, default "Vertical"; one of: `Vertical` | `Horizontal` | `Grid`) — How cards flow inside the group.
- `ItemSpacing` (Integer, default 8; pixels ≥ 0) — Gap between cards.
- `ItemsPerRow` (Integer, default 1; integer ≥ 1) — Cards per row when LayoutDirection = `Grid`.
- `PlacementEffect` (String, default "None"; one of: `None` | `Deal` | `FadeIn` | `ZoomIn` | `ZoomOut`) — Card entrance animation when data binds.
- `CardAppearDuration` (Integer, default 200; milliseconds ≥ 0) — Duration of the card entrance animation.
- `CloneEvents` (Boolean, default 1; `1` (true) or `0` (false)) — On (default): every card of a repeating group fires the template's event handlers, with `CONTROL-ARRAY-INDEX` = the card's number. Off: only the designed card (1) fires; its clones are display only.
- `PreviewItemCount` (Integer, default 1; integer 1-500) — Cards shown on the design canvas, and at run time by an unbound group whose ItemCount is 0.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `LayoutMode` (String, default "Absolute"; one of: `Absolute` | `Flex` | `Grid` | `Flow` (default `Absolute`)) — Form, Panel, GroupBox, TabControl: how the container places its children on a responsive form — anchors and docking, a flex row/column, a grid of tracks, or a wrapping flow. (The Viewer's own `Layout` is a different property.)
- `FlexDirection` (String, default "Row"; one of: `Row` | `Column` | `RowReverse` | `ColumnReverse` (default `Row`)) — Flex container: the main axis.
- `FlexWrap` (String, default "NoWrap"; one of: `NoWrap` | `Wrap` | `WrapReverse` (default `NoWrap`)) — Flex container: whether items that do not fit start a new line.
- `JustifyContent` (String, default "Start"; one of: `Start` | `Center` | `End` | `SpaceBetween` | `SpaceAround` | `SpaceEvenly` (default `Start`)) — Flex container: where the leftover main-axis space goes.
- `AlignItems` (String, default "Stretch"; one of: `Stretch` | `Start` | `Center` | `End` (default `Stretch`)) — Flex or grid container: how items sit across their line or cell.
- `AlignContent` (String, default "Stretch"; one of: `Stretch` | `Start` | `Center` | `End` | `SpaceBetween` | `SpaceAround` (default `Stretch`)) — Wrapping flex container: how the lines share the cross axis.
- `Gap` (Integer, default 0; form pixels ≥ 0 (default 0)) — Flex, grid or flow container: the space between items.
- `RowGap` (String, default (empty); form pixels, or empty (default) to use `Gap`) — Flex or grid container: the space between rows / between columns.
- `ColumnGap` (String, default (empty); form pixels, or empty (default) to use `Gap`) — Flex or grid container: the space between rows / between columns.
- `GridColumns` (String, default (empty); track list: `Npx` `N%` `Nfr` `Auto` `MinMax(a, b)` `Repeat(n, tracks)` `Repeat(AutoFill, MinMax(min, b))`) — Grid container: the column / row tracks, e.g. `200px 1fr 2fr`. An empty `GridRows` adds `Auto` rows as needed.
- `GridRows` (String, default (empty); track list: `Npx` `N%` `Nfr` `Auto` `MinMax(a, b)` `Repeat(n, tracks)` `Repeat(AutoFill, MinMax(min, b))`) — Grid container: the column / row tracks, e.g. `200px 1fr 2fr`. An empty `GridRows` adds `Auto` rows as needed.
- `JustifyItems` (String, default "Stretch"; one of: `Stretch` | `Start` | `Center` | `End` (default `Stretch`)) — Grid container: how items sit horizontally in their cell.
- `FlowDirection` (String, default "LeftToRight"; one of: `LeftToRight` | `TopDown` | `RightToLeft` | `BottomUp` (default `LeftToRight`)) — Flow container: the direction items run.
- `WrapContents` (Boolean, default 1; `1` (true) or `0` (false)) — Flow container: start a new line when the next item does not fit (default 1).

### Supported Events
- `onChildAdded` — a child control was added to the container
- `onChildRemoved` — a child control was removed from the container
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetCaption(text: String)` — Replace the caption.
- `GetCaption() → String` — Read the caption.
- `RefreshBinding() → Integer` — Repeating group: re-hydrate the cards from the bound data source.

### Repeating groups (control arrays)
With `IsRepeatingGroup = 1` the GroupBox becomes a card template: set `ItemCount` (or bind `DataSource`) and address instance members as `Member(index)::Property` (1-based index). Handlers on members receive `CONTROL-ARRAY-INDEX`.

---

## Control: Panel

Plain container for grouping child controls. Default size 200×150 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `HScroll` (Boolean, default 0; `1` (true) or `0` (false)) — Panel, GroupBox, TabControl: horizontal scrolling when the children reach past the content area.
- `VScroll` (Boolean, default 0; `1` (true) or `0` (false)) — Panel, GroupBox, TabControl: vertical scrolling when the children reach past the content area.
- `HideBackground` (Boolean, default 0; `1` (true) or `0` (false)) — Hides the fill/border while keeping the content visible.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `LayoutMode` (String, default "Absolute"; one of: `Absolute` | `Flex` | `Grid` | `Flow` (default `Absolute`)) — Form, Panel, GroupBox, TabControl: how the container places its children on a responsive form — anchors and docking, a flex row/column, a grid of tracks, or a wrapping flow. (The Viewer's own `Layout` is a different property.)
- `FlexDirection` (String, default "Row"; one of: `Row` | `Column` | `RowReverse` | `ColumnReverse` (default `Row`)) — Flex container: the main axis.
- `FlexWrap` (String, default "NoWrap"; one of: `NoWrap` | `Wrap` | `WrapReverse` (default `NoWrap`)) — Flex container: whether items that do not fit start a new line.
- `JustifyContent` (String, default "Start"; one of: `Start` | `Center` | `End` | `SpaceBetween` | `SpaceAround` | `SpaceEvenly` (default `Start`)) — Flex container: where the leftover main-axis space goes.
- `AlignItems` (String, default "Stretch"; one of: `Stretch` | `Start` | `Center` | `End` (default `Stretch`)) — Flex or grid container: how items sit across their line or cell.
- `AlignContent` (String, default "Stretch"; one of: `Stretch` | `Start` | `Center` | `End` | `SpaceBetween` | `SpaceAround` (default `Stretch`)) — Wrapping flex container: how the lines share the cross axis.
- `Gap` (Integer, default 0; form pixels ≥ 0 (default 0)) — Flex, grid or flow container: the space between items.
- `RowGap` (String, default (empty); form pixels, or empty (default) to use `Gap`) — Flex or grid container: the space between rows / between columns.
- `ColumnGap` (String, default (empty); form pixels, or empty (default) to use `Gap`) — Flex or grid container: the space between rows / between columns.
- `GridColumns` (String, default (empty); track list: `Npx` `N%` `Nfr` `Auto` `MinMax(a, b)` `Repeat(n, tracks)` `Repeat(AutoFill, MinMax(min, b))`) — Grid container: the column / row tracks, e.g. `200px 1fr 2fr`. An empty `GridRows` adds `Auto` rows as needed.
- `GridRows` (String, default (empty); track list: `Npx` `N%` `Nfr` `Auto` `MinMax(a, b)` `Repeat(n, tracks)` `Repeat(AutoFill, MinMax(min, b))`) — Grid container: the column / row tracks, e.g. `200px 1fr 2fr`. An empty `GridRows` adds `Auto` rows as needed.
- `JustifyItems` (String, default "Stretch"; one of: `Stretch` | `Start` | `Center` | `End` (default `Stretch`)) — Grid container: how items sit horizontally in their cell.
- `FlowDirection` (String, default "LeftToRight"; one of: `LeftToRight` | `TopDown` | `RightToLeft` | `BottomUp` (default `LeftToRight`)) — Flow container: the direction items run.
- `WrapContents` (Boolean, default 1; `1` (true) or `0` (false)) — Flow container: start a new line when the next item does not fit (default 1).

### Supported Events
- `onChildAdded` — a child control was added to the container
- `onChildRemoved` — a child control was removed from the container
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.

---

## Control: TabControl

Multi-page container with a tab strip. Default size 300×200 px.

**Content structure.** The pages are the `Tabs` property: one tab title per line, in order. A control sits on a page through its own `Tab` property (the 0-based page index) with this TabControl as its `Parent`, so reordering titles in `Tabs` does not move the controls with them: a page move also needs each child's `Tab` changed.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Tabs` (String, default "Tab1
Tab2"; one tab title per line) — The tab pages.
- `TabPosition` (String, default "Top"; one of: `Top` | `Bottom` | `Left` | `Right`) — Edge the tab strip sits on.
- `SelectedTab` (Integer, default 0; 0-based tab index) — Currently active tab. The operator clicking a tab header writes this, and writing it from COBOL turns the page exactly as a click does — the page a running form shows is always this value, never the one the form was designed with.
- `ActiveTabColor` (String, default "#2C6FD2FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Fill of the selected tab (default blue `#2C6FD2FF`). The selected tab flows into the page: its side against the page is straight, with no line between them, so this fill is what marks the active tab.
- `ActiveTabForegroundColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Text color of the selected tab's title. Empty (the default) = whichever of white or black reads on `ActiveTabColor` (white on the default blue). Settable from COBOL: `MOVE "#FFFFFFFF" TO TAB-1::ActiveTabForegroundColor`.
- `InactiveTabColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Fill of every tab that is not selected. Empty (the default) = a tone a step off the page's own surface.
- `InactiveTabForegroundColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Text color of the tabs that are not selected. Empty (the default) = the control's `ForegroundColor`, or black/white if that would not read on the tab.
- `TabPadding` (Integer, default 16; integer 0-64 (default 16)) — The space between a tab's title and its left and right edges; each tab is as wide as its title plus this on both sides. Tabs sit edge to edge and the strip joins the page with no gap. (Before 1.70.272 this was the gap between tabs.)
- `HScroll` (Boolean, default 0; `1` (true) or `0` (false)) — Panel, GroupBox, TabControl: horizontal scrolling when the children reach past the content area.
- `VScroll` (Boolean, default 0; `1` (true) or `0` (false)) — Panel, GroupBox, TabControl: vertical scrolling when the children reach past the content area.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `LayoutMode` (String, default "Absolute"; one of: `Absolute` | `Flex` | `Grid` | `Flow` (default `Absolute`)) — Form, Panel, GroupBox, TabControl: how the container places its children on a responsive form — anchors and docking, a flex row/column, a grid of tracks, or a wrapping flow. (The Viewer's own `Layout` is a different property.)
- `FlexDirection` (String, default "Row"; one of: `Row` | `Column` | `RowReverse` | `ColumnReverse` (default `Row`)) — Flex container: the main axis.
- `FlexWrap` (String, default "NoWrap"; one of: `NoWrap` | `Wrap` | `WrapReverse` (default `NoWrap`)) — Flex container: whether items that do not fit start a new line.
- `JustifyContent` (String, default "Start"; one of: `Start` | `Center` | `End` | `SpaceBetween` | `SpaceAround` | `SpaceEvenly` (default `Start`)) — Flex container: where the leftover main-axis space goes.
- `AlignItems` (String, default "Stretch"; one of: `Stretch` | `Start` | `Center` | `End` (default `Stretch`)) — Flex or grid container: how items sit across their line or cell.
- `AlignContent` (String, default "Stretch"; one of: `Stretch` | `Start` | `Center` | `End` | `SpaceBetween` | `SpaceAround` (default `Stretch`)) — Wrapping flex container: how the lines share the cross axis.
- `Gap` (Integer, default 0; form pixels ≥ 0 (default 0)) — Flex, grid or flow container: the space between items.
- `RowGap` (String, default (empty); form pixels, or empty (default) to use `Gap`) — Flex or grid container: the space between rows / between columns.
- `ColumnGap` (String, default (empty); form pixels, or empty (default) to use `Gap`) — Flex or grid container: the space between rows / between columns.
- `GridColumns` (String, default (empty); track list: `Npx` `N%` `Nfr` `Auto` `MinMax(a, b)` `Repeat(n, tracks)` `Repeat(AutoFill, MinMax(min, b))`) — Grid container: the column / row tracks, e.g. `200px 1fr 2fr`. An empty `GridRows` adds `Auto` rows as needed.
- `GridRows` (String, default (empty); track list: `Npx` `N%` `Nfr` `Auto` `MinMax(a, b)` `Repeat(n, tracks)` `Repeat(AutoFill, MinMax(min, b))`) — Grid container: the column / row tracks, e.g. `200px 1fr 2fr`. An empty `GridRows` adds `Auto` rows as needed.
- `JustifyItems` (String, default "Stretch"; one of: `Stretch` | `Start` | `Center` | `End` (default `Stretch`)) — Grid container: how items sit horizontally in their cell.
- `FlowDirection` (String, default "LeftToRight"; one of: `LeftToRight` | `TopDown` | `RightToLeft` | `BottomUp` (default `LeftToRight`)) — Flow container: the direction items run.
- `WrapContents` (Boolean, default 1; `1` (true) or `0` (false)) — Flow container: start a new line when the next item does not fit (default 1).

### Supported Events
- `onTabChanged` — the active tab changed
- `onTabClick` — a tab header was clicked
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.

---

## Control: DataGrid

Tabular rows/columns grid with sorting, filtering, freezing and CSV export. Default size 300×200 px.

**Content structure.** The columns are the `Columns` property, one `Name:Type` per line in display order (`string`, `number` or `datetime`); the cells are `Rows`, one row per line with cells separated by TAB, usually filled at run time. An agent changes the columns with `set_property` on `Columns`, sending every column.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Columns` (String, default (empty); one `Name:Type` per line; Type ∈ `string` | `number` | `datetime` (default `string`)) — Column definitions.
- `Rows` (String, default (empty); rows separated by newline, cells within a row by TAB) — Cell data (usually populated at runtime).
- `AlternatingRowColor` (String, default "#F0F8FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Tint applied to alternating rows/columns.
- `AlternatingRowOpacity` (Integer, default 20; 0-100 (percent)) — Strength of the alternating tint.
- `AlternatingMode` (String, default "Rows"; one of: `Rows` | `Columns` | `None`) — Axis the alternating highlight applies to.
- `HeaderBackgroundColor` (String, default "#E0E0E0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Header row fill.
- `HeaderForegroundColor` (String, default "#000000"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Header row text color.
- `FilterForegroundColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Filter row text color. Empty = the form theme decides.
- `FilterBackgroundColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Filter row field fill. Empty = the form theme decides.
- `GridLineColor` (String, default "#CCCCCC"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Grid line color.
- `GridBackgroundImage` (String, default (empty); image path or empty) — Watermark image behind the cells.
- `GridBackgroundPattern` (String, default "None"; one of: `None` | `Stripes` | `Dots` | `Cross` | `X` | `X Dots` | `O`) — Procedural background pattern.
- `RowBackgroundPattern` (String, default "None"; one of: `None` | `Stripes` | `Dots` | `Cross` | `X` | `X Dots` | `O`) — Per-row background pattern.
- `GridBackgroundImageMode` (String, default "Fill"; one of: `Fill` | `Fit` | `Stretch` | `Tile` | `Center`) — How the background image scales.
- `SelectionMode` (String, default "Row"; one of: `Row` (default) | `Cell` | `Column`) — What a click on a cell highlights: its whole row (`Row`), the cell alone (`Cell`) or its whole column (`Column`). Ctrl+C copies what is highlighted — the cell, the row's cells joined by `CSVDelimiter`, or the column's values in the rows shown, one per line. `onCellClick` reports the cell either way.
- `RowHeight` (Integer, default 22; integer 14-120 (default 22)) — The height of EVERY row, exactly — a grid's rows are uniform. Dragging a row edge (`AllowRowResize`) writes it, `SetRowHeight` sets it from COBOL, and so does a plain write of the property.
- `AllowSorting` (Boolean, default 1; `1` (true) or `0` (false)) — A click on a column title sorts the rows shown by that column — ascending, then descending on the next click — and a ▲/▼ marks it. A column declared numeric, or whose every value is a number, sorts by value (9 before 100); any other sorts as text, ignoring case. Display order only: `Rows` keeps its order, and a click still reports each row's own index. A column whose settings turn sorting off is not sorted. `onColumnClick` fires either way. The `Sort` method, by contrast, reorders `Rows` itself.
- `AllowColumnResize` (Boolean, default 1; `1` (true) or `0` (false)) — Drag header edges to resize.
- `AllowColumnReorder` (Boolean, default 1; `1` (true) or `0` (false)) — Shows a ‹ and a › button in each column title (on columns at least 58 points wide) that move the column one place left or right. Columns are not dragged.
- `AllowRowResize` (Boolean, default 1; `1` (true) or `0` (false)) — Drag row edges to resize.
- `AllowCellEditing` (Boolean, default 0; `1` (true) or `0` (false)) — DataGrid (default false): the operator may edit a cell in place — a double-click, or F2 on the selected cell, opens it in a text box; Enter or clicking away commits, Escape cancels. A commit that changed the text writes it into `Rows` and fires `onCellEdited` with `EditedRow`, `EditedColumn` (the DATA row and column, numbered from 1 — `GetCellValue`'s), `EditedValue` and `PreviousValue`; the handler validates and stores it, and can put `PreviousValue` back with `SetCellValue` to refuse it. A column with no data behind it, or one showing its value as an image, is not editable; a typed tab or line break becomes a space. The retired `ReadOnly` does not control this.
- `AdvancedGrid` (String, default (empty); internal serialized settings; leave empty) — Advanced designer-managed grid settings.
- `ShowRowNumbers` (Boolean, default 0; `1` (true) or `0` (false)) — Shows a gutter left of the columns numbering the rows as shown, from 1, in the header's colours. It takes its width from the columns rather than covering the first one, and stays put when the grid scrolls sideways.
- `ShowColumnFilters` (Boolean, default 0; `1` (true) or `0` (false)) — Shows the per-column filter row.
- `Title` (String, default (empty); free text) — Chart title — and, on a DataGrid, the caption centred on the band that carries the CSV button; empty means no caption. Also the window title on Form methods.
- `AutoFitColumns` (Boolean, default 0; `1` (true) or `0` (false)) — Make the columns fill the grid's width exactly instead of scrolling or leaving a gap. The difference is absorbed by the columns measured in points, in proportion to their size, so a column declared as a percentage keeps the share it asked for. Off by default: the columns keep their declared widths. Each column chooses its own unit in **Edit DataGrid settings…** — points for the narrow, predictable ones, a percentage for those that should follow the form.
- `ExportCSV` (Boolean, default 1; `1` (true) or `0` (false)) — Master switch for the built-in CSV button: off, the button is hidden even when `ShowCSVExportButton` is on. The `ExportCSV` method and the generated `<id>-EXPORT-CSV` paragraph work either way.
- `ShowCSVExportButton` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the built-in export button. It sits hard right on its own band above the column titles, so it never covers a column title; `Title` shares that band.
- `CSVDelimiter` (String, default ","; single character, default `,`) — CSV field delimiter.
- `CSVExportMode` (String, default "Filtered"; `Filtered` | `AllRows`) — Whether export honours active filters.
- `FrozenColumns` (Integer, default 0; integer ≥ 0) — Leading columns that do not scroll sideways. A write from COBOL takes effect at once, like `FreezeColumns` — also on a grid configured in **Edit DataGrid settings…**.
- `FrozenRows` (Integer, default 0; integer ≥ 0) — Leading rows that do not scroll. A write from COBOL takes effect at once, like `FreezeRows` — also on a grid configured in **Edit DataGrid settings…**.
- `FrozenShadow` (Boolean, default 1; `1` (true) or `0` (false)) — Soft shadow cast by frozen rows/columns.
- `GridLineStyle` (String, default "Solid"; one of: `Solid` | `Dash` | `Dots` | `DashDot` | `None` (`Dot` reads as `Dots`)) — How the grid's lines are drawn. A write from COBOL takes effect at once — also on a grid configured in **Edit DataGrid settings…**. A rounded grid's outer outline stays solid: a dashed stroke cannot follow a corner arc.
- `RowHeightOverrides` (String, default (empty); `row=height` pairs separated by `;` — rows numbered from 1 (e.g. `1=40;8=64`); empty = every row is `RowHeight`) — DataGrid: rows with a height of their own, 14–400 points; every other row stays `RowHeight`. The row is the DATA row, so a height stays with its row when the grid is sorted or filtered, and `Sort`, `DeleteRow` and `ClearRows` carry it along. Set it in the grid's settings, or from COBOL with `SetRowHeight(row, pixels)` (0 pixels hands the row back to `RowHeight`). Dragging the lower edge of such a row resizes that row alone; dragging any other row edge still resizes every row.
- `ColumnFilters` (String, default (empty); `column=value` pairs, one per line) — The active filters: a row shows only when each named column contains its value, ignoring case. Set in the designer, the grid starts filtered. The filter row, `SetFilter` and a COBOL write of the property all keep it current, and a write takes effect at once — also on a grid configured in **Edit DataGrid settings…**.
- `SelectableText` (Boolean, default 1; `1` (true) or `0` (false)) — Cell text can be selected/copied.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `EditedRow` — DataGrid: in `onCellEdited`, the data row the operator edited, numbered from 1.
- `EditedColumn` — DataGrid: in `onCellEdited`, the data column the operator edited, numbered from 1.
- `EditedValue` — DataGrid: in `onCellEdited`, the cell's new text — already in `Rows`.
- `PreviousValue` — DataGrid: in `onCellEdited`, what the cell held before the edit.
- `ClickedRow` — DataGrid: the data row a click (or double-click) landed on, numbered from 1 as `GetCellValue` counts — written just before `onCellClick` / `onCellDoubleClick`, so the handler knows which row's button was pressed.
- `ClickedColumn` — DataGrid: the data column a click landed on, numbered from 1 (its position in `Columns`); 0 for a column with no data behind it. Written just before `onCellClick` / `onCellDoubleClick`.

### Supported Events
- `onCellClick` — a cell was clicked — `ClickedRow` / `ClickedColumn` say which. A column whose kind is Button draws its cell value as a button; a value `icon:<name>` (e.g. `icon:pencil`, `icon:trash`) draws that catalogue icon instead, the usual way to give each row Edit and Delete buttons
- `onCellDoubleClick` — a cell was double-clicked
- `onCellEdited` — DataGrid with `AllowCellEditing`: the operator changed a cell — `EditedRow`, `EditedColumn`, `EditedValue`, `PreviousValue` say which and how; the new text is already in `Rows`
- `onRowSelect` — a row became selected
- `onRowDoubleClick` — a row was double-clicked
- `onColumnClick` — a column header was clicked
- `onSelectionChanged` — the selected item/cell set changed
- `onScroll` — content scrolled
- `onExportCSV` — the built-in CSV export ran
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `GetRowCount() → Integer` — Number of data rows.
- `GetCellValue(row: Integer, column: Integer) → String` — Read one cell. Rows and columns are numbered from 1, as COBOL numbers a table: `GetCellValue(1, 1)` is the first row's first cell, and the column is the DATA column (its position in `Columns`). 0 is also read as the first — so a program counting from 0 gets the right cell at 0 only, and from 1 on the one BEFORE the one it meant.
- `SetCellValue(row: Integer, column: Integer, value: String)` — Write one cell; row and column numbered from 1, as in `GetCellValue`.
- `AddRow(cells: String)` — Append a row; cells separated by TAB.
- `DeleteRow(row: Integer)` — Remove one row, numbered from 1.
- `ClearRows()` — Remove all rows.
- `Sort(column: Integer)` — Sort `Rows` itself by a column, numbered from 1.
- `SetFilter(column: String, value: String)` — Filter a column.
- `ClearFilters()` — Drop all column filters.
- `FreezeColumns(count: Integer)` — Freeze the first N columns.
- `FreezeRows(count: Integer)` — Freeze the first N rows.
- `SetRowHeight(pixels: Integer)` — Set the uniform row height. With two arguments, `SetRowHeight(row, pixels)`, gives one row (numbered from 1) its own height in `RowHeightOverrides`; 0 pixels hands it back to the uniform one.
- `SetColumnWidth(column: Integer, pixels: Integer)` — Set one column's width.
- `SetColumnTitle(column: String, title: String)` — Set one column's heading at run time — how a grid's headings follow the program's language (a designed title is one language only). Name the column by its id as designed in Edit DataGrid settings (a number is ambiguous there).
- `GetSelectedText() → String` — Text of the current selection.
- `CopySelection()` — Copy the selection to the clipboard.
- `ExportCSV() → String` — Serialise the grid as CSV.
- `RefreshBinding() → Integer` — Re-hydrate rows from the bound data source; returns the row count. A grid bound to a COBOL table or an indexed file also loads by itself as the form opens (after onLoad); call this after the program changes the table.

---

## Control: PictureBox

Displays a still image. Default size 120×120 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `ImagePath` (String, default (empty); project-relative or absolute image path) — Image file to display.
- `SizeMode` (String, default "Normal"; PictureBox: `Normal` | `Stretch` | `Zoom` | `CenterImage` | `AutoSize`; Animator: `Fit` | `Fill` | `Stretch` | `Center`) — How the image is scaled inside the control. PictureBox: Normal = its own size, shrunk only when it does not fit, placed by ImageAlignment; Zoom = as large as fits, aspect kept; Stretch = fills the box; CenterImage = its own size (shrunk to fit), centred; AutoSize = the control takes the image's own size.
- `ImageAlignment` (String, default "MiddleCenter"; anchor name, e.g. `MiddleCenter`, `TopLeft`, `BottomRight`) — PictureBox: where the image sits when it does not fill the box (SizeMode Normal or Zoom). CenterImage always centres; Stretch and Fill cover the box.
- `BorderStyle` (String, default "None"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `ShowFrame` (Boolean, default 0; `1` (true) or `0` (false)) — Draws the frame/background behind the image. A PictureBox placed in the designer starts with it off (only the image shows); a form saved without it keeps the frame.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onImageLoaded` — the image finished loading
- `onImageError` — the image failed to load
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.

---

## Control: Animator

Plays an animated image (GIF / WebP / APNG). Default size 160×120 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Source` (String, default (empty); path to GIF / WebP / APNG / still image) — Animated image the Animator plays.
- `AutoPlay` (Boolean, default 1; `1` (true) or `0` (false)) — Starts playing when the form loads.
- `Loop` (Boolean, default 1; `1` (true) or `0` (false)) — Restarts the animation when it ends.
- `SizeMode` (String, default "Fit"; PictureBox: `Normal` | `Stretch` | `Zoom` | `CenterImage` | `AutoSize`; Animator: `Fit` | `Fill` | `Stretch` | `Center`) — How the image is scaled inside the control. PictureBox: Normal = its own size, shrunk only when it does not fit, placed by ImageAlignment; Zoom = as large as fits, aspect kept; Stretch = fills the box; CenterImage = its own size (shrunk to fit), centred; AutoSize = the control takes the image's own size.
- `BorderStyle` (String, default "None"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onStarted` — animation started
- `onEnded` — animation reached its end
- `onFrameChanged` — animation advanced a frame
- `onLooped` — animation restarted a loop
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `Play() / PlayAnimation(name: String?)` — Start playing (optionally a named animation).
- `StopAnimation()` — Stop playing.
- `Pause()` — Pause playback.

---

## Control: ProgressBar

Shows progress within Minimum..Maximum. Default size 200×22 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Minimum` (Integer, default 0; integer ≤ Maximum) — Lower bound of the value range.
- `Maximum` (Integer, default 100; integer ≥ Minimum) — Upper bound of the value range.
- `Value` (Integer, default 0; ProgressBar/Slider/NumericUpDown: integer within Minimum..Maximum; DateTimePicker: `YYYY-MM-DD`, `HH:MM`, or `YYYY-MM-DD HH:MM`) — Current value. A DateTimePicker always stores ISO, whatever Format displays, so a COBOL handler reading Value gets one shape; which halves are present follows Format (a Time picker stores the time alone).
- `BarColor` (String, default "#00AA00"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Filled-portion color of the progress bar (how far it has travelled). The UNTRAVELLED part -- the trough -- is the control's BackgroundColor; left at its default either one follows the active theme.
- `Orientation` (String, default "Horizontal"; `Horizontal` | `Vertical`) — Layout axis.
- `Style` (String, default "Continuous"; `Continuous` | `Blocks`) — Progress bar fill style: one unbroken run, or a row of segments.
- `BlockSize` (Integer, default 0; integer ≥ 0 (px, 0 = automatic)) — Length of one block under `Style = Blocks`, along the axis the bar travels. 0 sizes each block from the bar's own thickness.
- `ShowValue` (Boolean, default 0; `1` (true) or `0` (false)) — Draws the progress as a PERCENTAGE of the range (e.g. `45%`), centred on the bar — not the raw Value.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#8C8CA0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `CornerRadius` (Integer, default 10; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onValueChanged` — value changed (the new value is delivered)
- `onCompleted` — Value reached Maximum
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetValue(value: Integer)` — Set the current value.
- `GetValue() → Integer` — Read the current value.
- `Increment()` — Add Step to Value.
- `Decrement()` — Subtract Step from Value.
- `Reset()` — Return Value to Minimum.

---

## Control: MenuBar

Window menu bar (menu structure is edited in the designer and stored in a `.menu.yaml` sidecar, not in a property). The item chosen — by a click or its accelerator — is read from the run-time property `SelectedItemId` in the `onMenuClick` handler (`MOVE MenuBar-1::SelectedItemId TO WS-ITEM`); `COBOL-CONTROL-ID` holds the MenuBar's own id, not the item's. The menu editor gives every new item a fixed id of four lowercase letters (e.g. `kqpv`), unique among all menu items and toolbar groups/buttons in the window; it cannot be edited, and the editor shows it beside the item's label with a Copy button for pasting into a handler. Opening an older MenuBar, SideMenu or ToolBar in its editor replaces every id that is not four lowercase letters, or that repeats another in the window, with a generated one (saved on OK), so COBOL naming an old id such as `item-3` or `button-2` — a WHEN on SelectedItemId or LastButton, a SideMenu AddItem parent-id — must be updated. Property `MenuBarStyle` (Free | Responsive, default Free) decides its width: Free is the width it was drawn at, Responsive runs the form's full width, follows a resize in the designer, and at run time spans the running window as the operator resizes it. Menu items may carry an icon from the built-in catalogue: 660+ pure-vector icons in 26 categories (documents, editing, navigation, commerce, payroll, receivables, payments, stock control, transportation, logistics, financial, company departments, transaction kinds, civilian vehicles, military equipment, and more). Icons are resolution-independent line work tinted by the item's colour; the engine can also apply a second accent colour, a drop shadow, or a neumorphic emboss. Default size 400×24 px.

**Content structure.** The menu items are NOT properties of the control. They are a tree stored in `<control id>.menu.yaml` beside the form, written by the menu editor (inspector button 'Edit Menu...') and protected by a hash, so a hand-edited file is refused. Each item has an `id` (four lowercase letters, fixed once given), a `label`, a `type` (`action`, or `separator` for a divider), an optional `icon`, an optional `action` and `enabled`; a group is an item with child `items`, at most 3 levels deep. Order in the file is display order. Actions: `open-form:<form>` loads that form into the ContentPane (it must be FormFormat Embedded or Both), `open-standalone-sync:<form>` and `open-standalone-async:<form>` open it in its own window (Standalone or Both), and `home` (SideMenu only) shows the shell form's own content. AN AGENT READS THE TREE from the `MENU ITEMS` block of its CONTEXT: one line per item, indentation is nesting, `(id ...)` is the item id and `-> ...` its action. No change-set operation edits it, and `set_property` cannot: moving, reordering, renaming or regrouping designed items is done in the menu editor, so say so to the developer and quote the items from `MENU ITEMS` rather than inventing an operation. At run time a program can add its own rows (`AddItem` and the other row methods) and relabel or disable designed ones, but never move a designed row.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `HighlightBgColor` (String, default "#4488FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Hovered menu item background.
- `HighlightFgColor` (String, default "#FFFFFF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — MenuBar: the text of the title under the pointer, and of the dropdown item under the pointer or flashing after a click.
- `SelectedBgColor` (String, default "#3366CC"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Open/selected menu background.
- `SelectedFgColor` (String, default "#FFFFFF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — MenuBar: the open title's text, on SelectedBgColor. SideMenu: the active row's text and icon, on its accent pill.
- `MenuBarStyle` (String, default "Free"; one of: `Free` | `Responsive`) — How the bar decides its own width. `Free` (the default) leaves it exactly as wide as it was drawn — the historical behaviour, so no existing form moves. `Responsive` pins it to the form's FULL WIDTH and keeps it there through a resize of the form in the designer, and each time the form is loaded — which is what a menu bar is normally expected to do. At run time it also follows the RUNNING window: widen the window and the bar widens with it to the right edge (never narrower than the form). No other control moves — a form keeps its designed layout. Only x and width are taken; the bar's Y and Height stay yours. It is the horizontal mirror of the SideMenu's `FullHeight`.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `SelectedItemId` — SideMenu: the id of the row the user last clicked — designed or added at run time — written just before `onMenuItemClick` fires. Writing it from COBOL highlights that row. MenuBar: the id of the item chosen by a click or its accelerator, written just before `onMenuClick` fires.

### Supported Events
- `onMenuClick` — a MenuBar item was chosen, by a click or its accelerator — the item's id is in the MenuBar's `SelectedItemId` property, written just before this event fires
- `onMenuItemClick` — a menu item was activated
- `onMenuOpen` — a menu opened
- `onMenuClose` — a menu closed
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.

---

## Control: SideMenu

Vertical sidebar menu (spec 049). On the MAIN form it puts the application in SHELL mode: one window with a MenuPane, a breadcrumb and a ContentPane. The menu structure is edited in the SAME menu editor a MenuBar uses (inspector button 'Edit Menu...') and stored in a `.menu.yaml` sidecar keyed by control id; a MenuBar deliberately does NOT trigger the shell, so existing projects keep classic multi-window mode. Property `FullHeight` (default true): true = the sidebar owns the window's whole vertical extent and the breadcrumb starts at its right edge; false = the breadcrumb spans the full width and the sidebar fills the height beneath it. While FullHeight is true the control's Y and Height are inert (greyed in the inspector, drawn down the form's full height in the designer and following a form resize); Width stays developer-set. Property `Collapsed` (default false) is the pane state the application OPENS in; the operator's own remembered choice (persisted per application) wins over it from then on. The sidebar also owns the BREADCRUMB FRAME, which always runs from its right edge to the window's right edge (no width or position property exists): `BreadcrumbHeight` (16..200, default 28) and `BreadcrumbBackgroundColor` (empty = follow the ContentPane's backdrop; alpha allowed, the frame is still painted opaque) and `BreadcrumbTextAlign` (Top | Middle | Bottom, default Middle — it places the chain AND the Open/Collapsed toggle together as ONE GROUP: the alignment moves the pair inside the frame and the chain then centres on the toggle's own line, so the text sits on the icon's middle at Top and at Bottom just as it does at Middle, however large the icon; their SIZES stay separate), `BreadcrumbFontSize` (0 = follow the rail's FontSize, the historical behaviour; otherwise 4..200) and `BreadcrumbIconSize` (0 = the toggle stays a square of the frame's height capped at 48, the historical behaviour; otherwise 8..200, never taller than the frame). THE FRAME'S HEIGHT, THE CHAIN'S TEXT SIZE AND THE TOGGLE'S SIZE ARE THREE SEPARATE DIALS: changing one moves nothing else. In particular the chain no longer has to share the rail's FontSize (that one property used to size the menu labels and the navigation chain together, so neither could be set alone), and raising `BreadcrumbHeight` to make room for your own controls no longer grows the toggle arrow with it. The frame's height is independent of the breadcrumb's font — a bigger FontSize never grows it, a smaller one never shrinks it, and text too big for the frame is CLIPPED by it rather than drawn outside — which is what gives `BreadcrumbTextAlign` something to do: on a frame taller than its text it puts the chain against the top, in the middle, or against the bottom. A form LOADED INTO THE CONTENTPANE starts BELOW the frame, never over it, so an embedded form's first row of controls can never land on the navigation chain. While `FullHeight` is on the frame OVERLAYS the top band of the SHELL form's own coordinate space, exactly as the designer canvas draws it, so THE SHELL FORM'S OWN CONTROLS MAY BE PLACED OVER IT — the frame is chrome, NOT a container: such a control is nobody's child, is not clipped by or scrolled with the frame, keeps every property and event, paints on top and takes the click. The Open/Collapsed control is painted at the TOP of the sidebar in the designer and at run time, in both pane states and whether or not the menu has items: the HEADER PANE is that control — clicking anywhere in it folds or unfolds the rail — and it shows the developer's `HeaderImage` open, their `HeaderIcon` collapsed. With NO `HeaderIcon` a collapsed pane used to paint nothing at all, so the rail looked like it could not be opened again; it now draws the fold/unfold arrow itself (1.65.103), which matters most in an EMBEDDED form, where there is no breadcrumb of its own above the rail carrying that control. A rail that HAS a HeaderIcon keeps showing it, since that mark is already visible and already clickable; the sidebar's ☰, items and empty hint are all top-anchored, never vertically centred. Menu-item ICONS render in the sidebar on every surface (designer canvas, preview, Run Form pane and the shell MenuPane). Property `IconEffect` (None | Shadow | Neumorphic, default None) styles those icons, and they are sized per rail state: `IconSize` (default 22) while the rail is OPEN and `IconSizeCollapsed` (default 22) while it is COLLAPSED, since an icon beside a label and an icon that IS the row are two different pictures; a form with no `IconSizeCollapsed` uses `IconSize` for both. Property `CollapsedWidth` (24-200, default 48) is how wide the COLLAPSED icon rail is — one value on every surface (running shell pane, designer canvas, preview), while the OPEN pane stays as wide as the control was drawn. EXPANDED, a group's items are indented under it one level at a time, the whole row moving together so an item's icon stays beside its own label at every level. COLLAPSED, the rail carries an item when, and only when, it has an icon, has an action and is not a group — a group is dropped and its qualifying children come up in its place, flattened from wherever they sit, so the rail is the shortcuts rather than the structure; section dividers survive only between two icons. On that rail an item whose action is `home` is followed by a whole row's worth of extra space, so the distance from it to the icon below is twice the distance between any other two; it is the ACTION that earns the space, never the label, and nothing is added where a divider already falls beneath it. In preview and Run Form the sidebar is LIVE: the ☰ toggles the rail (firing onMenuOpen/onMenuClose) and item rows click (SelectedItemId + onMenuItemClick). THE OPEN/COLLAPSED CONTROL IN A STAND-ALONE WINDOW (1.65.102): a form carrying a SideMenu opens as a SHELL when it is the root — `rcrun run-form`, a built application — and the shell's breadcrumb carries that control at its head. Opened as a CHILD WINDOW by another form's sidebar (a menu item's `Open Stand Alone Form (Sync)`/`(Async)`, or the `OpenStandAloneFormSync`/`Async` methods) the same form is a plain window with no shell over it, and it had no chrome at all: the rail could still be folded by clicking its header, but there was nothing to see or aim at. Such a window now draws the breadcrumb strip itself, with a live toggle and ONE STATIC SEGMENT naming the form — a navigation chain is a fact of the SHELL, and a child window is not in one, so there is nothing else to honestly show. A form with no SideMenu draws no strip and is unchanged. The menu editor's Indent/Outdent buttons restructure items across sections and levels (3 levels max). Menu-item ACTIONS (spec 051): `Open form` loads the target into the ContentPane as its own program instance (target must be FormFormat Embedded or Both); `Open Stand Alone Form (Sync)`/`(Async)` open the target in its OWN window, same process, parented to the shell — Sync is implicitly modal (the whole shell face waits until the child closes), Async is modeless (target must be Standalone or Both); the Target picker lists only the forms the chosen action may load. `Home (main content pane)` takes NO target and opens nothing: it puts the shell form's OWN ContentPane content back on screen, so a 'main screen' needs no form of its own. Home PARKS rather than destroys — the outgoing occupant gets onDeactivate but no onDestroy, keeps its WORKING-STORAGE, and a later load of it revives that same instance; every other live form, child windows included, is untouched. The breadcrumb collapses to the shell form and the contextual menu section empties; Home while already home does nothing. Home is offered on a SideMenu only, since a MenuBar form has no ContentPane to restore. The control also exposes the methods `OpenStandAloneFormSync`/`OpenStandAloneFormAsync` (see its Methods) for opening those windows from COBOL. BADGES: a designed row can carry a short tag at its right edge (an unread count, "New"), set in the menu editor's Badge field (empty = none) with a Badge style of Pill (default, filled rounded tag), Count (filled circle, for a number) or Outline (accent outline, no fill); only a SideMenu draws badges, so the MenuBar editor does not offer the fields, and SetItemBadge reaches only rows the program added. ROWS ADDED AT RUN TIME (spec 066): a program adds its own rows — a list of conversations, the documents in a folder — with `AddItem(id, label [, icon [, parentId [, action]]])`, and changes or removes them with `SetItemLabel`/`SetItemIcon`/`SetItemBadge`/`SetItemEnabled`/`SetItemAction`, `RemoveItem` and `Clear`. They appear after the designed rows, click exactly like designed ones (`SelectedItemId` + `onMenuItemClick`, or their action in the shell), and exist only while the program runs: the `.menu.yaml` and the designer canvas never see them. A designed row can never be removed or replaced from COBOL, nor have its icon, badge or action changed — such a call answers `0` and leaves the menu untouched. Two things about a designed row ARE the program's: its LABEL (`SetItemLabel`, so a menu designed in the RAD — visible in the designer and the preview — can follow the interface language) and whether it is ENABLED (`SetItemEnabled`, so a menu can stay shut until the application is set up; a disabled row ignores clicks). `Clear` removes only the program's own rows and keeps what it set on designed ones; `GetCount` counts only the program's rows. THE FOOTER PANEL: every SideMenu owns a Panel in its footer band (id `<sidemenu-id>-Footer`), and it is an ordinary container the developer fills — a clock, a user badge, a version string, a Log-out button — styled through the inspector like any other, with events that fire normally. Its RECT is not the developer's: it is re-pinned to the footer band on every change, so it follows a form resize, a `FooterHeight` edit and a collapse; tell a developer to size it with `FooterHeight`, never to drag it. In a SHELL the rail is chrome beside the ContentPane, so the footer Panel and its contents are drawn by the RAIL rather than with the form's content — invisible to the developer (a control sits where the designer showed it) but the reason a footer control's designed X is not measured from the form's left edge. Default size 200×400 px.

**Content structure.** The menu items are NOT properties of the control. They are a tree stored in `<control id>.menu.yaml` beside the form, written by the menu editor (inspector button 'Edit Menu...') and protected by a hash, so a hand-edited file is refused. Each item has an `id` (four lowercase letters, fixed once given), a `label`, a `type` (`action`, or `separator` for a divider), an optional `icon`, an optional `action` and `enabled`; a group is an item with child `items`, at most 3 levels deep. Order in the file is display order. Actions: `open-form:<form>` loads that form into the ContentPane (it must be FormFormat Embedded or Both), `open-standalone-sync:<form>` and `open-standalone-async:<form>` open it in its own window (Standalone or Both), and `home` (SideMenu only) shows the shell form's own content. AN AGENT READS THE TREE from the `MENU ITEMS` block of its CONTEXT: one line per item, indentation is nesting, `(id ...)` is the item id and `-> ...` its action. No change-set operation edits it, and `set_property` cannot: moving, reordering, renaming or regrouping designed items is done in the menu editor, so say so to the developer and quote the items from `MENU ITEMS` rather than inventing an operation. At run time a program can add its own rows (`AddItem` and the other row methods) and relabel or disable designed ones, but never move a designed row.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `HighlightBgColor` (String, default "#4488FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The hovered row's background, laid down as a soft TINT (22 % of the colour) so a hover never reads as the active row, which wears SelectedBgColor solid. Empty falls back to a tint of SelectedBgColor.
- `HighlightFgColor` (String, default "#FFFFFF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The hovered row's text and icon — used where it clears WCAG AA (4.5:1) against what is actually behind it (the tint over the rail over the form); where it would not, the row keeps ForegroundColor, so the seeded white never vanishes on a pale rail.
- `SelectedBgColor` (String, default "#3366CC"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Open/selected menu background.
- `SelectedFgColor` (String, default "#FFFFFF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — MenuBar: the open title's text, on SelectedBgColor. SideMenu: the active row's text and icon, on its accent pill.
- `FullHeight` (Boolean, default 1; `1` (true) or `0` (false)) — SideMenu only. True (default): the sidebar spans the window's whole height, its Y and Height are inert, and the breadcrumb starts at its right edge. False: the breadcrumb spans the full width and the sidebar fills the height beneath it.
- `Collapsed` (Boolean, default 0; `1` (true) or `0` (false)) — SideMenu only. The pane state the application opens in: collapsed to the icon rail or open (default). At run time the operator's own remembered choice wins over it.
- `IconEffect` (String, default "None"; one of: `None` | `Shadow` | `Neumorphic`) — SideMenu only. How menu-item icons and the breadcrumb toggle are painted: flat (None, the default), with a drop shadow, or with neumorphic relief.
- `IconSize` (Integer, default 22; points (default 22; below 4 is ignored)) — Menu-item icon size while the rail is OPEN (see IconSizeCollapsed for the collapsed rail).
- `IconSizeCollapsed` (Integer, default 22; pixels, 8-64 (SideMenu only)) — Menu-item icon size while the sidebar is COLLAPSED — its own value because the two rail states are two designs: open, the icon sits beside a label; collapsed, the icon IS the row. Unset (a form designed before this property existed) falls back to IconSize.
- `CollapsedWidth` (Integer, default 48; points, 24-200 (SideMenu only), default 48) — How wide the COLLAPSED icon rail is, on every surface — the running shell's MenuPane, the designer canvas and the preview all narrow the rail to this one value, so the rail the developer designs against is exactly the rail their users see. The OPEN width stays the control's own drawn Width. Values under 24 are raised to 24 (below it an icon row has nothing to fit in); unset (a form designed before this property existed) falls back to 48, the width the rail has always collapsed to.
- `AppTitle` (String, default (empty); free text or empty) — SideMenu only. Application title drawn in the accent colour (SelectedBgColor) in the header of an OPEN sidebar, beside the logo box. The title gets its room first (up to 60 % of the header) and the logo box shrinks, keeping its shape and moving to the left, into what is left; a box narrower than 24 pt is not drawn and the title takes the header. Never shown on a collapsed rail.
- `HeaderHeight` (Integer, default 120; points ≥ 0 (default 120)) — SideMenu only. Height of the header band that carries the logo (HeaderImage, or HeaderIcon when collapsed). Clicking anywhere in it folds or unfolds the rail; it is never resized at run time.
- `FooterHeight` (Integer, default 72; points ≥ 0 (default 72)) — SideMenu only. Height of the footer band; the footer Panel (`<id>-Footer`) is re-pinned to it on every change. 0 removes the footer.
- `BreadcrumbHeight` (Integer, default 28; points ≥ 16 (default 28)) — SideMenu only. Height of the breadcrumb frame. Independent of the font: text too big is clipped by the frame, and a form loaded into the ContentPane starts below it.
- `BreadcrumbBackgroundColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`, or empty) — SideMenu only. Background of the breadcrumb frame the sidebar owns. Empty (the default) follows the ContentPane's backdrop; alpha is accepted but the frame is still painted opaque.
- `BreadcrumbTextAlign` (String, default "Middle"; one of: `Top` | `Middle` | `Bottom`) — SideMenu only. Places the breadcrumb chain and its toggle, as one group, against the top, in the middle (default) or against the bottom of the frame.
- `BreadcrumbFontSize` (Integer, default 0; points 4–200, or 0) — SideMenu only. Text size of the breadcrumb chain. 0 (the default) follows the sidebar's FontSize; any other value sets the chain alone without moving the menu labels.
- `BreadcrumbIconSize` (Integer, default 0; points 8–200, or 0) — SideMenu only. Size of the Open/Collapsed toggle at the head of the breadcrumb, never taller than the frame. 0 (the default) keeps it a square of the frame's height, capped at 48.
- `HeaderImage` (String, default (empty); image path or empty) — SideMenu only. The logo at the top of an OPEN sidebar. Its box is 270x80 points and that box is a LIMIT, not a shape to fill: a smaller logo is drawn at its own size, centred, and a bigger one is scaled down to fit keeping its aspect ratio (540x80 draws 270x40; 270x240 draws 90x80). Empty outlines the box instead. A collapsed rail shows HeaderIcon, not this.
- `HeaderIcon` (String, default (empty); image path or empty) — SideMenu only. The mark shown 45x45, centred, in the header of a COLLAPSED rail. Empty draws the fold/unfold arrow instead, so the rail can always be reopened.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `RuntimeRows` — SideMenu: the rows the program added at run time, after the designed ones. Never set it by hand — use `AddItem`, `RemoveItem`, `SetItem…`, `AddSection` and `Clear`, which keep it well-formed and refuse edits to designed rows. Never saved in the `.cfrm`.
- `SelectedItemId` — SideMenu: the id of the row the user last clicked — designed or added at run time — written just before `onMenuItemClick` fires. Writing it from COBOL highlights that row. MenuBar: the id of the item chosen by a click or its accelerator, written just before `onMenuClick` fires.

### Supported Events
- `onMenuClick` — a MenuBar item was chosen, by a click or its accelerator — the item's id is in the MenuBar's `SelectedItemId` property, written just before this event fires
- `onMenuItemClick` — a menu item was activated
- `onMenuOpen` — a menu opened
- `onMenuClose` — a menu closed
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddItem(id: String, label: String, icon: String, parentId: String, action: String) → 1/0` — Add a row after the designed ones — or, when a row the program added already has this id, replace it where it stands. Only `id` and `label` are required: `icon` names a catalogue icon, `parentId` hangs it as the LAST child of that row (designed or added; three levels at most), and `action` is what a click does, exactly as on a designed row (`open-form:<FORM>`, `open-standalone-sync:<FORM>`, `open-standalone-async:<FORM>`, `home`, `close-application`; empty = `event`, which raises `onMenuItemClick`). Actions navigate in the shell's MenuPane; a SideMenu in a plain window raises `onMenuItemClick` for every row, designed or added. Answers `0` and changes nothing for a designed row's id, an unknown parent, or a fourth level.
- `AddSection(title: String) → id` — Add a section title after the rows; answers its generated id (`section-1`, …), which `RemoveItem` accepts.
- `RemoveItem(id: String) → 1/0` — Remove a row the program added, and every row under it. `0` for a designed row or an unknown id.
- `SetItemLabel(id: String, label: String) → 1/0` — Rename a row — the program's own, or a designed one (how a designed menu follows the interface language). `0` for an unknown id.
- `SetItemIcon(id: String, icon: String) → 1/0` — Change (or, empty, clear) a run-time row's icon.
- `SetItemBadge(id: String, text: String) → 1/0` — Show a badge ("New", a count) on a run-time row; empty removes it.
- `SetItemEnabled(id: String, enabled: Boolean) → 1/0` — Grey a row out, or back — the program's own or a designed one, so a menu can stay shut until the application is set up. A disabled row ignores clicks.
- `SetItemAction(id: String, action: String) → 1/0` — Change what clicking a run-time row does (empty = `event`).
- `Clear()` — Remove every row the program added. The designed menu stays exactly as designed.
- `GetCount() → Integer` — How many rows the program has added (sections included).
- `HasItem(id: String) → 1/0` — Whether a row with this id exists — designed or added.
- `ActivateItem(id: String) → 1/0` — Do what a click on that row does — open its form in the ContentPane (`open-form:`), go `home`, raise `onMenuClick` — from code, e.g. to show a welcome form on first run. Shell only (a main form whose SideMenu is its menu pane). `0` for an unknown id; a disabled row does nothing.
- `OpenStandAloneFormSync(formId: String, windowState: String, x: Integer, y: Integer, width: Integer, height: Integer, modal: Boolean)` — Open `formId` in its OWN window, parented to the SHELL (whatever form invokes it), and BLOCK the calling handler until the child closes — Sync is implicitly modal, and the whole shell face waits with it. The space form requires every parameter; the comma form `SideMenu-1::"OpenStandAloneFormSync"("REPORT")` defaults the rest from the target's RAD design. The target's FormFormat must be Standalone or Both (build-checked for literal ids). RETURNING is NULL by the time the call resumes (the child is closed).
- `OpenStandAloneFormAsync(formId: String, windowState: String, x: Integer, y: Integer, width: Integer, height: Integer)` — Open `formId` in its OWN window, parented to the shell, and return at once. RETURNING binds a windowHandler that drives the child (`Focus`, `Close`, `SetProperty`, …) and becomes NULL when it closes. Never modal. Same parameter rules and FormFormat gate as the Sync form.

---

## Control: ToolBar

Groups of buttons in a horizontal strip. Each group is a frame with its own border and corner radius, separated from the next by an invisible gap; each button carries an icon, its own colours and an action. Built in the designer's Toolbar Editor (`ToolbarLayout`), not from a property list. THE BAR'S OWN FRAME is separate from the groups: `BackgroundColor`, `BorderStyle`, `BorderColor`, `BorderWidth`, `CornerRadius`, `Transparency`. A new toolbar is rounded at 10, has no border and is 100 % transparent, so it reads as buttons on the form. GIVING IT A BackgroundColor TURNS THE FRAME ON — the developer does NOT also have to lower `Transparency`, because that seeded 100 is what every toolbar carries rather than something anyone chose; a Transparency they did move still fades the face. The colour named is the colour painted, never substituted by the active theme's own card fill. Default size 400×32 px.

**Content structure.** The buttons are in the `ToolbarLayout` property: JSON written by the Toolbar Editor, holding the groups in order, each group's frame and button defaults, and each button's label or icon, tooltip, enabled state and action. An agent reads it in the control's `CONTROLS` line. Change it in the Toolbar Editor; a `set_property` on `ToolbarLayout` replaces the whole definition, so it must carry every group and button, not only the changed one.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Items` (String, default (empty); newline-separated entries (TreeView: two-space indentation nests children)) — The list content, one item per line.
- `CornerRadius` (Integer, default 10; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "None"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `ToolbarLayout` (String, default "{"groups":[{"id":"group-1","label":"File","buttons":[{"id":"button-1","tooltip":"Open","icon":"folder-open"}],"border_style":"None","border_width":1,"corner_radius":10,"padding":6,"separator_width":12}],"button_gap":4}"; serialised toolbar definition (edited in the Toolbar Editor, not by hand)) — A ToolBar's groups of buttons: each group's frame (border style/colour/width, corner radius, padding, background, separator), the DEFAULT appearance for that group's buttons, and each button's label-or-icon, tooltip, enabled state, action and its own appearance overrides. A group's appearance settings are the defaults for every button in it, and a button's own values win field by field — so an icon size set once on the group dresses all six buttons. A button carries a label OR an icon, never both: setting one clears the other. Adding a button in the editor copies the previous button's appearance (never its icon, tooltip or action). Set it all through the designer's Toolbar Editor. Absent, a populated `Items` is read as one unframed group of labelled buttons, so a toolbar built before groups existed still works. Corner radius defaults to 10; every colour defaults to unset, meaning the group, then the theme, decides.

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `LastButton` — Which toolbar button was pressed last. Written before `onClick` fires, so ONE handler can serve a whole toolbar: `EVALUATE TOOLBAR-1::LastButton`.

### Supported Events
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddItem(text: String)` — Append one item.
- `RemoveItem(text: String)` — Remove the first item equal to `text`.
- `GetSelected() → String` — Read the selected item's value.
- `GetSelectedIndex() → Integer` — Read the 0-based selected index (-1 = none).
- `SetSelectedIndex(index: Integer)` — Select by 0-based index.
- `GetCount() → Integer` — Number of items.
- `Clear()` — Remove all items (a ComboBox / ListBox also clears its selection: SelectedIndex -1, Value empty).

### Groups of buttons, built in the Toolbar Editor
A ToolBar is not a list of words — it is groups of buttons. Its whole definition lives in the `ToolbarLayout` property, edited in the designer (properties pane → **Edit Toolbar…**), never written by hand. Each group is a frame with a border style (`Single`/`None`/`Fixed3D`), border colour and width, corner radius, its own padding and an optional invisible separator after it; `None` still groups but draws no frame. Each button has a label OR an icon (never both — setting one clears the other), a tooltip, an enabled flag, an action, and an appearance: icon size and colour, width/height, corner radius, a solid or gradient background, a foreground colour and a drop shadow. Corner radius defaults to 10.

### Three levels of appearance
A button's own value wins; where it says nothing its GROUP decides; where the group says nothing too, the form's theme does. So an icon size or a background set once on the group dresses every button in it, and one button can still disagree field by field. Adding a button in the editor copies the previous button's appearance — never its icon, tooltip or action, which are what make it a different button.

### The bar's own frame
Separately from the groups, the ToolBar control itself has `BorderStyle`, `BorderColor`, `BorderWidth`, `CornerRadius`, `Transparency` and `BackgroundColor`. A new toolbar is rounded at 10, has NO border and is 100 % transparent, so it reads as buttons sitting on the form rather than a panel laid over it — and it arrives holding one group with one folder-open button, so a dropped ToolBar shows what a toolbar is.

### What a press does
`event` fires the toolbar's own `onClick` (the default). `procedure:<NAME>` runs one of the form's procedures by name. `open-modal:<FORM>` opens a STANDALONE form as a modal window, and the press waits until that window closes. The rest are the platform's: `print:<path>` opens the document where its print dialog is, `share` captures the form's window and hands the image to the OS, `screenshot` puts that image on the clipboard, `copy`/`cut`/`paste` use the OS clipboard on whichever control has focus, `run-app:<path args>` launches an application, `open-terminal:<dir>` opens a terminal.

Whatever the action, the form ALSO gets an `onClick` on the toolbar, and `LastButton` names the button that was pressed — written first, so one handler can serve the whole bar:

```cobol
TOOLBAR-1--ONCLICK.
EVALUATE TOOLBAR-1::LastButton
WHEN "bnsq"  PERFORM SAVE-RECORD
WHEN "dlrx"  PERFORM DELETE-RECORD
WHEN OTHER   CONTINUE
END-EVALUATE.
```

Every group and button has a fixed id of four lowercase letters (e.g. `bnsq`), given when it is added, unique among all toolbar and menu items in the window, and shown in the Toolbar Editor with a Copy button; it cannot be edited. Opening an older toolbar converts `group-N`/`button-N` ids to generated ones (saved on Save), which also changes each button's derived name.

`run-app` and `open-terminal` start a real process: the target is split on whitespace and handed to the OS DIRECTLY, never to a shell, so a target built from a data item cannot become a shell command. A toolbar wider than its control loses whole groups off the end rather than drawing half of one. A ToolBar with only a legacy `Items` list is read as one unframed group of labelled buttons, so it keeps working untouched.

### A button's own handler
A button carries its OWN code, not just the toolbar's one `onClick`. In the Toolbar Editor select a button, and under **Events** bind `onClick` with **Edit code** — that keeps the toolbar (as Save would) and opens the COBOL editor on the handler; saving puts it back into the toolbar. `onClick` is the only event offered, because it is the only one a button can raise. Where a button has more than one thing to run, the order is fixed: the TOOLBAR's `onClick` first, then the BUTTON's own `onClick`, then its action — so an `open-modal:` button whose handler prepares what the modal reads works as written.

### Changing a button while the form runs
COBOL may write a button's COLOURS and its TOOLTIP, and nothing else: `Tooltip`, `BackgroundColor`, `ForegroundColor`, `IconColor`, `GradientStartColor`, `GradientEndColor`, `ShadowColor`. A colour set to SPACES goes back to inheriting (group, then theme), the same meaning the editor's ✕ has. `MOVE "#204080FF" TO TOOLBAR-1-FMTG-BNSQ::BackgroundColor.`

Anything else — width, height, corner radius, label, icon, enabled, action — is a RUNTIME ERROR naming the property and the allowed set, through all three doors (`x::Prop`, `COBOL::"SET-PROPERTY"( … )` — the same as `CALL "COBOL-SET-PROPERTY"` — and `INVOKE x "SetProperty"`). A button is laid out BY ITS TOOLBAR, so a button that could move itself would leave nothing to put it back; a silent no-op would be worse. Reads are never refused. The COBOL editor flags a refused property as it is typed.

### How a button reaches COBOL
A toolbar button is NOT a control — the toolbar owns the layout, so a button has no entry in `form.controls`. It is named by a DERIVED id instead, `<toolbar>-<group>-<button>` upper-cased: `TOOLBAR-1` + `fmtg` + `bnsq` ⇒ `TOOLBAR-1-FMTG-BNSQ`. The press arrives under that id and the generated event loop dispatches on it, which is how `procedure:` and `open-modal:` reach anything — a `procedure:` button becomes `CALL "<NAME>"` (a user procedure is a nested program, IS COMMON) and an `open-modal:` button becomes `INVOKE ME::"OpenFormSync"("<FORM>")`, whose one-argument form is modal. Nothing types the derived id by hand.

A ToolBar's buttons belong to the form HOLDING the toolbar, Standalone or Embedded alike: they are seeded as objects of that form's program (one builder serves the root form, a child window and a ContentPane occupant), so an embedded form's own COBOL reads and recolours its own buttons and two forms with identically-named toolbars never see each other's. A toolbar nested inside a Panel or a tab page is no different.

`COBOL-CONTROL-ID` is `PIC X(64)`, so the three names together must fit 64 characters. A button whose derived id is longer, or a `procedure:`/`open-modal:` button naming nothing, gets a COMMENT in the generated source saying which button it is and what to fix — never a `WHEN` that could not fire.

### Pressing a button in Preview
Preview honours the platform actions — `print`, `run-app`, `open-terminal`, `copy`, `cut`, `paste` — so a toolbar can be tried while it is being built; every press writes its result (or its reason for failing) to the Output pane. The two CAPTURES do not run there: Preview is a pane inside the IDE window, so `screenshot` and `share` would return a picture of the IDE rather than of the form, and they say so instead. Run Form gives the form a window of its own to capture. The three COBOL actions (`event`, `procedure:`, `open-modal:`) need the interpreter, so they too belong to Run Form.

### What a platform press reports at run time
Every platform press shows its outcome — "Copied 5 character(s) from TXT-1", or the reason it failed — as a brief notice at the bottom of the form's own window, so a press never appears to do nothing. A toolbar living in a SideMenu's footer panel carries out platform actions like any other.

### copy / cut / paste — what they act on
They act on the text field that had focus WHEN the button was pressed: the press itself is a click elsewhere, which takes the field's focus away, so it is the field the user was in that counts. Each verb then hands the focus BACK with the caret where the edit ended, so typing continues where it left off.

`copy` takes **the selection** when there is one, leaving the caret right after the last character copied; with nothing selected it takes the whole field. `cut` follows the same rule and removes what it took, leaving the caret where the removed text began. `paste` **replaces the selection** when there is one, and otherwise **inserts at the caret** — the caret ends right after the last character pasted. With no field focused at all, `paste` changes nothing and says so. An untouched field yields its designed text. The rules count CHARACTERS, not bytes, so accented and CJK text is never cut through the middle of a character.

---

## Control: StatusBar

Bottom status strip. TWO RULES IT ENFORCES ITSELF, with no property to set and no way to opt out (operator, 2026-09-09). ITS WIDTH IS THE WINDOW'S: X is 0 and Width is the form's width, on every surface, and it follows a form resize on its own — at run time it spans the running window as the operator widens it — a status bar reports on the window, so a strip narrower than the window is not one. Y and Height stay the developer's: where along the bottom it sits, and how tall it is, are still theirs. In the designer its left and right resize knobs are therefore not offered (only the top and bottom ones are), and X and Width are greyed in the inspector — the numbers stay readable, they just are not editable. This is NOT the MenuBar's `MenuBarStyle`, which is opt-in and defaults to Free; the status bar has no such choice. IT IS NEVER INSIDE A CONTAINER: dropping or dragging one over a Panel, a GroupBox, a Splitter pane or a TabControl page parents it to the FORM instead, and no container highlights as a drop target while one is dragged. A form whose XML nests a status bar in a container is repaired on load — the bar is moved out to the form, keeping every property, event handler and its id. Do not tell a developer to place one in a container, and do not offer Width or X as settable on it. Default size 400×22 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Items` (String, default (empty); newline-separated texts) — The texts the bar shows, left to right, one per line — in the bar's own ForegroundColor, font and font styles, on its designed face, on the designer canvas and in the running form alike.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Supported Events
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddItem(text: String)` — Append one item.
- `RemoveItem(text: String)` — Remove the first item equal to `text`.
- `GetSelected() → String` — Read the selected item's value.
- `GetSelectedIndex() → Integer` — Read the 0-based selected index (-1 = none).
- `SetSelectedIndex(index: Integer)` — Select by 0-based index.
- `GetCount() → Integer` — Number of items.
- `Clear()` — Remove all items (a ComboBox / ListBox also clears its selection: SelectedIndex -1, Value empty).

---

## Control: Line

Decorative straight line. Default size 200×4 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `LineColor` (String, default "#000000"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Connector/line color (TreeView, Line, Shape). On a Splitter, the division line between its two panes; empty = the form theme's own rule colour.
- `LineThickness` (Integer, default 1; pixels > 0) — Stroke thickness.
- `LineDirection` (String, default "Horizontal"; `Horizontal` | `Vertical` | `Diagonal`) — Axis the Line control draws along.
- `DashStyle` (String, default "Solid"; one of: `Solid` | `Dash` | `Dot` | `DashDot`) — Line dash pattern.
- `RoundedEnds` (Boolean, default 0; `1` (true) or `0` (false)) — Rounds the line end caps.

### Supported Events
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.

---

## Control: DateTimePicker

Date/time input with calendar or spinner. Default size 200×24 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `EnterAsTab` (Boolean, default 1; `1` (true) or `0` (false)) — Enter moves the focus to the next control in the tab order, as Tab does; `onEnterPressed` still fires. On by default for TextBox, ComboBox, NumericUpDown, DateTimePicker, CheckBox and RadioButton. Ignored by a multi-line TextBox (Enter is its new line), and an open ComboBox list keeps Enter to pick its item. An Enter that moves the focus does not also press the form's default button.
- `Value` (String, default (empty); ProgressBar/Slider/NumericUpDown: integer within Minimum..Maximum; DateTimePicker: `YYYY-MM-DD`, `HH:MM`, or `YYYY-MM-DD HH:MM`) — Current value. A DateTimePicker always stores ISO, whatever Format displays, so a COBOL handler reading Value gets one shape; which halves are present follows Format (a Time picker stores the time alone).
- `Format` (String, default "Short"; one of: `Short` | `Long` | `Time` | `Custom`) — Which halves of a date-time the picker edits and how it shows them. `Short` shows the date as `YYYY-MM-DD`; `Long` as a long date (`Thursday, 3 September 2026`); both are the date alone and the popup is a calendar. `Time` is the time alone: the popup is an hour/minute clock. `Custom` is decided by CustomFormat's own letters. It never changes how Value is stored — that is always ISO.
- `CustomFormat` (String, default (empty); format pattern, e.g. `dd/MM/yyyy HH:mm`) — Pattern used when Format = `Custom`: the field SHOWS the value laid out through it — `yyyy` `yy` `MMMM` `MMM` `MM` `M` `dddd` `ddd` `dd` `d` `HH` `H` `hh` `h` `mm` `m` `tt`, anything else as written — and an empty field shows the pattern as its hint. Its letters also decide what the popup offers: `y`/`M`/`d` ask for a calendar, `H`/`h`/`m` for a clock, both for both. Case matters here and nowhere else on this control — `M` is the month, `m` the minute. A pattern naming neither falls back to a date.
- `ShowUpDown` (Boolean, default 0; `1` (true) or `0` (false)) — ▲▼ steppers instead of the popup: ▲ / ▼, the ↑ / ↓ keys while the picker has the keyboard, or the wheel over it step the DAY (the minute, on a time-only picker), within MinimumDate..MaximumDate.
- `MinimumDate` (String, default (empty); `YYYY-MM-DD` or empty) — Earliest date the operator can pick: earlier days are dimmed and refuse a click, and a stepped or committed date never goes below it. A Value set from COBOL is kept as written.
- `MaximumDate` (String, default (empty); `YYYY-MM-DD` or empty) — Latest date the operator can pick: later days are dimmed and refuse a click, and a stepped or committed date never goes above it. A Value set from COBOL is kept as written.
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Supported Events
- `onChange` — value or text changed
- `onValueChanged` — value changed (the new value is delivered)
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetValue(value: Integer)` — Set the current value.
- `GetValue() → Integer` — Read the current value.
- `Increment()` — Add Step to Value.
- `Decrement()` — Subtract Step from Value.
- `Reset()` — Return Value to Minimum.

---

## Control: NumericUpDown

Integer input with spinner arrows. Default size 120×24 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `EnterAsTab` (Boolean, default 1; `1` (true) or `0` (false)) — Enter moves the focus to the next control in the tab order, as Tab does; `onEnterPressed` still fires. On by default for TextBox, ComboBox, NumericUpDown, DateTimePicker, CheckBox and RadioButton. Ignored by a multi-line TextBox (Enter is its new line), and an open ComboBox list keeps Enter to pick its item. An Enter that moves the focus does not also press the form's default button.
- `Value` (Integer, default 0; ProgressBar/Slider/NumericUpDown: integer within Minimum..Maximum; DateTimePicker: `YYYY-MM-DD`, `HH:MM`, or `YYYY-MM-DD HH:MM`) — Current value. A DateTimePicker always stores ISO, whatever Format displays, so a COBOL handler reading Value gets one shape; which halves are present follows Format (a Time picker stores the time alone).
- `Minimum` (Integer, default 0; integer ≤ Maximum) — Lower bound of the value range.
- `Maximum` (Integer, default 100; integer ≥ Minimum) — Upper bound of the value range.
- `Step` (Integer, default 1; number > 0) — How far one step moves the value: the ↑/↓ keys on a NumericUpDown, ←/→/↑/↓ on a Slider, the wheel, and the Increment()/Decrement() methods — which stop at Minimum and Maximum. A fractional Step (0.5) steps in fractions.
- `DecimalPlaces` (Integer, default 0; 0-6) — NumericUpDown: how many fractional digits the field shows and writes into Value — never fewer than Step carries, so a Step of 0.25 is always visible.
- `ThousandsSeparator` (Boolean, default 0; `1` (true) or `0` (false)) — NumericUpDown: groups the whole part in thousands with a comma (12,345.50). Display only; Value stays a plain number.
- `ReadOnly` (Boolean, default 0; `1` (true) or `0` (false)) — The operator cannot change the value — no drag, wheel or arrow key moves it — but it stays shown and focusable. COBOL still sets it.
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Supported Events
- `onChange` — value or text changed
- `onValueChanged` — value changed (the new value is delivered)
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetValue(value: Integer)` — Set the current value.
- `GetValue() → Integer` — Read the current value.
- `Increment()` — Add Step to Value.
- `Decrement()` — Subtract Step from Value.
- `Reset()` — Return Value to Minimum.

---

## Control: TreeView

Hierarchical node list. `Items` IS the tree: one node per line, TWO SPACES (or one tab) of indent per level. It is drawn by one renderer on the designer canvas and in the running form, so what you lay out is what runs — before 1.61.153 the canvas showed only a `[TreeView]` placeholder and the running form a flat bulleted list. The tree writes its nodes in the control's own FontName/FontSize/ForegroundColor, draws its connector lines per `ShowLines`/`ShowRootLines` in `LineColor`, ticks per `CheckBoxes`/`CheckedNodes`, and highlights per `HotTracking`. A click selects (`SelectedNode`, `onNodeClick`/`onNodeSelect`); a click on a tick box checks (`CheckedNodes`, `onNodeCheck`). EXPAND/COLLAPSE (1.61.157): a node with children draws a disclosure arrow — right when shut, down when open — and clicking it writes `CollapsedNodes` and fires `onNodeCollapse`/`onNodeExpand`. The arrow's slot is reserved on every row, so labels line up whether or not a node folds. ICONS: on by default from the platform's catalogue, a node naming its own after a TAB in its `Items` line and the rest taking `ParentIcon`/`ParentIconOpen`/`LeafIcon`. Every metric is a property — `RowHeight`, `IndentWidth`, `IconSize`, `CheckBoxSize` — and so are `SelectionColor`, `HotTrackColor` and `IconColor`. A NODE'S OWN DRESS (1.61.159): an `Items` line is `label`, then up to three TAB-separated fields of its own — `label\ticon\tcolour\tbackground` — so `Overdue\t\t#C81E1E` is a node written in red with its icon left to the tree; an empty field means 'as the tree draws it', and the row colour paints UNDER the selection band so a coloured row still shows that it is selected. TICK BOX (1.61.159): it wears the CheckBox's own five properties — `CheckBoxColor`, `CheckBoxBorderStyle`, `CheckBoxBorderColor`, `CheckBoxBorderWidth`, `CheckColor`, `CheckSize` — and draws the same tick mark; before that it was a black-alpha well, a 1px rim and a tick at 28 % of the box, none of them reachable. WALKING THE TREE (1.61.159): `NodeParent`, `NodeFirstChild`/`NodeLastChild`, `NodeNextSibling`/`NodePrevSibling`, `NodeChildCount`, plus `NodeText`/`NodePath`/`NodeLevel`/`NodeIcon`/`NodeColor`/`NodeBackColor`/`NodeChecked`/`NodeCollapsed` and `NodeCount`/`NodeIndexOf` — every one keyed by the node INDEX the event already hands the handler, and the traversal calls return an index so they chain. Build a tree from COBOL with `AddNode(level, text)`, NOT `AddItem` (which trims its argument, so an indented literal cannot make a child). SCROLLING (1.61.160): a tree taller than its control scrolls — the wheel while the pointer is over it, a DRAG anywhere on it, and Up/Down/Home/End once it has focus (a click gives it focus), with the view following a keyboard selection only as far as it must. Before that the overflow was simply dropped and those nodes could not be reached at all. How far it scrolls is measured against the rows it SHOWS, so folding a tree shortens it. There is no scroll property: the offset is view state and is deliberately NOT saved in the `.cfrm`. Default size 200×200 px.

**Content structure.** The tree is the `Items` property: one node per line, in display order, each child indented two spaces (or one tab) more than its parent, optionally followed by TAB-separated icon, colour and background fields. An agent reads it in the control's `CONTROLS` line and changes it with `set_property` on `Items`, sending the whole tree. From COBOL, `AddNode(level, text)` builds it at run time.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Items` (String, default "Node 1
  Child 1
  Child 2
Node 2"; newline-separated entries (TreeView: two-space indentation nests children)) — The list content, one item per line.
- `AllowEdit` (Boolean, default 0; `1` (true) or `0` (false)) — TreeView (default false): the operator may rename a node in place — a double-click on its label, or F2 on the selected node, opens the label in a text box; Enter or clicking away keeps it, Escape drops it, and an empty label is refused. A kept rename rewrites that node's line in `Items` (its indentation and icon/colour fields kept), follows it in `SelectedNode`, `CheckedNodes` and `CollapsedNodes`, writes the old label to `PreviousNodeText`, and fires `onNodeRenamed` with the node — the new label in `CONTROL-NODE`. Storing the new name is the handler's job; to refuse it, write `Items` back. A tab or line break typed into the label becomes a space.
- `AllowDrag` (Boolean, default 0; `1` (true) or `0` (false)) — TreeView (default false): the operator may drag a node onto another. Pressing a node and moving picks it up — the node under the pointer is ringed and the label follows the pointer, and the tree scrolls when the pointer nears its top or bottom edge; the wheel still scrolls. Letting go over another node fires `onNodeDrop` with the dragged node in `CONTROL-NODE`/`CONTROL-NODE-INDEX`/`CONTROL-NODE-LEVEL`/`CONTROL-NODE-CHECKED` and the target in `CONTROL-TARGET-INDEX` and `CONTROL-TARGET-NODE`; letting go over the tree's empty space gives `CONTROL-TARGET-INDEX` 0 and a blank target — read it as the top level; letting go outside the tree, or back on the same node, fires nothing. The tree does not move anything itself: the handler decides what a drop means and rebuilds `Items`. Off, a drag scrolls the tree as it always did.
- `CheckBoxes` (Boolean, default 0; `1` (true) or `0` (false)) — Draws a tick box on every node. A click ON THE BOX ticks it (a click anywhere else on the row selects the node) and the ticked nodes land in `CheckedNodes`, one per line, with `onNodeCheck` carrying the node.
- `ShowLines` (Boolean, default 1; `1` (true) or `0` (false)) — Draws the connector lines between a node and its parent, in `LineColor`. On by default.
- `ShowRootLines` (Boolean, default 1; `1` (true) or `0` (false)) — Draws the spine joining the TOP-LEVEL nodes, in `LineColor` — separate from `ShowLines`, which joins children to parents. On by default.
- `Sorted` (Boolean, default 0; `1` (true) or `0` (false)) — Shows the items in alphabetical order, by TEXT and ignoring case, so 10 sorts before 9. Display order only - the stored Items keeps the order it was written in. ListBox, ComboBox and TreeView. A TreeView sorts SIBLINGS only — every child stays under the parent it was written under — and a node's handle (`CONTROL-NODE-INDEX`, the `Node…` methods) is still its line as written, so sorting never renumbers a handler.
- `HotTracking` (Boolean, default 0; `1` (true) or `0` (false)) — Lifts the node under the pointer, faintly — half the weight of the selection band, so the two are never confused. Off by default.
- `LineColor` (String, default "#AAAAAA"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Connector/line color (TreeView, Line, Shape). On a Splitter, the division line between its two panes; empty = the form theme's own rule colour.
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `CheckedNodes` (String, default (empty); newline-separated node labels) — Which boxes are ticked, one node per line — the `CheckBoxes` companion, read and written exactly like `SelectedNode`. Writing it from COBOL ticks those nodes.
- `CollapsedNodes` (String, default (empty); newline-separated node labels) — Which nodes are FOLDED SHUT, one per line — so EMPTY means the whole tree is open, which is what a tree shows untouched. A node with children draws a disclosure arrow; clicking it folds or unfolds and fires `onNodeCollapse`/`onNodeExpand` with that node. Writing this from COBOL folds a tree to any shape without touching `Items`.
- `ShowIcons` (Boolean, default 1; `1` (true) or `0` (false)) — Draws an icon on every node, from the platform's own catalogue — the same one menus and toolbars use. On by default. A node names its OWN icon after a TAB in its `Items` line (`Warehouse\tbox`); nodes that name none take `ParentIcon`/`ParentIconOpen`/`LeafIcon`.
- `ParentIcon` (String, default "folder"; icon name from the catalogue (default `folder`)) — The icon on a node that HOLDS other nodes while it is folded shut.
- `ParentIconOpen` (String, default "folder-open"; icon name from the catalogue (default `folder-open`)) — The icon on a node that holds other nodes while it is open — a folded and an open folder are different pictures, which is how the state reads at a glance.
- `LeafIcon` (String, default "doc-text"; icon name from the catalogue (default `doc-text`)) — The icon on a node with nothing under it.
- `IconSize` (Integer, default 14; integer 6-64 (default 14)) — Icon and disclosure-arrow size in points. The arrow's slot is reserved on EVERY row whether or not the node has children, so labels line up in a column.
- `IconColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Icons and disclosure arrows. Empty — the default — follows the node ink, so legible text means legible icons.
- `HighContrastText` (Boolean, default 1; `1` (true) or `0` (false)) — ON by default: node ink is picked by CONTRAST RATIO against the face the tree is actually painted on, so it clears WCAG AA on a white face, a dark card or a glass surface alike. Off falls back to the theme's own text colour, for a developer who wants the tree to match the theme even where that costs legibility. An explicit `ForegroundColor` outranks both.
- `RowHeight` (Integer, default 18; integer 8-200 (default 18)) — The row's MINIMUM height in points — a floor, not a ceiling. A row is never shorter than what it holds, so growing `IconSize` or `CheckBoxSize` grows the row with it rather than letting a big icon paint over the nodes above and below.
- `NodeSpacing` (Integer, default 0; integer 0-100 (default 0)) — Extra gap BETWEEN nodes, on top of whatever the row needs for its icon and box. Reach for this when the rows read as a wall of text.
- `IndentWidth` (Integer, default 16; integer 0-200 (default 16)) — How far one level of the tree steps right.
- `CheckBoxSize` (Integer, default 12; integer 6-64 (default 12)) — The tick box's size, when `CheckBoxes` is on.
- `CheckBoxColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The tick box's own fill (TreeView, CheckBox, RadioButton). Empty — the default — means 'not chosen', so the theme keeps painting the box; that is also why naming white is possible. On a TreeView, empty keeps the recessed well the box has always drawn.
- `CheckBoxBorderStyle` (String, default "Single"; `None` | `Single` | `Fixed3D` | `Raised` | `Sunken`) — The rim around the TICK BOX, separate from the frame's `BorderStyle`, which rims the whole control. On a TreeView it is seeded `Single` — what the box has always been drawn with — so `None` is how you switch it off.
- `CheckBoxBorderColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The tick box's rim colour. On a TreeView, empty follows the node ink, so legible text means a legible box.
- `CheckBoxBorderWidth` (Integer, default 1; integer 0-10 (default 1)) — The tick box's rim width.
- `CheckColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The colour of the TICK itself (TreeView, CheckBox, RadioButton). On a TreeView, empty follows the node ink.
- `CheckSize` (Integer, default 70; integer 10-100 (default 70)) — How much of the tick BOX the tick fills, as a percentage — not the box's size, which is `CheckBoxSize` on a TreeView and the font on a CheckBox. A fuller tick also draws a heavier stroke.
- `SelectionColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The band behind the selected node, used EXACTLY as given — alpha included, since a selection band is mostly alpha. Empty is the theme's focus colour at the weight a band has always had.
- `HotTrackColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The band behind the node under the pointer, when `HotTracking` is on. Empty is half the selection's weight, so the two are never confused.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `PreviousNodeText` — TreeView: in `onNodeRenamed`, the label the node had before the operator renamed it; the new one is `CONTROL-NODE`.

### Supported Events
- `onNodeClick` — a tree node was clicked
- `onNodeDblClick` — a tree node was double-clicked
- `onNodeDoubleClick` — a tree node was double-clicked
- `onNodeSelect` — a tree node became selected
- `onNodeCheck` — a tree node's tick box was ticked or cleared
- `onNodeCollapse` — a tree node was folded shut
- `onNodeExpand` — a tree node was opened
- `onNodeRenamed` — TreeView with `AllowEdit`: the operator renamed a node in place — `CONTROL-NODE` holds the new label, `PreviousNodeText` the old one; `Items` already carries it
- `onNodeDrop` — TreeView with `AllowDrag`: the operator dropped one node on another — the DRAGGED node in `CONTROL-NODE`, `CONTROL-NODE-INDEX`, `CONTROL-NODE-LEVEL`, `CONTROL-NODE-CHECKED`, and the target in `CONTROL-TARGET-INDEX` (1-based line in `Items`, 0 for the tree's empty space) and `CONTROL-TARGET-NODE` (its label, blank for empty space). The handler decides what the drop means and rebuilds `Items`
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddNode(level: Integer, text: String [, icon, color, background])` — Append a node at `level` (0 = root). The level is a NUMBER because `AddItem` trims its argument and a node's level is leading spaces — an indented literal cannot build a child. The optional fields are the node's own icon, label colour and row colour.
- `NodeCount() → Integer` — How many nodes the tree holds.
- `NodeIndexOf(text: String) → Integer` — The index of the first node with this label, -1 when there is none — how a handler holding only a name (from `SelectedNode`) gets a handle to walk from.
- `NodeText(index) → String (alias NodeName)` — The node's label.
- `NodePath(index) → String` — Root to node joined by `/` — what tells two nodes with the same label apart.
- `NodeLevel(index) → Integer` — How deep it sits, 0 for a root.
- `NodeIcon(index) → String` — The icon it names, empty when it names none.
- `NodeColor(index) → String (alias NodeColour)` — Its own label colour, empty when it names none.
- `NodeBackColor(index) → String (alias NodeBackground)` — Its own row colour, empty when it names none.
- `NodeParent(index) → Integer` — The node it hangs under, -1 on a root.
- `NodeFirstChild(index) / NodeLastChild(index) → Integer` — Its first/last direct child, -1 on a leaf.
- `NodeNextSibling(index) / NodePrevSibling(index) → Integer` — The next/previous node at the SAME level under the same parent, -1 at the end of the run. A sibling walk never descends into children and never escapes into the next parent.
- `NodeChildCount(index) → Integer` — How many nodes hang DIRECTLY under it — grandchildren not counted.
- `NodeHasChildren(index) → 1/0` — Whether anything hangs under it.
- `NodeChecked(index) → 1/0` — Whether its box is ticked — read from the control's live `CheckedNodes`, not from the node's line.
- `NodeCollapsed(index) → 1/0` — Whether it is folded shut — read from the control's live `CollapsedNodes`.
- `RemoveNode(index) → 1/0` — Remove the node AND everything under it — a child left behind would re-parent onto the node above. `0` when the index names no node.
- `ExpandAll()` — Open every node: empties `CollapsedNodes`.
- `CollapseAll()` — Fold every node that has children: `CollapsedNodes` lists them all.
- `GetSelectedNode() → String` — The selected node's label (`SelectedNode`).
- `SetSelectedNode(label: String)` — Select the node with this label (writes `SelectedNode`).

---

## Control: Splitter

A themed PANEL divided in two by a draggable line — as of 1.61.164 it IS a container, and the two halves are real controls. Dropping one creates `<id>-Pane1` and `<id>-Pane2`: borderless, transparent Panels parented to the splitter, which you drop controls into exactly like any other Panel. Their geometry is DERIVED from the division and is not editable — moving the line moves them. `Orientation` names how the PANES sit, not the line: `Horizontal` = pane 1 LEFT, pane 2 RIGHT, divided by a vertical line; `Vertical` = pane 1 TOP, pane 2 BOTTOM. (This is the opposite of what `Orientation` meant before 1.61.164, when the control was a bar between two neighbouring controls; a form saved earlier opens with its panes the other way round.) `SplitPosition` is a PERCENTAGE 0–100 of the inner span, not a pixel offset, so it survives the splitter being resized; 0 and 100 are legal and close one pane completely, and the grip is clipped by the splitter's edge so half of it stays visible there. Drag the line (or its grip) to redistribute, double-click it to go back to 50 %, and the pointer becomes a grab hand over it — on the designer canvas and in the running form alike. Style it with `LineColor`, `LineSize`, `GripStyle` (FilledPill | HollowPill | FilledCircle | HollowCircle), `GripSize` and `GripColor`; the panel itself follows the form theme until `BackgroundColor` / `BorderStyle` / `BorderColor` say otherwise. Each pane also carries `ResizeBehavior` — what it does with the controls inside it when the line moves (Translate with divider by default, or Scale within the pane, or Anchor to the outer edge), set per pane so the two halves can differ; dragging the division in the RAD rewrites those children's X/Y for real, as one undo step. A CONTAINER inside a pane (Panel, GroupBox, TabControl) carries its whole subtree: the container reflows per the pane's ResizeBehavior and its contents travel rigidly with it — under Scale too, where spreading a container's contents by their own fractions would tear them out of it — on the canvas, in preview and at run time alike. This holds to ANY depth, a SPLITTER INSIDE A PANE included: the inner splitter travels with the outer division, and its own panes and their contents travel with it. A PANE NEVER RESIZES WHAT IS IN IT: moving the division changes the pane's own rectangle and the POSITIONS of its contents, never their Width or Height — the pane is a viewport, and a control too big for it is clipped by the pane's edge, not shrunk to fit. STILL NOT: `AllowEdit` (no in-place rename surface) — never tell a developer it works. Default size 320×220 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Orientation` (String, default "Horizontal"; `Horizontal` | `Vertical`) — Layout axis.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#CCCCCC"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderWidth` (Integer, default 1; pixels 0-64) — The width of the splitter's frame. The panes are inset by it (2 pixels at least), so a wide frame is never covered by the panes and what they hold; with BorderStyle None the inset is 2.
- `SplitPosition` (Integer, default 50; 0–100) — Where the division line sits, as a PERCENTAGE of the splitter's inner width (Horizontal) or height (Vertical). 0 closes pane 1, 100 closes pane 2; both are legal.
- `LineColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Connector/line color (TreeView, Line, Shape). On a Splitter, the division line between its two panes; empty = the form theme's own rule colour.
- `LineSize` (Integer, default 2; points 1–40) — Thickness of the division line.
- `GripStyle` (String, default "FilledPill"; one of: `FilledPill` | `HollowPill` | `FilledCircle` | `HollowCircle`) — How the drag grip on the division line is drawn.
- `GripSize` (Integer, default 28; points 0–400) — The grip's extent along the line (pill length / circle diameter). 0 hides the grip; the line stays draggable.
- `GripColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Grip colour. Empty = the division line's colour.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.

---

## Control: Timer

Non-visual: fires `onTick` every Interval ms. Steady cadence — each tick schedules the next ONE INTERVAL on, so the rate does not drift with frame timing — and it never repays missed time: a handler slower than the interval, or a stalled form, gets ONE tick on return, not a burst. A handler eight events behind has its ticks coalesced until it catches up; a click, an edit or a focus change is never coalesced. `Enabled` is the timer's OWN property — the on/off switch the runtime reads and the one codegen seeds `WS-<timer>-ENABLED` from — and it is settable at design time (inspector: `Enabled at start`) and from COBOL at run time (`SET TIMER-1::Enabled TO 0` stops it). It is NOT the chrome enabled flag every control has; before 1.61.164 both spellings landed on the chrome flag and the timer could not be stopped at all. Default size 48×48 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Interval` (Integer, default 1000; milliseconds ≥ 10) — Delay between `onTick` events.
- `Enabled` (Boolean, default 1; `1` (true) or `0` (false)) — Timer running state (this is the timer's own property, distinct from control chrome).

### Supported Events
- `onTick` — fires every `Interval` ms while `Enabled` = 1

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `Start()` — Set Enabled = 1 (ticks resume).
- `Stop()` — Set Enabled = 0 (ticks stop).
- `SetInterval(ms: Integer)` — Change the tick interval.
- `IsEnabled() → Boolean (0/1)` — Read the running state.

---

## Control: Shape

Decorative rectangle / circle / triangle. Default size 120×80 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `ShapeType` (String, default "Rectangle"; one of: `Rectangle` | `Circle` | `Triangle`) — Geometric shape drawn.
- `FormStyle` (Boolean, default 0; `1` (true) or `0` (false)) — Shape follows the form's glass style.
- `FillColor` (String, default "#C0C0C0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Shape interior fill. On a Slider, the travelled part of the rail — Minimum to Value — which is the part that reads as filled; left at its default the active theme paints.
- `FillStyle` (String, default "Solid"; one of: `Solid` | `None` | `Hatched`) — How the shape interior is filled: Solid with FillColor, None left empty, Hatched with diagonal lines in FillColor over a transparent face.
- `LineColor` (String, default "#000000"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Connector/line color (TreeView, Line, Shape). On a Splitter, the division line between its two panes; empty = the form theme's own rule colour.
- `LineThickness` (Integer, default 1; pixels > 0) — Stroke thickness.
- `LineStyle` (String, default "Solid"; one of: `Solid` | `Dash` | `Dot` | `DashDot`) — Shape outline dash pattern.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Supported Events
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.

---

## Control: AgentObject

Non-visual LLM client (ask a model from COBOL). Default size 56×56 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Configuration` (String, default (empty); empty (this control's own settings), or the name of a project connection) — Where this control gets its connection. **Empty — the default — means the control's own properties below**, exactly as it has always worked. Otherwise it names one of the project's connections (Settings → Integrations), and that connection's settings replace the control's own before the form runs. On a **RestClient** that is the address, method, authentication scheme, headers and timeouts; on a **WebSearch** it is the provider and its engine id or instance URL — and deliberately NOT `NumResults` or `SafeSearch`, which are per-call settings the form changes at run time and which stay the control's own. An **AgentObject** is different in kind: it selects one of the machine's configured **Model Providers** (Settings → Models) rather than a project connection, taking that provider's protocol, endpoint and key — while `AgentModel`, `Temperature`, `MaximumTokens` and `TimeoutSeconds` stay the control's own, because one provider offers many models. Its `API Key` row disappears while bound, which is the point: an agent's key is entered once, in the Model Providers Manager, and never copied onto a form. That binding is **machine-scoped** — a provider not configured on a given machine is reported as exactly that, not as a broken project — and reaches a running application through `COBOLT_AGENT_PROVIDERS`. Define a service once and every form that uses it stays in step, instead of six forms drifting apart. The **credential is never part of the connection record** — that record round-trips in `cobolt.toml` and is meant to be committed, while the key lives in the machine-local store and reaches a running form through the environment, so a checked-out project carries the connections and each developer supplies their own key. A built application carries the connections baked in and takes each key from `COBOLT_CONNECTION_KEY_<ID>` on the machine that runs it. A Configuration naming a connection the project no longer has is an error, not a silent fall back to the local settings: the control was told to ignore those.
- `ModelEntry` (String, default (empty); empty, or the name of a model-list entry (run time)) — AgentObject / KnowledgeBase (spec 076): a model-list entry the PROGRAM handed over while it runs, with `COBOL::"MODEL-SET"( name api url model [status] )` (the program keeps its list in its own indexed file and hands it over at start-up; the runtime stores no list). When set it WINS over `Configuration` and the control's own settings: the entry gives the API, the endpoint and the key (stored with `CALL "COBOL-KEY-SET" USING name key [status]`, removed with `COBOL-KEY-REMOVE`, checked with `COBOL-KEY-IS-SET USING name flag` → Y/N — no CALL ever returns a key); the entry's model when it names one, else the agent's own; temperature, token limit and timeout stay the agent's. An unknown entry, or an entry whose provider needs a key (every provider but local Ollama — the IDE's rule) with no key stored, fails the Ask at once with `onError` and sends nothing. The IDE's provider list, model listing and connection test are available to the program too: `COBOL-PROVIDER-COUNT USING count`, `COBOL-PROVIDER-GET USING index id label endpoint needs-key`, `COBOL-MODEL-LIST USING provider endpoint key count status [entry]` + `COBOL-MODEL-LIST-GET USING index model`, and `COBOL-MODEL-TEST USING provider endpoint model key status [entry]` (status `OK` or the IDE's own message; a blank key with an entry uses that entry's stored key, which is never returned). Keys live in `<app>/settings/model-keys.dat`, shared by every user of the installation, encrypted with a key derived from the installation folder and the machine's name — unreadable at a glance and useless copied elsewhere, but usable by anyone who can run the application on that machine. A key is never shown in a log (the verbose log masks it), an error, or anything sent to a model. A change to an entry an agent used raises `onModelChanged` on that agent. `COBOL-MODEL-REMOVE USING name` withdraws an entry.
- `AgentURL` (String, default "http://localhost:11434"; HTTP(S) URL) — Base URL of the LLM provider.
- `AgentModel` (String, default "llama3.2"; model id string) — Model requested from the provider.
- `AgentAPI` (String, default "Ollama"; a provider id — `openai` | `anthropic` | `cohere` | `gemini` | `perplexity` | `mistral` | `groq` | `openrouter` | `huggingface` | `together` | `deepseek` | `alibaba` | `xai` | `voyageai` | `ollama` | `ollama_cloud` | `llamafile` (the IDE's Model Providers list) — or `LMStudio` | `Custom`) — Provider. A provider from the list is addressed the way the IDE addresses it: Anthropic at `<API root>/messages`, Ollama at its native `/api/chat`, every other one at `<API root>/chat/completions`, where a stored full request URL or a bare origin is brought back to the API root.
- `AgentAPIKey` (String, default (empty); secret string or empty) — API key when the provider needs one. STORED ON YOUR MACHINE, never in the form: what you type in the designer goes to the local credential file and the running form is handed it at start-up, so the `.cfrm` you commit carries an empty value however the key was entered. Prefer binding the control to a Model Provider instead — one place to enter the key, one place to rotate it.
- `AgentEndpoint` (String, default (empty); full URL, a path starting with `/`, or empty) — Overrides the provider's default endpoint. A full URL is used as it stands; a path (`/v1/chat/completions`) is joined onto `AgentURL`'s scheme and host.
- `SystemPrompt` (String, default "You are a helpful assistant."; free text) — System prompt sent with every request.
- `Temperature` (Integer, default 70; 0-100 (maps to 0.0-1.0)) — Sampling temperature.
- `MaximumTokens` (Integer, default 1024; integer > 0) — Response token limit.
- `StreamReply` (Boolean, default 0; true | false (default false)) — AgentObject: show the reply while the model writes it. The request asks the provider for a streamed reply (OpenAI-compatible and Anthropic SSE, Ollama NDJSON); up to ten times a second `PartialReply` receives the text so far and `onPartialReply` fires. The Ask ends exactly as an unstreamed one: `LastReply`/`Result`/`ResponseDataItem` get the whole text and `onResponse` fires once (or `onError`). `Busy` stays true throughout, and `TimeoutSeconds` becomes a SILENCE limit — each piece that arrives restarts it, so a long answer is never cut off. An Ask that offers tools is never streamed. A provider or `Custom` endpoint that cannot stream fails the Ask with its own error; turn the switch off for it.
- `StartTimeoutSeconds` (Integer, default 60; seconds ≥ 0 (default 60; 0 = no limit)) — AgentObject: the longest the model may take to BEGIN its answer. Every agent request — each tool round included — is streamed, so the moment the first piece arrives is known. A request whose answer has not begun in time is CANCELLED: `LastError` says "The model did not start answering within N seconds: the call was cancelled.", `onTimeout` fires, `Busy` clears, and the connection is closed (a local model stops working for nobody). The same limit also bounds the connection itself, so a model that falls silent mid-answer for this long is cut off too. Raise it for a model that thinks for a long time before its first word, or for the first call to a local model that must load first.
- `TimeoutSeconds` (Integer, default 30; seconds > 0) — Request timeout. On an AgentObject: once the model has begun answering, the longest it may fall SILENT between two pieces of its answer (every agent request is streamed, so a long answer that keeps coming is never cut off); for a question that offers tools, the limit on the WHOLE question, every round and every handler wait included.
- `ResponseDataItem` (String, default (empty); COBOL data-item name) — AgentObject: a WORKING-STORAGE item that receives the reply when it arrives — written just before `onResponse` fires, and only when the program declares an item of that name. Empty = none; read `LastReply` in the handler instead.
- `Verbose` (Boolean, default 0; true | false) — Narrate the whole call into the program's output. Off by default; turn it on when the control appears to do nothing, because an operation that returned nothing and one that never ran produce the same empty log, and this is what separates them. **On an AgentObject**: the endpoint, every request header and the payload exactly as sent, then the HTTP status, the raw body and the reply read out of it. The headers include the API key **unmasked, on purpose** — a key wrong by one character is invisible once masked — and the log says so on the next line, so do not paste it into a bug report. The call is made the same way in the IDE, under `rcrun` and in a built application. **On a WebSearch**: the provider, the method and URL, the request headers, the body sent, whether the call is async or sync, then the HTTP status and the raw response — uncut, so it can be compared against the provider's own documentation. A WebSearch masks its credentials: a key in a header or in the URL query (Google signs there) prints as its first few characters and a length.
- `ToolProtocol` (String, default "Native"; one of: `Native` | `Fenced` | `None`) — AgentObject (spec 072): how tools are offered to the model. `None` offers THIS agent no tool at all, whatever the program allowed with `AllowFile`, `RegisterFile`, `AllowKnowledgeBase` or `AddTool` — those are program-wide, so this is the per-agent switch for a model that cannot call tools (spec 071). `Native` (default) uses the provider's own tool-calling fields — OpenAI-compatible `tools`/`tool_calls`, Ollama `tools`, Anthropic `tools`/`tool_use`. `Fenced` is for a model without function calling: the tools are described in the system prompt and the model calls one by answering with a fenced ```json block `{"tool_calls":[{"tool":…,"args":{…}}]}`; the results come back as a user message. The setting is yours — it is never guessed from the model's name. An agent that offers no tools sends exactly the same request whatever this says.
- `MaximumToolRounds` (Integer, default 8; integer > 0 (default 8)) — AgentObject (spec 072): how many rounds of tool calls one `Ask` may take. A model still calling tools when the limit is reached ends the question with `onError` (`LastError` names `MaximumToolRounds`) instead of looping for ever.
- `MaximumContinuations` (Integer, default 4; integer >= 0 (default 4)) — AgentObject: how many times an answer CUT OFF by the model's output limit (`MaximumTokens`) is continued. The provider says why the model stopped; when it says the limit was reached (`length`, `max_tokens`), the agent sends the conversation back with the answer so far and asks the model to go on exactly where it stopped, then joins the pieces. `onResponse` fires once, with the whole answer, and `StreamReply` shows the continuation as it arrives, after what was shown already. `0` turns it off: the answer is given as it came, with `Truncated` set.

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `LastReply` — AgentObject: the model's answer to the last `Ask`, written when it arrives and just before `onResponse` fires. Cleared on a failure, when `LastError` is set instead. `Result` carries the same text.
- `Result`
- `LastError` — Why the last async call failed, delivered with `onError`. Empty after a call that succeeded.
- `Busy` — Read-only runtime flag: an async operation is in flight.
- `PartialReply` — AgentObject: with `StreamReply` on, the reply received so far — written just before each `onPartialReply`. Emptied when an Ask starts. The finished reply is `LastReply`, in `onResponse`.
- `ReplyPiece` — AgentObject: with `StreamReply` on, only the text that arrived since the previous `onPartialReply` — the piece to hand a Viewer's `AppendToMessage`. The pieces, in order, add up to `PartialReply`. Emptied when an Ask starts.
- `LastInputTokens` — AgentObject (spec 072): the input (prompt) tokens the provider reported for the last `Ask`, summed over every tool round. 0 when the provider reports none. Written before `onResponse` / `onError`.
- `LastOutputTokens` — AgentObject (spec 072): the output (completion) tokens the provider reported for the last `Ask`, summed over every tool round. 0 when the provider reports none.
- `LastToolCallCount` — AgentObject (spec 072): how many tool calls the model made during the last `Ask` — indexed-file searches and program-answered tools alike. 0 for a question that used no tools.
- `StopReason` — AgentObject: why the model stopped writing its last answer, in the provider's own words: `stop` / `end_turn` (finished), `length` / `max_tokens` (the output limit cut it off), `tool_calls`. Empty when the provider reports none. Written before `onResponse`.
- `Truncated` — AgentObject: 1 when the last answer is INCOMPLETE — the output limit cut it off and `MaximumContinuations` was used up (or is 0). Raise `MaximumTokens` or `MaximumContinuations`, or ask for less. Written before `onResponse`.
- `ContinuationCount` — AgentObject: how many times the last answer was continued after the output limit cut it off. 0 when it came whole.
- `ToolCallId` — AgentObject (spec 072): during `onToolCall`, the id of the call the program is asked to answer. Pass it to `SetToolResult(ToolCallId, text)`.
- `ToolName` — AgentObject (spec 072): during `onToolCall`, the name of the tool the model called — one the program declared with `AddTool`; during `onToolUse`, the runtime-answered tool it is using (a Knowledge Base collection's or an indexed file's).
- `ToolArguments` — AgentObject (spec 072): during `onToolCall` (and `onToolUse`), the arguments the model sent, as a JSON object whose keys are the parameter names declared with `AddToolParameter`. Every value is a string.
- `ToolKind` — AgentObject: during `onToolUse`, which kind of runtime-answered tool the model is using.
- `RegisterResult` — AgentObject (spec 075): the outcome of the last `RegisterFile` — `MEMORY` (held in memory), `DISK` (read in place from disk, too large for memory), or the refusal code.
- `RegisterMessage` — AgentObject (spec 075): the last `RegisterFile` outcome in English, naming the file (any smb:// password masked). Translate from `RegisterResult` for the user.
- `RegisteredName` — AgentObject (spec 075): the name the last `RegisterFile` registered the file under — the one to pass to `UnregisterFile`; empty when refused.
- `RegisterFileBytes` — AgentObject (spec 075): the size of the file the last `RegisterFile` examined, in bytes.
- `RegisterLimitBytes` — AgentObject (spec 075): the limit the file was compared with — the project limit, or half the free memory when that decided a refusal.

### Supported Events
- `onResponse` — the LLM reply arrived
- `onPartialReply` — AgentObject with `StreamReply` on: more of the reply arrived — `PartialReply` holds the text so far and `ReplyPiece` what is new since the last one (at most ten times a second); `onResponse` still ends the Ask with the whole text
- `onError` — the operation failed (message in `LastError`)
- `onToolCall` — the model called a tool the program declared with `AddTool` — read `ToolCallId` / `ToolName` / `ToolArguments`, answer with `SetToolResult`; a handler that sets nothing sends an empty result
- `onToolUse` — the model is using a tool the RUNTIME answers itself — a Knowledge Base search (`ToolKind` = `KnowledgeBase`) or a registered indexed file (`ToolKind` = `IndexedFile`); `ToolName` and `ToolArguments` say which and with what. Raised before the tool runs, while the model is still working — for showing "searching the Knowledge Base…"; nothing to answer, the result goes straight to the model
- `onModelChanged` — AgentObject (spec 076): the model-list entry this agent last used (`ModelEntry`) was changed or withdrawn — by this form or any other in the application; re-read your settings or choose another entry

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `Ask(prompt: String) → String` — Send the prompt to the configured provider. ASYNCHRONOUS: the call is handed to a background worker and the statement returns the EMPTY string at once — the answer does not exist yet. Read it in the `onResponse` handler, from `LastReply`; `Result` carries the same text. A failure writes `LastError`, clears `LastReply` and fires `onError`; a call that outlives `TimeoutSeconds` fires `onTimeout`. `Busy` is true from the Ask until one of those events, and a second Ask while it is true is ignored rather than raced. Never write `MOVE agent::Ask(q) TO X` — that moves the empty string; the handler is the only place the reply exists. The request is shaped from `AgentAPI`, `AgentURL`/`AgentEndpoint`, `AgentModel`, `AgentAPIKey`, `SystemPrompt`, `Temperature` and `MaximumTokens`. Turn `Verbose` on to see the endpoint, the payload, the headers, the status and the raw response in the program output. With `StreamReply` on, the reply is also shown while it arrives: `PartialReply` + `onPartialReply` before the final `onResponse`.
- `SetPrompt(text: String)` — Replace the SystemPrompt.
- `SetModel(model: String)` — Switch the model id.
- `GetResult() → String` — Read the `Result` property.
- `Cancel()` — Cancel the in-flight request.
- `IsBusy() → Boolean (0/1)` — An async request is in flight.
- `AddTool(name: String, description: String) → Boolean (0/1)` — Spec 072: offer the model a tool that THIS PROGRAM answers. When the model calls it, `onToolCall` fires with `ToolCallId`, `ToolName` and `ToolArguments` set; the handler answers with `SetToolResult`. Calls are handed to the program one at a time, in the order the model made them. Adding a name again replaces it. Declared tools belong to this control only.
- `AddToolParameter(tool: String, name: String, description: String) → Boolean (0/1)` — Spec 072: describe one argument of a tool declared with `AddTool` (a string). `0` when no such tool is declared.
- `RemoveTool(name: String) → Boolean (0/1)` — Spec 072: stop offering a declared tool. `0` when it was not declared.
- `SetToolResult(call-id: String, text: String)` — Spec 072: the answer to the tool call `call-id` (use `ToolCallId`), sent back to the model when the `onToolCall` handler returns. A handler that sets nothing sends an empty result; the wait counts against `TimeoutSeconds`.
- `AllowFile(fd-name: String, cidx-path: String?) → Boolean (0/1)` — Spec 072: let every AgentObject in the application search an indexed file (read-only). What the file means comes from its `.cidx` definition (`cidx-path`, or found under the delivered `indexed/` folder by file name); how its records are laid out always comes from this program's own `FD`. The model sees one `search_<file>` tool per allowed file, and the search runs inside the program — never through `onToolCall`. `0` when the FD or the definition cannot be found.
- `DenyFile(fd-name: String)` — Spec 072: stop offering an indexed file allowed with `AllowFile`.
- `RegisterFile(data-path: String, cidx-path: String, name: String?) → Boolean (0/1)` — Spec 075: let every AgentObject search an indexed file named by its PATH while the program runs — no `FD` needed. The path may be local, the OS's own network path (`\\server\share\…` on Windows, a mounted share such as `/Volumes/…` or `/mnt/…`), or `smb://[domain;][user[:password]@]server[:port]/share/path` (no mount; no user = guest login). The layout comes from the `.cidx`, checked first against the schema the data file stores about itself (record length and every key). The file is ONLY read: never written, converted or recovered, and it needs no write permission. Held in memory when it is under the project's file-search memory limit (`[agents] file_memory_limit_mb`, default 64) AND at most half of the free memory; otherwise a local STORAGE IS DISK file is read in place from disk, and anything else is refused. The model sees `search_<name>` (name = the `.cidx`'s file name unless given). Result in `RegisterResult` (`MEMORY`, `DISK`, or a code), `RegisterMessage`, `RegisteredName`, `RegisterFileBytes`, `RegisterLimitBytes`. Refusal codes: `NOT-FOUND`, `CIDX-NOT-FOUND`, `ACCESS-DENIED`, `UNREACHABLE`, `BAD-PATH`, `CIDX-INVALID`, `NO-PURPOSE`, `NO-FIELDS`, `NO-FIELD-DESCRIPTIONS`, `NOT-INDEXED`, `FORMAT-UNSUPPORTED`, `RECORD-LENGTH-MISMATCH`, `KEY-MISMATCH`, `CORRUPT`, `JOURNAL-PRESENT`, `NEEDS-UPGRADE`, `TOO-LARGE-FOR-LIMIT`, `TOO-LARGE-FOR-FREE-MEMORY`, `SMB-UNAVAILABLE`. A registration lasts until the program ends; register again at start-up.
- `UnregisterFile(name: String) → Boolean (0/1)` — Spec 075: withdraw a file registered with `RegisterFile`, freeing its records. Never removes a file allowed with `AllowFile` (use `DenyFile`).
- `AllowKnowledgeBase(kb-control: String, collection: String?) → Boolean (0/1)` — Spec 068: let THIS agent search a KnowledgeBase collection (the control's `Collection` when none is named) as a tool, `kb_<control>_<collection>`. The model sends a query and gets numbered passages, each naming its document and section. A grant is the agent's own: other AgentObjects do not see the collection until it is granted to them too, and denying it to another agent never takes it from this one.
- `DenyKnowledgeBase(kb-control: String, collection: String?)` — Spec 068: stop offering a collection to THIS agent (all of the control's collections when none is named); other agents keep theirs.

---

## Control: RestClient

Non-visual HTTP/REST client (async by default). Default size 56×56 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Configuration` (String, default (empty); empty (this control's own settings), or the name of a project connection) — Where this control gets its connection. **Empty — the default — means the control's own properties below**, exactly as it has always worked. Otherwise it names one of the project's connections (Settings → Integrations), and that connection's settings replace the control's own before the form runs. On a **RestClient** that is the address, method, authentication scheme, headers and timeouts; on a **WebSearch** it is the provider and its engine id or instance URL — and deliberately NOT `NumResults` or `SafeSearch`, which are per-call settings the form changes at run time and which stay the control's own. An **AgentObject** is different in kind: it selects one of the machine's configured **Model Providers** (Settings → Models) rather than a project connection, taking that provider's protocol, endpoint and key — while `AgentModel`, `Temperature`, `MaximumTokens` and `TimeoutSeconds` stay the control's own, because one provider offers many models. Its `API Key` row disappears while bound, which is the point: an agent's key is entered once, in the Model Providers Manager, and never copied onto a form. That binding is **machine-scoped** — a provider not configured on a given machine is reported as exactly that, not as a broken project — and reaches a running application through `COBOLT_AGENT_PROVIDERS`. Define a service once and every form that uses it stays in step, instead of six forms drifting apart. The **credential is never part of the connection record** — that record round-trips in `cobolt.toml` and is meant to be committed, while the key lives in the machine-local store and reaches a running form through the environment, so a checked-out project carries the connections and each developer supplies their own key. A built application carries the connections baked in and takes each key from `COBOLT_CONNECTION_KEY_<ID>` on the machine that runs it. A Configuration naming a connection the project no longer has is an error, not a silent fall back to the local settings: the control was told to ignore those.
- `BaseURL` (String, default "https://api.example.com"; HTTP(S) URL) — The address the control's verbs request. A verb called with no URL argument uses it as it stands; a relative argument is joined onto it; an argument carrying its own scheme (`https://...`) is used unchanged.
- `DefaultMethod` (String, default "GET"; one of: `GET` | `POST` | `PUT` | `PATCH` | `DELETE` | `HEAD` | `OPTIONS`) — The verb `Call()` uses when given no method argument. The named verbs (`get`, `post`, `put`, `delete`) always use their own.
- `AuthType` (String, default "None"; one of: `None` | `Bearer` | `Basic` | `APIKey`) — Authentication scheme, applied to every request the control sends. `Bearer` sends `Authorization: Bearer <AuthToken>`; `Basic` sends `Authorization: Basic <AuthToken>`, base64-encoding the token when it is written `user:password`; `APIKey` sends `X-API-Key: <AuthToken>`. An API wanting a different header name uses `DefaultHeaders` instead.
- `AuthToken` (String, default (empty); secret string or empty) — Token/credentials for AuthType. Empty sends no authentication header at all, rather than an empty one. STORED ON YOUR MACHINE, never in the form: what you type in the designer goes to the local credential file and the running form is handed it at start-up, so the `.cfrm` you commit carries an empty value however the token was entered. A named REST connection is the better home for one shared by several forms.
- `DefaultHeaders` (String, default (empty); `key:value` pairs, newline-separated) — Headers sent with every request. A line with no colon is ignored. A header set at run time with `COBOL-HTTP-SET-HEADER` overrides the one named here.
- `TimeoutSeconds` (Integer, default 30; seconds > 0) — Request timeout. On an AgentObject: once the model has begun answering, the longest it may fall SILENT between two pieces of its answer (every agent request is streamed, so a long answer that keeps coming is never cut off); for a question that offers tools, the limit on the WHOLE question, every round and every handler wait included.
- `FollowRedirects` (Boolean, default 1; `1` (true) or `0` (false)) — Follows HTTP redirects.
- `VerifyTLS` (Boolean, default 1; `1` (true) or `0` (false)) — Verifies TLS certificates.
- `RequestDataItem` (String, default (empty); COBOL data-item name) — RestClient: when a POST/PUT/PATCH is called with no body argument, the value of this item is sent as the body.
- `ResponseDataItem` (String, default (empty); COBOL data-item name) — AgentObject: a WORKING-STORAGE item that receives the reply when it arrives — written just before `onResponse` fires, and only when the program declares an item of that name. Empty = none; read `LastReply` in the handler instead.
- `StatusDataItem` (String, default (empty); COBOL data-item name) — RestClient: receives every request's HTTP status (0 when no response came), from the ::Get/Post/Put/Delete/Call verbs, Sync or Async. IndexedFile: receives the engine's own FILE STATUS after every facade operation (00, 10, 22, 23, 35, 39 …).
- `Mode` (String, default "Async"; `Async` | `Sync`) — Async fires onComplete/onError later; Sync blocks and returns in-statement.
- `Busy` (Boolean, default 0; `1` (true) or `0` (false)) — Read-only runtime flag: an async operation is in flight.
- `TimeoutMs` (Integer, default 30000; milliseconds ≥ 0) — How long a request may take, in either Mode — Async or Sync. 0 falls back to `TimeoutSeconds` where the control has it (a RestClient); a WebSearch has none, so there 0 means no timeout.

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `ResponseBody` — The answer to the last async call, delivered with `onComplete`. Its shape is the method's — a Directions answer is seven TAB-separated fields, PlacesSearch is one line per result, a RestClient verb is the raw body. UNSTRING it in the onComplete handler; it is empty before the first call completes.
- `StatusCode` — The HTTP status of the last call. `0` when the request never reached a server.
- `Busy` — Read-only runtime flag: an async operation is in flight.
- `LastError` — Why the last async call failed, delivered with `onError`. Empty after a call that succeeded.

### Supported Events
- `onError` — the operation failed (message in `LastError`)
- `onTimeout` — the async operation exceeded its timeout; it has been cancelled, and `LastError` says so ("No answer within N seconds: the call was cancelled.")
- `onComplete` — the async operation finished successfully
- `onCancelled` — the async operation was cancelled

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `Get(url: String) → String` — HTTP GET. Async mode: returns immediately, response lands in `ResponseBody`/`StatusCode` + `onComplete`. Sync mode: returns the body.
- `Post(url: String, body: String) → String` — HTTP POST (same async/sync contract).
- `Put(url: String, body: String) → String` — HTTP PUT.
- `Delete(url: String) → String` — HTTP DELETE.
- `Call(verb: String, url: String, body: String?) → String` — Any verb by name.
- `SetHeader(name: String, value: String)` — Add a header for subsequent requests.
- `ClearHeaders()` — Drop all added headers.
- `SetTimeout(seconds: Integer)` — Set the request timeout.
- `Cancel()` — Cancel the in-flight request.
- `IsBusy() → Boolean (0/1)` — An async request is in flight.

---

## Control: SqlDatabase

Non-visual SQL connection (sqlite / postgres / mysql / mssql). Default size 64×64 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Driver` (String, default "sqlite"; one of: `sqlite` | `postgres` | `mysql`) — A label for the generated comments. The engine is chosen by the ConnectionString's scheme, not by this.
- `ConnectionString` (String, default "sqlite::memory:"; e.g. `sqlite::memory:`, `postgres://user:pw@host/db`) — Connection string; the scheme selects the engine. Used by the generated `<id>-CONNECT` paragraph, and by `Open()` when it is called with no argument.
- `AutoConnect` (Boolean, default 0; `1` (true) or `0` (false)) — Connects as the form starts (the generated `<id>-CONNECT`, before any handler runs) and closes as it ends.
- `ConnectionDataItem` (String, default (empty); COBOL data-item name) — An item the program declares that receives the connection handle — from the generated `<id>-CONNECT` and from `Open()` — for the COBOL-EXEC-SQL CALL surface.
- `ResultSetDataItem` (String, default (empty); COBOL data-item name) — An item the program declares that receives each row `Fetch()` returns (tab-separated), and spaces once the rows run out.
- `Mode` (String, default "Sync"; one of: `Sync` | `Async` (default Sync)) — `Sync` (the default, and the historical behaviour): `Query` / `Execute` run in the statement and return their count. `Async`: they run on a background worker and return 0 at once; `Busy` is true until the result arrives, which raises the SAME `onQueryComplete` (count in `ResultCount`) or `onQueryError` (`LastError`) a synchronous call raises — or `onTimeout` after `TimeoutMs`, or `onCancelled` after `Cancel()`. Then `Fetch()` reads the rows as usual. While a statement is in flight its connection is on the worker: a second `Query`/`Execute` on the control is ignored, and a CALL on the same handle answers that the connection is busy. `Open`, `Fetch` and `Close` stay synchronous.
- `Busy` (Boolean, default 0; true | false (runtime-only, read-only)) — With `Mode = Async`: true from `Query`/`Execute` until its `onQueryComplete`, `onQueryError`, `onTimeout` or `onCancelled`. `IsBusy()` reads it. Always false in `Sync` mode.
- `TimeoutMs` (Integer, default 0; milliseconds >= 0 (default 0 = no timeout)) — With `Mode = Async`: how long a statement may run before `onTimeout` fires and `Busy` clears. The database is not interrupted — the statement finishes on its worker and its result is discarded — and the connection is usable again once it does. Ignored in `Sync` mode.

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `ResultCount` — With `Mode = Async`: the count an asynchronous `Query` (result rows) or `Execute` (affected rows) produced — what the statement itself returns in `Sync` mode. Written just before `onQueryComplete`; 0 on `onQueryError`.
- `LastError` — Why the last async call failed, delivered with `onError`. Empty after a call that succeeded.
- `StatusCode` — The HTTP status of the last call. `0` when the request never reached a server.

### Supported Events
- `onQueryComplete` — the SQL statement finished
- `onConnectOk` — the database connection opened
- `onConnectError` — the database connection failed
- `onQueryError` — the SQL statement failed
- `onRowFetched` — Fetch() advanced to a row
- `onComplete` — the async operation finished successfully
- `onError` — the operation failed (message in `LastError`)
- `onCancelled` — the async operation was cancelled
- `onTimeout` — the async operation exceeded its timeout; it has been cancelled, and `LastError` says so ("No answer within N seconds: the call was cancelled.")

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `Open(connectionString: String) → Integer` — Open the connection; returns the handle (fires `onConnectOk`/`onConnectError`).
- `Execute(sql: String) → Integer` — Run a statement; returns the affected-row count (alias `Exec`). With `Mode = Async` it runs on a worker, returns 0 at once, and the count arrives in `ResultCount` with `onQueryComplete`.
- `Query(sql: String) → Integer` — Run a query; returns the result-row count. With `Mode = Async` it runs on a worker, returns 0 at once, and the count arrives in `ResultCount` with `onQueryComplete` — fetch the rows there.
- `Fetch() → Boolean (0/1)` — Advance to the next row (fires `onRowFetched`).
- `FetchAll() → Integer` — Row count of the current result set.
- `Close()` — Close the connection.

### Usage (inline `::` methods)
`Open(conn-string)` → handle · `Execute(sql)` / `Query(sql)` → row count · `Close()`.
Reading a result set:
- **`Fetch()` returns the next ROW**, its columns separated by TAB, and returns **EMPTY once the rows run out**. That is the loop's own terminator — write `PERFORM UNTIL 1 = 2 / MOVE DB-1::Fetch() TO WS-ROW / IF WS-ROW = SPACES EXIT PERFORM END-IF / ... / END-PERFORM`. It does NOT return a 1/0 flag; a loop testing it for `= 0` or `= 1` never ends.
- `ColumnNames()` → the column names of the last result set, TAB-separated in SELECT order, so they line up field-for-field with a row from `Fetch()`. Available even when the query matched no rows.
- `ColumnCount()` → how many · `ColumnName(n)` → the *n*-th name, 1-based.
Split a row on TAB with `UNSTRING ... DELIMITED BY X"09"`, or use reference modification if the widths are known.
### Usage (generated paragraphs and CALL API)
The IDE also generates paragraphs for a control named `DB-1`: `DB-1-CONNECT` (opens using `WS-DB-1-CONN-STRING`), `DB-1-EXEC` (runs `WS-SQL-QUERY`, row count in `WS-SQL-ROW-COUNT`), `DB-1-FETCH-ALL` (template row loop), `DB-1-COMMIT`, `DB-1-ROLLBACK`, `DB-1-CLOSE`. The low-level `CALL "COBOL-OPEN-DB" / "COBOL-EXEC-SQL" / "COBOL-FETCH-ROW" / ...` API is also available — it reads the CURRENT row one column at a time by INDEX (`COBOL-FETCH-ROW` + `COBOL-NEXT-ROW`). Do not interleave that traversal with `Fetch()` on one handle: `Fetch()` consumes as it reads, so mixing them skips rows.

---

## Control: IndexedFile

Non-visual COBOL indexed-file access (driven by generated PERFORM paragraphs). Default size 64×64 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `IndexedFile` (String, default (empty); `.cidx` schema name from the project) — Which indexed-file schema this control operates on.
- `OpenMode` (String, default "INPUT"; `INPUT` (read-only) | `I-O` (read-write)) — COBOL open mode used by the generated paragraphs.
- `AutoOpen` (Boolean, default 0; `1` (true) or `0` (false)) — Opens the file automatically when the form loads.
- `RecordName` (String, default (empty); COBOL record name or empty) — Overrides the schema's record item.
- `KeyName` (String, default (empty); COBOL key item or empty) — Overrides the schema's primary-key item.
- `CurrentKeyDataItem` (String, default (empty); COBOL data-item name or empty) — Item holding the key for START/READ positioning.
- `StatusDataItem` (String, default (empty); COBOL data-item name) — RestClient: receives every request's HTTP status (0 when no response came), from the ::Get/Post/Put/Delete/Call verbs, Sync or Async. IndexedFile: receives the engine's own FILE STATUS after every facade operation (00, 10, 22, 23, 35, 39 …).
- `CurrentRecordDataItem` (String, default (empty); COBOL data-item name or empty) — Every facade READ also lands the record here (`READ … INTO`). The program declares the item.
- `OperatorName` (String, default (empty); a name, a quoted literal, or a data-item name; or empty) — The operator recorded by `OPEN … REGISTERED USER`. A plain name is sent as a literal; the name of an item the form declares is sent as that item.

### Supported Events
- `onError` — the operation failed (message in `LastError`)
- `onComplete` — the async operation finished successfully
- `onCancelled` — the async operation was cancelled
- `onTimeout` — the async operation exceeded its timeout; it has been cancelled, and `LastError` says so ("No answer within N seconds: the call was cancelled.")

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.

### Usage (generated paragraphs — NOT `::` methods)
An IndexedFile control named `IXF-1` is driven with `PERFORM` on the paragraphs the IDE generates:
- `PERFORM IXF-1-OPEN` — open the file in `OpenMode` (`INPUT` or `I-O`).
- `MOVE key TO <key item>` then `PERFORM IXF-1-START` — position the record pointer (KEY >= value).
- `PERFORM IXF-1-READ-NEXT` / `IXF-1-READ-PREVIOUS` / `IXF-1-READ-FIRST` / `IXF-1-READ-LAST` — sequential reads; `WS-IXF-1-AT-END = 1` after the last record.
- `PERFORM IXF-1-READ-INVALID` — random read by key (INVALID KEY sets status `23`).
- `WRITE` / `REWRITE` / `DELETE` — use the plain COBOL verbs on the file's record.
- `PERFORM IXF-1-COMMIT` — flush pending writes (I-O mode).
- `PERFORM IXF-1-CLOSE` — close the file.
The two-character file status lands in the `StatusDataItem` (or `WS-IXF-1-STATUS`).

---

## Control: KnowledgeBase

Non-visual application Knowledge Base (spec 068): collections of the application's users' documents, each with a searchable index derived from them, kept under `Location` (default `<app>/assets/KB`). NOT the IDE's System or Project Knowledge Base, and a built application links nothing of the IDE for it. Documents may be Word (.docx and its macro/template variants), PowerPoint (.pptx family), Excel (.xlsx, and old .xls), OpenDocument text and spreadsheets (.odt, .ods), CSV/TSV tables, PDF (page text, no OCR), HTML, Markdown and plain text, and ZIP/TAR archives of any of these (spec 074). The format is recognised by content before name. Structure is kept, so a hit names its heading, `Slide N`, sheet or `Page N`, and a document inside an archive is named through it (`old.zip › legal/nda.docx`). Archives are read in memory within `ArchiveMaximumMegabytes`/`ArchiveMaximumFiles`/`ArchiveMaximumDepth`; nothing a document contains is executed. A document that cannot be read — old .doc/.ppt, password protected, a scanned PDF, damaged, unsupported — is skipped and reported by name with a code in `SkippedDocuments`, and never stops the rest. Every operation that writes or searches runs in the background and reports through events that carry their own property values — `onProgress`, then `onIndexed`, `onSearchComplete`, `onBusy` or `onError`. Several applications, on one machine or on a LAN share, can search and write one collection at once; a writer waits `WriteWaitMilliseconds` for another and then raises `onBusy`. Embedder `Lexical` matches words; `Endpoint` uses an embedding model on a server; `Builtin` runs the semantic model inside the application (project setting `[rag] embedder = "builtin"`, fetched once per installation with `FetchModel`). When the configured embedder cannot be used, search falls back to lexical and `SearchModeReason` says why. An AgentObject is given a collection as a tool with `AllowKnowledgeBase`; the model then searches it and cites the document each passage came from. Default size 56×56 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Location` (String, default "assets/KB"; folder path) — KnowledgeBase: the folder holding its collections. Relative paths are the application's folder, so the default `assets/KB` is `<app>/assets/KB`. May be a folder on another machine in the LAN.
- `Collection` (String, default (empty); collection name) — KnowledgeBase: the collection its document and search methods act on — a folder under `Location` holding `documents/` and its index.
- `Embedder` (String, default "Lexical"; one of: `Lexical` | `Endpoint` | `Builtin`) — KnowledgeBase: how text becomes vectors. `Lexical` (built in, matches words), `Endpoint` (an embedding model on a server), `Builtin` (the semantic model inside the application, linked only when the project sets `[rag] embedder = "builtin"`; the build then ships the model in `models/` beside `assets/`, so the application embeds offline from its first run).
- `Configuration` (String, default (empty); empty (this control's own settings), or the name of a project connection) — Where this control gets its connection. **Empty — the default — means the control's own properties below**, exactly as it has always worked. Otherwise it names one of the project's connections (Settings → Integrations), and that connection's settings replace the control's own before the form runs. On a **RestClient** that is the address, method, authentication scheme, headers and timeouts; on a **WebSearch** it is the provider and its engine id or instance URL — and deliberately NOT `NumResults` or `SafeSearch`, which are per-call settings the form changes at run time and which stay the control's own. An **AgentObject** is different in kind: it selects one of the machine's configured **Model Providers** (Settings → Models) rather than a project connection, taking that provider's protocol, endpoint and key — while `AgentModel`, `Temperature`, `MaximumTokens` and `TimeoutSeconds` stay the control's own, because one provider offers many models. Its `API Key` row disappears while bound, which is the point: an agent's key is entered once, in the Model Providers Manager, and never copied onto a form. That binding is **machine-scoped** — a provider not configured on a given machine is reported as exactly that, not as a broken project — and reaches a running application through `COBOLT_AGENT_PROVIDERS`. Define a service once and every form that uses it stays in step, instead of six forms drifting apart. The **credential is never part of the connection record** — that record round-trips in `cobolt.toml` and is meant to be committed, while the key lives in the machine-local store and reaches a running form through the environment, so a checked-out project carries the connections and each developer supplies their own key. A built application carries the connections baked in and takes each key from `COBOLT_CONNECTION_KEY_<ID>` on the machine that runs it. A Configuration naming a connection the project no longer has is an error, not a silent fall back to the local settings: the control was told to ignore those.
- `ModelEntry` (String, default (empty); empty, or the name of a model-list entry (run time)) — AgentObject / KnowledgeBase (spec 076): a model-list entry the PROGRAM handed over while it runs, with `COBOL::"MODEL-SET"( name api url model [status] )` (the program keeps its list in its own indexed file and hands it over at start-up; the runtime stores no list). When set it WINS over `Configuration` and the control's own settings: the entry gives the API, the endpoint and the key (stored with `CALL "COBOL-KEY-SET" USING name key [status]`, removed with `COBOL-KEY-REMOVE`, checked with `COBOL-KEY-IS-SET USING name flag` → Y/N — no CALL ever returns a key); the entry's model when it names one, else the agent's own; temperature, token limit and timeout stay the agent's. An unknown entry, or an entry whose provider needs a key (every provider but local Ollama — the IDE's rule) with no key stored, fails the Ask at once with `onError` and sends nothing. The IDE's provider list, model listing and connection test are available to the program too: `COBOL-PROVIDER-COUNT USING count`, `COBOL-PROVIDER-GET USING index id label endpoint needs-key`, `COBOL-MODEL-LIST USING provider endpoint key count status [entry]` + `COBOL-MODEL-LIST-GET USING index model`, and `COBOL-MODEL-TEST USING provider endpoint model key status [entry]` (status `OK` or the IDE's own message; a blank key with an entry uses that entry's stored key, which is never returned). Keys live in `<app>/settings/model-keys.dat`, shared by every user of the installation, encrypted with a key derived from the installation folder and the machine's name — unreadable at a glance and useless copied elsewhere, but usable by anyone who can run the application on that machine. A key is never shown in a log (the verbose log masks it), an error, or anything sent to a model. A change to an entry an agent used raises `onModelChanged` on that agent. `COBOL-MODEL-REMOVE USING name` withdraws an entry.
- `EmbeddingURL` (String, default "http://localhost:11434"; HTTP(S) URL) — KnowledgeBase (Endpoint): the embedding server's base URL; `/api/embed` (Ollama) or `/embeddings` (OpenAI-style) is added.
- `EmbeddingAPI` (String, default "Ollama"; one of: `Ollama` | `OpenAI` | `LMStudio` | `Custom`) — KnowledgeBase (Endpoint): the server's wire format — Ollama's, or the OpenAI-style one every other choice uses.
- `EmbeddingModel` (String, default "nomic-embed-text"; model id string) — KnowledgeBase (Endpoint): the embedding model requested, e.g. `nomic-embed-text`.
- `EmbeddingAPIKey` (String, default (empty); credential (never saved in the form)) — KnowledgeBase (Endpoint): the embedding server's key. Filled at run time from the machine's key store or the environment, like an AgentObject's `AgentAPIKey`; never written into the `.cfrm`.
- `WriteWaitMilliseconds` (Integer, default 5000; integer ms (default 5000)) — KnowledgeBase: how long a write waits while another application writes the same collection before raising `onBusy`.
- `MaximumResults` (Integer, default 5; integer (default 5)) — KnowledgeBase: hits a `Search` returns when it names no maximum.
- `ArchiveMaximumMegabytes` (Integer, default 500; integer MB (default 500)) — KnowledgeBase: the most an archive (ZIP/TAR, nested ones included) may unpack to; past it the whole archive is skipped as `too_large`.
- `ArchiveMaximumFiles` (Integer, default 10000; integer (default 10000)) — KnowledgeBase: the most files an archive (nested ones included) may hold; past it the whole archive is skipped as `too_large`.
- `ArchiveMaximumDepth` (Integer, default 3; integer (default 3)) — KnowledgeBase: how many archives deep documents are read — 1 reads an archive but none inside it; deeper nesting skips the archive as `too_large`.

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `Busy` — Read-only runtime flag: an async operation is in flight.
- `LastError` — Why the last async call failed, delivered with `onError`. Empty after a call that succeeded.
- `SearchMode` — KnowledgeBase: how the last search was scored, or how the last update stored its documents — set on EVERY update that indexed something, so `Semantic` in `onIndexed` confirms the documents got their embeddings; `SearchModeReason` is empty then.
- `SearchModeReason` — KnowledgeBase: why the search is lexical, when it is — the lexical embedder, an unreachable embedding server, a model not fetched, or a collection indexed with another embedder.
- `ProgressDocument` — KnowledgeBase: in `onProgress`, the document just handled.
- `ProgressCurrent` — KnowledgeBase: in `onProgress`, how many documents are done.
- `ProgressTotal` — KnowledgeBase: in `onProgress`, how many documents the update touches.
- `ProgressPassage` — KnowledgeBase: in `onProgress`, while `ProgressDocument` is being embedded, how many of its passages are done — reported after each batch of 16, from 0 (split into passages, embedding not begun) up to `ProgressPassages`. 0 in a report made after a document is finished.
- `ProgressPassages` — KnowledgeBase: in `onProgress`, how many passages `ProgressDocument` was split into, while it is being embedded; 0 otherwise. Embedding is most of an update's time, so this is the number to show a progress bar against.
- `PassageCount` — KnowledgeBase: in `onIndexed`, how many passages the update stored — read with `SearchMode` to confirm they were embedded (`Semantic`) or stored for word search only (`Lexical`, `SearchModeReason` saying why).
- `AddedCount` — KnowledgeBase: in `onIndexed`, documents newly indexed.
- `UpdatedCount` — KnowledgeBase: in `onIndexed`, documents re-indexed because they changed.
- `RemovedCount` — KnowledgeBase: in `onIndexed`, documents removed from the index.
- `SkippedCount` — KnowledgeBase: in `onIndexed`, documents that could not be read.
- `SkippedDocuments` — KnowledgeBase: in `onIndexed`, `document: code (explanation)` for each skipped document, separated by `; `. A document inside an archive is named through it: `old.zip › legal/nda.docx`. Codes: `unsupported`, `legacy_office` (.doc/.ppt), `password_protected`, `no_text` (a PDF with no text layer, e.g. a scan), `damaged`, `too_large` (an archive past its bounds), `unreadable` (the file could not be opened).
- `ResultCount` — KnowledgeBase: in `onSearchComplete`, how many hits the search returned; read each with `GetResult…(n)`.
- `CollectionCount` — KnowledgeBase: collections found by the last `ListCollections`.
- `DocumentCount` — KnowledgeBase: documents found by the last `ListDocuments`.

### Supported Events
- `onProgress` — KnowledgeBase: an update moved on — `ProgressDocument`, `ProgressCurrent`, `ProgressTotal` hold THIS event's values
- `onIndexed` — KnowledgeBase: an update finished (add, update, delete, import, refresh, reindex, model fetch) — read `AddedCount`/`UpdatedCount`/`RemovedCount`/`SkippedCount`/`SkippedDocuments`
- `onSearchComplete` — KnowledgeBase: a search finished — `ResultCount` hits, read with `GetResultDocument/Heading/Passage/Score(n)`; `SearchMode` says Semantic or Lexical
- `onBusy` — KnowledgeBase: another application held the collection's write lock past `WriteWaitMilliseconds`; `LastError` says so and nothing was written
- `onError` — the operation failed (message in `LastError`)

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `CreateCollection(name: String) → Boolean (0/1)` — Create a collection folder (and its empty index) under `Location`. Never touches an existing one.
- `RemoveCollection(name: String) → Boolean (0/1)` — Take a collection out of use by moving its folder aside (`<name>.removed-<time>`); its documents are kept, never deleted.
- `ListCollections() → Integer` — List the collections under `Location`; answers how many (also `CollectionCount`). Read each with `GetCollection(n)`.
- `GetCollection(n: Integer) → String` — The n-th collection's name, from the last `ListCollections` (1-based).
- `AddDocument(name: String, text: String) → Boolean (0/1)` — ASYNCHRONOUS: save `text` as the document `name` (sub-folders allowed, e.g. `policies/leave.md`) in the collection and index it; `onIndexed` follows. `0` with `LastError` when another operation is running on this control.
- `UpdateDocument(name: String, text: String) → Boolean (0/1)` — ASYNCHRONOUS: replace a document's text and re-index it; `onIndexed` follows.
- `ImportDocument(path: String, name: String?) → Boolean (0/1)` — ASYNCHRONOUS: copy a file into the collection (named after the file unless `name` is given) and index it; `onIndexed` follows.
- `DeleteDocument(name: String) → Boolean (0/1)` — ASYNCHRONOUS: delete a document from the collection's folder and from its index; `onIndexed` follows.
- `ListDocuments() → Integer` — List the collection's documents; answers how many (also `DocumentCount`). Read each with `GetDocument(n)`.
- `GetDocument(n: Integer) → String` — The n-th document's name, from the last `ListDocuments` (1-based).
- `Refresh() → Boolean (0/1)` — ASYNCHRONOUS: compare the documents folder with the index by content and index only what was added, changed or removed — including changes made outside the application; `onProgress`…`onIndexed` follow.
- `Reindex() → Boolean (0/1)` — ASYNCHRONOUS: index every document again with this control's embedder — how a collection changes embedder.
- `Search(query: String, max: Integer?) → Boolean (0/1)` — ASYNCHRONOUS: search the collection; `onSearchComplete` follows with `ResultCount` hits (at most `max`, else `MaximumResults`).
- `GetResultDocument(n: Integer) → String` — The n-th hit's document, from the last search (1-based).
- `GetResultHeading(n: Integer) → String` — The n-th hit's section: `document › heading › sub-heading`.
- `GetResultPassage(n: Integer) → String` — The n-th hit's text — a split section comes back whole.
- `GetResultScore(n: Integer) → String` — The n-th hit's score (higher is closer), four decimals.
- `FetchModel() → Boolean (0/1)` — ASYNCHRONOUS: fetch the built-in model (~470 MB) into `<app>/models` (beside `assets/`), once per installation; `onProgress` then `onIndexed`. A no-op when the model is already there — a build that links it ships it in `models/`, and an older `<app>/assets/models` copy is used as found.
- `Cancel()` — Stop the running operation; what it already committed stays.

### Usage (events carry their own values)
Every call that writes or searches answers at once with 1 (started) or 0 (see `LastError`); the result arrives as an event, and the properties you read in that event's branch are the values THAT event carried:
```cobol
MOVE KB-1::AddDocument("policies/leave.md", WS-TEXT) TO WS-OK
...
WHEN "onProgress"
MOVE KB-1::ProgressCurrent TO WS-DONE
MOVE KB-1::ProgressTotal   TO WS-TOTAL
WHEN "onIndexed"
MOVE KB-1::Search("annual leave") TO WS-OK
WHEN "onSearchComplete"
MOVE KB-1::GetResultDocument(1) TO WS-DOC
MOVE KB-1::GetResultPassage(1)  TO WS-PASSAGE
```
One operation at a time per control: a second call while one runs answers 0. To let a model search a collection: `MOVE AGT-1::AllowKnowledgeBase("KB-1") TO WS-OK`.

---

## Control: Slider

Draggable value selector within Minimum..Maximum. Default size 200×36 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Minimum` (Integer, default 0; integer ≤ Maximum) — Lower bound of the value range.
- `Maximum` (Integer, default 100; integer ≥ Minimum) — Upper bound of the value range.
- `Value` (Integer, default 0; ProgressBar/Slider/NumericUpDown: integer within Minimum..Maximum; DateTimePicker: `YYYY-MM-DD`, `HH:MM`, or `YYYY-MM-DD HH:MM`) — Current value. A DateTimePicker always stores ISO, whatever Format displays, so a COBOL handler reading Value gets one shape; which halves are present follows Format (a Time picker stores the time alone).
- `Step` (Integer, default 10; number > 0) — How far one step moves the value: the ↑/↓ keys on a NumericUpDown, ←/→/↑/↓ on a Slider, the wheel, and the Increment()/Decrement() methods — which stop at Minimum and Maximum. A fractional Step (0.5) steps in fractions.
- `LargeChange` (Integer, default 20; number > 0 (default 20)) — Slider: how far Page Up / Page Down move the value while the slider has the keyboard (the arrows move by Step; Home / End go to Minimum / Maximum).
- `Orientation` (String, default "Horizontal"; `Horizontal` | `Vertical`) — Layout axis.
- `TickFrequency` (Integer, default 10; integer > 0 (value units)) — Draw a tick every N units.
- `TickStyle` (String, default "Bottom"; one of: `None` | `Top` | `Bottom` | `Both`) — Where slider ticks are drawn. On a vertical slider `Top` is the left side and `Bottom` the right.
- `TrackColor` (String, default "#AAAAAA"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The part still to travel: a Slider's rail from Value to Maximum, a Knob's arc from Value round to Maximum. Outranks the Appearance BackgroundColor; left at its default the active theme paints.
- `ThumbColor` (String, default "#0078D7"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Slider knob color. Outranks the Appearance ForegroundColor; left at its default the active theme paints.
- `FillColor` (String, default "#0078D7"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Shape interior fill. On a Slider, the travelled part of the rail — Minimum to Value — which is the part that reads as filled; left at its default the active theme paints.
- `ShowValue` (Boolean, default 0; `1` (true) or `0` (false)) — Draws the numeric value on the control.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Supported Events
- `onChange` — value or text changed
- `onValueChanged` — value changed (the new value is delivered)
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetValue(value: Integer)` — Set the current value.
- `GetValue() → Integer` — Read the current value.
- `Increment()` — Add Step to Value.
- `Decrement()` — Subtract Step from Value.
- `Reset()` — Return Value to Minimum.

---

## Control: BarChart

Bar chart. Default size 320×220 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Title` (String, default (empty); free text) — Chart title — and, on a DataGrid, the caption centred on the band that carries the CSV button; empty means no caption. Also the window title on Form methods.
- `TitleFontSize` (Integer, default 0; points >= 0; 0 = follow the chart) — The title's own point size. **0** - the default - leaves it following the chart's own `FontSize`, which is how every chart drawn before this property behaves. Set it and the title is that size exactly, and the band reserved above the plot grows with it, so a large title takes room rather than printing over the plot it labels.
- `TitleColor` (String, default (empty); hex colour; empty = automatic) — The title's own colour. **Empty** - the default - keeps the automatic choice, which reads dark on a face that can carry it and switches to the readable pole when it cannot. That is why blank cannot simply mean grey: a fixed grey is invisible on a dark `Monochrome` face and near-invisible on a white one.
- `ShowLegend` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the series legend.
- `ShowGridLines` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the plot grid.
- `ShowXAxis` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the X axis line. Bar, Line, Area and Scatter charts only — a pie or a donut has no axes and does not carry it.
- `ShowYAxis` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the Y axis line. Bar, Line, Area and Scatter charts only — a pie or a donut has no axes and does not carry it.
- `ShowTooltips` (Boolean, default 1; `1` (true) or `0` (false)) — In the running form, the bar, point or slice under the pointer shows `label: value` in a tooltip. Live data only — the sample a chart shows before it has any is not data. On by default.
- `AnimateOnLoad` (Boolean, default 1; `1` (true) or `0` (false)) — The first time the running chart has data, every mark grows into place — bars and lines rise, a pie sweeps round — over `AnimationDuration` (250 ms at least). On by default. `AnimateValues` is the separate animation for data that CHANGES afterwards.
- `AnimateValues` (Boolean, default 0; `1` (true) or `0` (false)) — Animates a CHANGE OF DATA. With it on, a chart whose points are replaced TRAVELS from the values it is showing to the new ones instead of cutting to them; a point the new set added rises from zero, and one it dropped simply stops being drawn. The labels are the new set's from the first frame, so a half-played move never shows a point under the name it used to have. Off by default: a chart filled once, on load, should not spend two seconds arriving. The whole series moves together, so the chart settles in the same time with four points or forty. Set `AnimationDuration` to say how long.
- `AnimationDuration` (Integer, default 2000; milliseconds, 250 or more) — How long an `AnimateValues` move, and the `AnimateOnLoad` growth, take — for the WHOLE series rather than per point. Default 2000. Anything below 250 is raised to 250 — under that the eye reads a jump rather than a movement, and the property would be honoured in name only.
- `HideBackground` (Boolean, default 0; `1` (true) or `0` (false)) — Hides the fill/border while keeping the content visible.
- `Monochrome` (Boolean, default 0; `1` (true) or `0` (false)) — Tonal single-color rendering instead of the palette.
- `MonochromeColor` (String, default "#3F6FB5"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Base color for monochrome mode.
- `MonochromeGradient` (Boolean, default 0; `1` (true) or `0` (false)) — Diagonal light-to-dark shading in monochrome mode.
- `XAxisLabel` (String, default (empty); free text) — X axis caption.
- `YAxisLabel` (String, default (empty); free text) — Y axis caption.
- `SeriesColors` (String, default "#4C9BE8,#E87A4C,#4CE87A,#E84C9B,#9B4CE8,#E8C84C"; comma-separated colours, e.g. `#D01010,#10D010`) — The colours of the series — a pie's or donut's slices — in order, repeating when there are more series than colours. The seeded list means 'not chosen': the theme's palette paints the chart until you change it.
- `DataSource` (String, default (empty); COBOL table data-item name (charts / repeating GroupBox / DataGrid binding)) — Table the control binds to. On a chart, `SET-TABLE` reads each occurrence's label and value from the sub-fields named by `LabelField` and `ValueFields`; with no `LabelField` it reads the fixed `PIC X(64)` label + `PIC 9(18)V9(6)` value layout.
- `DataCount` (String, default (empty); COBOL data-item name) — Item holding the number of occupied table rows.
- `LabelField` (String, default (empty); sub-field name) — Charts: the table sub-field (an OCCURS item) whose value labels each point. With it and `ValueFields` set, `SET-TABLE` reads occurrences 1 to `DataCount` from these fields instead of the fixed layout.
- `ValueFields` (String, default (empty); comma-separated sub-field names) — Charts: the table sub-fields holding each point's values — the first is the first series, each further field one more series (a bar, line or area chart draws them all; pie, donut and scatter use the first). `SeriesLabels` names them in the legend; `Stacked` piles them up.
- `SeriesLabels` (String, default (empty); comma-separated display names) — The names the legend gives the series, in order; a series left unnamed shows as `Series n`. A pie's legend names its slices from the data instead.
- `Horizontal` (Boolean, default 0; `1` (true) or `0` (false)) — Horizontal bars instead of vertical.
- `Stacked` (Boolean, default 0; `1` (true) or `0` (false)) — BarChart / AreaChart (default false): with several series, pile each series on the ones before it — one bar per label made of coloured segments, or area bands laid one over the other — so a label's marks add up to its total, and the plot scales to the largest total. Off, the series stand side by side (bars) or overlap from the axis (areas). With one series it changes nothing. Series come from `AddPoint(label, v1, v2, …)` or from every field named in `ValueFields`.
- `BarCornerRadius` (Integer, default 3; pixels ≥ 0) — Rounding on every corner of a bar, held to half the bar's width and height so a short bar stays a bar.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `BorderColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderGradientEnabled` (Boolean, default 0; `1` (true) or `0` (false)) — Charts only. Draws the frame border as a two-colour gradient from `BorderGradientStartColor` to `BorderGradientEndColor` along `BorderGradientDirection`, `BorderWidth` wide, following `CornerRadius`. Off, the border is `BorderStyle` in `BorderColor`. Charts honour `BorderStyle`, `BorderWidth` and `BorderColor` like every other control (they used to draw a fixed 1 px line); `BorderStyle` `None` removes the border, gradient and blur alike.
- `BorderGradientStartColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient starts.
- `BorderGradientEndColor` (String, default "#8FB4FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient ends.
- `BorderGradientDirection` (String, default "South"; one of: `North` | `NorthEast` | `East` | `SouthEast` | `South` | `SouthWest` | `West` | `NorthWest`) — Charts only. Direction the border gradient flows toward.
- `BorderBlur` (Integer, default 0; `0`–`40` (pixels)) — Charts only. A soft glow outward from the frame border, this many pixels wide, in the border's colour (the gradient's mid colour when the gradient is on) — the shadow stack's own falloff, faintest outermost. `0` (the default) draws none. Drawn at the chart's inherited alpha, never at its own `Transparency`, so it stays with the border on a see-through chart.
- `BorderTransparency` (Integer, default 0; `0`–`100`) — Charts only. The frame border's own transparency: `0` (the default) opaque, `100` invisible. It fades the border line, the gradient ring and the blur rings together — separately from the chart's `Transparency`, which reaches only the face — so a frame can fade independently of what it frames.
- `CornerRadius` (Integer, default 8; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onDataChanged` — the chart's data-bearing properties changed
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddPoint(label: String, value: Number)` — Append one data point and repaint. On a BarChart, LineChart or AreaChart each further argument — `AddPoint(label, v1, v2, …)` — is the point's value in the next series (a point given fewer is 0 in the rest); on a ScatterChart the third argument is the bubble's size.
- `Clear()` — Remove all pushed data (chart falls back to its sample preview).
- `Refresh()` — Force a repaint with the current data.
- `RefreshBinding() → Integer` — Bound to a COBOL table: reload the points from it (category field, value field), replacing the series; returns the point count. Also runs by itself as the form opens, after onLoad.

### Data flow
Three equivalent ways to feed the chart:
1. Inline methods: `Chart-1::AddPoint("Jan", 150).` / `Chart-1::Clear().` / `Chart-1::Refresh().`
2. Generated paragraphs: `PERFORM Chart-1-ADD-POINT` (after `MOVE`s to `WS-Chart-1-SELECTED-LBL` / `-SELECTED-VAL`), `PERFORM Chart-1-SET-TABLE`, `PERFORM Chart-1-CLEAR`, `PERFORM Chart-1-REFRESH`.
3. Built-in calls: `COBOL::"CHART-ADD-POINT"( "Chart-1" label value )` and `COBOL::"CHART-SET-TABLE"( "Chart-1" table count )` (table rows: `PIC X(64)` label + `PIC 9(18)V9(6)` value).
Or bind declaratively with the `DataSource`/`DataCount`/`LabelField`/`ValueFields` properties.

---

## Control: LineChart

Line chart. Default size 320×220 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Title` (String, default (empty); free text) — Chart title — and, on a DataGrid, the caption centred on the band that carries the CSV button; empty means no caption. Also the window title on Form methods.
- `TitleFontSize` (Integer, default 0; points >= 0; 0 = follow the chart) — The title's own point size. **0** - the default - leaves it following the chart's own `FontSize`, which is how every chart drawn before this property behaves. Set it and the title is that size exactly, and the band reserved above the plot grows with it, so a large title takes room rather than printing over the plot it labels.
- `TitleColor` (String, default (empty); hex colour; empty = automatic) — The title's own colour. **Empty** - the default - keeps the automatic choice, which reads dark on a face that can carry it and switches to the readable pole when it cannot. That is why blank cannot simply mean grey: a fixed grey is invisible on a dark `Monochrome` face and near-invisible on a white one.
- `ShowLegend` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the series legend.
- `ShowGridLines` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the plot grid.
- `ShowXAxis` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the X axis line. Bar, Line, Area and Scatter charts only — a pie or a donut has no axes and does not carry it.
- `ShowYAxis` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the Y axis line. Bar, Line, Area and Scatter charts only — a pie or a donut has no axes and does not carry it.
- `ShowTooltips` (Boolean, default 1; `1` (true) or `0` (false)) — In the running form, the bar, point or slice under the pointer shows `label: value` in a tooltip. Live data only — the sample a chart shows before it has any is not data. On by default.
- `AnimateOnLoad` (Boolean, default 1; `1` (true) or `0` (false)) — The first time the running chart has data, every mark grows into place — bars and lines rise, a pie sweeps round — over `AnimationDuration` (250 ms at least). On by default. `AnimateValues` is the separate animation for data that CHANGES afterwards.
- `AnimateValues` (Boolean, default 0; `1` (true) or `0` (false)) — Animates a CHANGE OF DATA. With it on, a chart whose points are replaced TRAVELS from the values it is showing to the new ones instead of cutting to them; a point the new set added rises from zero, and one it dropped simply stops being drawn. The labels are the new set's from the first frame, so a half-played move never shows a point under the name it used to have. Off by default: a chart filled once, on load, should not spend two seconds arriving. The whole series moves together, so the chart settles in the same time with four points or forty. Set `AnimationDuration` to say how long.
- `AnimationDuration` (Integer, default 2000; milliseconds, 250 or more) — How long an `AnimateValues` move, and the `AnimateOnLoad` growth, take — for the WHOLE series rather than per point. Default 2000. Anything below 250 is raised to 250 — under that the eye reads a jump rather than a movement, and the property would be honoured in name only.
- `HideBackground` (Boolean, default 0; `1` (true) or `0` (false)) — Hides the fill/border while keeping the content visible.
- `Monochrome` (Boolean, default 0; `1` (true) or `0` (false)) — Tonal single-color rendering instead of the palette.
- `MonochromeColor` (String, default "#3F6FB5"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Base color for monochrome mode.
- `MonochromeGradient` (Boolean, default 0; `1` (true) or `0` (false)) — Diagonal light-to-dark shading in monochrome mode.
- `XAxisLabel` (String, default (empty); free text) — X axis caption.
- `YAxisLabel` (String, default (empty); free text) — Y axis caption.
- `SeriesColors` (String, default "#4C9BE8,#E87A4C,#4CE87A,#E84C9B,#9B4CE8,#E8C84C"; comma-separated colours, e.g. `#D01010,#10D010`) — The colours of the series — a pie's or donut's slices — in order, repeating when there are more series than colours. The seeded list means 'not chosen': the theme's palette paints the chart until you change it.
- `DataSource` (String, default (empty); COBOL table data-item name (charts / repeating GroupBox / DataGrid binding)) — Table the control binds to. On a chart, `SET-TABLE` reads each occurrence's label and value from the sub-fields named by `LabelField` and `ValueFields`; with no `LabelField` it reads the fixed `PIC X(64)` label + `PIC 9(18)V9(6)` value layout.
- `DataCount` (String, default (empty); COBOL data-item name) — Item holding the number of occupied table rows.
- `LabelField` (String, default (empty); sub-field name) — Charts: the table sub-field (an OCCURS item) whose value labels each point. With it and `ValueFields` set, `SET-TABLE` reads occurrences 1 to `DataCount` from these fields instead of the fixed layout.
- `ValueFields` (String, default (empty); comma-separated sub-field names) — Charts: the table sub-fields holding each point's values — the first is the first series, each further field one more series (a bar, line or area chart draws them all; pie, donut and scatter use the first). `SeriesLabels` names them in the legend; `Stacked` piles them up.
- `SeriesLabels` (String, default (empty); comma-separated display names) — The names the legend gives the series, in order; a series left unnamed shows as `Series n`. A pie's legend names its slices from the data instead.
- `Smooth` (Boolean, default 1; `1` (true) or `0` (false)) — Catmull-Rom smoothing of the polyline.
- `ShowPoints` (Boolean, default 1; `1` (true) or `0` (false)) — Draws a marker on every point of a Line or Area chart.
- `PointRadius` (Integer, default 4; pixels > 0) — The marker radius on a Line or Area chart, and a Scatter point's radius when it has no bubble sizes.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `BorderColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderGradientEnabled` (Boolean, default 0; `1` (true) or `0` (false)) — Charts only. Draws the frame border as a two-colour gradient from `BorderGradientStartColor` to `BorderGradientEndColor` along `BorderGradientDirection`, `BorderWidth` wide, following `CornerRadius`. Off, the border is `BorderStyle` in `BorderColor`. Charts honour `BorderStyle`, `BorderWidth` and `BorderColor` like every other control (they used to draw a fixed 1 px line); `BorderStyle` `None` removes the border, gradient and blur alike.
- `BorderGradientStartColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient starts.
- `BorderGradientEndColor` (String, default "#8FB4FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient ends.
- `BorderGradientDirection` (String, default "South"; one of: `North` | `NorthEast` | `East` | `SouthEast` | `South` | `SouthWest` | `West` | `NorthWest`) — Charts only. Direction the border gradient flows toward.
- `BorderBlur` (Integer, default 0; `0`–`40` (pixels)) — Charts only. A soft glow outward from the frame border, this many pixels wide, in the border's colour (the gradient's mid colour when the gradient is on) — the shadow stack's own falloff, faintest outermost. `0` (the default) draws none. Drawn at the chart's inherited alpha, never at its own `Transparency`, so it stays with the border on a see-through chart.
- `BorderTransparency` (Integer, default 0; `0`–`100`) — Charts only. The frame border's own transparency: `0` (the default) opaque, `100` invisible. It fades the border line, the gradient ring and the blur rings together — separately from the chart's `Transparency`, which reaches only the face — so a frame can fade independently of what it frames.
- `CornerRadius` (Integer, default 8; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onDataChanged` — the chart's data-bearing properties changed
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddPoint(label: String, value: Number)` — Append one data point and repaint. On a BarChart, LineChart or AreaChart each further argument — `AddPoint(label, v1, v2, …)` — is the point's value in the next series (a point given fewer is 0 in the rest); on a ScatterChart the third argument is the bubble's size.
- `Clear()` — Remove all pushed data (chart falls back to its sample preview).
- `Refresh()` — Force a repaint with the current data.
- `RefreshBinding() → Integer` — Bound to a COBOL table: reload the points from it (category field, value field), replacing the series; returns the point count. Also runs by itself as the form opens, after onLoad.

### Data flow
Three equivalent ways to feed the chart:
1. Inline methods: `Chart-1::AddPoint("Jan", 150).` / `Chart-1::Clear().` / `Chart-1::Refresh().`
2. Generated paragraphs: `PERFORM Chart-1-ADD-POINT` (after `MOVE`s to `WS-Chart-1-SELECTED-LBL` / `-SELECTED-VAL`), `PERFORM Chart-1-SET-TABLE`, `PERFORM Chart-1-CLEAR`, `PERFORM Chart-1-REFRESH`.
3. Built-in calls: `COBOL::"CHART-ADD-POINT"( "Chart-1" label value )` and `COBOL::"CHART-SET-TABLE"( "Chart-1" table count )` (table rows: `PIC X(64)` label + `PIC 9(18)V9(6)` value).
Or bind declaratively with the `DataSource`/`DataCount`/`LabelField`/`ValueFields` properties.

---

## Control: PieChart

Pie chart. Default size 240×240 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Title` (String, default (empty); free text) — Chart title — and, on a DataGrid, the caption centred on the band that carries the CSV button; empty means no caption. Also the window title on Form methods.
- `TitleFontSize` (Integer, default 0; points >= 0; 0 = follow the chart) — The title's own point size. **0** - the default - leaves it following the chart's own `FontSize`, which is how every chart drawn before this property behaves. Set it and the title is that size exactly, and the band reserved above the plot grows with it, so a large title takes room rather than printing over the plot it labels.
- `TitleColor` (String, default (empty); hex colour; empty = automatic) — The title's own colour. **Empty** - the default - keeps the automatic choice, which reads dark on a face that can carry it and switches to the readable pole when it cannot. That is why blank cannot simply mean grey: a fixed grey is invisible on a dark `Monochrome` face and near-invisible on a white one.
- `ShowLegend` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the series legend.
- `ShowGridLines` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the plot grid.
- `ShowTooltips` (Boolean, default 1; `1` (true) or `0` (false)) — In the running form, the bar, point or slice under the pointer shows `label: value` in a tooltip. Live data only — the sample a chart shows before it has any is not data. On by default.
- `AnimateOnLoad` (Boolean, default 1; `1` (true) or `0` (false)) — The first time the running chart has data, every mark grows into place — bars and lines rise, a pie sweeps round — over `AnimationDuration` (250 ms at least). On by default. `AnimateValues` is the separate animation for data that CHANGES afterwards.
- `AnimateValues` (Boolean, default 0; `1` (true) or `0` (false)) — Animates a CHANGE OF DATA. With it on, a chart whose points are replaced TRAVELS from the values it is showing to the new ones instead of cutting to them; a point the new set added rises from zero, and one it dropped simply stops being drawn. The labels are the new set's from the first frame, so a half-played move never shows a point under the name it used to have. Off by default: a chart filled once, on load, should not spend two seconds arriving. The whole series moves together, so the chart settles in the same time with four points or forty. Set `AnimationDuration` to say how long.
- `AnimationDuration` (Integer, default 2000; milliseconds, 250 or more) — How long an `AnimateValues` move, and the `AnimateOnLoad` growth, take — for the WHOLE series rather than per point. Default 2000. Anything below 250 is raised to 250 — under that the eye reads a jump rather than a movement, and the property would be honoured in name only.
- `HideBackground` (Boolean, default 0; `1` (true) or `0` (false)) — Hides the fill/border while keeping the content visible.
- `Monochrome` (Boolean, default 0; `1` (true) or `0` (false)) — Tonal single-color rendering instead of the palette.
- `MonochromeColor` (String, default "#3F6FB5"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Base color for monochrome mode.
- `MonochromeGradient` (Boolean, default 0; `1` (true) or `0` (false)) — Diagonal light-to-dark shading in monochrome mode.
- `XAxisLabel` (String, default (empty); free text) — X axis caption.
- `YAxisLabel` (String, default (empty); free text) — Y axis caption.
- `SeriesColors` (String, default "#4C9BE8,#E87A4C,#4CE87A,#E84C9B,#9B4CE8,#E8C84C"; comma-separated colours, e.g. `#D01010,#10D010`) — The colours of the series — a pie's or donut's slices — in order, repeating when there are more series than colours. The seeded list means 'not chosen': the theme's palette paints the chart until you change it.
- `DataSource` (String, default (empty); COBOL table data-item name (charts / repeating GroupBox / DataGrid binding)) — Table the control binds to. On a chart, `SET-TABLE` reads each occurrence's label and value from the sub-fields named by `LabelField` and `ValueFields`; with no `LabelField` it reads the fixed `PIC X(64)` label + `PIC 9(18)V9(6)` value layout.
- `DataCount` (String, default (empty); COBOL data-item name) — Item holding the number of occupied table rows.
- `LabelField` (String, default (empty); sub-field name) — Charts: the table sub-field (an OCCURS item) whose value labels each point. With it and `ValueFields` set, `SET-TABLE` reads occurrences 1 to `DataCount` from these fields instead of the fixed layout.
- `ValueFields` (String, default (empty); comma-separated sub-field names) — Charts: the table sub-fields holding each point's values — the first is the first series, each further field one more series (a bar, line or area chart draws them all; pie, donut and scatter use the first). `SeriesLabels` names them in the legend; `Stacked` piles them up.
- `SeriesLabels` (String, default (empty); comma-separated display names) — The names the legend gives the series, in order; a series left unnamed shows as `Series n`. A pie's legend names its slices from the data instead.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `BorderColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderGradientEnabled` (Boolean, default 0; `1` (true) or `0` (false)) — Charts only. Draws the frame border as a two-colour gradient from `BorderGradientStartColor` to `BorderGradientEndColor` along `BorderGradientDirection`, `BorderWidth` wide, following `CornerRadius`. Off, the border is `BorderStyle` in `BorderColor`. Charts honour `BorderStyle`, `BorderWidth` and `BorderColor` like every other control (they used to draw a fixed 1 px line); `BorderStyle` `None` removes the border, gradient and blur alike.
- `BorderGradientStartColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient starts.
- `BorderGradientEndColor` (String, default "#8FB4FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient ends.
- `BorderGradientDirection` (String, default "South"; one of: `North` | `NorthEast` | `East` | `SouthEast` | `South` | `SouthWest` | `West` | `NorthWest`) — Charts only. Direction the border gradient flows toward.
- `BorderBlur` (Integer, default 0; `0`–`40` (pixels)) — Charts only. A soft glow outward from the frame border, this many pixels wide, in the border's colour (the gradient's mid colour when the gradient is on) — the shadow stack's own falloff, faintest outermost. `0` (the default) draws none. Drawn at the chart's inherited alpha, never at its own `Transparency`, so it stays with the border on a see-through chart.
- `BorderTransparency` (Integer, default 0; `0`–`100`) — Charts only. The frame border's own transparency: `0` (the default) opaque, `100` invisible. It fades the border line, the gradient ring and the blur rings together — separately from the chart's `Transparency`, which reaches only the face — so a frame can fade independently of what it frames.
- `ShowLabels` (Boolean, default 1; `1` (true) or `0` (false)) — Draws slice labels.
- `LabelFormat` (String, default "percent"; one of: `percent` | `value` | `label`) — What pie/donut slice labels show.
- `CornerRadius` (Integer, default 8; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onDataChanged` — the chart's data-bearing properties changed
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddPoint(label: String, value: Number)` — Append one data point and repaint. On a BarChart, LineChart or AreaChart each further argument — `AddPoint(label, v1, v2, …)` — is the point's value in the next series (a point given fewer is 0 in the rest); on a ScatterChart the third argument is the bubble's size.
- `Clear()` — Remove all pushed data (chart falls back to its sample preview).
- `Refresh()` — Force a repaint with the current data.
- `RefreshBinding() → Integer` — Bound to a COBOL table: reload the points from it (category field, value field), replacing the series; returns the point count. Also runs by itself as the form opens, after onLoad.

### Data flow
Three equivalent ways to feed the chart:
1. Inline methods: `Chart-1::AddPoint("Jan", 150).` / `Chart-1::Clear().` / `Chart-1::Refresh().`
2. Generated paragraphs: `PERFORM Chart-1-ADD-POINT` (after `MOVE`s to `WS-Chart-1-SELECTED-LBL` / `-SELECTED-VAL`), `PERFORM Chart-1-SET-TABLE`, `PERFORM Chart-1-CLEAR`, `PERFORM Chart-1-REFRESH`.
3. Built-in calls: `COBOL::"CHART-ADD-POINT"( "Chart-1" label value )` and `COBOL::"CHART-SET-TABLE"( "Chart-1" table count )` (table rows: `PIC X(64)` label + `PIC 9(18)V9(6)` value).
Or bind declaratively with the `DataSource`/`DataCount`/`LabelField`/`ValueFields` properties.

---

## Control: AreaChart

Filled area chart. Default size 320×220 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Title` (String, default (empty); free text) — Chart title — and, on a DataGrid, the caption centred on the band that carries the CSV button; empty means no caption. Also the window title on Form methods.
- `TitleFontSize` (Integer, default 0; points >= 0; 0 = follow the chart) — The title's own point size. **0** - the default - leaves it following the chart's own `FontSize`, which is how every chart drawn before this property behaves. Set it and the title is that size exactly, and the band reserved above the plot grows with it, so a large title takes room rather than printing over the plot it labels.
- `TitleColor` (String, default (empty); hex colour; empty = automatic) — The title's own colour. **Empty** - the default - keeps the automatic choice, which reads dark on a face that can carry it and switches to the readable pole when it cannot. That is why blank cannot simply mean grey: a fixed grey is invisible on a dark `Monochrome` face and near-invisible on a white one.
- `ShowLegend` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the series legend.
- `ShowGridLines` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the plot grid.
- `ShowXAxis` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the X axis line. Bar, Line, Area and Scatter charts only — a pie or a donut has no axes and does not carry it.
- `ShowYAxis` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the Y axis line. Bar, Line, Area and Scatter charts only — a pie or a donut has no axes and does not carry it.
- `ShowTooltips` (Boolean, default 1; `1` (true) or `0` (false)) — In the running form, the bar, point or slice under the pointer shows `label: value` in a tooltip. Live data only — the sample a chart shows before it has any is not data. On by default.
- `AnimateOnLoad` (Boolean, default 1; `1` (true) or `0` (false)) — The first time the running chart has data, every mark grows into place — bars and lines rise, a pie sweeps round — over `AnimationDuration` (250 ms at least). On by default. `AnimateValues` is the separate animation for data that CHANGES afterwards.
- `AnimateValues` (Boolean, default 0; `1` (true) or `0` (false)) — Animates a CHANGE OF DATA. With it on, a chart whose points are replaced TRAVELS from the values it is showing to the new ones instead of cutting to them; a point the new set added rises from zero, and one it dropped simply stops being drawn. The labels are the new set's from the first frame, so a half-played move never shows a point under the name it used to have. Off by default: a chart filled once, on load, should not spend two seconds arriving. The whole series moves together, so the chart settles in the same time with four points or forty. Set `AnimationDuration` to say how long.
- `AnimationDuration` (Integer, default 2000; milliseconds, 250 or more) — How long an `AnimateValues` move, and the `AnimateOnLoad` growth, take — for the WHOLE series rather than per point. Default 2000. Anything below 250 is raised to 250 — under that the eye reads a jump rather than a movement, and the property would be honoured in name only.
- `HideBackground` (Boolean, default 0; `1` (true) or `0` (false)) — Hides the fill/border while keeping the content visible.
- `Monochrome` (Boolean, default 0; `1` (true) or `0` (false)) — Tonal single-color rendering instead of the palette.
- `MonochromeColor` (String, default "#3F6FB5"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Base color for monochrome mode.
- `MonochromeGradient` (Boolean, default 0; `1` (true) or `0` (false)) — Diagonal light-to-dark shading in monochrome mode.
- `XAxisLabel` (String, default (empty); free text) — X axis caption.
- `YAxisLabel` (String, default (empty); free text) — Y axis caption.
- `SeriesColors` (String, default "#4C9BE8,#E87A4C,#4CE87A,#E84C9B,#9B4CE8,#E8C84C"; comma-separated colours, e.g. `#D01010,#10D010`) — The colours of the series — a pie's or donut's slices — in order, repeating when there are more series than colours. The seeded list means 'not chosen': the theme's palette paints the chart until you change it.
- `DataSource` (String, default (empty); COBOL table data-item name (charts / repeating GroupBox / DataGrid binding)) — Table the control binds to. On a chart, `SET-TABLE` reads each occurrence's label and value from the sub-fields named by `LabelField` and `ValueFields`; with no `LabelField` it reads the fixed `PIC X(64)` label + `PIC 9(18)V9(6)` value layout.
- `DataCount` (String, default (empty); COBOL data-item name) — Item holding the number of occupied table rows.
- `LabelField` (String, default (empty); sub-field name) — Charts: the table sub-field (an OCCURS item) whose value labels each point. With it and `ValueFields` set, `SET-TABLE` reads occurrences 1 to `DataCount` from these fields instead of the fixed layout.
- `ValueFields` (String, default (empty); comma-separated sub-field names) — Charts: the table sub-fields holding each point's values — the first is the first series, each further field one more series (a bar, line or area chart draws them all; pie, donut and scatter use the first). `SeriesLabels` names them in the legend; `Stacked` piles them up.
- `SeriesLabels` (String, default (empty); comma-separated display names) — The names the legend gives the series, in order; a series left unnamed shows as `Series n`. A pie's legend names its slices from the data instead.
- `Smooth` (Boolean, default 1; `1` (true) or `0` (false)) — Catmull-Rom smoothing of the polyline.
- `ShowPoints` (Boolean, default 1; `1` (true) or `0` (false)) — Draws a marker on every point of a Line or Area chart.
- `PointRadius` (Integer, default 4; pixels > 0) — The marker radius on a Line or Area chart, and a Scatter point's radius when it has no bubble sizes.
- `FillAlpha` (Integer, default 40; 0-100 (percent)) — Area fill opacity.
- `Stacked` (Boolean, default 0; `1` (true) or `0` (false)) — BarChart / AreaChart (default false): with several series, pile each series on the ones before it — one bar per label made of coloured segments, or area bands laid one over the other — so a label's marks add up to its total, and the plot scales to the largest total. Off, the series stand side by side (bars) or overlap from the axis (areas). With one series it changes nothing. Series come from `AddPoint(label, v1, v2, …)` or from every field named in `ValueFields`.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `BorderColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderGradientEnabled` (Boolean, default 0; `1` (true) or `0` (false)) — Charts only. Draws the frame border as a two-colour gradient from `BorderGradientStartColor` to `BorderGradientEndColor` along `BorderGradientDirection`, `BorderWidth` wide, following `CornerRadius`. Off, the border is `BorderStyle` in `BorderColor`. Charts honour `BorderStyle`, `BorderWidth` and `BorderColor` like every other control (they used to draw a fixed 1 px line); `BorderStyle` `None` removes the border, gradient and blur alike.
- `BorderGradientStartColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient starts.
- `BorderGradientEndColor` (String, default "#8FB4FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient ends.
- `BorderGradientDirection` (String, default "South"; one of: `North` | `NorthEast` | `East` | `SouthEast` | `South` | `SouthWest` | `West` | `NorthWest`) — Charts only. Direction the border gradient flows toward.
- `BorderBlur` (Integer, default 0; `0`–`40` (pixels)) — Charts only. A soft glow outward from the frame border, this many pixels wide, in the border's colour (the gradient's mid colour when the gradient is on) — the shadow stack's own falloff, faintest outermost. `0` (the default) draws none. Drawn at the chart's inherited alpha, never at its own `Transparency`, so it stays with the border on a see-through chart.
- `BorderTransparency` (Integer, default 0; `0`–`100`) — Charts only. The frame border's own transparency: `0` (the default) opaque, `100` invisible. It fades the border line, the gradient ring and the blur rings together — separately from the chart's `Transparency`, which reaches only the face — so a frame can fade independently of what it frames.
- `CornerRadius` (Integer, default 8; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onDataChanged` — the chart's data-bearing properties changed
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddPoint(label: String, value: Number)` — Append one data point and repaint. On a BarChart, LineChart or AreaChart each further argument — `AddPoint(label, v1, v2, …)` — is the point's value in the next series (a point given fewer is 0 in the rest); on a ScatterChart the third argument is the bubble's size.
- `Clear()` — Remove all pushed data (chart falls back to its sample preview).
- `Refresh()` — Force a repaint with the current data.
- `RefreshBinding() → Integer` — Bound to a COBOL table: reload the points from it (category field, value field), replacing the series; returns the point count. Also runs by itself as the form opens, after onLoad.

### Data flow
Three equivalent ways to feed the chart:
1. Inline methods: `Chart-1::AddPoint("Jan", 150).` / `Chart-1::Clear().` / `Chart-1::Refresh().`
2. Generated paragraphs: `PERFORM Chart-1-ADD-POINT` (after `MOVE`s to `WS-Chart-1-SELECTED-LBL` / `-SELECTED-VAL`), `PERFORM Chart-1-SET-TABLE`, `PERFORM Chart-1-CLEAR`, `PERFORM Chart-1-REFRESH`.
3. Built-in calls: `COBOL::"CHART-ADD-POINT"( "Chart-1" label value )` and `COBOL::"CHART-SET-TABLE"( "Chart-1" table count )` (table rows: `PIC X(64)` label + `PIC 9(18)V9(6)` value).
Or bind declaratively with the `DataSource`/`DataCount`/`LabelField`/`ValueFields` properties.

---

## Control: ScatterChart

Scatter/bubble chart. Default size 320×220 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Title` (String, default (empty); free text) — Chart title — and, on a DataGrid, the caption centred on the band that carries the CSV button; empty means no caption. Also the window title on Form methods.
- `TitleFontSize` (Integer, default 0; points >= 0; 0 = follow the chart) — The title's own point size. **0** - the default - leaves it following the chart's own `FontSize`, which is how every chart drawn before this property behaves. Set it and the title is that size exactly, and the band reserved above the plot grows with it, so a large title takes room rather than printing over the plot it labels.
- `TitleColor` (String, default (empty); hex colour; empty = automatic) — The title's own colour. **Empty** - the default - keeps the automatic choice, which reads dark on a face that can carry it and switches to the readable pole when it cannot. That is why blank cannot simply mean grey: a fixed grey is invisible on a dark `Monochrome` face and near-invisible on a white one.
- `ShowLegend` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the series legend.
- `ShowGridLines` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the plot grid.
- `ShowXAxis` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the X axis line. Bar, Line, Area and Scatter charts only — a pie or a donut has no axes and does not carry it.
- `ShowYAxis` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the Y axis line. Bar, Line, Area and Scatter charts only — a pie or a donut has no axes and does not carry it.
- `ShowTooltips` (Boolean, default 1; `1` (true) or `0` (false)) — In the running form, the bar, point or slice under the pointer shows `label: value` in a tooltip. Live data only — the sample a chart shows before it has any is not data. On by default.
- `AnimateOnLoad` (Boolean, default 1; `1` (true) or `0` (false)) — The first time the running chart has data, every mark grows into place — bars and lines rise, a pie sweeps round — over `AnimationDuration` (250 ms at least). On by default. `AnimateValues` is the separate animation for data that CHANGES afterwards.
- `AnimateValues` (Boolean, default 0; `1` (true) or `0` (false)) — Animates a CHANGE OF DATA. With it on, a chart whose points are replaced TRAVELS from the values it is showing to the new ones instead of cutting to them; a point the new set added rises from zero, and one it dropped simply stops being drawn. The labels are the new set's from the first frame, so a half-played move never shows a point under the name it used to have. Off by default: a chart filled once, on load, should not spend two seconds arriving. The whole series moves together, so the chart settles in the same time with four points or forty. Set `AnimationDuration` to say how long.
- `AnimationDuration` (Integer, default 2000; milliseconds, 250 or more) — How long an `AnimateValues` move, and the `AnimateOnLoad` growth, take — for the WHOLE series rather than per point. Default 2000. Anything below 250 is raised to 250 — under that the eye reads a jump rather than a movement, and the property would be honoured in name only.
- `HideBackground` (Boolean, default 0; `1` (true) or `0` (false)) — Hides the fill/border while keeping the content visible.
- `Monochrome` (Boolean, default 0; `1` (true) or `0` (false)) — Tonal single-color rendering instead of the palette.
- `MonochromeColor` (String, default "#3F6FB5"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Base color for monochrome mode.
- `MonochromeGradient` (Boolean, default 0; `1` (true) or `0` (false)) — Diagonal light-to-dark shading in monochrome mode.
- `XAxisLabel` (String, default (empty); free text) — X axis caption.
- `YAxisLabel` (String, default (empty); free text) — Y axis caption.
- `SeriesColors` (String, default "#4C9BE8,#E87A4C,#4CE87A,#E84C9B,#9B4CE8,#E8C84C"; comma-separated colours, e.g. `#D01010,#10D010`) — The colours of the series — a pie's or donut's slices — in order, repeating when there are more series than colours. The seeded list means 'not chosen': the theme's palette paints the chart until you change it.
- `DataSource` (String, default (empty); COBOL table data-item name (charts / repeating GroupBox / DataGrid binding)) — Table the control binds to. On a chart, `SET-TABLE` reads each occurrence's label and value from the sub-fields named by `LabelField` and `ValueFields`; with no `LabelField` it reads the fixed `PIC X(64)` label + `PIC 9(18)V9(6)` value layout.
- `DataCount` (String, default (empty); COBOL data-item name) — Item holding the number of occupied table rows.
- `LabelField` (String, default (empty); sub-field name) — Charts: the table sub-field (an OCCURS item) whose value labels each point. With it and `ValueFields` set, `SET-TABLE` reads occurrences 1 to `DataCount` from these fields instead of the fixed layout.
- `ValueFields` (String, default (empty); comma-separated sub-field names) — Charts: the table sub-fields holding each point's values — the first is the first series, each further field one more series (a bar, line or area chart draws them all; pie, donut and scatter use the first). `SeriesLabels` names them in the legend; `Stacked` piles them up.
- `SeriesLabels` (String, default (empty); comma-separated display names) — The names the legend gives the series, in order; a series left unnamed shows as `Series n`. A pie's legend names its slices from the data instead.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `BorderColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderGradientEnabled` (Boolean, default 0; `1` (true) or `0` (false)) — Charts only. Draws the frame border as a two-colour gradient from `BorderGradientStartColor` to `BorderGradientEndColor` along `BorderGradientDirection`, `BorderWidth` wide, following `CornerRadius`. Off, the border is `BorderStyle` in `BorderColor`. Charts honour `BorderStyle`, `BorderWidth` and `BorderColor` like every other control (they used to draw a fixed 1 px line); `BorderStyle` `None` removes the border, gradient and blur alike.
- `BorderGradientStartColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient starts.
- `BorderGradientEndColor` (String, default "#8FB4FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient ends.
- `BorderGradientDirection` (String, default "South"; one of: `North` | `NorthEast` | `East` | `SouthEast` | `South` | `SouthWest` | `West` | `NorthWest`) — Charts only. Direction the border gradient flows toward.
- `BorderBlur` (Integer, default 0; `0`–`40` (pixels)) — Charts only. A soft glow outward from the frame border, this many pixels wide, in the border's colour (the gradient's mid colour when the gradient is on) — the shadow stack's own falloff, faintest outermost. `0` (the default) draws none. Drawn at the chart's inherited alpha, never at its own `Transparency`, so it stays with the border on a see-through chart.
- `BorderTransparency` (Integer, default 0; `0`–`100`) — Charts only. The frame border's own transparency: `0` (the default) opaque, `100` invisible. It fades the border line, the gradient ring and the blur rings together — separately from the chart's `Transparency`, which reaches only the face — so a frame can fade independently of what it frames.
- `BubbleField` (String, default (empty); sub-field name or empty) — ScatterChart: the table sub-field whose value sizes each bubble, read by `SET-TABLE` with `LabelField`/`ValueFields`. `AddPoint(label, value, size)` gives a size one point at a time.
- `BubbleScale` (Integer, default 20; pixels > 0 (default 20)) — ScatterChart: the radius of the LARGEST bubble; the others are sized in proportion (2 pixels at least). Without bubble sizes every point is a `PointRadius` marker.
- `CornerRadius` (Integer, default 8; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onDataChanged` — the chart's data-bearing properties changed
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddPoint(label: String, value: Number)` — Append one data point and repaint. On a BarChart, LineChart or AreaChart each further argument — `AddPoint(label, v1, v2, …)` — is the point's value in the next series (a point given fewer is 0 in the rest); on a ScatterChart the third argument is the bubble's size.
- `Clear()` — Remove all pushed data (chart falls back to its sample preview).
- `Refresh()` — Force a repaint with the current data.
- `RefreshBinding() → Integer` — Bound to a COBOL table: reload the points from it (category field, value field), replacing the series; returns the point count. Also runs by itself as the form opens, after onLoad.

### Data flow
Three equivalent ways to feed the chart:
1. Inline methods: `Chart-1::AddPoint("Jan", 150).` / `Chart-1::Clear().` / `Chart-1::Refresh().`
2. Generated paragraphs: `PERFORM Chart-1-ADD-POINT` (after `MOVE`s to `WS-Chart-1-SELECTED-LBL` / `-SELECTED-VAL`), `PERFORM Chart-1-SET-TABLE`, `PERFORM Chart-1-CLEAR`, `PERFORM Chart-1-REFRESH`.
3. Built-in calls: `COBOL::"CHART-ADD-POINT"( "Chart-1" label value )` and `COBOL::"CHART-SET-TABLE"( "Chart-1" table count )` (table rows: `PIC X(64)` label + `PIC 9(18)V9(6)` value).
Or bind declaratively with the `DataSource`/`DataCount`/`LabelField`/`ValueFields` properties.

---

## Control: DonutChart

Donut chart. Default size 240×240 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Title` (String, default (empty); free text) — Chart title — and, on a DataGrid, the caption centred on the band that carries the CSV button; empty means no caption. Also the window title on Form methods.
- `TitleFontSize` (Integer, default 0; points >= 0; 0 = follow the chart) — The title's own point size. **0** - the default - leaves it following the chart's own `FontSize`, which is how every chart drawn before this property behaves. Set it and the title is that size exactly, and the band reserved above the plot grows with it, so a large title takes room rather than printing over the plot it labels.
- `TitleColor` (String, default (empty); hex colour; empty = automatic) — The title's own colour. **Empty** - the default - keeps the automatic choice, which reads dark on a face that can carry it and switches to the readable pole when it cannot. That is why blank cannot simply mean grey: a fixed grey is invisible on a dark `Monochrome` face and near-invisible on a white one.
- `ShowLegend` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the series legend.
- `ShowGridLines` (Boolean, default 1; `1` (true) or `0` (false)) — Shows the plot grid.
- `ShowTooltips` (Boolean, default 1; `1` (true) or `0` (false)) — In the running form, the bar, point or slice under the pointer shows `label: value` in a tooltip. Live data only — the sample a chart shows before it has any is not data. On by default.
- `AnimateOnLoad` (Boolean, default 1; `1` (true) or `0` (false)) — The first time the running chart has data, every mark grows into place — bars and lines rise, a pie sweeps round — over `AnimationDuration` (250 ms at least). On by default. `AnimateValues` is the separate animation for data that CHANGES afterwards.
- `AnimateValues` (Boolean, default 0; `1` (true) or `0` (false)) — Animates a CHANGE OF DATA. With it on, a chart whose points are replaced TRAVELS from the values it is showing to the new ones instead of cutting to them; a point the new set added rises from zero, and one it dropped simply stops being drawn. The labels are the new set's from the first frame, so a half-played move never shows a point under the name it used to have. Off by default: a chart filled once, on load, should not spend two seconds arriving. The whole series moves together, so the chart settles in the same time with four points or forty. Set `AnimationDuration` to say how long.
- `AnimationDuration` (Integer, default 2000; milliseconds, 250 or more) — How long an `AnimateValues` move, and the `AnimateOnLoad` growth, take — for the WHOLE series rather than per point. Default 2000. Anything below 250 is raised to 250 — under that the eye reads a jump rather than a movement, and the property would be honoured in name only.
- `HideBackground` (Boolean, default 0; `1` (true) or `0` (false)) — Hides the fill/border while keeping the content visible.
- `Monochrome` (Boolean, default 0; `1` (true) or `0` (false)) — Tonal single-color rendering instead of the palette.
- `MonochromeColor` (String, default "#3F6FB5"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Base color for monochrome mode.
- `MonochromeGradient` (Boolean, default 0; `1` (true) or `0` (false)) — Diagonal light-to-dark shading in monochrome mode.
- `XAxisLabel` (String, default (empty); free text) — X axis caption.
- `YAxisLabel` (String, default (empty); free text) — Y axis caption.
- `SeriesColors` (String, default "#4C9BE8,#E87A4C,#4CE87A,#E84C9B,#9B4CE8,#E8C84C"; comma-separated colours, e.g. `#D01010,#10D010`) — The colours of the series — a pie's or donut's slices — in order, repeating when there are more series than colours. The seeded list means 'not chosen': the theme's palette paints the chart until you change it.
- `DataSource` (String, default (empty); COBOL table data-item name (charts / repeating GroupBox / DataGrid binding)) — Table the control binds to. On a chart, `SET-TABLE` reads each occurrence's label and value from the sub-fields named by `LabelField` and `ValueFields`; with no `LabelField` it reads the fixed `PIC X(64)` label + `PIC 9(18)V9(6)` value layout.
- `DataCount` (String, default (empty); COBOL data-item name) — Item holding the number of occupied table rows.
- `LabelField` (String, default (empty); sub-field name) — Charts: the table sub-field (an OCCURS item) whose value labels each point. With it and `ValueFields` set, `SET-TABLE` reads occurrences 1 to `DataCount` from these fields instead of the fixed layout.
- `ValueFields` (String, default (empty); comma-separated sub-field names) — Charts: the table sub-fields holding each point's values — the first is the first series, each further field one more series (a bar, line or area chart draws them all; pie, donut and scatter use the first). `SeriesLabels` names them in the legend; `Stacked` piles them up.
- `SeriesLabels` (String, default (empty); comma-separated display names) — The names the legend gives the series, in order; a series left unnamed shows as `Series n`. A pie's legend names its slices from the data instead.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `BorderColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderGradientEnabled` (Boolean, default 0; `1` (true) or `0` (false)) — Charts only. Draws the frame border as a two-colour gradient from `BorderGradientStartColor` to `BorderGradientEndColor` along `BorderGradientDirection`, `BorderWidth` wide, following `CornerRadius`. Off, the border is `BorderStyle` in `BorderColor`. Charts honour `BorderStyle`, `BorderWidth` and `BorderColor` like every other control (they used to draw a fixed 1 px line); `BorderStyle` `None` removes the border, gradient and blur alike.
- `BorderGradientStartColor` (String, default "#3C50A0"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient starts.
- `BorderGradientEndColor` (String, default "#8FB4FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Charts only. Where the border gradient ends.
- `BorderGradientDirection` (String, default "South"; one of: `North` | `NorthEast` | `East` | `SouthEast` | `South` | `SouthWest` | `West` | `NorthWest`) — Charts only. Direction the border gradient flows toward.
- `BorderBlur` (Integer, default 0; `0`–`40` (pixels)) — Charts only. A soft glow outward from the frame border, this many pixels wide, in the border's colour (the gradient's mid colour when the gradient is on) — the shadow stack's own falloff, faintest outermost. `0` (the default) draws none. Drawn at the chart's inherited alpha, never at its own `Transparency`, so it stays with the border on a see-through chart.
- `BorderTransparency` (Integer, default 0; `0`–`100`) — Charts only. The frame border's own transparency: `0` (the default) opaque, `100` invisible. It fades the border line, the gradient ring and the blur rings together — separately from the chart's `Transparency`, which reaches only the face — so a frame can fade independently of what it frames.
- `ShowLabels` (Boolean, default 1; `1` (true) or `0` (false)) — Draws slice labels.
- `LabelFormat` (String, default "percent"; one of: `percent` | `value` | `label`) — What pie/donut slice labels show.
- `InnerRadius` (Integer, default 40; 0-100 (% of outer radius)) — Donut hole size.
- `CornerRadius` (Integer, default 8; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onDataChanged` — the chart's data-bearing properties changed
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `AddPoint(label: String, value: Number)` — Append one data point and repaint. On a BarChart, LineChart or AreaChart each further argument — `AddPoint(label, v1, v2, …)` — is the point's value in the next series (a point given fewer is 0 in the rest); on a ScatterChart the third argument is the bubble's size.
- `Clear()` — Remove all pushed data (chart falls back to its sample preview).
- `Refresh()` — Force a repaint with the current data.
- `RefreshBinding() → Integer` — Bound to a COBOL table: reload the points from it (category field, value field), replacing the series; returns the point count. Also runs by itself as the form opens, after onLoad.

### Data flow
Three equivalent ways to feed the chart:
1. Inline methods: `Chart-1::AddPoint("Jan", 150).` / `Chart-1::Clear().` / `Chart-1::Refresh().`
2. Generated paragraphs: `PERFORM Chart-1-ADD-POINT` (after `MOVE`s to `WS-Chart-1-SELECTED-LBL` / `-SELECTED-VAL`), `PERFORM Chart-1-SET-TABLE`, `PERFORM Chart-1-CLEAR`, `PERFORM Chart-1-REFRESH`.
3. Built-in calls: `COBOL::"CHART-ADD-POINT"( "Chart-1" label value )` and `COBOL::"CHART-SET-TABLE"( "Chart-1" table count )` (table rows: `PIC X(64)` label + `PIC 9(18)V9(6)` value).
Or bind declaratively with the `DataSource`/`DataCount`/`LabelField`/`ValueFields` properties.

---

## Control: Knob

Rotary dial that sets a numeric Value within Minimum..Maximum by dragging. Default size 80×96 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Minimum` (Integer, default 0; integer ≤ Maximum) — Lower bound of the value range.
- `Maximum` (Integer, default 100; integer ≥ Minimum) — Upper bound of the value range.
- `Value` (Integer, default 0; ProgressBar/Slider/NumericUpDown: integer within Minimum..Maximum; DateTimePicker: `YYYY-MM-DD`, `HH:MM`, or `YYYY-MM-DD HH:MM`) — Current value. A DateTimePicker always stores ISO, whatever Format displays, so a COBOL handler reading Value gets one shape; which halves are present follows Format (a Time picker stores the time alone).
- `Step` (Integer, default 1; number > 0) — How far one step moves the value: the ↑/↓ keys on a NumericUpDown, ←/→/↑/↓ on a Slider, the wheel, and the Increment()/Decrement() methods — which stop at Minimum and Maximum. A fractional Step (0.5) steps in fractions.
- `Accent` (String, default "Blue"; hex color string, or one of: `Blue` | `Green` | `Red` | `Purple` | `Amber` | `Sky`) — Accent color: the Knob's arc and indicator, the Switch's ON track. BOTH take ANY color from the designer's picker (which carries the colour memory); the six names still resolve for forms that stored one, and an unrecognised value falls back to `Blue`. On a SWITCH the inspector calls this row **Checked color**, because that is what it colours there — `Accent` is only the stored key, and the row offered six fixed names until 1.61.152. There is no Size property — the Knob's dial is drawn at whatever size the control was given.
- `FaceColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Knob dial face — the round body the indicator turns over. Empty (the default) leaves it to the theme. The rim's own fill is this colour lightened, so a face colour carries the whole dial.
- `RimColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Knob rim and inner ring — the two outlines around the dial face. Empty (the default) leaves them to the theme.
- `TrackColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The part still to travel: a Slider's rail from Value to Maximum, a Knob's arc from Value round to Maximum. Outranks the Appearance BackgroundColor; left at its default the active theme paints.
- `Bipolar` (Boolean, default 0; `1` (true) or `0` (false)) — Knob fill grows from the center (both directions) instead of from Minimum.
- `ShowValue` (Boolean, default 1; `1` (true) or `0` (false)) — Draws the numeric value on the control.
- `DefaultValue` (Integer, default 0; integer within Minimum..Maximum) — Value a double-click/reset returns the Knob to.
- `Label` (String, default (empty); free text or empty) — Caption drawn under the Knob.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Supported Events
- `onChange` — value or text changed
- `onValueChanged` — value changed (the new value is delivered)
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetValue(value: Integer)` — Set the current value.
- `GetValue() → Integer` — Read the current value.
- `Increment()` — Add Step to Value.
- `Decrement()` — Subtract Step from Value.
- `Reset()` — Return Value to Minimum.

---

## Control: Gauge

Read-only KPI display (Radial | Linear | Donut) — never changed by user interaction. With BOTH `WarningThreshold` and `CriticalThreshold` set, the fill KEEPS EACH ZONE'S COLOUR ALONG ITS OWN STRETCH (1.61.172): green up to the warning mark, amber from there to the critical one, red beyond — so a gauge reading 88 against marks at 70/90 is green to 70 and amber from 70 to 88, with no red at all. Before that the whole fill took the current zone's colour, which read as 'all red' the moment the needle crossed a mark. The NEEDLE (and a Linear's thumb) still takes the colour of the zone the reading is IN, so the current state is still legible at a glance. Thresholds are FRACTIONS of the Minimum..Maximum span (0.0-1.0), not readings on it. Default size 140×90 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `GaugeStyle` (String, default "Radial"; one of: `Radial` | `Linear` | `Donut`) — Which meter the Gauge draws: a half-circle speedometer, a horizontal bar, or a full ring.
- `Minimum` (Integer, default 0; integer ≤ Maximum) — Lower bound of the value range.
- `Maximum` (Integer, default 100; integer ≥ Minimum) — Upper bound of the value range.
- `Value` (Integer, default 0; ProgressBar/Slider/NumericUpDown: integer within Minimum..Maximum; DateTimePicker: `YYYY-MM-DD`, `HH:MM`, or `YYYY-MM-DD HH:MM`) — Current value. A DateTimePicker always stores ISO, whatever Format displays, so a COBOL handler reading Value gets one shape; which halves are present follows Format (a Time picker stores the time alone).
- `Color` (String, default (empty); hex color string or empty) — Gauge fill color; empty uses the control's ForegroundColor, else the active theme's accent. Ignored while zone coloring is on (see WarningThreshold), and it never paints the needle — that is NeedleColor's alone. No longer offered in the designer's property pane: colour the meter with ForegroundColor and the track with BackgroundColor, like every other control. A value already stored in a form still paints that gauge's meter.
- `WarningThreshold` (String, default (empty); fraction of Minimum..Maximum, `0.0`-`1.0`, or empty) — Where the Gauge's fill turns amber. Empty = zone coloring off; both this and CriticalThreshold must be set together, and while they are the zone owns the fill color: green below WarningThreshold, amber from it, red from CriticalThreshold.
- `CriticalThreshold` (String, default (empty); fraction of Minimum..Maximum, `0.0`-`1.0`, or empty) — Where the Gauge's fill turns red (see WarningThreshold).
- `NormalColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The Gauge zone BELOW the warning mark. Empty = the built-in `#2E7D32` green. Only used while BOTH thresholds are set — that is what turns zones on.
- `WarningColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The Gauge zone from the warning mark to the critical one. Empty = the built-in `#F57C00` amber.
- `CriticalColor` (String, default (empty); hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — The Gauge zone from the critical mark up. Empty = the built-in `#C62828` red.
- `Unit` (String, default (empty); free text or empty, e.g. `"%"`, `"rpm"`) — Suffix after the Gauge's numeric readout, in every style. A unit that starts with a letter or digit is spaced off the number (`"Parts"` reads `23 Parts`); a symbol is not (`"%"` reads `23%`, `"°C"` reads `19°C`). Leading spaces you type are kept as typed.
- `Text` (String, default (empty); free text) — Current text content.
- `ShowNeedle` (Boolean, default 1; `1` (true) or `0` (false)) — Draws the Gauge's needle (Radial and Donut styles).
- `ShowScale` (Boolean, default 1; `1` (true) or `0` (false)) — Draws the Radial Gauge's tick scale.
- `NeedleColor` (String, default (empty); hex color string or empty) — Colour of the Gauge's needle and its hub (Radial and Donut), and the ONLY property that paints them. Empty = the control's own ForegroundColor, else the theme accent — never the meter's Color, which used to reach the needle as well and gave one needle two owners. The meter's band is unaffected: this is the needle's colour alone.
- `ReadoutPosition` (String, default "Up"; one of: `Up` | `Down`) — Where a Radial Gauge prints its value + Unit: `Up` inside the dial, above the needle's pivot (the default), or `Down` 5 px below the pivot, where a speedometer prints its number. On `Down` the dial gives up that much height so the reading stays inside the control. Radial only — a Donut reads out in its hole and a Linear under its bar.
- `BarHeight` (Integer, default 14; pixels > 0) — Linear Gauge bar thickness.
- `ShowThumb` (Boolean, default 1; `1` (true) or `0` (false)) — Draws the Linear Gauge's end-of-fill thumb marker.
- `StrokeWidth` (Integer, default 8; pixels > 0) — Donut Gauge ring thickness.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Supported Events
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `SetValue(value: Integer)` — Set the current value (Gauge is read-only via the UI, R10 — this is the only way to change it).
- `GetValue() → Integer` — Read the current value.

---

## Control: Switch

Boolean on/off visual toggle. Default size 52×28 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Checked` (Boolean, default 0; `1` (true) or `0` (false)) — Checked state of a CheckBox or a Switch. A **RadioButton** uses `Selected` instead — a radio is selected, not checked. `Checked` is still accepted on a radio so programs written before the rename keep working, and it resolves to `Selected` on the way in.
- `Accent` (String, default "Blue"; hex color string, or one of: `Blue` | `Green` | `Red` | `Purple` | `Amber` | `Sky`) — Accent color: the Knob's arc and indicator, the Switch's ON track. BOTH take ANY color from the designer's picker (which carries the colour memory); the six names still resolve for forms that stored one, and an unrecognised value falls back to `Blue`. On a SWITCH the inspector calls this row **Checked color**, because that is what it colours there — `Accent` is only the stored key, and the row offered six fixed names until 1.61.152. There is no Size property — the Knob's dial is drawn at whatever size the control was given.
- `BorderStyle` (String, default "None"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderColor` (String, default "#888888"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.

### Supported Events
- `onCheck` — the toggle went ON (fires only in that direction)
- `onUncheck` — the toggle went OFF (fires only in that direction)
- `onCheckedChanged` — checked state flipped, either way (carries the new state)
- `onValueChanged` — value changed (the new value is delivered)
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `IsChecked() → Boolean (0/1)` — Read the checked state.
- `SetChecked(value: Boolean)` — Set the checked state (`1`/`0`, also accepts true/false/yes/on).
- `Toggle()` — Flip the checked state.

---

## Control: FileDropZone

Non-visual: accepts files via drag-and-drop or a native file-picker click. Default size 220×100 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Hint` (String, default (empty); free text) — Placeholder text shown inside the empty drop zone.
- `AllowedExtensions` (String, default (empty); comma/space separated extensions, e.g. `csv, xlsx` (blank accepts any file)) — What the zone takes. Case-blind, with or without the dot; a file whose extension is not listed is refused.
- `MaximumFileSizeKB` (Integer, default 0; KB, 0 = no limit) — Largest file the zone takes. A bigger file is refused rather than reported.
- `DestinationFolder` (String, default (empty); local folder path (blank leaves files where they are)) — Accepted files are copied here, the folder being created if needed; an existing name is never overwritten (`report.csv` becomes `report (2).csv`). A RELATIVE path starts at the application's folder — the same place a KnowledgeBase `Location` starts — not at the directory the program was launched from, so `assets/KB/<collection>/documents` puts a file where that collection reads it.
- `StageOnly` (Boolean, default 0; `1` (true) or `0` (false)) — Off (the default): a drop copies into DestinationFolder then and there. On: a drop copies NOTHING — the files are held, listed for review in FileListControl with a tick box each, and your COBOL calls `CommitFiles()` to do the copying once the operator is happy. Use it whenever the operator should be able to change their mind before anything is written.
- `FileListControl` (String, default (empty); the id of a ListBox on the same form (blank = no list)) — Where a staged drop shows what it is holding: one tick-boxed row per file, reading `<path> (12.345 MB)`. Unticking a row leaves that file out of the next `CommitFiles()` without removing it from the list, so the operator can see what they excluded and put it back. Dropping a FileDropZone in the designer creates this ListBox next to it, at the zone's own size, and names it here — it is an ordinary ListBox from then on, and naming a control that no longer exists simply means no list.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `DroppedFiles` — The files THIS drop accepted — one absolute path per line, the copy in DestinationFolder when one is set, the original when the zone is staging. Not the basket: a staging zone's running total is StagedFiles.
- `RejectedFiles` — Files the zone turned away, each with `extension` or `too-big` after a TAB.
- `StagedFiles` — What a StageOnly zone is holding, in list order, at their ORIGINAL paths — nothing has been copied yet. A second drop adds to this rather than replacing it, and the same file dropped twice is held once.
- `CommitSummary` — One line about the intake, for a Label or a DISPLAY: `3 files staged, 24.310 MB` before the form goes ahead, `7 of 8 copied, 24.310 MB` after `CommitFiles()`. Megabytes count 1,000,000 bytes, so a size here matches the one the operator's own file browser shows.

### Supported Events
- `onFilesDropped` — one or more files were dropped or picked (read `DroppedFiles`)
- `onFilesRejected`
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseEnter`, `onMouseLeave`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `CommitFiles() → String` — Copy the files a staged drop is holding into DestinationFolder, and return the summary (`7 of 8 copied, 24.310 MB`). Only meaningful with StageOnly on: call it when the operator has finished reviewing the list. Files whose row was unticked are skipped and stay listed. Afterwards DroppedFiles is the included files at their new paths, each row carries `✓` and its new path or `✗` and the reason, and CommitSummary is the returned line.

### Usage — a UI gesture in, one method out
There is no method to open the picker or read a drop programmatically. The user drags a file onto the control OR clicks it to open the native file picker; either way the platform runs the zone's intake and fires an event. Read the paths with `MOVE FDZ-1::DroppedFiles TO WS-PATHS`. The one method the zone has is `CommitFiles()`, which belongs to the staged flow below.

### What the zone accepts, and where it puts it
Three design-time properties decide, and both routes in (drop and picker) obey them:

- `AllowedExtensions` — `csv, xlsx`. Case-blind, dots optional. Blank accepts any file.
- `MaximumFileSizeKB` — largest file taken, in KB. `0` is no limit.
- `DestinationFolder` — accepted files are COPIED here (the folder is created if missing). An existing name is never overwritten: `report.csv` lands as `report (2).csv`. Blank leaves files where they are. A relative path starts at the application's folder, as a KnowledgeBase `Location` does.

Accepted files appear in `DroppedFiles` — at their NEW path when a destination is set — and fire `onFilesDropped`. Refused files appear in `RejectedFiles`, one `path<TAB>reason` per line where reason is `extension` or `too-big`, and fire `onFilesRejected`. A drop of ten files where three are refused fires BOTH events. Nothing is refused silently.

### Letting the operator confirm first (`StageOnly`)
By default the copy happens the instant the file lands, which gives the operator no chance to change their mind. Turn `StageOnly` on and a drop copies **nothing**:

1. The drop is judged as usual — refused files still fire `onFilesRejected` — and the accepted ones are HELD at their original paths in `StagedFiles`. `onFilesDropped` fires, and `DroppedFiles` holds what THAT drop brought — at its original path, since nothing was copied — while `StagedFiles` is everything held so far. Read `DroppedFiles` when your handler wants the file just dropped, such as opening one document into a Viewer; read `StagedFiles` when you want the basket. `DestinationFolder` is not even created.
2. They are listed in the ListBox named by `FileListControl`, one tick-boxed row each, reading `<path> (12.345 MB)`. `CommitSummary` reads `3 files staged, 24.310 MB`.
3. The operator unticks anything they did not mean to send. An unticked row stays in the list, so the exclusion is visible and reversible.
4. Your own COBOL decides when the form goes ahead — a Submit button, a validated field, whatever the form means by confirmation — and calls `INVOKE FDZ-1 'CommitFiles'`. Ticked files are copied by exactly the rules above; unticked ones are skipped.
5. Each row becomes `✓ <new path> (12.345 MB)` or `✗ <path> (12.345 MB) — <reason>`, `CommitSummary` becomes `7 of 8 copied, 24.310 MB` (also the method's return value), and `DroppedFiles` becomes the included files at their new paths — at their original path for any whose copy failed, because the form must still get the file it was given.

```cobol
SUBMIT-BUTTON--ONCLICK.
MOVE FDZ-1::CommitFiles() TO WS-SUMMARY
MOVE WS-SUMMARY TO STATUS-LABEL::Caption
MOVE FDZ-1::DroppedFiles TO WS-PATHS
PERFORM SEND-TO-APPLICATION.
```

A second drop adds to what is already staged rather than replacing it, and the same file dropped twice is held once. Calling `CommitFiles()` on a zone holding nothing is not an error — it reports `0 of 0 copied`. Megabytes count 1,000,000 bytes, matching the operator's own file browser.

---

## Control: Maps

Embedded, pannable/zoomable OpenStreetMap view with optional google_maps-backed location data (Directions/Geocoding/Places/Distance-Matrix). Wheel zoom is continuous: one notch is one level, released a slice per frame, and while it travels the map is drawn BETWEEN levels by scaling the tiles it already has, with the point under the pointer held fixed and markers/routes/regions scaling along with the basemap. Default size 320×240 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `CenterLat` (String, default "0"; decimal degrees as a string, e.g. `"48.8566"`) — Map center latitude. Where the map OPENS is set in the properties pane (`Start latitude`), and a pan or a COBOL write updates it.
- `CenterLng` (String, default "0"; decimal degrees as a string, e.g. `"2.3522"`) — Map center longitude. Set in the properties pane as `Start longitude`; a pan or a COBOL write updates it.
- `Zoom` (Integer, default 2; integer, typically 0-20) — OpenStreetMap zoom level (higher = closer). Always a WHOLE level — the one whose tiles are fetched, and the value a handler reads, writes and receives on onBoundsChanged. Wheel zooming glides between levels by drawing the map at a fractional scale, but that fraction is view state and is never published, so this property never carries one.
- `Markers` (String, default (empty); one marker per line, TAB-separated: `id\tlat\tlng\tlabel\tinfo`) — Pins drawn on the map. Prefer the AddMarker/RemoveMarker methods over hand-formatting this.
- `Routes` (String, default (empty); one route per line, TAB-separated: `id\tcolour\twidth\tgeometry`) — Lines traced over the basemap — a planned delivery run, a driven route. `colour` is `#RRGGBB` (empty = the default blue), `width` is pixels (0 = default). `geometry` is either an encoded polyline — exactly what the sixth field of a Directions ResponseBody carries, so a route can be traced straight from the answer — or an explicit `lat,lng;lat,lng;…` list for geometry the program worked out itself. Prefer AddRoute/RemoveRoute/ClearRoutes over hand-formatting this. Needs no API key: the geometry comes from your program.
- `Regions` (String, default (empty); one region per line, TAB-separated: `id\tfill\tstroke\twidth\tgeometry\tlabel\tinfo`) — Filled areas over the basemap — sales territories, delivery zones, coverage. `fill` is `#RRGGBB` or `#RRGGBBAA` (give it an alpha so the streets stay readable underneath); `stroke` is the outline, empty for none. `geometry` is the same as Routes and the ring is closed for you. A region MAY be concave — the fill is triangulated rather than assumed convex, so a territory that follows a coastline or a border fills correctly. Prefer AddRegion/RemoveRegion/ClearRegions. Needs no API key.
- `InfoBackgroundColor` (String, default (empty); `#RRGGBB`, or empty to follow the form) — Background of the info window shown when a marker or region is hovered or clicked. Empty — the default — takes the control's own BackgroundColor, so the window matches the form without being configured.
- `InfoForegroundColor` (String, default (empty); `#RRGGBB`, or empty for automatic high contrast) — Text colour of the info window. Left EMPTY — the default — it is DERIVED from whichever background the window ended up with, choosing black or white for the higher contrast, so the window is legible on any card (at least 4.5:1, WCAG's floor for body text). Set this only when a specific colour is required: an explicit value is used as given and its contrast is the caller's business.
- `InfoBorderColor` (String, default (empty); `#RRGGBB`, or empty for the default) — Outline of the info window.
- `InfoCornerRadius` (Integer, default 8; integer 0-32 (default 8)) — Corner rounding of the info window; 0 is square.
- `InfoShadow` (Boolean, default 1; `1`/`0` (default 1)) — Drop shadow under the info window. Turn it off on a flat or high-contrast form.
- `MarkerColor` (String, default (empty); `#RRGGBB[AA]`, or empty for the built-in `#C82828`) — Fill of every pin on this map. Pins used to be red with no way to say otherwise; this is that colour, now yours. `AddMarker` carries no colour of its own, so this is what sets it.
- `MarkerBorderColor` (String, default (empty); `#RRGGBB[AA]`, or empty for the built-in `#FFFFFF`) — The ring drawn around every pin so it reads against a busy basemap.
- `RouteColor` (String, default (empty); `#RRGGBB[AA]`, or empty for the built-in `#1E6EDC`) — Colour for a route whose own `Routes` line names none. A route drawn with a colour — `AddRoute` USING id colour width geometry — keeps that colour; this is only the fallback.
- `RouteCasingColor` (String, default (empty); `#RRGGBB[AA]`, or empty for the built-in `#FFFFFFB4`) — The casing under EVERY route: the bright halo that makes a thin line readable over mixed terrain, the way a road map draws one. Unlike RouteColor this applies to every route, whatever colour it names.
- `RegionFillColor` (String, default (empty); `#RRGGBB[AA]`, or empty for the built-in translucent blue) — Fill for a region whose own `Regions` line names none. Give any replacement an alpha, or the territory hides the streets under it.
- `RegionBorderColor` (String, default (empty); `#RRGGBB[AA]`, or empty for NO border) — Outline for a region whose own line names no stroke. Empty is not a colour here — it means such a region is drawn without a border at all, which is what it has always done; naming a colour gives every unstyled region an outline.
- `TileBackgroundColor` (String, default (empty); `#RRGGBB[AA]`, or empty for the built-in `#C8C8C8`) — Painted under the whole map before any tile has arrived — what the operator sees for the first instant, and behind the map wherever the world has no tiles.
- `TileLoadingColor` (String, default (empty); `#RRGGBB[AA]`, or empty for the built-in `#D2D2D2`) — One tile that has not arrived yet, in its own square — seen only when there is NOTHING to borrow from. A missing tile normally shows the ground rescaled instead: zooming in, the nearest loaded lower level magnified and cropped to that tile's own quadrant; zooming out, the four tiles of the level just left, shrunk into their quarters. So this colour appears on the first view of a place and on a tile that failed, not on every zoom. Set it and TileBackgroundColor to the same value for a map that fills in without a visible grid.
- `ApiKeySource` (String, default (empty); reserved — currently unused) — Declared but not read by any runtime or codegen path today. The google_maps API key is resolved entirely from the project's Google Maps credential slot (Settings → Integrations), never from a control property — do not rely on this property for anything.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `ResponseBody` — The answer to the last async call, delivered with `onComplete`. Its shape is the method's — a Directions answer is seven TAB-separated fields, PlacesSearch is one line per result, a RestClient verb is the raw body. UNSTRING it in the onComplete handler; it is empty before the first call completes.
- `StatusCode` — The HTTP status of the last call. `0` when the request never reached a server.
- `Busy` — Read-only runtime flag: an async operation is in flight.
- `LastError` — Why the last async call failed, delivered with `onError`. Empty after a call that succeeded.
- `SelectedMarkerId` — Id of the marker the user last clicked, delivered with onMarkerClick.
- `SelectedRegionId` — Id of the region whose info card is OPEN — set by a click, cleared by clicking bare map. Writing it opens or closes a card from COBOL.
- `HoveredMarkerId` — Id of the marker the pointer is over, delivered with onMarkerHover.
- `HoveredRegionId` — Id of the region the pointer is over, delivered with onRegionHover.

### Supported Events
- `onComplete` — the async operation finished successfully
- `onError` — the operation failed (message in `LastError`)
- `onTimeout` — the async operation exceeded its timeout; it has been cancelled, and `LastError` says so ("No answer within N seconds: the call was cancelled.")
- `onCancelled` — the async operation was cancelled
- `onMapClick` — the map background was clicked (not a marker) — the primary event
- `onMarkerClick` — a marker was clicked (`SelectedMarkerId` holds its id)
- `onMarkerHover`
- `onRegionHover`
- `onRegionClick`
- `onBoundsChanged` — the map was panned or zoomed (`CenterLat`/`CenterLng`/`Zoom` updated)
- Plus the universal events: `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseEnter`, `onMouseLeave`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `Geocode(address: String)` — **Async** — starts the lookup and returns an EMPTY string at once. `onComplete` delivers `lat\tlng\tformatted_address` in `ResponseBody`. Fails "not configured" with no google_maps key set (R33).
- `ReverseGeocode(lat: String, lng: String)` — **Async** — `onComplete` delivers the formatted address in `ResponseBody`.
- `Directions(origin: String, destination: String)` — **Async** — `onComplete` delivers SEVEN TAB-separated fields in `ResponseBody`: `distance_text\tduration_text\troute_summary\tdistance_METRES\tduration_SECONDS\tencoded_polyline\ttraffic_SECONDS`. The first three read; the numbers are what you COMPUTE with; the polyline goes straight into `AddRoute` to trace the route. The LAST field is the drive time with CURRENT TRAFFIC (0 when Google supplied none) — the traffic-aware answer to "how long, leaving now".
- `DistanceMatrix(origin: String, destination: String)` — **Async** — `onComplete` delivers `distance_text\tduration_text\tdistance_METRES\tduration_SECONDS` in `ResponseBody`.
- `PlacesSearch(query: String, radiusMeters: String)` — **Async** — `onComplete` delivers one `place_id\tname\taddress\tlat\tlng` line per result in `ResponseBody`.
- `TraceRoad(apiKey: String, fromLat: String, fromLng: String, toLat: String, toLng: String)` — **Async** — a road route from **OpenRouteService**, for programs with no Google credential. `onComplete` delivers THREE TAB-separated fields in `ResponseBody`: `distance_METRES\tduration_SECONDS\tencoded_polyline`; the polyline goes straight into `AddRoute`. **The key is the first ARGUMENT, not a project setting** — ask the operator for it (a TextBox on the form) and pass it in; PowerRustCOBOL never stores it. A blank key fails on `onError` without a network call. Use this when you need the ROAD; a hand-written waypoint list is only ever as close to it as its own points.
- `AddMarker(id: String, lat: String, lng: String, label: String, info: String)` — Append one pin to Markers (ergonomic alternative to hand-formatting the TAB-separated property).
- `RemoveMarker(id: String)` — Remove the marker whose id matches, if any.
- `AddRoute(id: String, colour: String, width: String, geometry: String)` — Trace a line over the basemap. `geometry` is an encoded polyline (Directions' sixth field) or `lat,lng;lat,lng;…`. Re-using an id REPLACES that route rather than stacking a second copy. Needs no API key.
- `RemoveRoute(id: String)` — Remove the route with that id.
- `ClearRoutes()` — Remove every route.
- `AddRegion(id: String, fill: String, stroke: String, width: String, geometry: String [, label: String, info: String])` — Fill an area over the basemap — a sales territory, a delivery zone. `fill` takes an alpha (`#RRGGBBAA`) so the map stays readable underneath. The ring closes itself and MAY be concave. Re-using an id replaces it. Needs no API key.
- `RemoveRegion(id: String)` — Remove the region with that id.
- `ClearRegions()` — Remove every region.

### Usage — basemap vs. data verbs, and the API key
The OpenStreetMap basemap (pan/zoom, `CenterLat`/`CenterLng`/`Zoom`, `Markers`) needs **no API key at all**. Only the five data methods (`Geocode`, `ReverseGeocode`, `Directions`, `DistanceMatrix`, `PlacesSearch`) call the real Google Maps API and need a project-level `google-maps` credential (Settings → Integrations) — with none configured they fail immediately with `LastError` = "not configured" and fire `onError`, never a crash, never a network call (R33). The key itself never appears in any property, generated `.cbl`, or the `.cfrm`.

### Colours — nothing on a map is hard-coded
Every colour the map paints is a property, in the inspector's **Basic properties** section and writable from COBOL: `MarkerColor`, `MarkerBorderColor`, `RouteColor`, `RouteCasingColor`, `RegionFillColor`, `RegionBorderColor`, `TileBackgroundColor`, `TileLoadingColor`. Each starts EMPTY, meaning the built-in the map has always painted, so a form that sets none of them is unchanged.

Colour carried by the DATA still wins: `AddRoute` USING id colour width geometry keeps that route's own colour, and `AddRegion`'s fill and stroke keep theirs — `RouteColor`, `RegionFillColor` and `RegionBorderColor` are what a line naming none falls back to. Two exceptions, because their data carries no colour at all: `MarkerColor`/`MarkerBorderColor` (an `AddMarker` has no colour argument) and `RouteCasingColor` (the halo under EVERY route, whatever colour the route itself names). Never tell a developer a map colour cannot be changed, and never suggest editing the `.cfrm` by hand to change one.

---

## Control: WebSearch

Non-visual Google Custom Search JSON API client (async by default, same lifecycle as RestClient). Default size 56×56 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Configuration` (String, default (empty); empty (this control's own settings), or the name of a project connection) — Where this control gets its connection. **Empty — the default — means the control's own properties below**, exactly as it has always worked. Otherwise it names one of the project's connections (Settings → Integrations), and that connection's settings replace the control's own before the form runs. On a **RestClient** that is the address, method, authentication scheme, headers and timeouts; on a **WebSearch** it is the provider and its engine id or instance URL — and deliberately NOT `NumResults` or `SafeSearch`, which are per-call settings the form changes at run time and which stay the control's own. An **AgentObject** is different in kind: it selects one of the machine's configured **Model Providers** (Settings → Models) rather than a project connection, taking that provider's protocol, endpoint and key — while `AgentModel`, `Temperature`, `MaximumTokens` and `TimeoutSeconds` stay the control's own, because one provider offers many models. Its `API Key` row disappears while bound, which is the point: an agent's key is entered once, in the Model Providers Manager, and never copied onto a form. That binding is **machine-scoped** — a provider not configured on a given machine is reported as exactly that, not as a broken project — and reaches a running application through `COBOLT_AGENT_PROVIDERS`. Define a service once and every form that uses it stays in step, instead of six forms drifting apart. The **credential is never part of the connection record** — that record round-trips in `cobolt.toml` and is meant to be committed, while the key lives in the machine-local store and reaches a running form through the environment, so a checked-out project carries the connections and each developer supplies their own key. A built application carries the connections baked in and takes each key from `COBOLT_CONNECTION_KEY_<ID>` on the machine that runs it. A Configuration naming a connection the project no longer has is an error, not a silent fall back to the local settings: the control was told to ignore those.
- `Provider` (String, default "Google"; one of: `Google` | `Brave` | `Serper` | `Tavily` | `SearXNG`) — Which search back end answers. `Google` is the default and what an unset or unrecognised value falls back to, so an older form keeps working. Every provider returns results through the same accessors (ResultCount/TopTitle/TopSnippet/TopLink/GetResult), so changing this does not change your COBOL. Google additionally needs SearchEngineId; SearXNG needs Endpoint and no key at all.
- `Endpoint` (String, default (empty); base URL of your SearXNG instance, e.g. `https://search.example.com`) — Only SearXNG reads this — the hosted providers each have one address of their own. Required when Provider is SearXNG; the instance must have `format=json` enabled in its own settings, which is off by default.
- `ApiKey` (String, default (empty); secret string, or empty) — Per-control override of the project's search credential (Settings → Integrations). Empty — the normal case — means "use the project's key". Set it only when one form must search under a different account than the project default. SearXNG needs no key. STORED ON YOUR MACHINE, never in the form: what you type in the designer goes to the local credential file and the running form is handed it at start-up, so the `.cfrm` you commit carries an empty value however the key was entered.
- `SearchEngineId` (String, default (empty); Google Programmable Search Engine `cx` value) — Which Custom Search engine to query — a plain, non-secret id, not the API key. Read only when Provider is Google; the other providers search the whole web without being told where.
- `Query` (String, default (empty); free text) — Search query text. Set this before INVOKE 'Search'.
- `NumResults` (Integer, default 10; integer, clamped to the provider's own cap) — Results requested per search. Clamped to what the chosen provider accepts — Google 10, Brave 20, Tavily 20, SearXNG 50, Serper 100 — because asking for more is an HTTP error, not more results.
- `SafeSearch` (String, default "Off"; one of: `Off` | `Medium` | `High`) — SafeSearch filtering level, mapped to each provider's own vocabulary: Google has two levels (`off`/`active`) so Medium and High both filter; Brave takes `off`/`moderate`/`strict`; SearXNG takes 0/1/2. **Serper and Tavily expose no SafeSearch setting, so the property is not sent to them** — do not assume filtering is running there.
- `Mode` (String, default "Async"; `Async` | `Sync`) — Async fires onComplete/onError later; Sync blocks and returns in-statement.
- `Busy` (Boolean, default 0; `1` (true) or `0` (false)) — Read-only runtime flag: an async operation is in flight.
- `TimeoutMs` (Integer, default 30000; milliseconds ≥ 0) — How long a request may take, in either Mode — Async or Sync. 0 falls back to `TimeoutSeconds` where the control has it (a RestClient); a WebSearch has none, so there 0 means no timeout.
- `Verbose` (Boolean, default 0; true | false) — Narrate the whole call into the program's output. Off by default; turn it on when the control appears to do nothing, because an operation that returned nothing and one that never ran produce the same empty log, and this is what separates them. **On an AgentObject**: the endpoint, every request header and the payload exactly as sent, then the HTTP status, the raw body and the reply read out of it. The headers include the API key **unmasked, on purpose** — a key wrong by one character is invisible once masked — and the log says so on the next line, so do not paste it into a bug report. The call is made the same way in the IDE, under `rcrun` and in a built application. **On a WebSearch**: the provider, the method and URL, the request headers, the body sent, whether the call is async or sync, then the HTTP status and the raw response — uncut, so it can be compared against the provider's own documentation. A WebSearch masks its credentials: a key in a header or in the URL query (Google signs there) prints as its first few characters and a length.

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `ResponseBody` — The answer to the last async call, delivered with `onComplete`. Its shape is the method's — a Directions answer is seven TAB-separated fields, PlacesSearch is one line per result, a RestClient verb is the raw body. UNSTRING it in the onComplete handler; it is empty before the first call completes.
- `StatusCode` — The HTTP status of the last call. `0` when the request never reached a server.
- `Busy` — Read-only runtime flag: an async operation is in flight.
- `LastError` — Why the last async call failed, delivered with `onError`. Empty after a call that succeeded.

### Supported Events
- `onResultsReceived` — Fired when a search comes back with results, before the uniform `onComplete`. This is WebSearch's PRIMARY event — the one a double-click on the control binds — and it is the natural place to read `ResultCount`/`TopTitle`/`GetResult(n)`. Until 1.65.75 it was documented as a mere label and nothing raised it, so a handler bound here never ran: the search succeeded, `ResponseBody` filled, and no COBOL executed. Both events are raised now, so a form bound to `onComplete` instead is unaffected. Errors and timeouts still arrive on `onError`/`onTimeout`.
- `onError` — the operation failed (message in `LastError`)
- `onTimeout` — the async operation exceeded its timeout; it has been cancelled, and `LastError` says so ("No answer within N seconds: the call was cancelled.")
- `onComplete` — the async operation finished successfully
- `onCancelled` — the async operation was cancelled

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `Search()` — Run a search on the control's Provider using the current Query/NumResults/SafeSearch (plus SearchEngineId for Google, Endpoint for SearXNG). Async mode: returns immediately, raw JSON lands in ResponseBody + onComplete. Sync mode: returns the raw JSON body. Fails "not configured" before sending anything when the chosen provider's key — or SearXNG's Endpoint — is missing (R33).
- `ResultCount() → Integer` — Number of result items in the last response (parses ResponseBody fresh each call).
- `TopTitle() → String` — First result's title, or empty before any search.
- `TopSnippet() → String` — First result's snippet, or empty before any search.
- `TopLink() → String` — First result's URL, or empty before any search.
- `GetResult(index: Integer) → String` — 1-based indexed result as `title\tsnippet\tlink`; an out-of-range index returns empty, never an error.
- `Cancel()` — Cancel the in-flight search.
- `IsBusy() → Boolean (0/1)` — A search is in flight.

### Usage — the generated paragraph vs. `INVOKE 'Search'`
Every `WebSearch` control also gets a generated `<id>-SEARCH` paragraph (`PERFORM SEARCH-1-SEARCH`) that builds a Custom Search URL and calls `COBOL-HTTP-GET` directly — but it does PLAIN, UNENCODED string concatenation: a multi-word `Query` truncates at its first space, and it never includes the API key (so it 401s against the real API on its own). **Use `INVOKE <id> 'Search'` instead** — it percent-encodes the query and resolves the credential-store key automatically; the paragraph exists only as a low-level fallback. Same "not configured" contract as Maps: no `google-custom-search` key configured (Settings → Integrations) fails immediately with `onError`, no request sent (R33).

---

## Control: Snackbar

Non-visual: a transient, NON-MODAL notification — a short message, an optional category icon and up to three action buttons, shown over the form for a few seconds and then gone. It never blocks the program and never demands an answer: a handler raises one and carries straight on. The control you drop is the TEMPLATE, not the notification — it carries the defaults and paints nothing where it sits; every `Show()` mints a NEW notification from the values current at that moment and adds it to the stack, so two calls in one handler put up two messages. Several live at once, stacked VERTICALLY (never horizontally) against one of nine `Anchor` positions. Every move is animated: the ones already up GLIDE over 300 ms to make room or to close a gap, a message that leaves FADES over 300 ms where it stood, and an arriving one zooms and fades in over 600 ms — 200 ms for `Critical`, the only effect a category changes. They arrive ONE AT A TIME: a second `Show()` waits until the first message has finished arriving, then the stack glides clear and it appears into that room, so three raised together take about two and a half seconds to all be up. The queue is per anchor — messages in opposite corners never wait for each other — and a `Timeout` counts from when the message BECOMES VISIBLE, so a queued one is still read in full. The anchor is resolved against the FORM'S OWN SURFACE — an Embedded form's messages stay inside its ContentPane and never cover the shell's rail or breadcrumb. `Category` (Info | Question | Warning | Error | Critical) supplies the colours, the icon and the timeout; every one of those is overridable, and setting one overrides that one alone. Leave a colour EMPTY to mean "the category decides". `Timeout` is milliseconds: `-1` takes the category's own (4000/6000/6000/8000, and Critical's 0), and `0` means it stays until dismissed. Build the message in COBOL with STRING or MOVE before `Show()` — `Text` is data, not a format string. Default size 56×56 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Text` (String, default (empty); free text) — Current text content.
- `Category` (String, default "Info"; Info | Question | Warning | Error | Critical) — What the notification is ABOUT. Supplies the background, the ink, the icon and the timeout for everything you did not set yourself: Info 4000 ms, Question 6000 ms, Warning 6000 ms, Error 8000 ms, and Critical 0 — Critical stays until it is dismissed. These are DEFAULTS, not a fixed appearance: any property you set explicitly wins, and it wins alone, so a chosen `BackgroundColor` still leaves the category's icon and ink in place.
- `Size` (String, default "Medium"; one of: `Small` | `Medium` | `Large`) — Snackbar: the size class. Sets the width range (220–420 / 280–520 / 320–620), padding, default icon size (18/22/26), button height and how many text lines fit (1/2/3). Default Medium.
- `ShowCategoryIcon` (Boolean, default 1; `1` (true) or `0` (false)) — Draw the category's icon at the head of the notification. Off leaves the message hard against the left margin.
- `CategoryIconSize` (Integer, default 0; pixels > 0, or 0 = from the Size class) — The category icon's square side. 0 takes the size class's own (Small 18, Medium 22, Large 26).
- `CategoryIconColor` (String, default (empty); #RRGGBB[AA], empty = the category's) — The category icon's colour. Empty means the category decides — which is what keeps changing `Category` able to move the whole look at once.
- `BackgroundImage` (String, default (empty); image path or empty) — Snackbar: an image drawn over each notification's background colour, faded by BackgroundImageOpacity and fitted by BackgroundImageMode. Empty = none.
- `BackgroundImageMode` (String, default "Fill"; one of: `Fill` | `Fit` | `Stretch` | `Center` | `Tile`) — Snackbar: how BackgroundImage fits the notification. Default Fill.
- `BackgroundImageOpacity` (Integer, default 15; percent 0–100 (default 15)) — Snackbar: opacity of BackgroundImage over the background colour; 0 hides the image.
- `TextWrap` (Boolean, default 1; `1` (true) or `0` (false)) — Snackbar: wrap long text onto further lines, up to the Size class's line budget (1/2/3); text that still does not fit ends in an ellipsis. Off keeps a single line. Default on.
- `CornerRadius` (Integer, default 12; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `CornerRadiusTopLeft` (Integer, default -1; pixels ≥ 0, or -1) — Snackbar: the notification's top-left corner radius. -1 (default) takes CornerRadius (12).
- `CornerRadiusTopRight` (Integer, default -1; pixels ≥ 0, or -1) — Snackbar: the notification's top-right corner radius. -1 (default) takes CornerRadius (12).
- `CornerRadiusBottomLeft` (Integer, default -1; pixels ≥ 0, or -1) — Snackbar: the notification's bottom-left corner radius. -1 (default) takes CornerRadius (12).
- `CornerRadiusBottomRight` (Integer, default -1; pixels ≥ 0, or -1) — Snackbar: the notification's bottom-right corner radius. -1 (default) takes CornerRadius (12).
- `BorderStyle` (String, default "None"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).
- `BorderWidth` (Integer, default 1; pixels ≥ 0) — Border line thickness.
- `BorderColor` (String, default "#00000000"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Border line color. Under `BorderStyle` `Glow` it is the discreet edge between the glowing corners, drawn at 40 %.
- `Timeout` (Integer, default -1; milliseconds, -1 or 0) — Snackbar: how long a notification stays up, counted from when it BECOMES VISIBLE. -1 (default) takes the Category's (Info 4000, Question 6000, Warning 6000, Error 8000, Critical 0); 0 keeps it until dismissed.
- `PauseTimeoutOnHover` (Boolean, default 1; `1` (true) or `0` (false)) — Hold the timeout while the pointer is over the notification or one of its buttons, and resume it — with exactly what was left — when the pointer leaves. On by default: a message the operator is still reading should not vanish under the cursor.
- `StackAnchor` (String, default "BottomRight"; TopLeft | TopCenter | TopRight | CenterLeft | Center | CenterRight | BottomLeft | BottomCenter | BottomRight) — Where the stack sits on the FORM'S OWN SURFACE — the ContentPane for an Embedded form, the window for a standalone one, never the desktop. A Top anchor grows the stack DOWNWARD and a Bottom anchor grows it UPWARD, in both cases with the newest nearest the anchor; the Centre row places the newest first and grows down.
- `Margin` (Integer, default 16; pixels ≥ 0 (default 16)) — Snackbar: distance between the stack and the edges of the form's surface at StackAnchor.
- `StackSpacing` (Integer, default 8; pixels >= 0) — The gap between two stacked notifications. The stack is vertical only — there is no horizontal stacking, by contract.
- `StackOrder` (String, default "Auto"; Auto | NewestFirst | NewestLast) — Which end of the stack the newest notification takes. `Auto` follows the anchor (newest nearest it); the other two override that rule — `NewestFirst` nearest the anchor, `NewestLast` furthest from it.
- `MaximumVisible` (Integer, default 5; integer >= 1) — How many of THIS control's notifications may be up at once. Reaching it hands the next `Show()` to `OverflowBehavior`.
- `OverflowBehavior` (String, default "Queue"; Queue | DiscardOldest | DiscardNewest) — What a `Show()` does once `MaximumVisible` are already up. `Queue` holds the new one back and raises it when a slot frees — its timeout then counts from the moment it BECAME VISIBLE, not from the call. `DiscardOldest` closes the oldest to make room (reason `Overflow`). `DiscardNewest` drops the arrival, which never appears at all.
- `Buttons` (String, default (empty); one button per line: id|text|icon|position|dismiss) — The notification's action buttons, at most THREE — a fourth is reported in the designer, never silently dropped. Fields are `|`-separated and trailing ones may be omitted: **id** is what `onButtonClick` reports (English, your own choice — a value COBOL compares against, not a label); **text** is the caption, empty for an icon-only button; **icon** is a catalogue icon name (`refresh`, `x-mark`, `undo`, `check`, …) or empty; **position** is None | Left | Right, defaulting to Left when an icon is given; **dismiss** is true | false, default true — true closes the notification after `onButtonClick` fires, false leaves it up.

### Runtime Properties (read-only)
Written by the runtime when it has something to report, never by the designer — so they carry no default, do not appear in the property pane and are not saved in the form. **Read them; never try to set them.**

- `LastButtonId` — Which of the notification's buttons was pressed. Written before `onButtonClick` fires, so ONE handler can serve every button on the message: `EVALUATE SNACK-1::LastButtonId`. It holds the `id` field from the `Buttons` line — your own English name for the button.
- `LastButtonIndex` — The 0-based position of the button that was pressed, in `Buttons` order. Written alongside `LastButtonId`, for a handler that would rather switch on the position than the name.

### Supported Events
- `onShown`
- `onClosing`
- `onClosed`
- `onTimeout` — the async operation exceeded its timeout; it has been cancelled, and `LastError` says so ("No answer within N seconds: the call was cancelled.")
- `onButtonClick`

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `Show()` — Raise a NEW notification from the control's CURRENT property values and add it to the stack. Call it twice and two messages appear; it is not a visibility toggle and it never blocks. Set `Text` (and anything else you want changed) immediately before it — the notification is a snapshot, so editing the template afterwards does not rewrite a message already on screen. It returns at once and the message is on the stack at once — `onShown` fires there — but the PICTURE queues: messages arrive one at a time, in the order they were raised, so the second one is not on screen yet when the handler carries on.
- `DismissAll()` — Dismiss every live notification RAISED BY THIS CONTROL, with reason `Programmatic`; anything this control had queued is discarded too. Other Snackbar controls on the form are untouched. There is no COBOL-callable `Hide()`/`Dismiss(id)` for ONE notification — under `Show()`-as-factory it could not say WHICH notification it meant. The operator has their own way to close one: every notification carries a built-in close button (top-right, reason `User`) regardless of category or whatever buttons the developer added — that is UI, not a CALL, so it has no effect on what your COBOL code can dismiss programmatically.
- `Clear()` — Empty the button row so `AddButton()` can build a fresh one. It clears the BUTTONS AND NOTHING ELSE — `Text`, `Category`, the colours and every other property keep what they hold, which is the opposite of `Clear()` on a TextBox or a list. A notification already on screen is a snapshot and is not disturbed: what is being cleared is the TEMPLATE, for the next `Show()`.
- `AddButton(spec: String)` — Declare ONE action button on the template, as `key=value` pairs separated by commas: `INVOKE SNACK-1::AddButton("id=undo,caption=Undo,icon=undo,position=1,dismiss=true")`. Call it once per button, up to three. This exists because `Buttons` is one line per button and a COBOL literal cannot contain a newline — a `MOVE` into `Buttons` can only ever declare ONE, whatever it says. Keys: **id** (REQUIRED — what `onButtonClick` reports through `LastButtonId`; a spec without one declares no button and says so in the diagnostics trace), **caption** (or `text`), **icon** (a catalogue icon name), **position** (the button's 1-based ordinal LEFT TO RIGHT; omitted = the end, in call order), **dismiss** (true | false, default true), **iconposition** (None | Left | Right; omitted = Left when an icon is given). It writes the `Buttons` property, so it composes with a row the designer already set — call `Clear()` first to replace that row instead of adding to it.

---

## Control: Viewer

A document viewer inside the form: plain text, Markdown, images, PDF and an HTML subset, shown with a toolbar, page navigation, zoom, a filmstrip of page thumbnails, Find, Print and Save As. It exists because a COBOL program that produces or receives a document had nowhere to SHOW it — it wrote a PDF or a report and handed it to an external program. `Source` takes a PATH or a `http://` / `https://` **web address**: a URL is fetched to a local file once and then opened exactly like any other path, so every format, layout and page-at-a-time read works on it unchanged — and it is **not a browser**, nothing is executed, no script runs and no sub-resource is followed. The download runs off the UI thread, is cached, is capped at 256 MB, and a failure reaches `onError` with `LastError` set exactly as an unreadable file does. **Format is resolved from CONTENT first and the extension second**, so a PNG named `.txt` still opens as an image; the resolved name lands in `Format`. **Fidelity is a published contract, not a hope**: plain text and Markdown (tables, task lists, footnotes, strikethrough) in full at any size; every common image format including animation; PDF as its TEXT, basic vector, page geometry and page breaks — not a faithful raster of a complex page, not forms or annotations; Mermaid as FLOWCHART and SEQUENCE diagrams only, with any other Mermaid diagram type refused by name rather than half-drawn, and a block that is not Mermaid at all (a text drawing of brackets and arrows) shown as its text — from a ```mermaid Markdown fence or from an HTML page's `<div class="mermaid">` / `<pre class="mermaid">` element, which the Viewer draws itself (the page's mermaid.js is never run); a flowchart's connectors are ORTHOGONAL and meet each shape at the middle of a side (a decision diamond at a vertex), going round other shapes in lanes of their own, with each label placed where it covers no shape; HTML as a SUBSET renderer — block and inline layout, typography, colours, borders, tables and images — STYLED BY THE PAGE'S OWN CSS: its `<style>` blocks and `style` attributes cascade (specificity, source order, `!important`, inheritance, `var(--x)` custom properties; selectors by tag, class, id, descendant and child, and `:first-child`/`:last-child`/`:nth-child()`), and text (colour, size, weight, style, monospaced family, decoration, transform, letter spacing, line height), boxes (background colour or `linear-gradient`, borders per side, radius, padding, margins with `0 auto` centring, width and max-width in px or %, box-shadow), `text-align`, `display: none` and every table cell's background, padding, alignment and borders are painted; CSS lengths scale with the Viewer's zoom (16 CSS px = its base font). It is emphatically **not a browser**: no JavaScript, and CSS LAYOUT through flex and grid: `display: flex` rows and columns (`flex-wrap`, `gap`, `align-items`, `justify-content`, and per item `flex`, `flex-grow`, `flex-basis`, `align-self`) and `display: grid` (`grid-template-columns` with lengths, %, `fr`, `repeat(N, …)` and `repeat(auto-fit, minmax(min, 1fr))`, `gap`, `grid-column: span N`), plus `height`/`min-height` and `border-radius: 50%` circles — so an infographic's round step numbers sit beside their text and cards line up in columns; floats and positioning are not built, so such a page keeps that content stacked in order; `@media` rules, `:hover`, `::before`, attribute selectors and sibling combinators are skipped; nothing is fetched (`@import`, `<link>`, `url()`, web fonts). A layout this renderer cannot follow loses its layout and keeps every word of its content. Word, PowerPoint, Excel and OpenDocument files (`.docx`, `.pptx`, `.xlsx`, `.odt`, `.ods`, `.odp` and their macro/template variants) open as their TEXT — converted to Markdown by the Knowledge Base's own converter, so headings, lists and tables survive while page layout, fonts and pictures do not; `Format` reads `Markdown` for them. **A document's text is BLACK in every layout** — it does not follow the form's theme, because a document is a document wherever it is shown; only a colour the CONTENT states (`<font color="…">` in the HTML subset) overrides it. `Layout` picks `Raw` (the literal source), `Web` (formatted, no margins), `Print` (page margins, a paper border and a shadow), `Page` (print layout on a black-on-white body) or `Streamed` (§8.8: ONE content pane, no chrome at all, for hosting a chatbot conversation). In `Cards` a card is the page **in miniature**, painted by the same renderer that paints the page; one click selects a card and a DOUBLE click opens it, leaving `Cards` for `Full` on that page. While the pointer is over the control the WHEEL belongs to it — a notch scrolls the document and never also scrolls the form or container behind it. `SplitMode` shows TWO views at once, each with its own document, page, zoom, scroll position, view mode, filmstrip and Find — and pointing both at the same document ATTACHES to the one decode rather than reading it twice. Every view property is addressable as `View1X`/`View2X`; the plain names (`Zoom`, `SearchText`, …) are aliases for the FIRST view. `Save As` writes the source's ORIGINAL BYTES, never a re-encode, and the control never modifies the document it is showing. Print hands the document to the operating system, so its Complete/Cancelled events report what the platform actually did. A platform takes a FILE, so a `LoadBytes` document is written out first under R18.1's proposed name. **Print** goes to the print system (`lp` on macOS and Linux, the shell's `Print` verb on Windows): accepted by the spooler is `onPrintComplete`, refused is `onPrintCancelled` with the reason in `LastError`. Decoding and indexing run off the UI thread: a large document never stalls the form. Default size 400×320 px.

### Settable Properties
All universal properties above apply. Type-specific properties (type, default, allowed values):

- `Format` (String, default (empty); one of: `Text` | `Markdown` | `Image` | `Pdf` | `HtmlSubset` (runtime-set)) — The format the runtime detected for the loaded document, from its extension and content; written on every load. Setting it does not change how the document is decoded.
- `Layout` (String, default "Page"; `Raw` | `Web` | `Print` | `Page` | `Streamed`) — Viewer. How the document is laid out. `Raw` shows the literal stored source, monospace and unformatted. `Web` formats it with no page margins. `Print` adds page margins, a paper border and a paper shadow. `Page` is print layout PLUS a black-on-white document body, whatever the form's theme. `Streamed` is the conversation surface (§8.8): one content pane and NO chrome at all — no toolbar, no Find bar, no thumbnails, no filmstrip — regardless of what was showing before. Every line of a conversation is reachable: the wheel scrolls it, a scrollbar appears on its right edge whenever it is taller than the pane (drag the thumb; a click on the track moves a page), and with the pointer over it the arrow, Page Up/Down, Home and End keys move it — unless another control holds the caret. A drag across its text selects the text; it does not scroll. `Page` applies to any paginated content, Markdown and plain text included; it is not tied to a document class.
- `Fullscreen` (Boolean, default 0; `1` (true) or `0` (false)) — Viewer. While on, the toolbar is hidden and the document takes its height; leaving restores it. Esc leaves fullscreen first and returns `Zoom` to 100 % on a second press.
- `SplitMode` (String, default "None"; `None` | `LeftRight` | `TopBottom`) — Viewer. `None` shows one view. `LeftRight` and `TopBottom` show TWO, each with its own source, page, zoom, scroll position, view mode, filmstrip and Find state — so one section can be read while another is browsed. Pointing both views at the SAME document attaches to the one decode rather than reading it twice, and searching one side never disturbs the other. A control too narrow to give both views a usable width shows one rather than two unreadable slivers.
- `SplitPercent` (Integer, default 50; `0`–`100`) — Viewer. Where the divider sits along the span the two views divide. Only meaningful once `SplitMode` is set; the divider is also draggable, and neither view is ever squeezed below a grabbable width.
- `RenderAsHtml` (Boolean, default 1; `1` (true) or `0` (false)) — Viewer, conversation mode (§8.1). A blanket safety override, default ON. Turned OFF, EVERY append behaves as `Raw` whichever method was called, so a program showing content it does not control can say so once and be believed. Content already appended is never reinterpreted when it is turned back on.
- `UserBubbleColor` (String, default "#2E7D32FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Viewer, conversation mode. Fill of the bubble a message appended with role `"user"` is drawn in, on the right of the pane. Default green `#2E7D32FF`.
- `UserBubbleTextColor` (String, default "#FFFFFFFF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Viewer, conversation mode. Text colour inside the user's bubble. Default white.
- `AgentBubbleColor` (String, default "#2C6FD2FF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Viewer, conversation mode. Fill of the bubble a message appended with role `"agent"` is drawn in, on the left of the pane. Default blue `#2C6FD2FF`.
- `AgentBubbleTextColor` (String, default "#FFFFFFFF"; hex color string `"#RRGGBB"` or `"#RRGGBBAA"`) — Viewer, conversation mode. Text colour inside the agent's bubble. Default white.
- `UserBubbleBold` (Boolean, default 0; `1` (true) or `0` (false)) — Viewer, conversation mode. On, the text of every message appended with role `"user"` is drawn in the bold face (the system's bold Arial or Helvetica). Default off.
- `HistoryList` (String, default (empty); read-only, one `id|title` per line) — Viewer, conversation mode (§8.8). Up to ten past conversations, as `id|title`, one per line — the same multi-line convention `Buttons` uses on Snackbar — so the developer's own UI can enumerate, sort or search them. History holds an id and a title and **never a conversation's content**: selecting one is always a fresh request back to your program, never a cache restore, which is what keeps a session that runs all day from growing without limit.
- `Progress` (Integer, default 0; `0`–`100`, read-only) — Viewer. How far a document is through opening. `onLoadProgress` reports it as it climbs and `onLoaded` follows at 100.
- `LastError` (String, default (empty); runtime-only, read-only) — Why the last async call failed, delivered with `onError`. Empty after a call that succeeded.
- `View1Source` (String, default (empty); file path or URL) — Viewer. The document view 1 shows. Setting it starts an asynchronous load (onLoadProgress, then onLoaded or onError); the format is detected from the extension and content. The unprefixed `Source` is an alias for it.
- `View1Page` (Integer, default 1; integer ≥ 1) — Viewer. The page view 1 is on, counted from 1. Default 1. The unprefixed `Page` is an alias for it.
- `View1Zoom` (Integer, default 100; percent, 25–1600) — Viewer. View 1's zoom. Default 100; values outside the range are clamped. Changed by the wheel, a double-click and the slider. The unprefixed `Zoom` is an alias for it.
- `View1ViewMode` (String, default "Full"; one of: `Full` | `Cards`) — Viewer. What view 1 shows: the document (`Full`, the default) or a grid of page cards (`Cards`). An unrecognised value reads as `Full`. The unprefixed `ViewMode` is an alias for it.
- `View1CardSize` (Integer, default 55; `0`–`100` (percent)) — Viewer. Card size for view 1 in `Cards` mode. Default 55; larger means fewer, bigger cards per row. The bottom-right slider drives it in `Cards` mode and `Zoom` in `Full`. The unprefixed `CardSize` is an alias for it.
- `View1ShowFilmstrip` (Boolean, default 0; `1` (true) or `0` (false)) — Viewer. Docks a strip of page thumbnails beside view 1's document. Default off; not drawn while the view is in `Cards` mode. The unprefixed `ShowFilmstrip` is an alias for it.
- `View1ScrollPosition` (Integer, default 0; pixels ≥ 0) — Viewer. View 1's vertical scroll offset; 0 (the default) is the top. Written back as the user scrolls; a COBOL write moves the view. The unprefixed `ScrollPosition` is an alias for it.
- `View1SearchText` (String, default (empty); free text) — Viewer. The text view 1's Find looks for. Matches are counted in SearchMatchCount and highlighted while SearchHighlightEnabled is on. The unprefixed `SearchText` is an alias for it.
- `View1SearchCaseSensitive` (Boolean, default 0; `1` (true) or `0` (false)) — Viewer. Makes view 1's Find case-sensitive. Default off. The unprefixed `SearchCaseSensitive` is an alias for it.
- `View1SearchHighlightEnabled` (Boolean, default 1; `1` (true) or `0` (false)) — Viewer. Highlights every Find match in view 1. Default on. The unprefixed `SearchHighlightEnabled` is an alias for it.
- `View1SearchCurrentMatch` (Integer, default 0; integer ≥ 0) — Viewer. Index of the selected Find match in view 1, counted from 0 (the bar shows `1 / N`). Moves with Find next/previous. The unprefixed `SearchCurrentMatch` is an alias for it.
- `View1SearchMatchCount` (Integer, default 0; integer, read-only) — Viewer. Number of Find matches in view 1's document. The unprefixed `SearchMatchCount` is an alias for it.
- `View1FindOpen` (Boolean, default 0; `1` (true) or `0` (false)) — Viewer. Whether view 1's Find bar is open. Default off; Esc closes it. The unprefixed `FindOpen` is an alias for it.
- `View2Source` (String, default (empty); file path or URL) — Viewer. The document view 2 shows. Setting it starts an asynchronous load (onLoadProgress, then onLoaded or onError); the format is detected from the extension and content. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2Page` (Integer, default 1; integer ≥ 1) — Viewer. The page view 2 is on, counted from 1. Default 1. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2Zoom` (Integer, default 100; percent, 25–1600) — Viewer. View 2's zoom. Default 100; values outside the range are clamped. Changed by the wheel, a double-click and the slider. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2ViewMode` (String, default "Full"; one of: `Full` | `Cards`) — Viewer. What view 2 shows: the document (`Full`, the default) or a grid of page cards (`Cards`). An unrecognised value reads as `Full`. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2CardSize` (Integer, default 55; `0`–`100` (percent)) — Viewer. Card size for view 2 in `Cards` mode. Default 55; larger means fewer, bigger cards per row. The bottom-right slider drives it in `Cards` mode and `Zoom` in `Full`. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2ShowFilmstrip` (Boolean, default 0; `1` (true) or `0` (false)) — Viewer. Docks a strip of page thumbnails beside view 2's document. Default off; not drawn while the view is in `Cards` mode. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2ScrollPosition` (Integer, default 0; pixels ≥ 0) — Viewer. View 2's vertical scroll offset; 0 (the default) is the top. Written back as the user scrolls; a COBOL write moves the view. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2SearchText` (String, default (empty); free text) — Viewer. The text view 2's Find looks for. Matches are counted in SearchMatchCount and highlighted while SearchHighlightEnabled is on. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2SearchCaseSensitive` (Boolean, default 0; `1` (true) or `0` (false)) — Viewer. Makes view 2's Find case-sensitive. Default off. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2SearchHighlightEnabled` (Boolean, default 1; `1` (true) or `0` (false)) — Viewer. Highlights every Find match in view 2. Default on. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2SearchCurrentMatch` (Integer, default 0; integer ≥ 0) — Viewer. Index of the selected Find match in view 2, counted from 0 (the bar shows `1 / N`). Moves with Find next/previous. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2SearchMatchCount` (Integer, default 0; integer, read-only) — Viewer. Number of Find matches in view 2's document. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `View2FindOpen` (Boolean, default 0; `1` (true) or `0` (false)) — Viewer. Whether view 2's Find bar is open. Default off; Esc closes it. Meaningful only when `SplitMode` is `LeftRight` or `TopBottom`.
- `CornerRadius` (Integer, default 0; pixels ≥ 0) — Rounded-corner radius. Carried by EVERY visual control including Label and the MenuBar/ToolBar/StatusBar bars (spec 016 Q4, settled 2026-09-03): a frame at Transparency = 100 is invisible, not absent, so it still has corners. Defaults: 3 on Button, 8 on charts, 10 on ProgressBar and ToolBar, 12 on Snackbar, 0 elsewhere.
- `BorderStyle` (String, default "Single"; one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken` | `Glow`) — Border drawing style, on every control that carries the property (Button included). `Single` is one line of `BorderWidth` in `BorderColor`; `Fixed3D`/`Raised` draw a relief lit from the top-left (lighter top and left, darker bottom and right); `Sunken` inverts it. ALL of them follow the control's `CornerRadius`, meeting halfway round each corner arc, and all draw the same whatever paints the face — glass, background gradient, form theme or asset pack. Under the Neumorphic glass style `Fixed3D` and `Raised` come from the form's own shadow stack and read as RAISED, and `Sunken` turns that relief over (shadow top-left, highlight bottom-right) so the control reads as pressed IN — while `Single` stays the developer's own flat line in `BorderColor` / `BorderWidth`, as on every other style (before 1.70.263 it was relief there too, so the colour did nothing). `Glow` (spec 083) draws a discreet edge in `BorderColor` at 40 % whose four corners glow in `BorderGlowTopLeft`, `BorderGlowTopRight`, `BorderGlowBottomRight` and `BorderGlowBottomLeft`, each fading out along both edges; unset, they are a white specular glow brightest at the top-left. Like every style it follows `CornerRadius` and `BorderWidth`, and the four colours are ordinary colour properties, settable in the inspector (shown under BorderStyle while it is Glow) and from COBOL (`SET CARD-1::BorderGlowTopLeft TO "#60BEFF"`).

### Supported Events
- `onError` — the operation failed (message in `LastError`)
- `onLoadProgress` — a document is opening; `Progress` carries 0-100
- `onLoaded` — a document finished opening; `Format` carries what it resolved to
- `onLayoutChanged` — `Layout` changed (Raw/Web/Print/Page/Streamed)
- `onZoomChanged` — `Zoom` SETTLED after a wheel, a double-click, the slider or a programmatic change — once per gesture, never once per notch
- `onCardSizeChanged` — `CardSize` settled after the slider or a programmatic change
- `onScrolled` — the content came to rest — after a key, a throw's glide, or a programmatic move; never mid-glide
- `onViewModeChanged` — a view switched between `Full` and `Cards`
- `onFilmstripToggled` — a view's filmstrip opened or closed
- `onFullscreenEntered` — the Viewer went fullscreen and hid its toolbar
- `onFullscreenExited` — the Viewer left fullscreen and restored its toolbar
- `onFindOpened` — the Find bar opened
- `onFindClosed` — the Find bar closed
- `onSplitModeChanged` — `SplitMode` changed between one view and two
- `onPrintComplete` — the OS print handoff finished — only the OS dialog knows, so this is what IT reported
- `onPrintCancelled` — the user cancelled the OS print dialog
- `onShareComplete`
- `onShareCancelled`
- `onSaveComplete` — Save As finished writing the document's original bytes
- `onSaveCancelled` — the user cancelled the save dialog
- `onConversationCreated` — `NewConversation()` archived the open conversation and cleared the pane — which is already empty when this fires
- `onConversationSelected` — `SelectConversation(id)` made a past conversation current, carrying its id in `ConversationId`. This is your cue to send that conversation's content back with the Append methods: the control holds none of it
- `onContentRendered` — appended content finished LAYING OUT — not merely being accepted
- Plus the universal events: `onClick`, `onDblClick`, `onDoubleClick`, `onRightClick`, `onMiddleClick`, `onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onMouseWheel`, `onContextMenu`, `onGotFocus`, `onLostFocus`, `onKeyDown`, `onKeyUp`, `onKeyPress`, `onEnterPressed`, `onEscapePressed`, `onHoverEnter`, `onHoverLeave`, `onResize`, `onResized`, `onMove`, `onMoved`, `onVisibleChanged`, `onEnabledChanged`, `onLoad` (see "Universal events").

### Methods
All controls support the universal methods (see the Control Methods Reference): `Show()`, `Hide()`, `Enable()`, `Disable()`, `SetFocus()`, `BringToFront()`, `SendToBack()`, `MoveTo(x, y)`, `Resize(w, h)`, `SetProperty(name, value)`, `GetProperty(name)`, `SetColor(color)`, `Refresh()`, and the property accessor forms `GET-<Prop>()` / `SET-<Prop>(value)`.
Type-specific methods:

- `LoadBytes(data: String)` — Open a document from bytes your program already holds, instead of from a file (`Source`). The format is resolved from the CONTENT, so a PDF, a PNG or Markdown all open correctly without a filename to go on. A COBOL item is fixed-length: `PIC X(100)` holding 42 characters delivers 100, padded — size the item to the document.
- `SaveAs(path: String)` — Write the document's ORIGINAL BYTES to `path`, unmodified — a copy, never a rendered or re-encoded document. Called with NO argument it asks the operator instead: the platform's own Save panel opens, with R18.1's proposed filename in the box — the document's own name when it came from a `Source`, and otherwise the first three words of its content plus the extension its `Format` implies. Dismissing the panel writes nothing and raises `onSaveCancelled`, which is not an error. The toolbar's Save As button is the same request. This matters most for a PDF, where the Viewer reads the file's structure in order to paint it and could so easily save that reading instead; it saves the FILE. From COBOL the path you give is always the path written: the proposed default filename is the interactive dialog's convenience, not this method's contract. Raises `onSaveComplete`, or `onError` with `LastError` set.
- `SaveAsPdf(path: String)` — Write what the Viewer shows as a PDF: under `Layout = Streamed` the CONVERSATION, otherwise a Markdown or text document; a PDF document is copied as it stands, and any other format raises `onError` saying what the method takes. Called with NO argument it asks the operator, through the same Save panel as `SaveAs()`, proposing `conversation.pdf` (or the document's own name as `.pdf`) and offering the PDF type; dismissing it writes nothing and raises `onSaveCancelled`. The PDF is what the Viewer PAINTS — its own painter lays the content out at an A4 page's width and every shape becomes the same shape in the PDF: bubbles in their colours, an HTML answer's CSS (backgrounds, gradients, borders, rounded corners, shadows, flex rows and grids), tables, code, a Mermaid diagram as its picture, the same fonts and emoji; the text stays selectable and searchable, fonts are embedded as subsets (a few pages is tens of kilobytes), and pages break between lines. Raises `onSaveComplete`, or `onError` with `LastError` set.
- `Print()` — Hand the document to the operating system's own print path — its dialog and its spooler. The Viewer implements no printing of its own. `onPrintComplete` or `onPrintCancelled` follows, from what the OS reported: only its dialog knows whether the user went through with it.
- `Find(text: String)` — Open the Find bar, optionally seeding the query, and start at the first match. Everything Find can do is COBOL-callable: no Find capability is reachable only by mouse.
- `FindNext()` — Move to the next match, wrapping past the last back to the first. With no matches it does nothing — that is not an error.
- `FindPrevious()` — Move to the previous match, wrapping past the first back to the last.
- `FindClose()` — Close the Find bar. Raises `onFindClosed`.
- `AppendHtml(content: String, role: String?)` — Conversation mode (§8.2). `role` (optional, on every Append method) says who the message is from: `"user"` draws it in a bubble on the right (`UserBubbleColor` / `UserBubbleTextColor`), `"agent"` (or `"assistant"`) in a bubble on the left (`AgentBubbleColor` / `AgentBubbleTextColor`); without one the message spans the pane as before. Streamed chunks (`AppendToMessage`) keep the message's role. Add a new message, rendered as HTML through the same subset renderer a `.html` document gets. The mode is stated by the call and NEVER inferred from the content. Only the new message is laid out — appending to a two-thousand-message conversation costs one layout pass, not two thousand and one.
- `AppendMarkdown(content: String, role: String?)` — Conversation mode. Add a new message, rendered as Markdown. `role`: `"user"` or `"agent"` draws it in that one's bubble (see `AppendHtml`).
- `AppendRaw(content: String, role: String?)` — Conversation mode. Add a new message shown LITERALLY: markup inside it is displayed, never interpreted, however valid it is, and its whitespace is preserved exactly. This is the safe mode for text your program did not write.
- `AppendToMessage(messageId: String, content: String, mode: String)` — Conversation mode. Extend a message already on screen, by its own id — what a streamed reply arriving a token at a time needs. `mode` is `Html`, `Markdown` or `Raw`. Consecutive chunks in the same mode are merged before layout, so a reply arriving word by word is one block rather than hundreds. An id no message carries raises `onError` rather than silently minting a new message.
- `ReplaceMessage(messageId: String, content: String, mode: String)` — Conversation mode. REPLACE a message's content, keeping its id, its role and its place — `mode` as for `AppendToMessage`. What a status bubble needs: append it once (`AppendMarkdown("*Thinking…*", "agent")` returns its id), change it as the work moves on, and finally replace it with the answer, so the reader sees one bubble that becomes the reply rather than a trail of status messages. Only that message is laid out again. An unknown id sets `LastError` and raises `onError`.
- `RemoveMessage(messageId: String)` — Conversation mode. Take a message out of the conversation. An unknown id sets `LastError` and raises `onError`.
- `NewConversation()` — Conversation mode (§8.8). File the open conversation into history under `ConversationId`, clear the pane, and raise `onConversationCreated` — in that order, so a handler bound to the event sees an empty pane. On a pane that is ALREADY empty it does nothing at all: no history entry, no event.
- `SelectConversation(id: String)` — Conversation mode. File the open conversation into history, take `id` back OUT of history to become current, clear the pane and raise `onConversationSelected` carrying that id. The control restores nothing: history holds an id and a title and no content, so this is your cue to send that conversation back with the Append methods.
- `RegisterConversation(id: String, title: String)` — Conversation mode. Seed a history entry for a conversation left over from an earlier run, so your own UI can list it. Its content, like every entry's, is fetched on selection and never held by the control.
- `JumpToLatest()` — Conversation mode (§8.4). Scroll to the end of the conversation, clear the new-content indicator and turn automatic following back on — all three.

---

