// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The small enumerations that appear as one-byte operands, with the values
//! they have on the wire. These numbers are part of display-list version 1 and
//! never change; a new member gets the next free number.

macro_rules! wire_enum {
    ($(#[$m:meta])* $name:ident { $( $(#[$vm:meta])* $v:ident = $n:literal ),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum $name { $( $(#[$vm])* $v = $n ),+ }

        impl $name {
            /// The wire value.
            pub const fn to_u8(self) -> u8 { self as u8 }
            /// From the wire value; `None` when it names nothing.
            pub const fn from_u8(v: u8) -> Option<Self> {
                match v { $( $n => Some(Self::$v), )+ _ => None }
            }
            /// Every member, in wire order.
            pub const ALL: &'static [Self] = &[ $( Self::$v ),+ ];
        }
    };
}

wire_enum! {
    /// Which rule decides what is inside a path.
    FillRule { /// Non-zero winding (the default).
        NonZero = 0, /// Even-odd.
        EvenOdd = 1 }
}

wire_enum! {
    /// How the end of a stroked line looks.
    LineCap { /// Flat at the end point.
        Butt = 0, /// Half circle.
        Round = 1, /// Half square.
        Square = 2 }
}

wire_enum! {
    /// How two stroked segments are joined.
    LineJoin { /// Sharp corner (up to the miter limit).
        Miter = 0, /// Rounded corner.
        Round = 1, /// Flattened corner.
        Bevel = 2 }
}

wire_enum! {
    /// The compositing operation (HTML `globalCompositeOperation`).
    BlendMode {
        /// `source-over`
        SourceOver = 0,
        /// `source-in`
        SourceIn = 1,
        /// `source-out`
        SourceOut = 2,
        /// `source-atop`
        SourceAtop = 3,
        /// `destination-over`
        DestinationOver = 4,
        /// `destination-in`
        DestinationIn = 5,
        /// `destination-out`
        DestinationOut = 6,
        /// `destination-atop`
        DestinationAtop = 7,
        /// `lighter`
        Lighter = 8,
        /// `copy`
        Copy = 9,
        /// `xor`
        Xor = 10,
        /// `multiply`
        Multiply = 11,
        /// `screen`
        Screen = 12,
        /// `overlay`
        Overlay = 13,
        /// `darken`
        Darken = 14,
        /// `lighten`
        Lighten = 15,
        /// `color-dodge`
        ColorDodge = 16,
        /// `color-burn`
        ColorBurn = 17,
        /// `hard-light`
        HardLight = 18,
        /// `soft-light`
        SoftLight = 19,
        /// `difference`
        Difference = 20,
        /// `exclusion`
        Exclusion = 21,
        /// `hue`
        Hue = 22,
        /// `saturation`
        Saturation = 23,
        /// `color`
        Color = 24,
        /// `luminosity`
        Luminosity = 25,
    }
}

impl BlendMode {
    /// The HTML `globalCompositeOperation` name.
    pub const fn css_name(self) -> &'static str {
        match self {
            Self::SourceOver => "source-over",
            Self::SourceIn => "source-in",
            Self::SourceOut => "source-out",
            Self::SourceAtop => "source-atop",
            Self::DestinationOver => "destination-over",
            Self::DestinationIn => "destination-in",
            Self::DestinationOut => "destination-out",
            Self::DestinationAtop => "destination-atop",
            Self::Lighter => "lighter",
            Self::Copy => "copy",
            Self::Xor => "xor",
            Self::Multiply => "multiply",
            Self::Screen => "screen",
            Self::Overlay => "overlay",
            Self::Darken => "darken",
            Self::Lighten => "lighten",
            Self::ColorDodge => "color-dodge",
            Self::ColorBurn => "color-burn",
            Self::HardLight => "hard-light",
            Self::SoftLight => "soft-light",
            Self::Difference => "difference",
            Self::Exclusion => "exclusion",
            Self::Hue => "hue",
            Self::Saturation => "saturation",
            Self::Color => "color",
            Self::Luminosity => "luminosity",
        }
    }

    /// From the HTML name (case-insensitive); `None` if unknown.
    pub fn from_css_name(name: &str) -> Option<Self> {
        let n = name.trim().to_ascii_lowercase();
        Self::ALL.iter().copied().find(|m| m.css_name() == n)
    }
}

wire_enum! {
    /// Where a gradient or pattern applies.
    StyleTarget { /// The fill style.
        Fill = 0, /// The stroke style.
        Stroke = 1 }
}

wire_enum! {
    /// How a pattern repeats.
    PatternRepeat {
        /// In both directions.
        Repeat = 0,
        /// Horizontally only.
        RepeatX = 1,
        /// Vertically only.
        RepeatY = 2,
        /// Not at all.
        NoRepeat = 3,
    }
}

wire_enum! {
    /// How images are resampled when drawn scaled (`SetSmoothing`).
    Smoothing { /// Nearest neighbour.
        Off = 0, /// Bilinear.
        Low = 1, /// Bicubic.
        High = 2 }
}

wire_enum! {
    /// Horizontal text alignment relative to the x of `DrawText`.
    TextAlign {
        /// Left in left-to-right text, right in right-to-left (the default).
        Start = 0,
        /// The opposite of `Start`.
        End = 1,
        /// Left.
        Left = 2,
        /// Right.
        Right = 3,
        /// Centered.
        Center = 4,
    }
}

wire_enum! {
    /// Which line of the text sits on the y of `DrawText`.
    TextBaseline {
        /// The alphabetic baseline (the default).
        Alphabetic = 0,
        /// The top of the em square.
        Top = 1,
        /// The hanging baseline.
        Hanging = 2,
        /// The middle of the em square.
        Middle = 3,
        /// The ideographic baseline.
        Ideographic = 4,
        /// The bottom of the em square.
        Bottom = 5,
    }
}

wire_enum! {
    /// Base direction of text.
    TextDirection {
        /// Decided from the first strong character of each paragraph (the default).
        Auto = 0,
        /// Left to right.
        Ltr = 1,
        /// Right to left.
        Rtl = 2,
    }
}

/// `mode` bit: fill the shape.
pub const MODE_FILL: u8 = 1;
/// `mode` bit: stroke the shape.
pub const MODE_STROKE: u8 = 2;
/// `mode` bit of `DrawPolygon`: close the polygon.
pub const MODE_CLOSE: u8 = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_enums_round_trip_and_reject_unknown() {
        for m in BlendMode::ALL {
            assert_eq!(BlendMode::from_u8(m.to_u8()), Some(*m));
            assert_eq!(BlendMode::from_css_name(m.css_name()), Some(*m));
        }
        assert_eq!(BlendMode::from_u8(26), None);
        assert_eq!(FillRule::from_u8(2), None);
        assert_eq!(LineJoin::from_u8(255), None);
        assert_eq!(BlendMode::from_css_name(" Source-Over "), Some(BlendMode::SourceOver));
        assert_eq!(BlendMode::from_css_name("nope"), None);
    }

    #[test]
    fn blend_names_are_unique() {
        let mut names: Vec<_> = BlendMode::ALL.iter().map(|m| m.css_name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), BlendMode::ALL.len());
    }
}
