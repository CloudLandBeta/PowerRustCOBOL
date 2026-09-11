---
name: installer-look
description: THE reference for how PowerRustCOBOL's installers are made to look — the one-source artwork compositor, the custom WiX wizard that lets the mascot face right, the .dmg window layout and licence gate, and every constraint and trap that shaped them. Read BEFORE touching any installer artwork, any WiX dialog, the .dmg step, or the .deb/.rpm packaging text — and before changing the mascot image. The look is settled; this is how to reproduce it, not a menu of options.
---

# The PowerRustCOBOL installer look

**Settled with the operator on 2026-09-11, at 1.65.113.** Every installer shows
the same picture: **dark edge to edge, the mascot on the right, the product's
own words in white on the left.** Reproduce that. Do not redesign it.

The reason this file exists is that three of the four installer formats fight
that picture, each for a different reason, and the fixes are not guessable.

---

## 1. The rule that decides every layout

> **The mascot lives on the dark ground. Anything the HOST draws lands on
> something it can be read against.**

Each surface has text or labels drawn by the platform, not by us, at
coordinates and in colours nothing we ship can change. Work out where that is
*first*; the artwork follows.

| Surface | Who draws text, where | Consequence |
|---|---|---|
| `.msi` welcome / finish | **us** — the dialogs are ours (§3) | dark edge to edge, mascot right, white copy left |
| `.msi` banner | WiX, **black**, at x ≈ 20 px | light panel on the left, mascot right |
| `.dmg` window | Finder, **dark icon labels**, wherever the icons are | dark, mascot right, a light **shelf** under the icons |
| `.deb` / `.rpm` | nobody — there is no UI | no artwork is possible at all |

**`.deb` and `.rpm` have no installer window.** `apt`, `dnf` and the graphical
software centres *are* the installer, and none of them will show artwork or a
licence gate for a package. Do not try. What a package controls is its text and
where its licence lands — `/usr/share/doc/powerrustcobol/copyright` for the
`.deb`, `/usr/share/licenses/powerrustcobol/LICENSE` for the `.rpm`.

---

## 2. One image in, three out

`crates/cobolt-media/examples/installer_art.rs` composes **every** piece of
installer artwork from the single `assets/images/chibi.png`:

```bash
cargo run --release -p cobolt-media --example installer_art -- \
  assets/images/chibi.png dist/art
```

| Out | Size | Used by |
|---|---|---|
| `wix-dialog.bmp` | 493 × 312 | the two custom `.msi` dialogs |
| `wix-banner.bmp` | 493 × 58 | every other `.msi` screen |
| `dmg-background.png` | 1100 × 800 | the `.dmg` volume window |

Each `.bmp` is written again as a `.png` beside it — WiX takes the BMP, and
nothing outside Windows will open one, so artwork nobody can look at is artwork
nobody will fix.

**Keep it that way.** No second copy of the mascot, no hand-drawn bitmap
checked in, no image tool on the runner. It is Rust, it runs from a workspace
the job has already built, and it costs one small example compile.

**No text is rasterised.** There is no font in this repository and adding one
to bake a wordmark would collide with the text each host draws anyway. If a
surface needs words, they come from that surface's own text controls.

### Traps this compositor already solved — do not reintroduce them

- **The mascot carries a baked contact shadow** drawn for a *light* page. On the
  dark ground it reads as a grey smudge. It cannot be cropped: measured on
  `chibi.png` it spans rows 530–550 while the boots reach 540. `drop_contact_shadow`
  removes it for what it is — near-grey and light, in the bottom sixth of the
  frame. A new mascot image needs this re-measured, not assumed.
- **A chevron is not two diagonals.** Clamp the arms to one side of the vertex
  or it renders as an ✕. It did.
- **Look at the output.** Both of the above were invisible in code review and
  obvious in the first render.

---

## 3. Windows: the wizard is ours, and that is the whole point

**WixUI's own welcome and finish dialogs draw their text in black at x ≥ 180 px,
and no property moves a stock control or recolours one.** Use them and the
artwork is pinned to a light right-hand panel with the mascot on the left —
the mirror of the agreed look. This was tried, shipped in 1.65.112, and
rejected.

So the two screens carrying the big bitmap are authored in the workflow:

- **`PrcWelcomeDlg`** — wordmark, `THE NEXT GENERATION COBOL RAD IDE` in ember,
  the welcome copy, the version bottom-left. All `Transparent="yes"` over the
  full-dialog `Bitmap` control, each naming a white style inline as `{\Style}`.
- **`PrcExitDlg`** — same frame; this is where the **build prerequisites** are
  named, and it carries the "open the Build Tools page" checkbox as a real
  `CheckBox` control (the `WIXUI_EXITDIALOGOPTIONAL*` properties only exist for
  WixUI's own exit screen).

**Everything else stays WixUI's** — licence, install folder, ready, progress,
error and maintenance dialogs — reached through the same sequence
`WixUI_InstallDir` uses, copied into our `<UI Id="PrcUI">`. Do not fork more
dialogs than the two that need it.

Details that matter:

- New `TextStyle` **ids** (`PrcMark`, `PrcSub`, `PrcTitle`, `PrcBody`, `PrcVer`).
  Redefining `WixUI_Font_Normal` collides with `WixUI_Common`.
- `<UIRef Id="WixUI_Common" />` supplies the common dialogs and fonts; our
  `<InstallUISequence>` `Show` overrides its exit dialog.
- `candle`/`light` need `-ext WixUIExtension -ext WixUtilExtension`.
- The licence screen needs an **RTF**, generated from the repository's own
  `LICENSE` at build time. Apache-2.0 is pure ASCII, so the only escaping is
  RTF's own three metacharacters.
- **Every string in the WXS stays ASCII.** One em dash in a summary field cost a
  whole run (`LGHT0311`, codepage 1252) — see 1.65.107.
- **MSI dialogs are a fixed size.** 493 × 360, set at build time; no property
  reads the display. A size expressed as a share of the screen cannot be
  expressed in MSI at all. The operator chose the native size over a larger
  fixed dialog a 1366×768 laptop would clip.

---

## 4. macOS: the same picture, and three traps

Dark edge to edge, mascot right — plus one light **shelf** behind the two icons,
because Finder paints its icon labels dark and nothing a disk image carries can
recolour them. The shelf is sized to the icon positions the AppleScript sets.

- **Finder can only address a volume as `disk "Name"` when it is at
  `/Volumes/Name`.** Mounting to a `-mountpoint` temp directory fails with
  *Can't get disk*. Read the real mount point back from what `attach` reports —
  macOS renames a clashing volume.
- **Finder scripting can block forever** on an automation-permission prompt a
  headless runner will never answer. Verified by hanging on a developer Mac.
  It runs under a **watchdog**: two minutes, then killed, and the job logs a
  warning and ships an unstyled — but perfectly installable — image.
- **The licence gate is real here.** `LPic` / `STR#` / `TEXT` resources applied
  with `hdiutil udifrez`, and **macOS refuses to mount the volume until Agree is
  clicked**. This is the only place on any platform where acceptance can be
  *required* rather than offered. The `LPic` and `STR#` blobs are fixed
  constants; only `TEXT` is built, from `LICENSE`, CR-terminated.
- **Never `yes | hdiutil attach`.** An agreement-bearing image mounts unattended
  when stdout is not a terminal, and the pipe would hand the step a SIGPIPE
  under `pipefail` — the trap that cost three runs on the `.deb`
  ([[pipefail-fails-a-check-that-passed]]).

---

## 5. Before you call it done

- [ ] Ran the compositor and **looked at all three images**.
- [ ] The mascot is whole — not clipped by its frame, no shadow smudge.
- [ ] Nothing the host draws lands on ground it cannot be read against.
- [ ] The WXS is ASCII throughout.
- [ ] `ruby -ryaml -e 'YAML.load_file(...)'` parses the workflow, and
      `bash -n` accepts every step's `run` extracted from it.
- [ ] The XML of the generated `product.wxs` parses.
- [ ] A runner has actually built it. **Custom WiX dialogs and Finder layout
      cannot be verified anywhere else** — local checks prove syntax, not that
      WiX accepts the dialog set or that Finder laid the window out.
