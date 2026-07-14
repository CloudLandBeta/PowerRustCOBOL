# PowerRustCOBOL Form Runtime API (`self`)

The runtime environment provides a `self` pseudo-object that allows interacting directly with the active form instance.

## Methods

- `self::Close()`
  Closes the current form.
- `self::OpenForm(<form-name>, <window type>, <window state>, <open effect>)`
  Opens another form.
  - **window type**: `NORMAL` or `MODAL`
  - **window state**: `NORMAL`, `MAXIMIZED`, or `MINIMIZED`
  - **open effect**: `APPEAR`, `FADE IN`, `ZOOM IN`, or `WARP` (window grows in fast speed with ephemeral tracks projected from its corners while it moves from the taskbar where the application icon is to its final position and size in the screen)
- `self::Alert("Message", <buttons>)`
  Displays an alert dialog.
  - **buttons**: `OK`, `OK, CANCEL`
- `self::Minimize()`
  Minimizes the form window to the taskbar.
- `self::Restore()`
  Restores the form window to its normal state.
- `self::Maximize()`
  Maximizes the form window.

## Properties

- `self::X` (integer): Horizontal position of the form.
- `self::Y` (integer): Vertical position of the form.
- `self::Width` (integer): Width of the form.
- `self::Height` (integer): Height of the form.
- `self::Title` (string): The title displayed on the form's title bar.
- `self::TitleBar` (string): State of the title bar. Can be `enabled` or `hidden`.
- `self::border` (string): Border style. Can be `resizable` or `fixed`.
- `self::icon` (string): Path to the window icon.
