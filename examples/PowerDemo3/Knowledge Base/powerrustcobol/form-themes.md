<!-- powerrustcobol-kit: 1.80.100 -->
# PowerRustCOBOL Form Themes

## What a form theme is

A **form theme** decides how a form's surfaces are painted: fills, borders,
corner radii, relief and shadow, and the colours of text and structural
surfaces. It is chosen **per form** (or per project) by a catalogue **id**, and
it applies to that whole form. There is no operation that applies a theme to a
single control, and a theme is never "installed" onto controls one at a time.

Two kinds exist:

- **Procedural** — drawn entirely in code. `liquid-glass`, `elegance` and `spatial`.
- **Asset pack** — composited from 9-slice images in `assets/themes/<id>/`,
  described by a `theme.toml` manifest. Packs are discovered at start-up, so a
  new one is a drop-in with no code change.

A theme answers only the questions it wants to. Anything it leaves unanswered
falls back to Liquid Glass, which is why a partial theme is a legitimate theme.

## The theme catalogue

| id | Display name | Kind | Self-contained |
|---|---|---|---|
| `liquid-glass` | Liquid Glass | Procedural | no |
| `elegance` | Elegance | Procedural | **yes** |
| `spatial` | Spatial | Procedural | **yes** |
| `neumorphic` | Neumorphic | Asset pack | declared in its manifest |
| `cobalt-steel` | Cobalt Steel | Asset pack | declared in its manifest |

`liquid-glass` is the default look and the base every other theme falls back to.
`elegance` is flat slate surfaces with a cool accent family, drawn in code.
`spatial` is warm grey translucent glass with white text and large radii, in
the manner of spatial-computing interfaces, and it is the one **see-through**
theme: the form's window is created transparent, its backdrop is the
theme's translucent glass (a solid `BackgroundColor`, gradient or picture on
the form is set aside, since it would hide the desktop), and the operating
system blurs the desktop behind the window and every other window of the
application. Blur works on macOS and Windows, and on Linux where the
compositor offers it (KDE on Wayland); elsewhere the window is see-through
without blur. White text reads best over a darker wallpaper. Pair it with
`BorderStyle` `Glow` for the corner-lit glass edge.

**An unknown id, an empty id, or no selection at all resolves to
`liquid-glass`.** Nothing fails and nothing is reported: that is the fallback
rule, not an error path. Do not tell a developer that a theme id was rejected.

## Selecting a theme — the resolution order

The effective theme is the first of these that is set:

1. the **form's** own `Theme` property, if non-empty;
2. the **project** default — `theme` under `[forms]` in the project manifest;
3. `liquid-glass`.

Whitespace counts as unset. Set the form-level override like any other form
property:

```json
{ "op": "set_property", "control_id": "Form", "key": "Theme", "value": "elegance" }
```

`UseThemeBackground` (form-level, Boolean, default false) opts the form into the
theme's own background art. While it is **false** the form's `BackgroundColor` /
background image apply; while it is **true** and the active pack supplies a
background, the pack's art replaces the form's own — on the designer canvas and
at run time alike.

## Self-contained themes and GlassStyle

`GlassStyle` selects a **Liquid Glass recipe** — `"Classic"`, `"Enhanced"`,
`"Neumorphic Light"`, `"Neumorphic Dark"`. It is therefore a setting *of Liquid
Glass*, not a general style register.

A theme that declares itself **self-contained** owns the whole look, and Liquid
Glass's ambient configuration — the `GlassStyle` register, its frost, its
neumorphic relief — is excluded from every control that theme paints. `elegance`
is self-contained; an asset pack declares `self_contained` in its manifest.

Consequences worth stating before a developer is surprised by them:

- On a self-contained theme the IDE **disables** the `GlassStyle` setting.
  Setting it anyway is stored and has **no visual effect**, and it changes
  nothing else about the form or its controls.
- Never advise changing `GlassStyle` to fix an appearance problem on such a
  form. It cannot be the cause and it cannot be the cure.

## What a theme never overrides

A theme supplies **defaults**, never overrides. The developer's own explicit
control properties always win — under every theme and every `GlassStyle`:

`BackgroundColor`, `ForegroundColor`, `CornerRadius`, `Transparency`, and the
whole `Shadow*` family.

In particular `ShadowEnabled` draws a drop shadow under every theme and every
`GlassStyle` value. If a shadow is missing, the cause is the shadow properties,
not the theme.

## Asset-pack themes — the `theme.toml` manifest

A pack is a folder `assets/themes/<id>/` holding a `theme.toml` manifest plus
its art. The manifest is the pack's public contract:

```toml
id = "cobalt-steel"
display_name = "Cobalt Steel"
self_contained = true

[background]
image = "background.png"      # optional themed background
tile  = false

[palette]
foreground = "#dfe7ff"                                  # default text colour
chart = ["#4C9BE8", "#E87A4C", "#4CE87A", "#E84C9B"]    # chart data palette

[chart_style]
stroke_width = 2.0
fill_texture = "chart_fill.png"    # optional material fill for bars and slices

[controls.button]
image    = "button.png"
slice    = [12, 12, 12, 12]        # 9-slice insets: left, top, right, bottom
hover    = "button_hover.png"      # optional per-state art
pressed  = "button_pressed.png"
disabled = "button_disabled.png"
focused  = "button_focused.png"
```

The 9-slice insets keep corners fixed while edges and centre stretch, so one
image skins a control at any size. States other than `Normal` fall back to
`Normal` when the pack does not provide them, and a control the pack does not
cover is painted by Liquid Glass. A pack id that collides with a built-in id
loses: the built-in wins.
