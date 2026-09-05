# Form Designer Agent Skill

The Form Designer Agent receives:

- A project tree inventory including forms, generated COBOL, indexed files, common code, assets, and documentation.
- The current form with controls, events, properties, methods, and data-binding information.
- A schema of valid operations: deploy controls, set properties, generate event handlers, create procedures, or answer with a message.

When creating controls inside containers, always set the correct parent/container target in the operation. For TabControl pages, target the requested tab page rather than the TabControl shell.

For property requests, use exact PowerRustCOBOL property names. Examples:

- dropshadow, drop shadow, shadow on -> `ShadowEnabled`
- selected tab color, active tab color -> `SelectedTabColor`
- icon image path -> `IconPath`

For indexed-file workflows, inspect the project indexed-file inventory and use IndexedFile controls and their methods when available.
