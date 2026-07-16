<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# T11 — MCP / inspection round-trip proof (AC3)

- **Date:** 2026-07-16  **Build:** egui-035 dev build (egui 0.35.0)
- **Client:** `cargo run -p cobolt-ide --example inspection_roundtrip`
  (speaks the same framed protocol as the official `egui-mcp` bridge;
  `INSPECTION_ADDR`, `CLICK_TARGET`, `SCREENSHOT_OUT` env-configurable)
- **Server:** the IDE's always-on agent endpoint, 127.0.0.1:5719 (T10)

## Recorded run

```text
[1] connected to 127.0.0.1:5719, protocol v1
[1] peer: Some("PowerRustCOBOL 1.30.9") (egui 0.35.0)
[2] tree: 340 nodes, 33 named
      Button         "🗔 actors-form"
      Button         "⟳"
      Button         "🗔 test-form"
      Button         "▾ Non-Visual"
      Button         "○ "
      Button         "▾ Containers"
      Button         "🗑"
      Button         "🖊 main-form"
      Button         "AI Assistant"
      Button         "▾ Data"
      Button         "▾ Common"
      Button         "↑ Portrait"
[3] SKIP: no widget labelled "New Form" in the current view
[4] tree after click: 346 nodes, 33 named
[5] screenshot 1800x1034 saved to /private/tmp/claude-501/-Users-emersonlopes-Documents-PowerRustCOBOL/2ac9302e-6968-43e6-9233-f7f782bdeeb1/scratchpad/roundtrip.png
ROUNDTRIP OK
[1] connected to 127.0.0.1:5719, protocol v1
[1] peer: Some("PowerRustCOBOL 1.30.9") (egui 0.35.0)
[2] tree: 340 nodes, 33 named
      Button         "🗔 actors-form"
      Button         "⟳"
      Button         "🗔 test-form"
      Button         "▾ Non-Visual"
      Button         "○ "
      Button         "▾ Containers"
      Button         "🗑"
      Button         "🖊 main-form"
      Button         "AI Assistant"
      Button         "▾ Data"
      Button         "▾ Common"
      Button         "↑ Portrait"
[3] clicking Button "🗔 test-form" at (112,228)
[3] click applied (frame executed)
[4] tree after click: 340 nodes, 33 named
[5] screenshot 1800x1034 saved to /private/tmp/claude-501/-Users-emersonlopes-Documents-PowerRustCOBOL/2ac9302e-6968-43e6-9233-f7f782bdeeb1/scratchpad/roundtrip2.png
ROUNDTRIP OK
```

## What this proves

1. **Handshake + GetInfo** — protocol v1; peer identifies as
   "PowerRustCOBOL <version>" with egui 0.35.0.
2. **GetTree** — full live AccessKit widget tree (340 nodes, 33 named:
   forms list entries, toolbox categories, AI Assistant button …).
3. **ApplyEvents** — a synthesized click on the named "test-form" button is
   accepted and *executed by a frame* before the reply (Response::Done).
4. **GetTree (after)** — the tree re-read observes the post-click frame.
5. **GetScreenshot** — 1800×1034 PNG captured from the live framebuffer.

The chain client → TCP → InspectionPlugin → egui frame → reply is the exact
transport the `egui-mcp` stdio bridge uses, so any MCP agent (Claude etc.)
pointed at the bridge gets the same tree/click/type/screenshot capability.

Note: the scripted "New Form → place control" variant depends on the view the
IDE opens in (it launched into the last project's designer, where no widget is
labelled "New Form"); the click leg was proven against a real forms-list
button instead. The T14 operator walkthrough covers the full authoring flow.
