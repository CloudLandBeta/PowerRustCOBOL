// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 092 R10/R11, AC4 — `COBOL::"IMAGE-FIT"` and `COBOL::"IMAGE-INFO"`,
//! driven from COBOL source through the interpreter.
//!
//! Every source picture is drawn here, at test time, with the `image` crate —
//! no binary fixture is checked in. Each one carries something a wrong
//! implementation would get visibly wrong: a coloured centre and coloured
//! corners (crop to fill, not stretch or letterbox), a band that must end up
//! on top (EXIF orientation 6), a second GIF frame that must NOT be used, a
//! PNG header that claims 100 megapixels over pixel data that is garbage.

#![cfg(feature = "images")]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;
use image::{DynamicImage, ImageEncoder, Rgb, RgbImage, Rgba, RgbaImage};

// ── Running COBOL ──────────────────────────────────────────────────────────

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

/// The value DISPLAYed as `KEY=[value]`.
fn field(lines: &[String], key: &str) -> String {
    let prefix = format!("{key}=[");
    lines
        .iter()
        .find_map(|l| l.strip_prefix(&prefix).and_then(|v| v.strip_suffix(']')))
        .unwrap_or_else(|| panic!("no {key} in {lines:?}"))
        .trim()
        .to_owned()
}

/// `COBOL::"IMAGE-FIT" ( source target width height status )` → status.
fn fit(source: &Path, target: &Path, width: i64, height: i64) -> String {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FIT-ONE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-SOURCE  PIC X(400) VALUE "{source}".
       01 WS-TARGET  PIC X(400) VALUE "{target}".
       01 WS-WIDTH   PIC S9(6)  VALUE {width}.
       01 WS-HEIGHT  PIC S9(6)  VALUE {height}.
       01 WS-STATUS  PIC X(12)  VALUE "UNSET".
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"IMAGE-FIT" ( WS-SOURCE WS-TARGET WS-WIDTH WS-HEIGHT WS-STATUS )
           DISPLAY "STATUS=[" WS-STATUS "]"
           STOP RUN.
"#,
        source = source.display(),
        target = target.display(),
    );
    field(&run(&src), "STATUS")
}

/// `COBOL::"IMAGE-INFO" ( path width height format status )`.
fn info(path: &Path) -> (u32, u32, String, String) {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INFO-ONE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-PATH    PIC X(400) VALUE "{path}".
       01 WS-WIDTH   PIC 9(6)   VALUE 999999.
       01 WS-HEIGHT  PIC 9(6)   VALUE 999999.
       01 WS-FORMAT  PIC X(8)   VALUE "UNSET".
       01 WS-STATUS  PIC X(12)  VALUE "UNSET".
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"IMAGE-INFO" ( WS-PATH WS-WIDTH WS-HEIGHT WS-FORMAT WS-STATUS )
           DISPLAY "WIDTH=[" WS-WIDTH "]"
           DISPLAY "HEIGHT=[" WS-HEIGHT "]"
           DISPLAY "FORMAT=[" WS-FORMAT "]"
           DISPLAY "STATUS=[" WS-STATUS "]"
           STOP RUN.
"#,
        path = path.display(),
    );
    let out = run(&src);
    (
        field(&out, "WIDTH").parse().expect("numeric width"),
        field(&out, "HEIGHT").parse().expect("numeric height"),
        field(&out, "FORMAT"),
        field(&out, "STATUS"),
    )
}

// ── Pictures ───────────────────────────────────────────────────────────────

fn scratch(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("prc-image-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const GREY: [u8; 3] = [128, 128, 128];
const MAGENTA: [u8; 3] = [255, 0, 255];
const RED: [u8; 3] = [230, 20, 20];
const GREEN: [u8; 3] = [20, 200, 20];
const BLUE: [u8; 3] = [20, 20, 230];
const YELLOW: [u8; 3] = [240, 230, 20];

/// 1200 × 800: grey, a magenta square at the centre (x 500–700, y 300–500)
/// and a 100 × 100 colour in each corner.
fn centre_and_corners() -> RgbImage {
    RgbImage::from_fn(1200, 800, |x, y| {
        let c = match (x, y) {
            (500..700, 300..500) => MAGENTA,
            (0..100, 0..100) => RED,
            (1100..1200, 0..100) => GREEN,
            (0..100, 700..800) => BLUE,
            (1100..1200, 700..800) => YELLOW,
            _ => GREY,
        };
        Rgb(c)
    })
}

fn jpeg_bytes(img: &RgbImage, quality: u8) -> Vec<u8> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8)
        .unwrap();
    out
}

/// A JPEG with a minimal EXIF APP1 segment, written by hand, carrying one IFD
/// entry: Orientation (0x0112), SHORT, `orientation`. Big-endian TIFF header.
fn with_exif_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "a JPEG starts with SOI");
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"MM\x00\x2A"); // big-endian, 42
    tiff.extend_from_slice(&8u32.to_be_bytes()); // first IFD right after the header
    tiff.extend_from_slice(&1u16.to_be_bytes()); // one entry
    tiff.extend_from_slice(&0x0112u16.to_be_bytes()); // Orientation
    tiff.extend_from_slice(&3u16.to_be_bytes()); // SHORT
    tiff.extend_from_slice(&1u32.to_be_bytes()); // count 1
    tiff.extend_from_slice(&orientation.to_be_bytes()); // value, left-justified
    tiff.extend_from_slice(&[0, 0]);
    tiff.extend_from_slice(&0u32.to_be_bytes()); // no next IFD
    let mut app1 = vec![0xFF, 0xE1];
    let len = (2 + 6 + tiff.len()) as u16;
    app1.extend_from_slice(&len.to_be_bytes());
    app1.extend_from_slice(b"Exif\x00\x00");
    app1.extend_from_slice(&tiff);
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&app1);
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn decode(path: &Path) -> DynamicImage {
    image::ImageReader::open(path)
        .unwrap()
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap_or_else(|e| panic!("{} does not decode: {e}", path.display()))
}

/// The mean colour of the 5 × 5 block centred on (x, y) — robust to JPEG noise.
fn mean(img: &DynamicImage, x: u32, y: u32) -> [u8; 3] {
    let rgb = img.to_rgb8();
    let (mut s, mut n) = ([0u32; 3], 0u32);
    for yy in y.saturating_sub(2)..=(y + 2).min(rgb.height() - 1) {
        for xx in x.saturating_sub(2)..=(x + 2).min(rgb.width() - 1) {
            let p = rgb.get_pixel(xx, yy).0;
            for c in 0..3 {
                s[c] += u32::from(p[c]);
            }
            n += 1;
        }
    }
    [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8]
}

fn near(got: [u8; 3], want: [u8; 3]) -> bool {
    got.iter().zip(want).all(|(g, w)| (i16::from(*g) - i16::from(w)).abs() <= 40)
}

#[track_caller]
fn assert_colour(img: &DynamicImage, x: u32, y: u32, want: [u8; 3], what: &str) {
    let got = mean(img, x, y);
    assert!(near(got, want), "{what}: pixel ({x},{y}) is {got:?}, expected about {want:?}");
}

/// Nothing at `target` and no `<target>.part` beside it.
#[track_caller]
fn assert_nothing_written(target: &Path) {
    assert!(!target.exists(), "{} must not be written", target.display());
    let mut part = target.as_os_str().to_owned();
    part.push(".part");
    assert!(!Path::new(&part).exists(), "no .part may be left beside {}", target.display());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A PNG whose IHDR claims `w` × `h` and whose only IDAT is 64 bytes of junk:
/// readable as far as its header, undecodable past it.
fn png_header_only(w: u32, h: u32) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB, no interlace
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &[0xAB; 64]);
    chunk(&mut png, b"IEND", &[]);
    png
}

// ── The tests ──────────────────────────────────────────────────────────────

/// AC4: a 1200 × 800 JPEG fitted to 400 × 400 keeps its centre at the centre —
/// cropped to fill, so the corners are gone, not squeezed in.
#[test]
fn image_fit_crops_to_fill_and_keeps_the_centre() {
    let dir = scratch("centre");
    let source = dir.join("photo.jpg");
    std::fs::write(&source, jpeg_bytes(&centre_and_corners(), 95)).unwrap();

    let t = Instant::now();
    let big = dir.join("out/photo-400.png");
    assert_eq!(fit(&source, &big, 400, 400), "OK");
    let ms_400 = t.elapsed().as_millis();
    let out = decode(&big);
    assert_eq!((out.width(), out.height()), (400, 400), "exactly the size asked for");
    // The centre kept 800 × 800 of the source (x 200–1000) at half scale, so
    // the magenta square (x 500–700) lands at 150–250 on both axes.
    assert_colour(&out, 200, 200, MAGENTA, "the centre of the output is the centre of the source");
    assert_colour(&out, 160, 160, MAGENTA, "inside the square's top-left corner");
    assert_colour(&out, 240, 240, MAGENTA, "inside the square's bottom-right corner");
    assert_colour(&out, 140, 140, GREY, "just outside the square");
    assert_colour(&out, 260, 260, GREY, "just outside the square, other side");
    // A stretched fit would show the red/blue corners here; a crop shows grey.
    for (x, y, what) in [(4, 4, "top-left"), (395, 4, "top-right"), (4, 395, "bottom-left"), (395, 395, "bottom-right")] {
        assert_colour(&out, x, y, GREY, &format!("{what} corner is cropped away, not stretched in"));
    }

    let t = Instant::now();
    let thumb = dir.join("out/photo-40.png");
    assert_eq!(fit(&source, &thumb, 40, 40), "OK");
    let ms_40 = t.elapsed().as_millis();
    let out = decode(&thumb);
    assert_eq!((out.width(), out.height()), (40, 40));
    assert_colour(&out, 20, 20, MAGENTA, "the 40 × 40 thumbnail keeps the centre too");
    assert_colour(&out, 2, 2, GREY, "and drops the corners");

    // The fitted file measures as what it is.
    assert_eq!(info(&big), (400, 400, "PNG".into(), "OK".into()));
    assert_eq!(info(&thumb), (40, 40, "PNG".into(), "OK".into()));

    println!("  IMAGE-FIT 1200×800 JPEG → 400×400 PNG : {ms_400} ms");
    println!("  IMAGE-FIT 1200×800 JPEG →   40×40 PNG : {ms_40} ms");
    println!("  checked: 5 points in/around the centre square, 4 cropped corners, both sizes, IMAGE-INFO of both");
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC4: a portrait photo stored on its side with EXIF orientation 6 comes out
/// upright — the band stored at the LEFT is on TOP — and `IMAGE-INFO` reports
/// it upright.
#[test]
fn image_fit_turns_a_photo_upright_from_its_exif_orientation() {
    let dir = scratch("exif");
    // Stored 1200 × 800 (landscape). Orientation 6 = rotate 90° clockwise to
    // view, so the stored left edge becomes the top: blue on top, red at the
    // bottom of an 800 × 1200 portrait.
    let stored = RgbImage::from_fn(1200, 800, |x, _| {
        Rgb(match x {
            0..300 => BLUE,
            900..1200 => RED,
            _ => GREEN,
        })
    });
    let source = dir.join("portrait.jpg");
    std::fs::write(&source, with_exif_orientation(&jpeg_bytes(&stored, 95), 6)).unwrap();

    assert_eq!(
        info(&source),
        (800, 1200, "JPEG".into(), "OK".into()),
        "IMAGE-INFO reports the picture as it is meant to be seen"
    );

    // 400 × 600 is the upright shape exactly, so nothing is cropped.
    let target = dir.join("upright.png");
    assert_eq!(fit(&source, &target, 400, 600), "OK");
    let out = decode(&target);
    assert_eq!((out.width(), out.height()), (400, 600));
    assert_colour(&out, 200, 40, BLUE, "the stored left band is now on top");
    assert_colour(&out, 200, 300, GREEN, "the middle stays in the middle");
    assert_colour(&out, 200, 560, RED, "the stored right band is now at the bottom");
    assert_eq!(info(&target), (400, 600, "PNG".into(), "OK".into()));

    // The same picture WITHOUT the tag is landscape, and a 400 × 600 fit of it
    // keeps only the green middle — the tag is what made the difference.
    let plain = dir.join("plain.jpg");
    std::fs::write(&plain, jpeg_bytes(&stored, 95)).unwrap();
    assert_eq!(info(&plain), (1200, 800, "JPEG".into(), "OK".into()));
    let target2 = dir.join("plain.png");
    assert_eq!(fit(&plain, &target2, 400, 600), "OK");
    assert_colour(&decode(&target2), 200, 40, GREEN, "untagged: no rotation");
    println!("  orientation 6: IMAGE-INFO 800×1200, fit 400×600 blue/green/red top→bottom; untagged control case 1200×800");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every format the built-ins promise is read — by its content, not its name —
/// and measured; a GIF contributes its FIRST frame.
#[test]
fn every_promised_format_is_read_and_measured() {
    let dir = scratch("formats");
    // 64 × 48, a red square at the centre on grey.
    let base = RgbaImage::from_fn(64, 48, |x, y| {
        let [r, g, b] = if (24..40).contains(&x) && (16..32).contains(&y) { RED } else { GREY };
        Rgba([r, g, b, 255])
    });
    let dyn_base = DynamicImage::ImageRgba8(base.clone());

    let mut files: Vec<(&str, PathBuf)> = Vec::new();
    for (name, format, file) in [
        ("PNG", image::ImageFormat::Png, "a.png"),
        ("BMP", image::ImageFormat::Bmp, "a.bmp"),
        ("TIFF", image::ImageFormat::Tiff, "a.tiff"),
        ("WEBP", image::ImageFormat::WebP, "a.webp"),
    ] {
        let p = dir.join(file);
        dyn_base.save_with_format(&p, format).unwrap();
        files.push((name, p));
    }
    let jpeg = dir.join("a.jpg");
    std::fs::write(&jpeg, jpeg_bytes(&dyn_base.to_rgb8(), 95)).unwrap();
    files.push(("JPEG", jpeg));

    // A two-frame GIF: red-centred first, solid blue second.
    let gif = dir.join("a.gif");
    {
        let second = RgbaImage::from_pixel(64, 48, Rgba([BLUE[0], BLUE[1], BLUE[2], 255]));
        let mut enc = image::codecs::gif::GifEncoder::new(std::fs::File::create(&gif).unwrap());
        enc.encode_frames(vec![image::Frame::new(base.clone()), image::Frame::new(second)])
            .unwrap();
    }
    files.push(("GIF", gif));

    // Content decides, not the extension: a PNG saved as `.jpg`.
    let disguised = dir.join("really-a-png.jpg");
    std::fs::copy(&files[0].1, &disguised).unwrap();
    files.push(("PNG", disguised));

    println!("  {:<18} {:<6} {:>9} {:>8}", "file", "format", "info", "fit 40²");
    for (name, path) in &files {
        let (w, h, fmt, status) = info(path);
        assert_eq!((w, h, fmt.as_str(), status.as_str()), (64, 48, *name, "OK"), "{}", path.display());
        let target = path.with_extension("fit.png");
        let t = Instant::now();
        assert_eq!(fit(path, &target, 40, 40), "OK", "{}", path.display());
        let ms = t.elapsed().as_millis();
        let out = decode(&target);
        assert_eq!((out.width(), out.height()), (40, 40));
        // 64 × 48 → crop 48 × 48 (x 8–56) → 40 × 40: the red square is centred.
        assert_colour(&out, 20, 20, RED, &format!("{name}: the centre (GIF: of the FIRST frame)"));
        println!(
            "  {:<18} {:<6} {:>4}×{:<4} {:>5} ms",
            path.file_name().unwrap().to_string_lossy(),
            fmt,
            w,
            h,
            ms
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The target's extension picks the encoding; anything but PNG or JPEG is
/// refused before the source is even opened.
#[test]
fn the_target_extension_chooses_png_or_jpeg_and_nothing_else() {
    let dir = scratch("ext");
    let source = dir.join("photo.jpg");
    std::fs::write(&source, jpeg_bytes(&centre_and_corners(), 90)).unwrap();

    let png_magic: &[u8] = b"\x89PNG\r\n\x1a\n";
    let jpeg_magic: &[u8] = &[0xFF, 0xD8, 0xFF];
    let accepted = [
        ("out.png", png_magic),
        ("out-upper.PNG", png_magic),
        ("out.jpg", jpeg_magic),
        ("out.jpeg", jpeg_magic),
        ("out-upper.JPG", jpeg_magic),
    ];
    for (name, magic) in accepted {
        let target = dir.join(name);
        assert_eq!(fit(&source, &target, 40, 40), "OK", "{name}");
        let bytes = std::fs::read(&target).unwrap();
        assert!(bytes.starts_with(magic), "{name} is written as its extension says");
        let (w, h, _, _) = info(&target);
        assert_eq!((w, h), (40, 40), "{name}");
        println!("  {name:<14} OK   {} bytes", bytes.len());
    }
    for name in ["out.gif", "out.webp", "out.bmp", "out", "out.png.txt"] {
        let target = dir.join(name);
        assert_eq!(fit(&source, &target, 40, 40), "BADEXT", "{name}");
        assert_nothing_written(&target);
        println!("  {name:<14} BADEXT, nothing written");
    }
    // The extension is checked first: a missing source with a bad target is BADEXT.
    assert_eq!(fit(&dir.join("missing.jpg"), &dir.join("x.gif"), 40, 40), "BADEXT");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A transparent PNG written as JPEG is laid on white, not on black.
#[test]
fn transparency_becomes_white_in_a_jpeg_and_stays_in_a_png() {
    let dir = scratch("alpha");
    let source = dir.join("clear.png");
    RgbaImage::from_pixel(80, 80, Rgba([0, 0, 0, 0])).save(&source).unwrap();
    let jpg = dir.join("clear.jpg");
    assert_eq!(fit(&source, &jpg, 40, 40), "OK");
    assert_colour(&decode(&jpg), 20, 20, [255, 255, 255], "transparent → white in a JPEG");
    let png = dir.join("clear-out.png");
    assert_eq!(fit(&source, &png, 40, 40), "OK");
    assert_eq!(decode(&png).to_rgba8().get_pixel(20, 20).0[3], 0, "a PNG keeps the transparency");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every refusal answers its word and writes nothing — and leaves a target
/// that already existed exactly as it was.
#[test]
fn bad_inputs_answer_their_status_word_and_write_nothing() {
    let dir = scratch("bad");
    let good = dir.join("good.jpg");
    std::fs::write(&good, jpeg_bytes(&centre_and_corners(), 90)).unwrap();
    let text = dir.join("not-a-picture.png");
    std::fs::write(&text, "this is text, not a picture").unwrap();
    let truncated = dir.join("truncated.jpg");
    std::fs::write(&truncated, &jpeg_bytes(&centre_and_corners(), 90)[..20]).unwrap();
    let empty = dir.join("empty.png");
    std::fs::write(&empty, b"").unwrap();

    let cases: [(&str, &Path, i64, i64, &str); 8] = [
        ("missing source", &dir.join("nope.jpg"), 40, 40, "NOFILE"),
        ("a folder as source", &dir, 40, 40, "NOFILE"),
        ("text named .png", &text, 40, 40, "BADIMAGE"),
        ("truncated JPEG", &truncated, 40, 40, "BADIMAGE"),
        ("empty file", &empty, 40, 40, "BADIMAGE"),
        ("width 0", &good, 0, 40, "BADSIZE"),
        ("height -5", &good, 40, -5, "BADSIZE"),
        ("target over 64 MP", &good, 9000, 9000, "BADSIZE"),
    ];
    println!("  {:<20} {:<9}", "case", "status");
    for (i, (what, source, w, h, want)) in cases.into_iter().enumerate() {
        let target = dir.join(format!("out-{i}.png"));
        assert_eq!(fit(source, &target, w, h), want, "{what}");
        assert_nothing_written(&target);
        println!("  {what:<20} {want:<9} nothing written");
    }

    // A failed fit leaves the existing target alone.
    let existing = dir.join("existing.png");
    std::fs::write(&existing, b"the old thumbnail").unwrap();
    assert_eq!(fit(&text, &existing, 40, 40), "BADIMAGE");
    assert_eq!(std::fs::read(&existing).unwrap(), b"the old thumbnail", "untouched");
    // …and a successful one replaces it.
    assert_eq!(fit(&good, &existing, 40, 40), "OK");
    assert_eq!(decode(&existing).width(), 40);

    // IMAGE-INFO's refusals clear the numbers and the format.
    for (what, path, want) in [
        ("missing", dir.join("nope.jpg"), "NOFILE"),
        ("text", text.clone(), "BADIMAGE"),
        ("empty", empty.clone(), "BADIMAGE"),
    ] {
        assert_eq!(info(&path), (0, 0, String::new(), want.to_string()), "IMAGE-INFO of {what}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC4 / R10: a header claiming more than 64 MP is refused BEFORE a pixel is
/// decoded — the pixel data here is junk, so reaching the decoder would answer
/// BADIMAGE, not TOOBIG. `IMAGE-INFO` reads the same header and reports it.
#[test]
fn a_picture_over_64_megapixels_is_refused_from_its_header() {
    let dir = scratch("huge");
    let huge = dir.join("huge.png");
    std::fs::write(&huge, png_header_only(10_000, 10_000)).unwrap();
    let target = dir.join("huge-fit.png");

    let t = Instant::now();
    assert_eq!(fit(&huge, &target, 400, 400), "TOOBIG");
    let ms = t.elapsed().as_millis();
    assert_nothing_written(&target);
    assert_eq!(info(&huge), (10_000, 10_000, "PNG".into(), "OK".into()), "IMAGE-INFO reads only the header");

    // 400 MP — over a gigabyte of pixels: measured all the same, since nothing
    // is decoded, and refused by IMAGE-FIT with the same word.
    let giant = dir.join("giant.png");
    std::fs::write(&giant, png_header_only(20_000, 20_000)).unwrap();
    assert_eq!(info(&giant), (20_000, 20_000, "PNG".into(), "OK".into()), "no memory limit on a header");
    assert_eq!(fit(&giant, &target, 400, 400), "TOOBIG");
    assert_nothing_written(&target);

    // Just under the limit is not refused for its size: the same junk pixels
    // now reach the decoder and fail there.
    let under = dir.join("under.png");
    std::fs::write(&under, png_header_only(8_000, 8_000)).unwrap();
    assert_eq!(fit(&under, &dir.join("under-fit.png"), 40, 40), "BADIMAGE", "64 MP exactly is allowed");
    println!("  100 MP header refused in {ms} ms without decoding; 64 MP header passes the size check");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same fit written as `CALL "COBOL-IMAGE-FIT" USING …` and as the inline
/// form is the same call, and the arguments may be data items or literals.
#[test]
fn the_inline_form_and_the_call_are_the_same_built_in() {
    let dir = scratch("inline");
    let source = dir.join("photo.jpg");
    std::fs::write(&source, jpeg_bytes(&centre_and_corners(), 90)).unwrap();
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BOTH-FORMS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-SOURCE  PIC X(400) VALUE "{source}".
       01 WS-W       PIC 9(4)   VALUE 64.
       01 WS-H       PIC 9(4)   VALUE 32.
       01 WS-A       PIC X(12).
       01 WS-B       PIC X(12).
       01 WS-WIDTH   PIC 9(6).
       01 WS-HEIGHT  PIC 9(6).
       01 WS-FORMAT  PIC X(8).
       01 WS-C       PIC X(12).
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-IMAGE-FIT" USING WS-SOURCE "{a}" WS-W WS-H WS-A
           COBOL::"IMAGE-FIT" ( WS-SOURCE "{b}" "64" "32" WS-B )
           COBOL::"IMAGE-INFO" ( "{b}" WS-WIDTH WS-HEIGHT WS-FORMAT WS-C )
           DISPLAY "A=[" WS-A "]"
           DISPLAY "B=[" WS-B "]"
           DISPLAY "C=[" WS-C "]"
           DISPLAY "SIZE=[" WS-WIDTH "x" WS-HEIGHT "]"
           DISPLAY "FORMAT=[" WS-FORMAT "]"
           STOP RUN.
"#,
        source = source.display(),
        a = dir.join("a.jpg").display(),
        b = dir.join("b.jpg").display(),
    );
    let out = run(&src);
    assert_eq!(field(&out, "A"), "OK");
    assert_eq!(field(&out, "B"), "OK");
    assert_eq!(field(&out, "C"), "OK");
    assert_eq!(field(&out, "SIZE"), "000064x000032");
    assert_eq!(field(&out, "FORMAT"), "JPEG");
    assert_eq!(
        std::fs::read(dir.join("a.jpg")).unwrap(),
        std::fs::read(dir.join("b.jpg")).unwrap(),
        "the two spellings produce the same file"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// GOLDEN RULE #7 — the quantified COBOL program
/// `tests/cobol/media/test-image-builtins.cbl` runs green: it fits two
/// pictures the repository ships, measures every copy back and checks every
/// refusal word. Its paths are pointed at the real assets and a scratch folder.
#[cfg(feature = "http")]
#[test]
fn the_cobol_test_program_passes() {
    let dir = scratch("cobol");
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/images/");
    let src = include_str!("../../../tests/cobol/media/test-image-builtins.cbl")
        .replace("\"assets/images/", &format!("\"{}", assets.display()))
        .replace("\"/tmp/prc-media/", &format!("\"{}/", dir.display()));
    let out = run(&src);
    for line in &out {
        println!("  {line}");
    }
    assert!(out.iter().any(|l| l == "OVERALL RESULT  : PASS"), "{out:#?}");
    assert!(out.iter().any(|l| l == "CASES RUN       : 16"), "all 16 cases ran: {out:#?}");
    assert!(!dir.join("r1.png").exists() && !dir.join("r3.gif").exists(), "refusals write nothing");
    let _ = std::fs::remove_dir_all(&dir);
}
