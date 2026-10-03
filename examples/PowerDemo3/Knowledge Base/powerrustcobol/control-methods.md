<!-- powerrustcobol-kit: 1.80.100 -->
# PowerRustCOBOL Control Methods Reference

Inline methods are invoked as `<control>::<Method>(args).` — as a statement, as an expression operand (`MOVE C::GetText() TO WS-X`), or with `RETURNING`. The method vocabulary is CLOSED: only the methods below (plus the `GET-`/`SET-` accessor forms) are recognised. Anything else is treated as a PROPERTY access — `C::Foo(x)` with an unknown `Foo` writes property `Foo`, it does not call a method. Do not invent methods.

## Parameter conventions

- `String` — a COBOL literal or data item; values are trimmed.
- `Integer` / `Number` — numeric literal or numeric data item.
- `Boolean` — `1`/`0` preferred; `true`/`yes`/`on` (any case) also count as true.
- Missing arguments default to empty / 0 — pass every listed argument unless marked optional (`?`).
- Methods that return a value can be used inline or with `RETURNING`; the value is a number when it parses as one, otherwise a string.

## Universal (every control)

- `Show() / Hide()` — Set `Visible` to 1 / 0.
- `Enable() / Disable()` — Set `Enabled` to 1 / 0.
- `SetFocus() (alias Focus())` — Give the control keyboard focus.
- `BringToFront() / SendToBack()` — Jump the control to the top / bottom of the z-order.
- `MoveTo(x: Integer, y: Integer)` — Move to form coordinates.
- `Resize(width: Integer, height: Integer)` — Resize in pixels.
- `SetProperty(name: String, value: Any)` — Write any property by name (also reaches User-Control children as `"Child.Prop"`).
- `GetProperty(name: String) → value` — Read any property by name.
- `SetColor(color: String)` — Set `ForegroundColor` (hex color).
- `Refresh()` — No-op on most controls; on charts re-sends the current data.
- `Validate()` — No-op (accepted for compatibility).
- `GET-<Prop>() → value / SET-<Prop>(value)` — Explicit accessor for ANY property, e.g. `C::GET-Width()`, `C::SET-Caption("Hi")`.
- `<Prop>() → value / <Prop>(value)` — Bare property name: no args reads, one arg writes.

## Caption controls (Button, Label, CheckBox, RadioButton, GroupBox)

- `SetCaption(text: String) / GetCaption() → String` — Write / read the `Caption`.

## Text controls (TextBox)

- `SetText(text: String) / GetText() → String` — Write / read the `Text`.
- `AppendText(text: String)` — Append to the text.
- `Clear()` — Empty `Text` (and `Items`).
- `SelectAll()` — Give the TextBox the keyboard focus and select its whole text.

## Check controls (CheckBox, RadioButton)

- `IsChecked() → Boolean (0/1)` — Read the checked state.
- `SetChecked(value: Boolean)` — Write the checked state.
- `Select()` — Check; a RadioButton also unchecks its GroupName siblings.
- `Toggle()` — Flip the checked state.

## Value controls (ProgressBar, Slider, NumericUpDown, DateTimePicker)

- `SetValue(value) / GetValue() → value` — Write / read `Value`.
- `Increment() / Decrement()` — Add / subtract `Step` (default 1).
- `Reset()` — Return `Value` to `Minimum` (or 0).

## Item lists (ListBox, ComboBox, ToolBar, StatusBar)

`Items` is a newline-separated list; indexes are 0-based.

- `AddItem(text: String)` — Append one item.
- `RemoveItem(text: String)` — Remove the first matching item.
- `GetSelected() → String` — The selected value.
- `GetSelectedIndex() → Integer (alias GetIndex())` — The selected index, -1 = none.
- `SetSelectedIndex(index: Integer) (alias SetIndex())` — Select by index.
- `GetCount() → Integer` — Item count.
- `Clear()` — Remove every item.

## TreeView nodes

Every call takes the node's INDEX — its 1-based line in `Items`, the same number a node event hands the handler in `CONTROL-NODE-INDEX`. That index IS the node's handle: there is no node object to hold, because a held object would go stale the moment `Items` changed. The traversal calls RETURN an index, so they chain; -1 means there is no such node, and that is what ends a walk.

- `AddNode(level, text [, icon, color, background])` — Append a node at `level` (0 = root). The level is a number, not leading spaces — `AddItem` trims its argument.
- `NodeCount() → Integer` — How many nodes the tree holds.
- `NodeIndexOf(text) → Integer` — The index of the first node with that label, -1 when none.
- `NodeText(index) → String (alias NodeName)` — Its label.
- `NodePath(index) → String` — Root to node joined by `/`.
- `NodeLevel(index) → Integer` — Its depth, 0 for a root.
- `NodeIcon / NodeColor / NodeBackColor (index) → String` — What the node itself carries; empty when it carries none.
- `NodeParent(index) → Integer` — The node it hangs under, -1 on a root.
- `NodeFirstChild / NodeLastChild (index) → Integer` — Its first/last direct child, -1 on a leaf.
- `NodeNextSibling / NodePrevSibling (index) → Integer` — The next/previous node at the same level under the same parent, -1 at the end.
- `NodeChildCount(index) → Integer` — Direct children only.
- `NodeHasChildren(index) → 1/0` — Whether anything hangs under it.
- `NodeChecked / NodeCollapsed (index) → 1/0` — Live state, read from `CheckedNodes` / `CollapsedNodes`.
- `RemoveNode(index) → 1/0` — Remove the node and everything under it; 0 when the index names no node.
- `ExpandAll() / CollapseAll()` — Open every node / fold every node that has children (writes `CollapsedNodes`).
- `GetSelectedNode() → String / SetSelectedNode(label)` — Read / set `SelectedNode`, the selected node's label.

## DataGrid

Rows and columns are numbered from 1 (`GetCellValue(1, 1)` is the first cell) in `GetCellValue`, `SetCellValue`, `DeleteRow`, `Sort` and `SetRowHeight(row, …)`; `AddRow` cells are TAB-separated.

- `GetRowCount() → Integer` — Data-row count.
- `GetCellValue(row, column) → String` — Read one cell.
- `SetCellValue(row, column, value)` — Write one cell.
- `AddRow(cells: String)` — Append a TAB-separated row.
- `DeleteRow(row: Integer)` — Remove a row.
- `ClearRows()` — Remove all rows.
- `Sort(column: Integer)` — Sort by column.
- `SetFilter(column: String, value: String) / ClearFilters()` — Column filtering.
- `FreezeColumns(n) / FreezeRows(n)` — Freeze leading columns/rows.
- `SetRowHeight(px) / SetColumnWidth(column, px)` — Geometry.
- `GetSelectedText() → String / CopySelection()` — Selection access.
- `ExportCSV() → String` — CSV of the grid.
- `RefreshBinding() → Integer` — Re-hydrate from the bound source (also on repeating GroupBoxes).

## Timer

- `Start() / Stop()` — Run / halt the timer (its `Enabled` property).
- `SetInterval(ms: Integer)` — Change the tick interval.
- `IsEnabled() → Boolean (0/1)` — Running state.

## Animator / animations

Any control with named animations accepts these.

- `Play() / PlayAnimation(name: String?)` — Start playback (optionally a named animation).
- `StopAnimation()` — Stop playback.
- `Pause()` — Pause playback.

## Charts (BarChart, LineChart, PieChart, AreaChart, ScatterChart, DonutChart)

Equivalent to the `COBOL::"CHART-…"` built-in calls and the generated `PERFORM <id>-ADD-POINT` paragraphs.

- `AddPoint(label: String, value: Number)` — Append one point and repaint.
- `Clear()` — Drop all pushed points.
- `Refresh()` — Repaint with current data.

## AgentObject (LLM)

- `Ask(prompt: String) → String` — Send a prompt; the reply also fires `onResponse`.
- `SetPrompt(text: String)` — Replace the `SystemPrompt`.
- `SetModel(model: String)` — Switch models.
- `GetResult() → String` — Read the `Result` property.
- `Cancel() / IsBusy() → Boolean` — Async control.
- `AddTool / AddToolParameter / RemoveTool` — Offer the model tools the program answers in `onToolCall`.
- `SetToolResult(call-id, text)` — Answer a tool call from `onToolCall`.
- `AllowFile(fd-name, cidx-path?) / DenyFile(fd-name)` — Let the model search an indexed file.
- `RegisterFile(data-path, cidx-path, name?) / UnregisterFile(name)` — Let the model search an indexed file by its path (local, network, smb://), read only.
- `AllowKnowledgeBase(kb, collection?) / DenyKnowledgeBase(kb, collection?)` — Let this agent's model search a KnowledgeBase collection, or stop it — each agent's own grant.

## KnowledgeBase (documents)

Operations that write or search are asynchronous: the method answers 1 (started) or 0 (`LastError`), then `onProgress` and `onIndexed` / `onSearchComplete` / `onBusy` / `onError` follow, each carrying its own property values.

- `CreateCollection / RemoveCollection / ListCollections / GetCollection(n)` — Manage the collections under `Location`.
- `AddDocument(name, text) / UpdateDocument(name, text) / ImportDocument(path, name?) / DeleteDocument(name)` — Change a document and its index.
- `ListDocuments / GetDocument(n)` — The collection's documents.
- `Refresh() / Reindex()` — Bring the index into agreement with the folder; re-embed everything.
- `Search(query, max?) / GetResultDocument / GetResultHeading / GetResultPassage / GetResultScore(n)` — Search, then read each hit.
- `FetchModel() / Cancel()` — Fetch the built-in model; stop the running operation.

## RestClient (HTTP)

Async by default (`Mode = "Async"`): the verb returns immediately, `Busy` = 1, and the response lands in `ResponseBody` / `StatusCode` with `onComplete` (or `onError` / `onTimeout`). With `Mode = "Sync"` the verb blocks and returns the body. The `url` argument is the FULL URL.

- `Get(url: String) → String` — HTTP GET.
- `Post(url: String, body: String) → String` — HTTP POST.
- `Put(url: String, body: String) → String` — HTTP PUT.
- `Delete(url: String) → String` — HTTP DELETE.
- `Call(verb: String, url: String, body: String?) → String` — Any verb by name.
- `SetHeader(name: String, value: String) / ClearHeaders()` — Request headers.
- `SetTimeout(seconds: Integer)` — Request timeout.
- `Cancel() / IsBusy() → Boolean` — Async control.

## SqlDatabase

Errors land in `LastError` and fire `onQueryError` / `onConnectError`.

- `Open(connectionString: String) → Integer` — Open; returns the handle, fires `onConnectOk`/`onConnectError`.
- `Execute(sql: String) → Integer (alias Exec)` — Run DML/DDL; affected rows.
- `Query(sql: String) → Integer` — Run a SELECT; result-row count.
- `Fetch() → Boolean (0/1)` — Advance the row cursor; fires `onRowFetched`.
- `FetchAll() → Integer` — Current result-set row count.
- `Close()` — Close the connection.

## Knob

Same value-controls contract as ProgressBar/Slider above (SetValue/GetValue/Increment/Decrement/Reset all apply).


## Gauge

Read-only via the UI (R10) — SetValue/GetValue are the only way to change it, no drag, no Increment/Decrement/Reset.

- `SetValue(value: Integer)` — Set the current value.
- `GetValue() → Integer` — Read the current value.

## Switch

Same check-controls contract as CheckBox/RadioButton above, minus `Select()` (no radio group).

- `IsChecked() → Boolean (0/1)` — Read the checked state.
- `SetChecked(value: Boolean)` — Write the checked state.
- `Toggle()` — Flip the checked state.

## FileDropZone

No inline methods at all — it is a pure UI gesture (drag-and-drop or a native-picker click), never invoked from COBOL.


## Maps

The OSM basemap needs no API key; only the five Google data methods below call the real Google Maps API and need a `google-maps` project credential (Settings → Integrations) — with none configured they fail immediately with `onError` (R33), never a crash or network call. `TraceRoad` is the exception in a different direction: it uses **OpenRouteService** and takes its key as an ARGUMENT, so a program with no Google credential can still trace a real road. **Every data method is ALWAYS async**: the call returns an empty string immediately, sets `Busy`, and the answer arrives in `ResponseBody` with `onComplete` (or `LastError` with `onError`). There is no Sync mode — do NOT write `MOVE Maps1::Geocode(...) TO X`, which only ever moves an empty string.

- `Geocode(address: String)` — Async. `onComplete`: `ResponseBody` = `lat\tlng\tformatted_address`.
- `ReverseGeocode(lat: String, lng: String)` — Async. `onComplete`: `ResponseBody` = the formatted address.
- `Directions(origin: String, destination: String)` — Async. `onComplete`: `ResponseBody` = `distance_text\tduration_text\troute_summary\tdistance_METRES\tduration_SECONDS\tencoded_polyline\ttraffic_SECONDS` — the numbers to COMPUTE with, the polyline for `AddRoute`, and the last field the drive time with current traffic (0 if none was supplied).
- `DistanceMatrix(origin: String, destination: String)` — Async. `onComplete`: `ResponseBody` = `distance_text\tduration_text\tdistance_METRES\tduration_SECONDS`.
- `PlacesSearch(query: String, radiusMeters: String)` — Async. `onComplete`: `ResponseBody` = one `place_id\tname\taddress\tlat\tlng` line per result.
- `TraceRoad(apiKey, fromLat, fromLng, toLat, toLng: String)` — Async, **OpenRouteService** — no Google credential. `onComplete`: `ResponseBody` = `distance_METRES\tduration_SECONDS\tencoded_polyline`. The key is the first ARGUMENT and is never stored: read it from a field the operator filled in. A blank key fires `onError` with no network call.
- `AddMarker(id, lat, lng, label, info: String)` — Append one pin to `Markers`.
- `RemoveMarker(id: String)` — Remove the marker with that id.
- `AddRoute(id, colour, width, geometry: String)` — Trace a line: an encoded polyline or `lat,lng;lat,lng;…`. Re-using an id replaces it. No API key needed.
- `RemoveRoute(id: String)` — Remove that route.
- `ClearRoutes()` — Remove every route.
- `AddRegion(id, fill, stroke, width, geometry [, label, info]: String)` — Fill an area — a sales territory. `fill` takes an alpha; the ring may be concave. `label`/`info` drive the info window on hover and click. No API key needed.
- `RemoveRegion(id: String)` — Remove that region.
- `ClearRegions()` — Remove every region.

## WebSearch

Searches the web through one of five back ends, chosen with `Provider`: Google Custom Search, Brave, Serper, Tavily, or a SearXNG instance you host. All five answer through the same accessors, so switching provider needs no COBOL change. Async by default, same `Mode`/`Busy`/`onComplete`/`onError` contract as RestClient above. Needs the chosen provider's key — the project credential (Settings → Integrations) or the control's own `ApiKey` — except SearXNG, which needs `Endpoint` instead; with neither configured `Search()` fails immediately with `onError` (R33). Prefer `Search()` over the generated `<id>-SEARCH` paragraph, which has no URL-encoding and no key.

- `Search()` — Run a search on the current Provider using Query/NumResults/SafeSearch. Async: returns immediately, raw JSON in `ResponseBody`. Sync: returns the raw JSON.
- `ResultCount() → Integer` — Result items in the last response.
- `TopTitle() / TopSnippet() / TopLink() → String` — First result's fields, empty before any search.
- `GetResult(index: Integer) → String` — 1-based indexed result as `title\tsnippet\tlink`; out-of-range → empty.
- `Cancel() / IsBusy() → Boolean` — Async control.

## Viewer (spec 058)

A document viewer: text, Markdown, images, PDF and an HTML subset, with Find, Save As, Print, split view and — under `Layout = Streamed` — an append-only chatbot conversation surface. Decoding runs off the UI thread. Every property is readable and writable too (`View1X`/`View2X` per view, with the plain names aliasing the FIRST view), so nothing here is reachable only by mouse.

- `LoadBytes(data: String)` — Open a document from bytes instead of a path; the format is resolved from the content.
- `SaveAs(path: String)` — Write the source's ORIGINAL bytes to `path` — a copy, never a re-encode. `onSaveComplete`, or `onError` with `LastError`.
- `SaveAsPdf(path: String)` — Write the conversation (`Streamed`) or a Markdown/text document as a PDF with its formatting; no argument opens the Save panel with `conversation.pdf` proposed. `onSaveComplete`, `onSaveCancelled`, or `onError` with `LastError`.
- `Print()` — Hand the document to the OS print path. `onPrintComplete`/`onPrintCancelled` — from what the OS reported.
- `Find(text: String)` — Open the Find bar and search for `text`.
- `FindNext() / FindPrevious()` — Move between matches, wrapping at both ends. No matches is a no-op, not an error.
- `FindClose()` — Close the Find bar.
- `AppendHtml(content) / AppendMarkdown(content) / AppendRaw(content: String), each with an optional role` — Conversation mode: add a message in that mode. Raw is shown literally — markup inside it is displayed, never interpreted. Only the new message is laid out. A second argument `"user"` or `"agent"` draws the message in a chat bubble — the user's on the right, the agent's on the left — coloured by `UserBubbleColor`/`UserBubbleTextColor` and `AgentBubbleColor`/`AgentBubbleTextColor`.
- `ReplaceMessage(messageId: String, content: String, mode: String) / RemoveMessage(messageId: String)` — Replace a message's content in place (same id, role and place) — a status bubble that becomes the answer — or take a message out. Unknown id → `onError`.
- `AppendToMessage(messageId: String, content: String, mode: String)` — Extend a message already on screen, by id — what a streamed reply needs. `mode` is `Html`, `Markdown` or `Raw`.
- `NewConversation()` — File the open conversation into history, clear the pane, raise `onConversationCreated`. A no-op on an already-empty pane.
- `SelectConversation(id: String)` — Make a past conversation current and raise `onConversationSelected` — your cue to send its content back with the Append methods. The control caches none of it.
- `RegisterConversation(id: String, title: String)` — Seed a history entry from an earlier run.
- `JumpToLatest()` — Scroll to the end of the conversation, clear the new-content indicator and re-enable automatic following.

## SideMenu (spec 051)

The sidebar's programmatic door to standalone child windows: the opened window is parented to the SHELL whatever form invokes the method, exactly like the sidebar's own `Open Stand Alone Form` menu actions. The target's FormFormat must be `Standalone` or `Both` (build-checked for literal ids). Space form: every parameter required; comma form: the form id alone is enough.

- `OpenStandAloneFormSync(formId, windowState: String, x, y, width, height: Integer, modal: Boolean)` — Open the form in its own window and BLOCK the calling handler until it closes — Sync is implicitly modal, the whole shell face waits. RETURNING is NULL on resume.
- `OpenStandAloneFormAsync(formId, windowState: String, x, y, width, height: Integer)` — Open the form in its own window and return at once. RETURNING binds a windowHandler (Focus/Close/SetProperty/… — NULL when the child closes). Never modal.

## IndexedFile — no inline methods

IndexedFile controls are driven by the GENERATED PARAGRAPHS (`PERFORM <id>-OPEN`, `<id>-START`, `<id>-READ-NEXT`, `<id>-READ-PREVIOUS`, `<id>-READ-FIRST`, `<id>-READ-LAST`, `<id>-READ-INVALID`, `<id>-COMMIT`, `<id>-CLOSE`) and the plain COBOL verbs `WRITE` / `REWRITE` / `DELETE`. Do NOT call `::Open()` on an IndexedFile — that method belongs to SqlDatabase.
