// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T48 (R10, R11): **the PowerCloud example behaves on the WebAssembly path as it
//! does under the interpreter.** The twin of `powercloud_hosts`.
//!
//! The example's forms are copied into a temporary project whose six AWS connections route the hosted
//! server to the fake MCP server (`cobolt-runtime`'s test binary), as `powercloud_hosts` does; a local
//! web server stands in for S3's temporary link. The project is built once on the WebAssembly path.
//! Then the same scenarios run, in the same order, under `rcrun run-form` (the interpreter) and in the
//! built program, each engine starting from the project's data as it was made: `rcrun run-form` in the
//! project's `data`, given the project's form theme as the IDE gives it (`--theme-default`), and the
//! built program in the `data` beside it, which the Build filled, as a built program works:
//! - every tab of `powercloud_hosts`' tab-by-tab test (Settings, DynamoDB, S3, Lambda, Polly, EC2,
//!   Rekognition, a new pet through its popup, edit, view and delete through theirs, an EC2 switch
//!   with its Working window);
//! - a photo refused with no stack and no bucket, and keys that cannot deploy;
//! - the six languages of the main form, a status line written again in a new language, and the
//!   popups told the language.
//!
//! For each scenario the values its reads answered, the windows left open, what the popups said and
//! the requests the fake server received must be equal on both engines, and the pictures equal in
//! every pixel; the photos the program fitted into its cache must be equal byte for byte. The
//! expectations of `powercloud_hosts` are checked on the built program. Nothing reaches AWS: the keys
//! are planted, the machine's own AWS sign-in is out of reach (`HOME` is the test's own, the `AWS_*`
//! variables removed), and no secret may reach any output.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// The planted sign-in: visibly fake, and not shaped like AWS's own key ids and account
/// numbers, so no credential scanner takes them for real ones. The program treats them as text.
const KEY_ID: &str = "PLANTED-KEY-ID-0000X";
const SECRET: &str = "planted-secret-value-0123456789abcdefghij";
const TOKEN: &str = "planted-session-token-9876543210";
const BUCKET: &str = "powercloud-photos-1c2fa620-982a-11e3-aff7-50e2416294e0";
const ARN: &str = "arn:aws:iam::DEMO-ACCOUNT:user/demo";
/// The variables through which the machine's own AWS sign-in could be reached.
const AWS_VARS: &[&str] = &["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN", "AWS_PROFILE", "AWS_DEFAULT_PROFILE", "AWS_CONFIG_FILE", "AWS_SHARED_CREDENTIALS_FILE", "AWS_REGION", "AWS_DEFAULT_REGION"];

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

fn fake_mcp() -> PathBuf {
    let path = rcrun().with_file_name(format!("fake_mcp{}", std::env::consts::EXE_SUFFIX));
    if !path.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let status = Command::new(cargo).args(["build", "-p", "cobolt-runtime", "--bin", "fake_mcp"]).status().expect("cargo builds the fake MCP server");
        assert!(status.success(), "the fake MCP server did not build");
    }
    path
}

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerCloud").canonicalize().unwrap()
}

/// A copy of a folder; at the top, what a Build or a run leaves (`bin`, `dist`, `temp`, `debug`,
/// `generated`, `target`) is left out.
fn copy_dir(from: &Path, to: &Path, top: bool) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if top && matches!(name.as_str(), "bin" | "dist" | "temp" | "debug" | "generated" | "target") {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        if src.is_dir() {
            copy_dir(&src, &dst, false);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A web server on a free port that answers every GET with the bytes of one JPEG: what the
/// temporary link of the Lambda function leads to.
fn photo_server(jpeg: Vec<u8>) -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&hits);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            counted.fetch_add(1, Ordering::SeqCst);
            let mut stream = stream;
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", jpeg.len());
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&jpeg);
        }
    });
    (port, hits)
}

/// Standard base64, as AWS's MCP server answers an object's bytes.
fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= c.len() { char::from(A[(n >> shift) as usize & 63]) } else { '=' });
        }
    }
    out
}

/// A picture of the given size, as a JPEG.
fn picture(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x * 255 / w) as u8, (y * 255 / h) as u8, 90]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn envelope(v: Value) -> Value {
    let text = json!({"status": "success", "stdout": "", "stderr": "", "return_value": v}).to_string();
    json!({"content": [{"type": "text", "text": text}], "isError": false})
}

/// What the hosted server's `aws___run_script` answers, by the AWS operation the script calls
/// (`powercloud_hosts`' rules). The first rule whose text is in the script wins.
fn hosted_rules(photo: &[u8], port: u16) -> Vec<Value> {
    let stack_ok = json!({"StackName": "powercloud", "StackStatus": "CREATE_COMPLETE", "Outputs": [
        {"OutputKey": "BucketName", "OutputValue": BUCKET},
        {"OutputKey": "FunctionName", "OutputValue": "powercloud-presign"},
        {"OutputKey": "TableName", "OutputValue": "PowerCloudPets"}]});
    let pets = json!([
        {"id": "20261001000001", "name": "Rex", "race": "Beagle", "age": 4, "photo": "photos/20261001000001.jpg"},
        {"id": "20261001000002", "name": "Luna", "race": "Siamese", "age": 2, "photo": "photos/20261001000002.jpg"}]);
    let presign = format!(r#"{{"url": "http://127.0.0.1:{port}/photo.jpg?X-Amz-Signature=SECRETSIG", "expires": 900}}"#);
    let rules: Vec<(&str, Value)> = vec![
        ("operation_name='Invoke'", json!({"Payload": presign, "FunctionError": ""})),
        (r#"operation_name=\"GetCallerIdentity\""#, json!({"Answer": {"Account": "DEMO-ACCOUNT", "Arn": ARN}, "Rows": [{"Account": "DEMO-ACCOUNT", "Arn": ARN}]})),
        (r#"operation_name=\"CreateStack\""#, json!({"Answer": {"StackId": "arn:aws:cloudformation:us-east-1:DEMO-ACCOUNT:stack/powercloud/1"}, "Rows": [{"StackId": "arn:aws:cloudformation:us-east-1:DEMO-ACCOUNT:stack/powercloud/1"}]})),
        (r#"operation_name=\"DescribeStacks\""#, json!({"Answer": {"Stacks": [stack_ok.clone()]}, "Rows": [stack_ok]})),
        (r#"operation_name=\"DescribeTable\""#, json!({"Status": "error", "Error": "ResourceNotFoundException: Requested resource not found"})),
        (r#"operation_name=\"CreateTable\""#, json!({"Answer": {"TableDescription": {"TableName": "PowerCloudPets"}}, "Rows": [{"TableName": "PowerCloudPets"}]})),
        ("operation_name='Scan'", json!({"Items": pets, "Count": 2})),
        ("operation_name='GetItem'", json!({"Items": [pets[0].clone()], "Found": "1"})),
        ("operation_name='PutItem'", json!({"Done": "1"})),
        ("operation_name='DeleteItem'", json!({"Done": "1"})),
        ("operation_name='PutObject'", json!({"ETag": "abc"})),
        ("operation_name='GetObject'", json!({"Base64": base64(photo), "Text": "", "ContentType": "image/jpeg"})),
        ("operation_name='DeleteObject'", json!({"Done": "1"})),
        ("operation_name='ListBuckets'", json!({"Buckets": [{"Name": "photos", "CreationDate": "2026-10-01T09:30:00+00:00"}, {"Name": "reports", "CreationDate": "2025-02-14T18:00:00+00:00"}]})),
        ("operation_name='ListObjectsV2'", json!({"Contents": [{"Key": "photos/20261001000001.jpg", "Size": 4096, "LastModified": "2026-10-07 10:00:00+00:00"}, {"Key": "photos/20261001000002.jpg", "Size": 5120, "LastModified": "2026-10-08 11:00:00+00:00"}]})),
        ("operation_name='SynthesizeSpeech'", json!({"Base64": "SUQzBAA=", "ContentType": "audio/mpeg", "Characters": 118})),
        ("operation_name='StartInstances'", json!({"Instances": [{"InstanceId": "i-0a1", "State": "pending", "Previous": "stopped"}]})),
        ("operation_name='StopInstances'", json!({"Instances": [{"InstanceId": "i-0a1", "State": "stopping", "Previous": "running"}]})),
        ("i-0a1", json!({"Instances": [{"InstanceId": "i-0a1", "Name": "web", "State": "running", "Type": "t3.micro"}]})),
        ("operation_name='DescribeInstances'", json!({"Instances": [
            {"InstanceId": "i-0a1", "Name": "web", "State": "stopped", "Type": "t3.micro"},
            {"InstanceId": "i-0b2", "Name": "database", "State": "running", "Type": "m5.large"}]})),
        ("operation_name='RecognizeCelebrities'", json!({"CelebrityFaces": [
            {"Name": "Ada Lovelace", "Id": "1a2B3c", "MatchConfidence": 99.5, "Urls": "www.wikidata.org/wiki/Q7259 www.imdb.com/name/nm0000001", "Face": {"BoundingBox": {"Left": 0.25, "Top": 0.25, "Width": 0.5, "Height": 0.5}}},
            {"Name": "Charles Babbage", "Id": "9z8Y7x", "MatchConfidence": 91.0, "Urls": "www.wikidata.org/wiki/Q46633", "Face": {"BoundingBox": {"Left": 0.0, "Top": 0.0, "Width": 0.1, "Height": 0.2}}}]})),
    ];
    rules.into_iter().map(|(contains, v)| json!({"tool": "aws___run_script", "contains": contains, "answer": envelope(v)})).collect()
}

/// The popups as `powercloud_hosts` stands them in: each answers the moment it opens.
fn write_doubles(root: &Path) {
    use cobolt_forms::{ControlType, Form};
    let code = |body: &str| {
        let indented: String = body.lines().map(|l| format!("           {l}\n")).collect();
        format!("       ENVIRONMENT DIVISION.\n\n       DATA DIVISION.\n\n       PROCEDURE DIVISION.\n\n{indented}")
    };
    let make = |name: &str, body: String| {
        let mut f = Form::new(name, name, 200, 100);
        f.user_ws_source = "       01 WS-TEXT PIC X(300) GLOBAL.\n       01 WS-ID PIC X(30) GLOBAL.\n       01 WS-PHOTO PIC X(300) GLOBAL.\n".to_owned();
        f.form_events[0].code = code(&body);
        f.controls.push(cobolt_forms::Control::new("Lbl-Dummy", ControlType::Label, 8, 8));
        cobolt_forms::save_form(&f, &root.join("forms").join(format!("{}.cfrm", name.to_lowercase()))).unwrap();
    };
    let photo = root.join("newpet.jpg").to_string_lossy().into_owned();
    make(
        "PET-FORM",
        format!(
            r#"INVOKE    super::GetProperty( "PetId" ) RETURNING WS-ID
IF WS-ID = SPACES
    INVOKE    super::SetProperty( "PetName", "Milo" )
    INVOKE    super::SetProperty( "PetRace", "Poodle" )
    INVOKE    super::SetProperty( "PetAge", "3" )
    INVOKE    super::SetProperty( "PetPhoto", "{photo}" )
ELSE
    INVOKE    super::GetProperty( "PetName" ) RETURNING WS-TEXT
    DISPLAY "EDIT-SEEN " WS-ID::Trim() "|" WS-TEXT::Trim()
    INVOKE    super::SetProperty( "PetName", "Rexy" )
    INVOKE    super::SetProperty( "PetPhoto", "" )
END-IF
INVOKE    super::SetProperty( "PetSaved", "Y" )
INVOKE    me::Close()"#
        ),
    );
    make(
        "VIEW-FORM",
        r#"INVOKE    super::GetProperty( "PetName" ) RETURNING WS-TEXT
INVOKE    super::GetProperty( "ViewPhoto" ) RETURNING WS-PHOTO
DISPLAY "VIEW-SEEN " WS-TEXT::Trim() "|" WS-PHOTO::Trim()
INVOKE    me::Close()"#
            .to_owned(),
    );
    make(
        "CONFIRM-FORM",
        r#"INVOKE    super::GetProperty( "UiLanguage" ) RETURNING WS-ID
DISPLAY "LANG-SEEN " WS-ID::Trim()
INVOKE    super::GetProperty( "ConfirmText" ) RETURNING WS-TEXT
DISPLAY "CONFIRM-SEEN " WS-TEXT::Trim()
INVOKE    super::SetProperty( "ConfirmAnswer", "Y" )
INVOKE    me::Close()"#
            .to_owned(),
    );
}

/// The project: the example's forms and programs, the popups' stand-ins, six connections that route
/// to the fake server, the pictures the scenarios hand the program. Returns the manifest.
fn project(root: &Path, routes: &str) -> PathBuf {
    let _ = std::fs::remove_dir_all(root);
    copy_dir(&example(), root, true);
    std::fs::create_dir_all(root.join("generated")).unwrap();
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::create_dir_all(root.join("home")).unwrap();
    std::fs::write(root.join("newpet.jpg"), picture(900, 600)).unwrap();
    std::fs::write(root.join("people.jpg"), picture(600, 400)).unwrap();
    write_doubles(root);
    let mut generated = Vec::new();
    for e in std::fs::read_dir(root.join("forms")).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(stem) = name.strip_suffix(".cfrm") {
            let form = cobolt_forms::load_form(&e.path()).unwrap();
            std::fs::write(root.join("generated").join(format!("{stem}.cbl")), cobolt_codegen::generate(&form)).unwrap();
            generated.push(toml::Value::String(format!("generated/{stem}.cbl")));
        }
    }
    generated.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    let manifest = root.join("PowerCloud.project.toml");
    let mut doc: toml::Value = toml::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    doc.get_mut("files").unwrap().as_table_mut().unwrap().insert("generated".into(), toml::Value::Array(generated));
    let connections: Vec<toml::Value> = ["us-east-1", "us-east-2", "us-west-2", "eu-west-1", "eu-central-1", "sa-east-1"]
        .iter()
        .map(|r| {
            let mut t = toml::map::Map::new();
            t.insert("id".into(), format!("pc-{r}").into());
            t.insert("name".into(), (*r).into());
            t.insert("profile".into(), "".into());
            t.insert("region".into(), (*r).into());
            t.insert("routes_override".into(), routes.into());
            toml::Value::Table(t)
        })
        .collect();
    let integrations = doc.as_table_mut().unwrap().entry("integrations").or_insert_with(|| toml::Value::Table(Default::default()));
    integrations.as_table_mut().unwrap().insert("aws_connections".into(), toml::Value::Array(connections));
    std::fs::write(&manifest, toml::to_string(&doc).unwrap()).unwrap();
    manifest
}

fn set(control: &str, property: &str, value: &str) -> Value {
    json!({"set": {"control": control, "property": property, "value": value}})
}
fn click(control: &str) -> Value {
    json!({"event": {"control": control, "name": "onClick"}})
}
fn get(control: &str, property: &str) -> Value {
    json!({"read": {"control": control, "property": property}})
}
fn wait(ms: u64) -> Value {
    json!({"wait_ms": ms})
}
fn event(control: &str, name: &str) -> Value {
    json!({"event": {"control": control, "name": name}})
}
fn shot(name: &str) -> Value {
    json!({"shot": name})
}
/// Picking a language in CMB-LANGUAGE, as the user does: the position, then the event.
fn pick(language: usize) -> Vec<Value> {
    vec![set("Cmb-Language", "SelectedIndex", &language.to_string()), event("Cmb-Language", "onSelectedIndexChanged"), wait(200)]
}

/// Every designed text the language picker changes (`powercloud_hosts`' list).
const LANGUAGE_TEXTS: &[(&str, &str)] = &[
    ("Lbl-Tagline", "Caption"), ("Tab-Main", "Tabs"), ("Lbl-SetHead", "Caption"), ("Lbl-SetHint", "Caption"), ("Lbl-Region", "Caption"),
    ("Lbl-KeyId", "Caption"), ("Lbl-Token", "Caption"), ("Txt-Token", "HintText"), ("Lbl-Secret", "Caption"), ("Btn-Connect", "Caption"),
    ("Lbl-Stack", "Caption"), ("Lbl-Bucket", "Caption"), ("Txt-Bucket", "HintText"), ("Lbl-Function", "Caption"), ("Btn-Deploy", "Caption"),
    ("Btn-Save", "Caption"), ("Lbl-Link", "Caption"), ("Lbl-DdbHead", "Caption"), ("Btn-CreateTable", "Caption"), ("Btn-NewPet", "Caption"),
    ("Btn-Reload", "Caption"), ("Btn-ClearFilters", "Caption"), ("Lbl-S3Head", "Caption"), ("Btn-S3Reload", "Caption"), ("Lbl-LamHead", "Caption"),
    ("Lbl-LamText", "Caption"), ("Lbl-LamSrc", "Caption"), ("Lbl-LamKey", "Caption"), ("Btn-LamTry", "Caption"), ("Lbl-PollyHead", "Caption"),
    ("Lbl-Lang", "Caption"), ("Cmb-Lang", "Items"), ("Btn-Speak", "Caption"), ("Lbl-Ec2Head", "Caption"), ("Btn-Ec2Reload", "Caption"),
    ("Lbl-RekHead", "Caption"), ("Drp-Rek", "Hint"), ("Btn-Recognize", "Caption"), ("Lbl-Conn", "Caption"), ("Lbl-StackState", "Caption"),
    ("Lbl-DdbState", "Caption"), ("Lbl-S3State", "Caption"), ("Lbl-LamResult", "Caption"), ("Lbl-PollyState", "Caption"), ("Lbl-Ec2State", "Caption"),
    ("Lbl-RekDetails", "Caption"),
];

/// One flow of the main form.
struct Scenario {
    name: &'static str,
    /// Run on the project's data as it was made (a test of its own in `powercloud_hosts`).
    fresh: bool,
    /// An answer of the fake server put before the others (the refusals).
    first_rule: Option<(&'static str, Value)>,
    steps: Vec<Value>,
    limit: u32,
    shots: Vec<&'static str>,
}

/// What one engine did in one scenario.
#[derive(Default, Clone)]
struct Seen {
    /// The value each step answered, by its place (`read` steps; the others answer nothing).
    values: Vec<Option<String>>,
    windows: Value,
    /// The lines the popups' stand-ins DISPLAYed.
    popups: Vec<String>,
    /// The requests the fake server received, the new pet's id masked, in order of arrival.
    calls: Vec<String>,
    ms: f64,
}

impl Seen {
    /// The last value read of `control::property`.
    fn read(&self, steps: &[Value], control: &str, property: &str) -> String {
        steps
            .iter()
            .zip(&self.values)
            .rev()
            .find(|(s, _)| s["read"]["control"].as_str() == Some(control) && s["read"]["property"].as_str() == Some(property))
            .and_then(|(_, v)| v.clone())
            .unwrap_or_default()
    }
}

/// A pet id made from the clock (the new pet's) is masked; the fixture's two are kept.
fn mask_ids(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_digit()) {
            let end = (i..b.len()).find(|&j| !b[j].is_ascii_digit()).unwrap_or(b.len());
            let run = &text[i..end];
            if run.len() == 14 && run.starts_with("20") && !matches!(run, "20261001000001" | "20261001000002") {
                out.push_str("<NEW-ID>");
            } else {
                out.push_str(run);
            }
            i = end;
        } else {
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// The photos in an engine's cache (`data` is its data folder): each name (the new pet's id masked) and
/// the sha-256 of its bytes.
fn cache(data: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(data.join("cache")).into_iter().flatten().flatten() {
        let bytes = std::fs::read(e.path()).unwrap_or_default();
        out.insert(mask_ids(&e.file_name().to_string_lossy()), format!("{:x}", Sha256::digest(&bytes)));
    }
    out
}

/// Pixels that differ between two pictures (all of them when the sizes differ or a picture is missing).
fn differing_pixels(a: &Path, b: &Path) -> (u64, (u32, u32)) {
    let (Ok(x), Ok(y)) = (image::open(a), image::open(b)) else { return (u64::MAX, (0, 0)) };
    let (x, y) = (x.to_rgba8(), y.to_rgba8());
    if x.dimensions() != y.dimensions() {
        return (u64::from(x.width()) * u64::from(x.height()), x.dimensions());
    }
    (x.pixels().zip(y.pixels()).filter(|(p, q)| p != q).count() as u64, x.dimensions())
}

#[derive(Clone, Copy, PartialEq)]
enum Engine {
    Interpreter,
    Runner,
}

/// The data as the project made it, put back before a fresh scenario.
fn restore(root: &Path, data: &Path) {
    let _ = std::fs::remove_dir_all(data);
    copy_dir(&root.join("seed-data"), data, false);
    let _ = std::fs::remove_file(root.join("hosted-calls.jsonl"));
}

/// The project's form theme (`[forms] theme`), which the IDE hands to `rcrun run-form` as
/// `--theme-default`.
fn forms_theme(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("PowerCloud.project.toml")).ok()?;
    let doc: toml::Value = toml::from_str(&text).ok()?;
    doc.get("forms")?.get("theme")?.as_str().map(str::to_owned).filter(|t| !t.is_empty())
}

/// The fake server's answers for a scenario.
fn write_hosted(root: &Path, rules: &[Value], first: Option<&(&'static str, Value)>) {
    let mut answers = rules.to_vec();
    if let Some((contains, v)) = first {
        answers.insert(0, json!({"tool": "aws___run_script", "contains": contains, "answer": envelope(v.clone())}));
    }
    let hosted = json!({
        "tools": [{"name": "aws___run_script", "inputSchema": {"type": "object", "properties": {"code": {"type": "string"}}, "required": ["code"]}}],
        "answers_when": answers,
        "call_log": root.join("hosted-calls.jsonl").to_string_lossy(),
        "pid_file": root.join("pids.txt").to_string_lossy(),
    });
    std::fs::write(root.join("fake-hosted.json"), hosted.to_string()).unwrap();
}

fn calls(root: &Path) -> Vec<String> {
    std::fs::read_to_string(root.join("hosted-calls.jsonl")).unwrap_or_default().lines().map(mask_ids).collect()
}

/// One scenario on one engine.
fn run(engine: Engine, root: &Path, exe: &Path, s: &Scenario, shots: &Path) -> Seen {
    let script = root.join("script.json");
    std::fs::write(&script, Value::Array(s.steps.clone()).to_string()).unwrap();
    let before = calls(root).len();
    let mut cmd = match engine {
        Engine::Interpreter => {
            let mut c = Command::new(rcrun());
            c.args(["run-form", "forms/main-form.cfrm", "generated/main-form.cbl", "--designer", "--headless"]).arg(&script).args(["--headless-limit", &s.limit.to_string()]);
            // The project's form theme, as the IDE hands it to run-form; a built program reads it from its
            // settings.
            if let Some(theme) = forms_theme(root) {
                c.args(["--theme-default", &theme]);
            }
            c
        }
        Engine::Runner => {
            let mut c = Command::new(exe);
            c.env("PRC_RUNNER_CACHE", root.join("runner-cache")).env("PRC_HEADLESS_SCRIPT", &script).env("PRC_HEADLESS_LIMIT", s.limit.to_string());
            c
        }
    };
    cmd.current_dir(root).env("HOME", root.join("home")).env("PRC_HEADLESS_SHOTS", shots);
    for v in AWS_VARS {
        cmd.env_remove(v);
    }
    let t = Instant::now();
    let out = cmd.output().expect("run the scenario");
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let who = if engine == Engine::Interpreter { "rcrun run-form" } else { "the built program" };
    assert!(out.status.success(), "{who} failed in {}:\n{stdout}\n{stderr}", s.name);
    for secret in [KEY_ID, SECRET, TOKEN, "SECRETSIG"] {
        assert!(!stdout.contains(secret) && !stderr.contains(secret), "a secret `{secret}` reached {who}'s output in {}", s.name);
    }
    let report: Value = stdout
        .lines()
        .find_map(|l| l.strip_prefix(cobolt_form_host::headless::RESULT_MARKER))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line from {who} in {}:\n{stdout}\n{stderr}", s.name));
    let values = report["steps"].as_array().map(|steps| steps.iter().map(|st| st["value"].as_str().map(str::to_owned)).collect()).unwrap_or_default();
    let popups = stdout.lines().filter(|l| ["EDIT-SEEN ", "VIEW-SEEN ", "CONFIRM-SEEN ", "LANG-SEEN "].iter().any(|p| l.trim_start().starts_with(p))).map(|l| l.trim().to_owned()).collect();
    Seen { values, windows: report["windows"].clone(), popups, calls: calls(root)[before..].to_vec(), ms }
}

fn scenarios(root: &Path) -> Vec<Scenario> {
    let people = root.join("people.jpg").to_string_lossy().into_owned();
    let sc =|name, fresh, steps: Vec<Value>, limit, shots: Vec<&'static str>| Scenario { name, fresh, first_rule: None, steps, limit, shots };
    let cell = |row: &str, column: &str| vec![set("Dg-Pets", "ClickedRow", row), set("Dg-Pets", "ClickedColumn", column), event("Dg-Pets", "onCellClick"), wait(2500)];
    let mut edit = vec![click("Btn-Reload"), wait(1500)];
    for (row, col) in [("1", "7"), ("1", "6"), ("2", "8")] {
        edit.extend(cell(row, col));
    }
    // The six languages: block 0 English as the form opens, 1 to 5 pt es fr jp cn, 6 English again.
    let reads = || LANGUAGE_TEXTS.iter().map(|(c, p)| get(c, p)).collect::<Vec<_>>();
    let mut languages = reads();
    for language in 2..=6 {
        languages.extend(pick(language));
        languages.extend(reads());
    }
    languages.extend(pick(1));
    languages.extend(reads());
    languages.push(shot("languages"));
    let mut status = vec![set("Txt-KeyId", "Text", KEY_ID), set("Txt-Secret", "Text", SECRET), set("Txt-Token", "Text", TOKEN), click("Btn-Connect"), wait(2500), get("Lbl-Conn", "Caption")];
    for language in [3, 5, 1] {
        status.extend(pick(language));
        status.push(get("Lbl-Conn", "Caption"));
    }
    status.extend([click("Btn-Reload"), wait(1500), get("Lbl-DdbState", "Caption")]);
    for language in [4, 6] {
        status.extend(pick(language));
        status.push(get("Lbl-DdbState", "Caption"));
    }
    status.extend(pick(1));
    status.extend([click("Btn-Ec2Reload"), wait(2000), get("Lbl-Ec2State", "Caption")]);
    status.extend(pick(2));
    status.push(get("Lbl-Ec2State", "Caption"));
    let mut popups = pick(2);
    popups.extend([click("Btn-Reload"), wait(1500)]);
    popups.extend(cell("2", "8"));
    vec![
        sc("Settings: keys handed over, GetCallerIdentity", true,
            vec![set("Txt-KeyId", "Text", KEY_ID), set("Txt-Secret", "Text", SECRET), set("Txt-Token", "Text", TOKEN), click("Btn-Connect"), wait(2500), get("Lbl-Conn", "Caption"), get("Txt-Secret", "Text"), get("Txt-KeyId", "Text"), shot("settings")], 60, vec!["settings"]),
        sc("Settings: stack created and polled", false, vec![click("Btn-Deploy"), wait(4500), get("Lbl-StackState", "Caption"), get("Txt-Bucket", "Text")], 60, vec![]),
        sc("DynamoDB: table, two pets, thumbnails", false,
            vec![click("Btn-CreateTable"), wait(1500), get("Lbl-DdbState", "Caption"), click("Btn-Reload"), wait(1500), get("Lbl-DdbState", "Caption"), get("Dg-Pets", "Rows"), wait(3000), shot("pets")], 60, vec!["pets"]),
        sc("S3: buckets and objects", false, vec![click("Btn-S3Reload"), wait(2000), get("Lbl-S3State", "Caption"), get("Dg-S3", "Rows")], 60, vec![]),
        sc("Lambda: a temporary link", false, vec![set("Txt-LamKey", "Text", "photos/20261001000001.jpg"), click("Btn-LamTry"), wait(2000), get("Lbl-LamResult", "Caption")], 60, vec![]),
        sc("Polly: a sentence spoken", false, vec![click("Btn-Speak"), wait(2500), get("Lbl-PollyState", "Caption")], 60, vec![]),
        sc("EC2: two servers", false, vec![click("Btn-Ec2Reload"), wait(2000), get("Lbl-Ec2State", "Caption"), get("Dg-Ec2", "Rows"), shot("servers")], 60, vec!["servers"]),
        sc("Rekognition: two boxes over a picture", false,
            vec![set("Drp-Rek", "DroppedFiles", &people), event("Drp-Rek", "onFilesDropped"), click("Btn-Recognize"), wait(2500), get("Lbl-RekDetails", "Caption"), get("Shp-Box1", "Visible"), get("Shp-Box1", "Width"), get("Shp-Box3", "Visible"), shot("faces")], 60, vec!["faces"]),
        sc("DynamoDB: a new pet (machine sign-in)", false,
            vec![click("Btn-Connect"), wait(2500), get("Lbl-Conn", "Caption"), get("Txt-Bucket", "Text"), click("Btn-NewPet"), wait(3500), get("Lbl-DdbState", "Caption")], 60, vec![]),
        sc("DynamoDB: edit, view and delete", false, edit, 90, vec![]),
        sc("EC2: a switch on, the Working window", false,
            vec![click("Btn-Ec2Reload"), wait(1500), set("Dg-Ec2", "EditedRow", "1"), set("Dg-Ec2", "EditedColumn", "7"), set("Dg-Ec2", "PreviousValue", "0"), set("Dg-Ec2", "EditedValue", "1"),
                event("Dg-Ec2", "onCellEdited"), wait(5500), get("Lbl-Ec2State", "Caption"), get("Dg-Ec2", "Enabled")], 60, vec![]),
        Scenario { name: "a photo with no stack and no bucket", fresh: true, first_rule: Some((r#"operation_name=\"DescribeStacks\""#, json!({"Status": "error", "Error": "ValidationError: Stack with id powercloud does not exist"}))),
            steps: vec![click("Btn-NewPet"), wait(3500), get("Lbl-DdbState", "Caption")], limit: 60, shots: vec![] },
        Scenario { name: "keys that cannot deploy", fresh: true, first_rule: Some((r#"operation_name=\"CreateStack\""#, json!({"Status": "error", "Error": "AccessDenied: User: arn:aws:iam::DEMO-ACCOUNT:user/demo is not authorized to perform: cloudformation:CreateStack"}))),
            steps: vec![click("Btn-Deploy"), wait(2500), get("Lbl-StackState", "Caption")], limit: 60, shots: vec![] },
        sc("six languages on the main form", true, languages, 60, vec!["languages"]),
        sc("a status line written again in a new language", true, status, 90, vec![]),
        sc("the popups told the language", true, popups, 90, vec![]),
    ]
}

/// What `powercloud_hosts` expects of each scenario, checked on the built program.
fn expected(s: &Scenario, seen: &Seen, data: &Path) -> Vec<(String, bool)> {
    let r = |c: &str, p: &str| seen.read(&s.steps, c, p);
    let calls = seen.calls.join("\n");
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut ck = |what: &str, ok: bool| out.push((format!("{}: {what}", s.name), ok));
    match s.name {
        "Settings: keys handed over, GetCallerIdentity" => {
            ck("connected as the demo user; the key boxes cleared", r("Lbl-Conn", "Caption").starts_with(&format!("Connected as {ARN}")) && r("Txt-Secret", "Text").is_empty() && r("Txt-KeyId", "Text").is_empty());
        }
        "Settings: stack created and polled" => {
            ck("CREATE_COMPLETE and the bucket read from the outputs", r("Lbl-StackState", "Caption") == "Stack powercloud: CREATE_COMPLETE" && r("Txt-Bucket", "Text") == BUCKET);
        }
        "DynamoDB: table, two pets, thumbnails" => {
            let rows = r("Dg-Pets", "Rows");
            ck("2 pets, Rex and Luna, each with its trash icon", r("Lbl-DdbState", "Caption") == "2 pet(s) in the table." && rows.lines().count() == 2 && rows.contains("Rex") && rows.contains("Luna") && rows.contains("icon:trash"));
            let thumbs = ["20261001000001", "20261001000002"].iter().all(|id| image::image_dimensions(data.join("cache").join(format!("{id}-40.png"))).is_ok_and(|d| d == (40, 40)));
            ck("two 40 x 40 thumbnails fitted", thumbs);
            let gets: Vec<&str> = seen.calls.iter().map(String::as_str).filter(|l| l.contains("operation_name='GetObject'")).collect();
            ck("each photo asked of the stack's bucket", ["photos/20261001000001.jpg", "photos/20261001000002.jpg"].iter().all(|k| gets.iter().any(|l| l.contains(k) && l.contains(BUCKET))));
        }
        "S3: buckets and objects" => ck("2 objects in photos, listed", r("Lbl-S3State", "Caption") == "2 object(s) in photos" && r("Dg-S3", "Rows").contains("photos/20261001000001.jpg")),
        "Lambda: a temporary link" => {
            let c = r("Lbl-LamResult", "Caption");
            ck("a link with its signature hidden", c.starts_with("Link: http://127.0.0.1:") && c.contains("signature hidden") && !c.contains("X-Amz"));
        }
        "Polly: a sentence spoken" => ck("118 characters; the audio saved", r("Lbl-PollyState", "Caption") == "Ready: 118 characters." && data.join("cache/speech.mp3").exists()),
        "EC2: two servers" => {
            let rows = r("Dg-Ec2", "Rows");
            ck("2 servers with vCPU and memory", r("Lbl-Ec2State", "Caption") == "2 server(s) in us-east-1" && rows.contains("t3.micro\t2\t1 GiB") && rows.contains("m5.large\t2\t8 GiB"));
        }
        "Rekognition: two boxes over a picture" => {
            let d = r("Lbl-RekDetails", "Caption");
            ck("two faces named; box 1 shown, 240 wide; box 3 hidden", d.contains("Ada Lovelace") && d.contains("Charles Babbage") && r("Shp-Box1", "Visible") == "true" && r("Shp-Box1", "Width") == "240" && r("Shp-Box3", "Visible") == "false");
        }
        "DynamoDB: a new pet (machine sign-in)" => {
            ck("Connect with empty boxes finds the bucket", r("Lbl-Conn", "Caption").starts_with(&format!("Connected as {ARN}")) && r("Txt-Bucket", "Text") == BUCKET);
            let put = seen.calls.iter().find(|l| l.contains("operation_name='PutObject'")).cloned().unwrap_or_default();
            ck("the photo put in the bucket; the item stored", put.contains(BUCKET) && calls.contains("photos/") && calls.contains("Milo") && calls.contains("Poodle"));
            let fitted = std::fs::read_dir(data.join("cache")).into_iter().flatten().flatten().any(|e| e.file_name().to_string_lossy().ends_with("-400.jpg") && image::image_dimensions(e.path()).is_ok_and(|d| d == (400, 400)));
            ck("a 400 x 400 photo fitted", fitted);
        }
        "DynamoDB: edit, view and delete" => {
            let said = seen.popups.join("\n");
            ck("the popups told the pet, its photo and the question", said.contains("EDIT-SEEN 20261001000001|Rex") && said.contains("VIEW-SEEN Rex|data/cache/20261001000001-400.jpg") && said.contains("CONFIRM-SEEN Delete this pet and its photo?"));
            ck("the edit stored, keeping the photo key; the pet and its photo deleted", calls.contains("Rexy") && calls.contains("photos/20261001000001.jpg") && calls.contains("DeleteItem") && calls.contains("20261001000002") && calls.contains("DeleteObject"));
        }
        "EC2: a switch on, the Working window" => {
            ck("the server started; the Working window closed; the grid enabled", calls.contains("StartInstances") && calls.contains("i-0a1") && seen.windows.as_array().map(Vec::len) == Some(0) && r("Dg-Ec2", "Enabled") == "true" && r("Lbl-Ec2State", "Caption") == "2 server(s) in us-east-1");
        }
        "a photo with no stack and no bucket" => {
            ck("refused with what to do; no PutObject", r("Lbl-DdbState", "Caption") == "No photo bucket yet: deploy the stack on the Settings tab, or type the bucket's name there." && calls.contains("DescribeStacks") && !calls.contains("operation_name='PutObject'"));
        }
        "keys that cannot deploy" => {
            ck("told how to deploy", r("Lbl-StackState", "Caption") == "These keys cannot deploy the stack: deploy with an administrator's keys, or Save template… and run it; then Connect.");
        }
        "six languages on the main form" => {
            let n = LANGUAGE_TEXTS.len();
            let block = |b: usize| -> Vec<String> {
                let first = if b == 0 { 0 } else { n + (b - 1) * (n + 3) + 3 };
                (0..n).map(|i| seen.values.get(first + i).cloned().flatten().unwrap_or_default()).collect()
            };
            let english = block(0);
            ck(&format!("{n} texts, none empty; English comes back as designed"), english.iter().all(|v| !v.is_empty()) && block(6) == english);
            let translated = (1..=5).all(|b| block(b).iter().zip(&english).all(|(v, e)| !v.is_empty() && v != e));
            ck("every text translated in pt es fr jp cn", translated);
            let at = |c: &str, p: &str| LANGUAGE_TEXTS.iter().position(|(x, y)| *x == c && *y == p).unwrap();
            let probes = [("Conectar", "Não conectado"), ("Conectar", "No conectado"), ("Se connecter", "Non connecté"), ("接続", "未接続"), ("连接", "未连接")];
            ck("Connect and the connection line in each language", probes.iter().enumerate().all(|(i, (b, l))| block(i + 1)[at("Btn-Connect", "Caption")] == *b && block(i + 1)[at("Lbl-Conn", "Caption")] == *l));
        }
        "a status line written again in a new language" => {
            let said: Vec<String> = s.steps.iter().zip(&seen.values).filter(|(st, _)| st.get("read").is_some()).map(|(_, v)| v.clone().unwrap_or_default()).collect();
            let want = [
                format!("Connected as {ARN}"), format!("Conectado como {ARN}"), format!("{ARN} として接続しました"), format!("Connected as {ARN}"),
                "2 pet(s) in the table.".into(), "2 animal(aux) dans la table.".into(), "表中有 2 只宠物。".into(), "2 server(s) in us-east-1".into(), "2 servidor(es) em us-east-1".into(),
            ];
            ck("9 lines in 3 tabs and 6 languages", said == want);
        }
        "the popups told the language" => {
            let said = seen.popups.join("\n");
            ck("the language and the question in Portuguese", said.contains("LANG-SEEN 2") && said.contains("CONFIRM-SEEN Excluir este animal e a foto dele?"));
        }
        _ => {}
    }
    out
}

#[test]
fn powercloud_agrees_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t48-powercloud-{}", std::process::id()));
    let photo = picture(1200, 800);
    let (port, photo_hits) = photo_server(photo.clone());
    let routes = format!("[servers.hosted]\ncommand = '{0}'\nargs = ['{1}']\n", fake_mcp().display(), root.join("fake-hosted.json").display());
    let manifest = project(&root, &routes);
    let rules = hosted_rules(&photo, port);

    // ── The runner, built beside rcrun, then the project on the WebAssembly path ──
    let t = Instant::now();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(&cargo).current_dir(&workspace).args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"]).status().expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-runner --bin prc-runner failed");
    let runner_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    let out = Command::new(rcrun())
        .arg("build")
        .arg(&manifest)
        .arg("--quiet")
        .env(cobolt_compiler::BUILD_DIR_ENV, root.join("builds"))
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env("HOME", root.join("home"))
        .env_remove(cobolt_compiler::RUNNER_ENV)
        .output()
        .expect("run rcrun build");
    let build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun build failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(said.lines().any(|l| l.trim() == "Path   : WebAssembly"), "the Build did not take the WebAssembly path:\n{said}");
    let exe = PathBuf::from(said.lines().find_map(|l| l.trim().strip_prefix("Binary : ")).expect("the Build names its binary").trim());
    let exe = if exe.is_absolute() { exe } else { root.join(exe) };
    copy_dir(&root.join("data"), &root.join("seed-data"), false);

    // ── Every scenario, in order, on each engine ──
    let list = scenarios(&root);
    let mut seen: [Vec<Seen>; 2] = [Vec::new(), Vec::new()];
    let mut caches: [BTreeMap<String, String>; 2] = [BTreeMap::new(), BTreeMap::new()];
    let mut expectations: Vec<(String, bool)> = Vec::new();
    let mut totals = [0.0f64; 2];
    for (k, engine) in [Engine::Interpreter, Engine::Runner].into_iter().enumerate() {
        // run-form works in the project's `data`; a built program in the `data` beside it, which the
        // Build filled.
        let data = if engine == Engine::Interpreter { root.join("data") } else { exe.parent().expect("the binary's folder").join("data") };
        let shots = root.join(if k == 0 { "shots-interpreter" } else { "shots-runner" });
        std::fs::create_dir_all(&shots).unwrap();
        for (i, s) in list.iter().enumerate() {
            if s.fresh {
                // The tabs end where the first fresh scenario after them begins.
                if i > 0 && caches[k].is_empty() {
                    caches[k] = cache(&data);
                }
                restore(&root, &data);
            }
            write_hosted(&root, &rules, s.first_rule.as_ref());
            let r = run(engine, &root, &exe, s, &shots);
            totals[k] += r.ms;
            if engine == Engine::Runner {
                expectations.extend(expected(s, &r, &data));
            }
            seen[k].push(r);
        }
    }

    // ── The two engines against each other ──
    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for (i, s) in list.iter().enumerate() {
        let (a, b) = (&seen[0][i], &seen[1][i]);
        let reads = s.steps.iter().filter(|st| st.get("read").is_some()).count();
        checks.push((format!("{}: {reads} reads equal", s.name), a.values == b.values));
        checks.push((format!("{}: {} hosted calls equal", s.name, a.calls.len()), a.calls == b.calls));
        checks.push((format!("{}: windows and popups equal", s.name), a.windows == b.windows && a.popups == b.popups));
        lines.push(format!("  {:<48} interpreter {:>6.1} s  runner {:>6.1} s  {:>2} reads  {:>2} calls", s.name, a.ms / 1000.0, b.ms / 1000.0, reads, a.calls.len()));
        if a.values != b.values {
            for (j, (x, y)) in a.values.iter().zip(&b.values).enumerate().filter(|(_, (x, y))| x != y) {
                lines.push(format!("    step {j} {}: interpreter {x:?} / runner {y:?}", s.steps[j]));
            }
        }
        if a.calls != b.calls {
            lines.push(format!("    calls: interpreter {} / runner {}", a.calls.len(), b.calls.len()));
            for (x, y) in a.calls.iter().zip(&b.calls).filter(|(x, y)| x != y).take(3) {
                let cut = |s: &str| s.chars().take(300).collect::<String>();
                lines.push(format!("      interpreter {}\n      runner      {}", cut(x), cut(y)));
            }
        }
        if a.popups != b.popups || a.windows != b.windows {
            lines.push(format!("    popups {:?} / {:?}; windows {} / {}", a.popups, b.popups, a.windows, b.windows));
        }
    }
    checks.push((format!("the photos in the cache after the tabs: {} files, equal byte for byte", caches[0].len()), !caches[0].is_empty() && caches[0] == caches[1]));
    let (mut pixels, mut differing) = (0u64, 0u64);
    for name in list.iter().flat_map(|s| s.shots.iter()) {
        let file = format!("{name}.png");
        let (diff, (w, h)) = differing_pixels(&root.join("shots-interpreter").join(&file), &root.join("shots-runner").join(&file));
        pixels += u64::from(w) * u64::from(h);
        differing = differing.saturating_add(diff);
        checks.push((format!("picture {name}: 0 differing pixels"), diff == 0));
        lines.push(format!("  picture {name}: {w}x{h}, {} differing", if diff == u64::MAX { "missing".to_string() } else { diff.to_string() }));
    }
    let hits = photo_hits.load(Ordering::SeqCst);
    checks.push(("no temporary link fetched outside the hosted server".into(), hits == 0));
    checks.extend(expectations);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== powercloud_agrees_on_both_engines (DESKTOP-WASM-001 T48: R10, R11; the twin of powercloud_hosts) ==");
    println!("PowerCloud: 5 forms (3 popups stood in), 6 AWS connections routed to the fake MCP server, {} scenarios on each engine, in order", list.len());
    println!("runner built in {runner_ms:.0} ms; rcrun build (WebAssembly path) {build_ms:.0} ms");
    println!("real time: interpreter {:.1} s, runner {:.1} s", totals[0] / 1000.0, totals[1] / 1000.0);
    for l in &lines {
        println!("{l}");
    }
    println!("pixels compared: {pixels}, differing: {differing}; hosted calls: {} and {}", seen[0].iter().map(|s| s.calls.len()).sum::<usize>(), seen[1].iter().map(|s| s.calls.len()).sum::<usize>());
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());
    if passed == checks.len() && std::env::var("PC_KEEP").is_err() {
        let _ = std::fs::remove_dir_all(&root);
    } else {
        println!("kept: {}", root.display());
    }
    assert_eq!(passed, checks.len());
}
