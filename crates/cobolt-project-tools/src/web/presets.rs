// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The device sizes the web Preview offers** (WASM-008 R4).
//!
//! A preset is a class of device, not a product: a phone, a tablet, a desktop,
//! each at a few widths, with the pixel ratio and the safe-area insets such a
//! screen has in portrait. Choosing one, and an orientation, gives the
//! [`Viewport`] the Preview shows the page in — its size in CSS pixels, its
//! `devicePixelRatio` and the insets the page's `env(safe-area-inset-*)` should
//! read. Nothing here resizes a window; the Preview does that only when the
//! developer picks a preset or drags the window's edge.

use cobolt_compiler::project_kind::{DeviceClass, ProjectKind};

/// The class of screen a preset stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetClass {
    Phone,
    Tablet,
    Desktop,
}

/// The insets of the screen that the system draws over, in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SafeArea {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

/// A device size: portrait width and height in CSS pixels, the pixel ratio and
/// the portrait insets. A desktop preset is landscape by nature, so for it the
/// "portrait" figures are the wide ones and an orientation swap gives the tall
/// window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DevicePreset {
    pub id: &'static str,
    /// English name; the IDE shows its own translation keyed by `id`.
    pub name: &'static str,
    pub class: PresetClass,
    pub width: u32,
    pub height: u32,
    pub dpr: f32,
    pub safe_area: SafeArea,
}

/// What the page is shown in: a preset in an orientation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub dpr: f32,
    pub safe_area: SafeArea,
    pub landscape: bool,
}

const fn sa(top: u32, right: u32, bottom: u32, left: u32) -> SafeArea {
    SafeArea { top, right, bottom, left }
}

/// The presets, smallest first.
pub const PRESETS: &[DevicePreset] = &[
    DevicePreset { id: "phone-compact", name: "Phone, compact", class: PresetClass::Phone, width: 360, height: 800, dpr: 3.0, safe_area: sa(24, 0, 16, 0) },
    DevicePreset { id: "phone", name: "Phone", class: PresetClass::Phone, width: 390, height: 844, dpr: 3.0, safe_area: sa(47, 0, 34, 0) },
    DevicePreset { id: "phone-large", name: "Phone, large", class: PresetClass::Phone, width: 430, height: 932, dpr: 3.0, safe_area: sa(59, 0, 34, 0) },
    DevicePreset { id: "tablet", name: "Tablet", class: PresetClass::Tablet, width: 820, height: 1180, dpr: 2.0, safe_area: sa(24, 0, 20, 0) },
    DevicePreset { id: "tablet-large", name: "Tablet, large", class: PresetClass::Tablet, width: 1024, height: 1366, dpr: 2.0, safe_area: sa(24, 0, 20, 0) },
    DevicePreset { id: "desktop", name: "Desktop", class: PresetClass::Desktop, width: 1280, height: 800, dpr: 1.0, safe_area: sa(0, 0, 0, 0) },
    DevicePreset { id: "desktop-wide", name: "Desktop, wide", class: PresetClass::Desktop, width: 1920, height: 1080, dpr: 1.0, safe_area: sa(0, 0, 0, 0) },
];

/// The pixel ratios the Preview offers besides a preset's own.
pub const PIXEL_RATIOS: &[f32] = &[1.0, 1.5, 2.0, 3.0];

/// The preset named `id`.
pub fn preset(id: &str) -> Option<&'static DevicePreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// The preset a project of this kind opens its Preview in: a Web page on a
/// desktop, a Mobile phone or tablet on that device.
pub fn default_for(kind: ProjectKind, device: Option<DeviceClass>) -> &'static DevicePreset {
    let id = match (kind, device) {
        (ProjectKind::Mobile, Some(DeviceClass::Tablet)) => "tablet",
        (ProjectKind::Mobile, _) => "phone",
        _ => "desktop",
    };
    preset(id).expect("the default presets exist")
}

impl DevicePreset {
    /// The viewport of this preset in an orientation. A phone or tablet is
    /// portrait by default and turns to landscape; a desktop is landscape by
    /// default and turns to portrait. In landscape the insets rotate with the
    /// screen: the notch side becomes left and right, the top clears.
    pub fn viewport(&self, landscape: bool) -> Viewport {
        let natural_landscape = self.class == PresetClass::Desktop;
        let turn = landscape != natural_landscape;
        let (width, height) = if turn { (self.height, self.width) } else { (self.width, self.height) };
        let safe_area = if self.class == PresetClass::Desktop || !landscape {
            self.safe_area
        } else {
            SafeArea {
                top: 0,
                right: self.safe_area.top,
                bottom: self.safe_area.bottom * 6 / 10,
                left: self.safe_area.top,
            }
        };
        Viewport { width, height, dpr: self.dpr, safe_area, landscape }
    }
}

impl Viewport {
    /// The same viewport at another pixel ratio.
    pub fn with_dpr(mut self, dpr: f32) -> Self {
        self.dpr = dpr;
        self
    }

    /// The window the page is shown in, in the points the operating system
    /// counts: the CSS size, which is what a web view's window measures.
    pub fn window_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_are_distinct_and_the_defaults_exist() {
        let mut ids: Vec<&str> = PRESETS.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PRESETS.len(), "ids are unique");
        for p in PRESETS {
            assert!(p.width >= 320 && p.height >= 320 && p.dpr >= 1.0, "{}", p.id);
        }
        assert_eq!(default_for(ProjectKind::Web, None).id, "desktop");
        assert_eq!(default_for(ProjectKind::Mobile, Some(DeviceClass::Phone)).id, "phone");
        assert_eq!(default_for(ProjectKind::Mobile, Some(DeviceClass::Tablet)).id, "tablet");
    }

    /// WASM-008 AC3: each preset and each orientation gives the viewport's size
    /// and `devicePixelRatio` as set.
    #[test]
    fn each_preset_in_each_orientation_gives_its_size_and_ratio() {
        let mut checked = 0;
        for p in PRESETS {
            let portrait = p.viewport(false);
            let landscape = p.viewport(true);
            assert!(!portrait.landscape && landscape.landscape);
            // The two orientations are the same screen turned.
            assert_eq!((portrait.width, portrait.height), (landscape.height, landscape.width), "{}", p.id);
            if p.class == PresetClass::Desktop {
                assert!(landscape.width > landscape.height, "{}: a desktop is wide by nature", p.id);
                assert_eq!((landscape.width, landscape.height), (p.width, p.height));
            } else {
                assert!(portrait.height > portrait.width, "{}: a phone or tablet is tall by nature", p.id);
                assert_eq!((portrait.width, portrait.height), (p.width, p.height));
            }
            assert_eq!(portrait.dpr, p.dpr);
            assert_eq!(landscape.dpr, p.dpr);
            checked += 2;
        }
        println!("presets: {} presets x 2 orientations = {checked} viewports, each the same screen turned, with its pixel ratio", PRESETS.len());
    }

    #[test]
    fn a_phones_insets_turn_with_it() {
        let p = preset("phone").unwrap();
        let portrait = p.viewport(false).safe_area;
        assert_eq!(portrait, SafeArea { top: 47, right: 0, bottom: 34, left: 0 });
        let landscape = p.viewport(true).safe_area;
        assert_eq!(landscape.top, 0, "the top clears when the phone is on its side");
        assert_eq!((landscape.left, landscape.right), (47, 47), "the notch side becomes the sides");
        assert_eq!(landscape.bottom, 20, "the home indicator stays at the bottom, shorter");
        // A desktop has none in either.
        let d = preset("desktop").unwrap();
        assert_eq!(d.viewport(true).safe_area, SafeArea::default());
        assert_eq!(d.viewport(false).safe_area, SafeArea::default());
    }

    #[test]
    fn the_pixel_ratio_can_be_set_apart_from_the_preset() {
        let v = preset("phone").unwrap().viewport(false).with_dpr(2.0);
        assert_eq!(v.dpr, 2.0);
        assert_eq!(v.window_size(), (390, 844));
        assert!(PIXEL_RATIOS.contains(&1.0) && PIXEL_RATIOS.contains(&3.0));
    }
}
