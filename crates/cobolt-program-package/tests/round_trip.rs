// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T14: the payload inside one executable.
//!
//! The round trip packages a copy of this test's own executable on this system's carrier (Mach-O
//! here on macOS, an overlay on Linux and Windows), writes it, reads the payload back from the
//! file, and runs the copy: the copy reads its own payload through `current_exe`, as the runner
//! will. On macOS `codesign --verify --strict` must accept it, the system's `codesign` must be able
//! to sign it again, and the kernel must run it. Whether our signature is byte for byte what a
//! given macOS's `codesign` writes is reported, with where the files differ, and not asserted.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_program_package::{self as pp, Kind, PackageError};

const CHILD: &str = "PRC_PACKAGE_CHILD";
const IDENT: &str = "prc-package-round-trip";

fn bytes(n: usize, seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

struct Sample {
    module: Vec<u8>,
    code: Vec<u8>,
    engine: Vec<u8>,
    extra: Vec<u8>,
}

fn sample() -> Sample {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    module.extend(bytes(180_000, 7));
    Sample { module, code: bytes(420_000, 11), engine: bytes(32, 13), extra: bytes(10, 17) }
}

fn payload(s: &Sample) -> Vec<u8> {
    pp::encode(&[(Kind::Module, &s.module), (Kind::MachineCode, &s.code), (Kind::MachineCodeEngine, &s.engine), (Kind::Other(99), &s.extra)])
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-package-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Run only inside the packaged copy: read this executable's own payload and report it.
#[test]
fn child_reports_its_own_payload() {
    if std::env::var(CHILD).as_deref() != Ok("1") {
        return;
    }
    let exe = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let payload = pp::locate(&exe).expect("the copy carries a payload");
    let p = pp::decode(payload).expect("its table of contents holds");
    let m = p.module().expect("its module matches its checksum");
    println!("PRC-CHILD module {} {}", m.len(), hex(&pp::sha256(m)));
    for part in &p.parts {
        println!("PRC-CHILD part {} {} {}", part.kind, part.bytes.len(), part.verified().is_ok());
    }
}

fn codesign(args: &[&str], path: &Path) -> (bool, String) {
    let out = Command::new("codesign").args(args).arg(path).output().expect("codesign runs");
    (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn field<'a>(text: &'a str, key: &str) -> &'a str {
    text.lines().find_map(|l| l.strip_prefix(key)).unwrap_or("?").trim()
}

/// The CodeDirectory header's fields by their offset in the structure (all of it big-endian).
fn cd_field(at: usize) -> &'static str {
    match at {
        0..=3 => "magic",
        4..=7 => "length",
        8..=11 => "version",
        12..=15 => "flags",
        16..=19 => "hash offset",
        20..=23 => "identifier offset",
        24..=27 => "special slots",
        28..=31 => "code slots",
        32..=35 => "code limit",
        36 => "hash size",
        37 => "hash type",
        38 => "platform",
        39 => "page size",
        40..=43 => "spare",
        44..=47 => "scatter offset",
        48..=51 => "team offset",
        52..=55 => "spare",
        56..=63 => "code limit 64",
        64..=71 => "executable segment base",
        72..=79 => "executable segment limit",
        80..=87 => "executable segment flags",
        _ => "identifier, special slot or page hash",
    }
}

/// Where two files differ, for the report of a signature that verifies but is not the one the
/// system's `codesign` writes: the lengths, then at most `limit` runs of differing bytes, each with
/// its place (a file offset before the signature, or an offset into the signature and the
/// CodeDirectory field it falls in) and the first bytes on each side, then the totals.
fn differences(ours: &[u8], theirs: &[u8], limit: usize) -> Vec<String> {
    const SIGNATURE: [u8; 4] = [0xfa, 0xde, 0x0c, 0xc0];
    const SUPERBLOB_HEADER: usize = 12 + 3 * 8;
    let mut out = Vec::new();
    if ours.len() != theirs.len() {
        out.push(format!("  lengths differ: ours {} bytes, codesign's {} bytes", ours.len(), theirs.len()));
    }
    let signature = ours.windows(4).rposition(|w| w == SIGNATURE);
    let shared = ours.len().min(theirs.len());
    let (mut runs, mut total, mut i) = (0usize, 0usize, 0usize);
    while i < shared {
        if ours[i] == theirs[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < shared && ours[i] != theirs[i] {
            i += 1;
        }
        runs += 1;
        total += i - start;
        if runs <= limit {
            let place = match signature {
                Some(s) if start >= s + SUPERBLOB_HEADER => {
                    format!("signature +{} (CodeDirectory +{}: {})", start - s, start - s - SUPERBLOB_HEADER, cd_field(start - s - SUPERBLOB_HEADER))
                }
                Some(s) if start >= s => format!("signature +{} (its header)", start - s),
                _ => format!("file offset {start}, before the signature"),
            };
            let shown = (i - start).min(16);
            out.push(format!("  differs at {place}: {} byte(s); ours {}, codesign's {}", i - start, hex(&ours[start..start + shown]), hex(&theirs[start..start + shown])));
        }
    }
    out.push(format!("  {runs} run(s) of differing bytes, {total} byte(s) in all{}", if runs > limit { format!(" (the first {limit} are listed)") } else { String::new() }));
    out
}

#[test]
fn a_copy_of_this_test_carries_a_program_and_still_runs() {
    if std::env::var(CHILD).is_ok() {
        return;
    }
    let s = sample();
    let payload = payload(&s);
    let me = std::env::current_exe().unwrap();
    let runner = std::fs::read(&me).unwrap();
    let format = pp::format_of(&runner).expect("this test is an executable");
    let mut checks: Vec<(String, bool)> = Vec::new();

    checks.push(("the bare executable carries no payload".into(), pp::locate(&runner) == Err(PackageError::NoPayload)));
    let t = Instant::now();
    let packed = pp::embed(&runner, &payload, IDENT).expect("embeds");
    let embed_ms = ms(t);
    checks.push(("a packaged executable is not packaged again".into(), pp::embed(&packed, &payload, IDENT) == Err(PackageError::AlreadyPackaged)));

    let dir = scratch("round-trip");
    let path = dir.join(format!("{IDENT}{}", std::env::consts::EXE_SUFFIX));
    let t = Instant::now();
    pp::write_executable(&path, &packed).expect("writes");
    let write_ms = ms(t);

    let t = Instant::now();
    let on_disk = std::fs::read(&path).unwrap();
    let read = pp::locate(&on_disk).and_then(pp::decode);
    let read_ms = ms(t);
    let read = read.expect("the payload reads back from the file");
    checks.push(("the payload reads back byte for byte".into(), pp::locate(&on_disk).unwrap() == payload.as_slice()));
    checks.push(("the module reads back, checked".into(), read.module() == Ok(s.module.as_slice())));
    for (kind, want) in [(Kind::MachineCode, &s.code), (Kind::MachineCodeEngine, &s.engine), (Kind::Other(99), &s.extra)] {
        checks.push((format!("{kind} reads back, checked"), read.part(kind).and_then(|p| p.verified().ok()) == Some(want.as_slice())));
    }

    let mut signing = Vec::new();
    if cfg!(target_os = "macos") {
        let (ok, text) = codesign(&["--verify", "--strict", "-v"], &path);
        checks.push(("codesign --verify --strict accepts it".into(), ok));
        signing.push(format!("codesign --verify --strict: {}", text.trim().replace('\n', " / ")));
        let (_, ours) = codesign(&["-d", "-vvv"], &path);
        let copy = dir.join("resigned-by-codesign");
        std::fs::copy(&path, &copy).unwrap();
        let (resigned, text) = codesign(&["-s", "-", "-f", "-i", IDENT], &copy);
        let (_, theirs) = codesign(&["-d", "-vvv"], &copy);
        signing.push(format!("ours: {}; {}; CDHash {}", field(&ours, "CodeDirectory"), field(&ours, "Identifier="), field(&ours, "CDHash=")));
        signing.push(format!(
            "codesign -s - of the same file{}: {}; CDHash {}",
            if resigned { "" } else { " (FAILED)" },
            field(&theirs, "CodeDirectory"),
            field(&theirs, "CDHash=")
        ));
        if !resigned {
            signing.push(text);
        }
        // What has to hold on every macOS: the system's own `codesign` signs the file again, which
        // means the layout is one Apple's tool accepts, and what it writes verifies. Whether our
        // CodeDirectory is byte for byte the one a given release writes is NOT a requirement, and
        // it was asserted here until 2026-11-22: the first CI run found macOS 15 and macOS 26 write
        // something other than macOS 27 does (same version, size and page count, another CDHash),
        // while both of them verified ours with --strict and ran it. It is reported instead, with
        // where the two files differ, so a change of what `codesign` writes can be seen and judged.
        checks.push(("codesign -s - signs it again".into(), resigned));
        let (resigned_verifies, _) = codesign(&["--verify", "--strict", "-v"], &copy);
        checks.push(("what codesign -s - writes for it verifies too".into(), resigned_verifies));
        let same = field(&ours, "CDHash=") == field(&theirs, "CDHash=");
        let written = std::fs::read(&copy).unwrap();
        let identical = written == packed;
        signing.push(format!(
            "its CodeDirectory is {} the one this system's codesign writes; the file codesign -s - writes for it is {} ours",
            if same { "(CDHash) the same as" } else { "NOT (CDHash) the same as" },
            if identical { "byte for byte" } else { "NOT" }
        ));
        if !identical {
            signing.extend(differences(&packed, &written, 8));
        }
    }

    let t = Instant::now();
    let run = Command::new(&path)
        .args(["--exact", "child_reports_its_own_payload", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .expect("the packaged copy starts");
    let run_ms = ms(t);
    let stdout = String::from_utf8_lossy(&run.stdout);
    let want = format!("PRC-CHILD module {} {}", s.module.len(), hex(&pp::sha256(&s.module)));
    checks.push(("the packaged copy runs".into(), run.status.success()));
    checks.push(("it reads its own module through current_exe".into(), stdout.contains(&want)));
    checks.push(("it sees all four parts, each checked".into(), stdout.matches("PRC-CHILD part").count() == 4 && !stdout.contains(" false")));
    if std::env::var_os("PRC_PACKAGE_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }

    println!("── a_copy_of_this_test_carries_a_program_and_still_runs (DESKTOP-WASM-001 T14) ──");
    println!("carrier: {format:?} on {}-{}", std::env::consts::ARCH, std::env::consts::OS);
    println!(
        "runner {} bytes + payload {} bytes (module {}, machine code {}, engine {}, an unknown kind {}) = {} bytes",
        runner.len(),
        payload.len(),
        s.module.len(),
        s.code.len(),
        s.engine.len(),
        s.extra.len(),
        packed.len()
    );
    println!("embed {embed_ms:.2} ms, write {write_ms:.2} ms, read back and decode {read_ms:.2} ms, run the copy {run_ms:.2} ms");
    for line in &signing {
        println!("{line}");
    }
    for line in stdout.lines().filter(|l| l.starts_with("PRC-CHILD")) {
        println!("  child: {}", line.trim_start_matches("PRC-CHILD "));
    }
    let passed = checks.iter().filter(|c| c.1).count();
    println!("checks: {passed} of {} passed", checks.len());
    for (name, ok) in &checks {
        if !ok {
            println!("  FAIL {name}");
        }
    }
    if !run.status.success() {
        println!("child stderr: {}", String::from_utf8_lossy(&run.stderr));
    }
    assert_eq!(passed, checks.len());
}

#[test]
fn the_report_of_a_different_signature_names_where_it_differs() {
    let t = Instant::now();
    let mut ours = vec![0u8; 300];
    ours[100..104].copy_from_slice(&[0xfa, 0xde, 0x0c, 0xc0]); // where the signature starts
    let mut theirs = ours.clone();
    theirs[10] ^= 1; // before the signature
    theirs[100 + 36 + 13] ^= 0xff; // the CodeDirectory's flags
    theirs[100 + 36 + 64] = 9; // its executable segment base
    theirs[100 + 36 + 120] ^= 1; // a hash, far past the header
    let report = differences(&ours, &theirs, 3).join("\n");
    let mut checks: Vec<(&str, bool)> = Vec::new();
    checks.push(("a difference before the signature is placed by file offset", report.contains("file offset 10, before the signature")));
    checks.push(("a difference in the CodeDirectory names its field (flags)", report.contains("CodeDirectory +13: flags")));
    checks.push(("a difference in the executable segment names it", report.contains("CodeDirectory +64: executable segment base")));
    checks.push(("only `limit` runs are listed, the totals count them all", report.contains("4 run(s) of differing bytes, 4 byte(s) in all (the first 3 are listed)")));
    checks.push(("two files that agree report no run", differences(&ours, &ours, 3).join("\n").contains("0 run(s)")));
    checks.push(("a different length is said first", differences(&ours, &ours[..250], 3)[0].contains("lengths differ: ours 300 bytes, codesign's 250 bytes")));
    let passed = checks.iter().filter(|c| c.1).count();
    println!("── the_report_of_a_different_signature_names_where_it_differs ──");
    println!("{report}");
    println!("{} cases in {:.2} ms: {passed} of {} passed", checks.len(), ms(t), checks.len());
    for (name, ok) in &checks {
        if !ok {
            println!("  FAIL {name}");
        }
    }
    assert_eq!(passed, checks.len());
}

/// The table of contents' own checksum, recomputed, so a test can make a payload that is whole but
/// newer.
fn rehash(p: &mut [u8]) {
    let count = u32::from_le_bytes(p[12..16].try_into().unwrap()) as usize;
    let mut toc = p[..24].to_vec();
    toc.extend_from_slice(&p[56..56 + 56 * count]);
    let h = pp::sha256(&toc);
    p[24..56].copy_from_slice(&h);
}

#[test]
fn a_damaged_table_of_contents_is_refused() {
    let s = sample();
    let good = payload(&s);
    let damaged = |f: &dyn Fn(&mut Vec<u8>)| {
        let mut p = good.clone();
        f(&mut p);
        pp::decode(&p).map(|_| ())
    };
    let is_damaged = |r: Result<(), PackageError>| matches!(r, Err(PackageError::Damaged(_)));
    let cases: Vec<(&str, bool)> = vec![
        ("a part's offset changed", is_damaged(damaged(&|p| p[56 + 8] ^= 0x10))),
        ("a part's length changed", is_damaged(damaged(&|p| p[56 + 56 + 16] ^= 0x01))),
        ("a part's kind changed", is_damaged(damaged(&|p| p[56] = 2))),
        ("a part's recorded sha256 changed", is_damaged(damaged(&|p| p[56 + 30] ^= 0xff))),
        ("the part count changed", is_damaged(damaged(&|p| p[12] = 3))),
        ("the recorded total length changed", is_damaged(damaged(&|p| p[16] ^= 0x01))),
        ("the table's own sha256 changed", is_damaged(damaged(&|p| p[40] ^= 0x01))),
        ("cut short", is_damaged(damaged(&|p| p.truncate(p.len() - 1)))),
        ("cut inside the table", is_damaged(damaged(&|p| p.truncate(100)))),
        ("no magic", is_damaged(damaged(&|p| p[0] = b'X'))),
        ("a part pointing outside the payload, rehashed", is_damaged(damaged(&|p| {
            p[56 + 16..56 + 24].copy_from_slice(&u64::MAX.to_le_bytes());
            rehash(p);
        }))),
        ("a newer format, whole", damaged(&|p| {
            p[8] = 2;
            rehash(p);
        }) == Err(PackageError::NewerFormat(2))),
        ("the good payload", pp::decode(&good).is_ok()),
    ];
    println!("── a_damaged_table_of_contents_is_refused ──");
    for (name, ok) in &cases {
        println!("  {} {name}", if *ok { "refused as expected:" } else { "FAIL" });
    }
    let passed = cases.iter().filter(|c| c.1).count();
    println!("checks: {passed} of {} passed", cases.len());
    assert_eq!(passed, cases.len());
}

#[test]
fn a_part_whose_sha256_does_not_match_is_refused() {
    let s = sample();
    let good = payload(&s);
    let p = pp::decode(&good).unwrap();
    let module_at = p.part(Kind::Module).unwrap().bytes.as_ptr() as usize - good.as_ptr() as usize;
    let code_at = p.part(Kind::MachineCode).unwrap().bytes.as_ptr() as usize - good.as_ptr() as usize;

    let mut bad_module = good.clone();
    bad_module[module_at + 1000] ^= 0x01;
    let bm = pp::decode(&bad_module).expect("the table of contents still holds");
    let mut bad_code = good.clone();
    bad_code[code_at + 5] ^= 0x80;
    let bc = pp::decode(&bad_code).expect("the table of contents still holds");

    let mut cases = vec![
        ("a damaged module is refused, naming MODULE", bm.module() == Err(PackageError::PartDamaged(Kind::Module))),
        ("damaged machine code is refused, naming MACHINE_CODE", bc.part(Kind::MachineCode).unwrap().verified() == Err(PackageError::PartDamaged(Kind::MachineCode))),
        ("the module beside damaged machine code still reads", bc.module() == Ok(s.module.as_slice())),
    ];
    // The same inside an executable.
    let runner = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let packed = pp::embed(&runner, &bad_module, IDENT).unwrap();
    let inside = pp::locate(&packed).and_then(pp::decode).map(|p| p.module().map(|_| ()));
    cases.push(("inside an executable too", inside == Ok(Err(PackageError::PartDamaged(Kind::Module)))));
    println!("── a_part_whose_sha256_does_not_match_is_refused ──");
    for (name, ok) in &cases {
        println!("  {} {name}", if *ok { "ok:" } else { "FAIL" });
    }
    let passed = cases.iter().filter(|c| c.1).count();
    println!("checks: {passed} of {} passed", cases.len());
    assert_eq!(passed, cases.len());
}

fn synthetic_elf() -> Vec<u8> {
    let mut b = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];
    b.extend(bytes(4_000, 23));
    b
}

fn synthetic_pe() -> Vec<u8> {
    let mut b = vec![0u8; 0x200];
    b[..2].copy_from_slice(b"MZ");
    b[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    b[0x80..0x84].copy_from_slice(b"PE\0\0");
    b.extend(bytes(3_000, 29));
    b
}

/// The overlay carrier, on every system: ELF and PE files made here.
#[test]
fn the_overlay_carrier_round_trips_on_any_system() {
    let s = sample();
    let payload = payload(&s);
    let mut cases = Vec::new();
    for (name, exe) in [("ELF", synthetic_elf()), ("PE", synthetic_pe())] {
        let packed = pp::embed(&exe, &payload, IDENT).unwrap();
        cases.push((format!("{name}: the image is kept"), packed[..exe.len()] == exe[..]));
        cases.push((format!("{name}: the payload reads back"), pp::locate(&packed) == Ok(payload.as_slice())));
        cases.push((format!("{name}: not packaged twice"), pp::embed(&packed, &payload, IDENT) == Err(PackageError::AlreadyPackaged)));
        cases.push((format!("{name}: a bare file has none"), pp::locate(&exe) == Err(PackageError::NoPayload)));
        let mut moved = packed.clone();
        let n = moved.len();
        moved[n - 32] ^= 0x04;
        cases.push((format!("{name}: a trailer pointing elsewhere is refused"), matches!(pp::locate(&moved), Err(PackageError::Damaged(_)))));
        let stripped = &packed[..exe.len() + 100];
        cases.push((format!("{name}: a stripped file says it has no payload"), pp::locate(stripped) == Err(PackageError::NoPayload)));
    }
    println!("── the_overlay_carrier_round_trips_on_any_system ──");
    for (name, ok) in &cases {
        println!("  {} {name}", if *ok { "ok:" } else { "FAIL" });
    }
    let passed = cases.iter().filter(|c| c.1).count();
    println!("checks: {passed} of {} passed", cases.len());
    assert_eq!(passed, cases.len());
}

fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put64(b: &mut [u8], at: usize, v: u64) {
    b[at..at + 8].copy_from_slice(&v.to_le_bytes());
}

fn get32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn get64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

/// A small Mach-O executable: `__PAGEZERO`, `__TEXT` with one section starting `room` bytes after
/// the load commands, `__LINKEDIT` holding a symbol table, a string table and a signature.
fn synthetic_macho(room: usize) -> Vec<u8> {
    let sizeofcmds = 72 + (72 + 80) + 72 + 24 + 16;
    let mut b = vec![0u8; 0x4160];
    put32(&mut b, 0, 0xfeed_facf);
    put32(&mut b, 4, 0x0100_000c);
    put32(&mut b, 12, 2);
    put32(&mut b, 16, 5);
    put32(&mut b, 20, sizeofcmds as u32);
    let mut at = 32;
    let seg = |b: &mut Vec<u8>, at: usize, name: &[u8], vm: u64, vmsize: u64, off: u64, size: u64, nsects: u32| {
        put32(b, at, 0x19);
        put32(b, at + 4, 72 + 80 * nsects);
        b[at + 8..at + 8 + name.len()].copy_from_slice(name);
        put64(b, at + 24, vm);
        put64(b, at + 32, vmsize);
        put64(b, at + 40, off);
        put64(b, at + 48, size);
        put32(b, at + 64, nsects);
    };
    seg(&mut b, at, b"__PAGEZERO", 0, 0x1_0000_0000, 0, 0, 0);
    at += 72;
    seg(&mut b, at, b"__TEXT", 0x1_0000_0000, 0x4000, 0, 0x4000, 1);
    b[at + 72..at + 72 + 6].copy_from_slice(b"__text");
    b[at + 72 + 16..at + 72 + 22].copy_from_slice(b"__TEXT");
    put32(&mut b, at + 72 + 48, (32 + sizeofcmds + room) as u32);
    at += 152;
    seg(&mut b, at, b"__LINKEDIT", 0x1_0000_4000, 0x4000, 0x4000, 0x160, 0);
    at += 72;
    put32(&mut b, at, 0x02); // LC_SYMTAB
    put32(&mut b, at + 4, 24);
    put32(&mut b, at + 8, 0x4000);
    put32(&mut b, at + 12, 4);
    put32(&mut b, at + 16, 0x4040);
    put32(&mut b, at + 20, 32);
    at += 24;
    put32(&mut b, at, 0x1d); // LC_CODE_SIGNATURE
    put32(&mut b, at + 4, 16);
    put32(&mut b, at + 8, 0x4060);
    put32(&mut b, at + 12, 0x100);
    b[0x4000..0x4060].copy_from_slice(&bytes(0x60, 31));
    b
}

/// The Mach-O carrier's layout, on every system, on a Mach-O made here.
#[test]
fn the_macho_carrier_moves_linkedit_and_signs_again() {
    let s = sample();
    let payload = payload(&s);
    let exe = synthetic_macho(200);
    let packed = pp::embed(&exe, &payload, IDENT).unwrap();
    let span = payload.len().div_ceil(0x4000) * 0x4000;
    let ncmds = get32(&packed, 16);
    let le = 32 + 72 + 152 + 72; // __LINKEDIT's command, after the new segment's
    let symtab = le + 72;
    let sig = symtab + 24;
    let sig_off = get32(&packed, sig + 8) as usize;
    let sig_len = get32(&packed, sig + 12) as usize;
    let cases = vec![
        ("one load command more", ncmds == 6),
        ("the payload reads back", pp::locate(&packed) == Ok(payload.as_slice())),
        ("__PRPAYLOAD sits where __LINKEDIT began", &packed[le - 72 + 8..le - 72 + 19] == b"__PRPAYLOAD" && get64(&packed, le - 72 + 40) == 0x4000),
        ("__LINKEDIT moved up by the payload's pages", get64(&packed, le + 40) == 0x4000 + span as u64 && get64(&packed, le + 24) == 0x1_0000_4000 + span as u64),
        ("the symbol and string tables moved with it", get32(&packed, symtab + 8) as usize == 0x4000 + span && get32(&packed, symtab + 16) as usize == 0x4040 + span),
        ("their bytes are where the offsets say", packed[0x4000 + span..0x4060 + span] == exe[0x4000..0x4060]),
        ("the new signature ends the file", sig_off + sig_len == packed.len() && packed[sig_off..sig_off + 4] == [0xfa, 0xde, 0x0c, 0xc0]),
        ("__LINKEDIT ends with it", get64(&packed, le + 40) + get64(&packed, le + 48) == packed.len() as u64),
        ("the text before __LINKEDIT is unchanged", packed[32 + 336 + 72..0x4000] == exe[32 + 336 + 72..0x4000]),
        ("not packaged twice", pp::embed(&packed, &payload, IDENT) == Err(PackageError::AlreadyPackaged)),
        ("too little header room is refused, naming the room", pp::embed(&synthetic_macho(40), &payload, IDENT) == Err(PackageError::NoHeaderRoom { free: 40, needed: 72 })),
        ("a bare Mach-O has no payload", pp::locate(&exe) == Err(PackageError::NoPayload)),
    ];
    println!("── the_macho_carrier_moves_linkedit_and_signs_again ── payload {} bytes, {span} bytes of pages", payload.len());
    for (name, ok) in &cases {
        println!("  {} {name}", if *ok { "ok:" } else { "FAIL" });
    }
    let passed = cases.iter().filter(|c| c.1).count();
    println!("checks: {passed} of {} passed", cases.len());
    assert_eq!(passed, cases.len());
}
