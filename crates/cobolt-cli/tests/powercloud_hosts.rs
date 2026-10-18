// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 092 R30 / AC7 — the PowerCloud example, tab by tab, under `rcrun run-form
//! --headless`. The example's own forms are copied into a temporary project whose six
//! AWS connections route the hosted server — every AWS call, the Lambda tab's included,
//! since 1.91.210 — to the fake MCP server (`cobolt-runtime`'s test binary), a local web
//! server stands in for S3's temporary link, and each tab's flow is driven by a script of
//! steps. Nothing reaches AWS.
//!
//! Every scenario reports what it exercised and how long it took; one summary block is
//! printed at the end (GOLDEN RULE #7).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};

const KEY_ID: &str = "AKIAPLANTEDKEY0000X";
const SECRET: &str = "planted-secret-value-0123456789abcdefghij";
const TOKEN: &str = "planted-session-token-9876543210";

struct Project {
    root: PathBuf,
    /// Requests that reached the server behind the temporary links. Every AWS call goes
    /// through MCP (operator, 2026-10-18), so the application never fetches a link itself.
    photo_hits: Arc<AtomicUsize>,
}

impl Drop for Project {
    fn drop(&mut self) {
        if std::env::var("PC_KEEP").is_ok() {
            println!("kept: {}", self.root.display());
            return;
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

fn fake_mcp() -> PathBuf {
    let path = rcrun().with_file_name(format!("fake_mcp{}", std::env::consts::EXE_SUFFIX));
    if !path.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let status = Command::new(cargo)
            .args(["build", "-p", "cobolt-runtime", "--bin", "fake_mcp"])
            .status()
            .expect("cargo builds the fake MCP server");
        assert!(status.success(), "the fake MCP server did not build");
    }
    path
}

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerCloud").canonicalize().unwrap()
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst);
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

/// What the hosted server's `aws___run_script` answers, by the AWS operation the script
/// calls. The first rule whose text is in the script wins, so the specific ones come first.
fn hosted_rules(photo: &[u8]) -> Value {
    let stack_ok = json!({"StackName": "powercloud", "StackStatus": "CREATE_COMPLETE", "Outputs": [
        {"OutputKey": "BucketName", "OutputValue": "powercloud-photos-1c2fa620-982a-11e3-aff7-50e2416294e0"},
        {"OutputKey": "FunctionName", "OutputValue": "powercloud-presign"},
        {"OutputKey": "TableName", "OutputValue": "PowerCloudPets"}]});
    let pets = json!([
        {"id": "20261001000001", "name": "Rex", "race": "Beagle", "age": 4, "photo": "photos/20261001000001.jpg"},
        {"id": "20261001000002", "name": "Luna", "race": "Siamese", "age": 2, "photo": "photos/20261001000002.jpg"}]);
    let rules: Vec<(&str, Value)> = vec![
        (r#"operation_name=\"GetCallerIdentity\""#, json!({"Answer": {"Account": "123456789012", "Arn": "arn:aws:iam::123456789012:user/demo"}, "Rows": [{"Account": "123456789012", "Arn": "arn:aws:iam::123456789012:user/demo"}]})),
        (r#"operation_name=\"CreateStack\""#, json!({"Answer": {"StackId": "arn:aws:cloudformation:us-east-1:123456789012:stack/powercloud/1"}, "Rows": [{"StackId": "arn:aws:cloudformation:us-east-1:123456789012:stack/powercloud/1"}]})),
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
        // Describe with an id in it (the Working window's poll): the server is up by now.
        ("i-0a1", json!({"Instances": [{"InstanceId": "i-0a1", "Name": "web", "State": "running", "Type": "t3.micro"}]})),
        ("operation_name='DescribeInstances'", json!({"Instances": [
            {"InstanceId": "i-0a1", "Name": "web", "State": "stopped", "Type": "t3.micro"},
            {"InstanceId": "i-0b2", "Name": "database", "State": "running", "Type": "m5.large"}]})),
        ("operation_name='RecognizeCelebrities'", json!({"CelebrityFaces": [
            {"Name": "Ada Lovelace", "Id": "1a2B3c", "MatchConfidence": 99.5, "Urls": "www.wikidata.org/wiki/Q7259 www.imdb.com/name/nm0000001", "Face": {"BoundingBox": {"Left": 0.25, "Top": 0.25, "Width": 0.5, "Height": 0.5}}},
            {"Name": "Charles Babbage", "Id": "9z8Y7x", "MatchConfidence": 91.0, "Urls": "www.wikidata.org/wiki/Q46633", "Face": {"BoundingBox": {"Left": 0.0, "Top": 0.0, "Width": 0.1, "Height": 0.2}}}]})),
    ];
    Value::Array(
        rules
            .into_iter()
            .map(|(contains, v)| json!({"tool": "aws___run_script", "contains": contains, "answer": envelope(v)}))
            .collect(),
    )
}

/// A popup that answers the moment it opens, standing in for the real one: the headless
/// harness drives the main window only, so a popup that waits for a click would block the
/// flow for good. The main form's side of every protocol (the properties it sets for the
/// popup, the ones it reads back, what it then sends to AWS) is exercised for real.
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

/// The example's forms and programs, six connections that route to the fake servers, a
/// local photo server, and the example's template where the form expects it.
fn project(tag: &str) -> Project {
    let root = std::env::temp_dir().join(format!("prc-092-hosts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&example(), &root);
    std::fs::create_dir_all(root.join("generated")).unwrap();
    std::fs::write(root.join("newpet.jpg"), picture(900, 600)).unwrap();
    write_doubles(&root);
    let mut forms = Vec::new();
    let mut generated = Vec::new();
    for e in std::fs::read_dir(root.join("forms")).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(stem) = name.strip_suffix(".cfrm") {
            let form = cobolt_forms::load_form(&e.path()).unwrap();
            std::fs::write(root.join("generated").join(format!("{stem}.cbl")), cobolt_codegen::generate(&form)).unwrap();
            forms.push(format!("\"forms/{stem}.cfrm\""));
            generated.push(format!("\"generated/{stem}.cbl\""));
        }
    }

    let photo = picture(1200, 800);
    let (port, photo_hits) = photo_server(photo.clone());
    // The presign function's answer, as Lambda's Invoke returns it to the hosted server's
    // script (since 1.91.210 the Lambda tab needs no uvx either).
    let presign = format!(r#"{{"url": "http://127.0.0.1:{port}/photo.jpg?X-Amz-Signature=SECRETSIG", "expires": 900}}"#);
    let mut rules = hosted_rules(&photo);
    rules.as_array_mut().unwrap().insert(
        0,
        json!({"tool": "aws___run_script", "contains": "operation_name='Invoke'", "answer": envelope(json!({"Payload": presign, "FunctionError": ""}))}),
    );
    let hosted = json!({
        "tools": [{"name": "aws___run_script", "inputSchema": {"type": "object", "properties": {"code": {"type": "string"}}, "required": ["code"]}}],
        "answers_when": rules,
        "call_log": root.join("hosted-calls.jsonl").to_string_lossy(),
        "pid_file": root.join("pids.txt").to_string_lossy(),
    });
    std::fs::write(root.join("fake-hosted.json"), hosted.to_string()).unwrap();
    let fake = fake_mcp();
    let routes = format!("[servers.hosted]\ncommand = '{0}'\nargs = ['{1}']\n", fake.display(), root.join("fake-hosted.json").display());

    // The example's manifest, with its own connections swapped for the test's.
    let manifest_path = root.join("PowerCloud.project.toml");
    let text = std::fs::read_to_string(&manifest_path).unwrap();
    let mut doc: toml::Value = toml::from_str(&text).unwrap();
    let files = doc.get_mut("files").unwrap().as_table_mut().unwrap();
    files.insert("generated".into(), toml::Value::Array(generated.iter().map(|g| toml::Value::String(g.trim_matches('"').to_owned())).collect()));
    let _ = forms;
    let connections: Vec<toml::Value> = ["us-east-1", "us-east-2", "us-west-2", "eu-west-1", "eu-central-1", "sa-east-1"]
        .iter()
        .map(|r| {
            let mut t = toml::map::Map::new();
            t.insert("id".into(), format!("pc-{r}").into());
            t.insert("name".into(), (*r).into());
            t.insert("profile".into(), "".into());
            t.insert("region".into(), (*r).into());
            t.insert("routes_override".into(), routes.clone().into());
            toml::Value::Table(t)
        })
        .collect();
    let integrations = doc.as_table_mut().unwrap().entry("integrations").or_insert_with(|| toml::Value::Table(Default::default()));
    integrations.as_table_mut().unwrap().insert("aws_connections".into(), toml::Value::Array(connections));
    std::fs::write(&manifest_path, toml::to_string(&doc).unwrap()).unwrap();
    Project { root, photo_hits }
}

fn run_form(p: &Project, steps: &Value, limit: u32) -> Value {
    let script = p.root.join("steps.json");
    std::fs::write(&script, steps.to_string()).unwrap();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/main-form.cfrm", "generated/main-form.cbl", "--designer", "--headless", script.to_str().unwrap(), "--headless-limit", &limit.to_string()])
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if std::env::var("PC_VERBOSE").is_ok() {
        println!("---- run-form stdout ----\n{stdout}\n---- stderr ----\n{stderr}");
    }
    assert!(out.status.success(), "run-form failed:\n{stdout}\n{stderr}");
    for secret in [KEY_ID, SECRET, TOKEN, "SECRETSIG"] {
        assert!(!stdout.contains(secret) && !stderr.contains(secret), "a secret `{secret}` reached the output:\n{stdout}\n{stderr}");
    }
    let mut report: Value = stdout
        .lines()
        .find_map(|l| l.strip_prefix("@RUN-FORM-RESULT "))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}\n{stderr}"));
    report["_stdout"] = Value::String(stdout);
    report
}

/// What the run DISPLAYed.
fn shown(report: &Value) -> &str {
    report["_stdout"].as_str().unwrap_or("")
}

/// The hosted server's call log, one JSON request per line, as text.
fn hosted_log(p: &Project) -> String {
    std::fs::read_to_string(p.root.join("hosted-calls.jsonl")).unwrap_or_default()
}

fn read(report: &Value, control: &str, property: &str) -> String {
    let key = format!("{control}::{property}");
    report["reads"]
        .as_object()
        .and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case(&key)))
        .and_then(|(_, v)| v.as_str())
        .unwrap_or_else(|| panic!("no read of {key} in {report}"))
        .to_owned()
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

fn calls_to(p: &Project) -> usize {
    std::fs::read_to_string(p.root.join("hosted-calls.jsonl")).map(|t| t.lines().count()).unwrap_or(0)
}

#[test]
fn every_tab_of_powercloud_runs_against_the_fake_servers() {
    let p = project("tabs");
    let mut summary: Vec<String> = Vec::new();
    let mut phase = |name: &str, what: &str, p: &Project, f: &dyn Fn(&Project)| {
        let t = Instant::now();
        let before = calls_to(p);
        f(p);
        let line = format!("  {name:<12} {what:<64} {:>6.1} s  {:>2} hosted call(s)", t.elapsed().as_secs_f32(), calls_to(p) - before);
        println!("{line}");
        summary.push(line);
    };

    phase("Settings", "keys handed over and cleared; GetCallerIdentity answers", &p, &|p| {
        let r = run_form(
            p,
            &json!([set("Txt-KeyId", "Text", KEY_ID), set("Txt-Secret", "Text", SECRET), set("Txt-Token", "Text", TOKEN), click("Btn-Connect"), wait(2500), get("Lbl-Conn", "Caption"), get("Txt-Secret", "Text"), get("Txt-KeyId", "Text")]),
            60,
        );
        assert!(read(&r, "Lbl-Conn", "Caption").starts_with("Connected as arn:aws:iam::123456789012:user/demo"), "{r}");
        assert_eq!(read(&r, "Txt-Secret", "Text"), "");
        assert_eq!(read(&r, "Txt-KeyId", "Text"), "");
    });

    phase("Settings", "stack created, polled until CREATE_COMPLETE, outputs read", &p, &|p| {
        let r = run_form(p, &json!([click("Btn-Deploy"), wait(4500), get("Lbl-StackState", "Caption"), get("Txt-Bucket", "Text")]), 60);
        assert_eq!(read(&r, "Lbl-StackState", "Caption"), "Stack powercloud: CREATE_COMPLETE", "{r}");
        assert_eq!(read(&r, "Txt-Bucket", "Text"), "powercloud-photos-1c2fa620-982a-11e3-aff7-50e2416294e0");
    });

    phase("DynamoDB", "table created; two pets read; thumbnails fetched and fitted", &p, &|p| {
        let r = run_form(
            p,
            &json!([click("Btn-CreateTable"), wait(1500), get("Lbl-DdbState", "Caption"), click("Btn-Reload"), wait(1500), get("Lbl-DdbState", "Caption"), get("Dg-Pets", "Rows"), wait(3000)]),
            60,
        );
        assert_eq!(read(&r, "Lbl-DdbState", "Caption"), "2 pet(s) in the table.");
        let rows = read(&r, "Dg-Pets", "Rows");
        assert_eq!(rows.lines().count(), 2, "{rows}");
        assert!(rows.contains("Rex") && rows.contains("Luna") && rows.contains("icon:trash"), "{rows}");
        for id in ["20261001000001", "20261001000002"] {
            let thumb = p.root.join("data/cache").join(format!("{id}-40.png"));
            let dims = image::image_dimensions(&thumb).unwrap_or_else(|e| panic!("no thumbnail {}: {e}", thumb.display()));
            assert_eq!(dims, (40, 40));
        }
        // A run that never pressed Connect: the bucket came from the stack, and the photos
        // through AWS's MCP server.
        let log = hosted_log(p);
        let gets: Vec<_> = log.lines().filter(|l| l.contains("operation_name='GetObject'")).collect();
        for key in ["photos/20261001000001.jpg", "photos/20261001000002.jpg"] {
            assert!(gets.iter().any(|l| l.contains(key) && l.contains("powercloud-photos-1c2fa620-982a-11e3-aff7-50e2416294e0")), "no GetObject of {key} in the bucket");
        }
    });

    phase("S3", "buckets listed; objects of the first one in a grid", &p, &|p| {
        let r = run_form(p, &json!([click("Btn-S3Reload"), wait(2000), get("Lbl-S3State", "Caption"), get("Dg-S3", "Rows")]), 60);
        assert_eq!(read(&r, "Lbl-S3State", "Caption"), "2 object(s) in photos");
        assert!(read(&r, "Dg-S3", "Rows").contains("photos/20261001000001.jpg"));
    });

    phase("Lambda", "a temporary link asked for; the signature stays hidden", &p, &|p| {
        let r = run_form(p, &json!([set("Txt-LamKey", "Text", "photos/20261001000001.jpg"), click("Btn-LamTry"), wait(2000), get("Lbl-LamResult", "Caption")]), 60);
        let caption = read(&r, "Lbl-LamResult", "Caption");
        assert!(caption.starts_with("Link: http://127.0.0.1:") && caption.contains("signature hidden") && !caption.contains("X-Amz"), "{caption}");
    });

    phase("Polly", "a sentence spoken; the audio saved; the player told", &p, &|p| {
        let r = run_form(p, &json!([click("Btn-Speak"), wait(2500), get("Lbl-PollyState", "Caption")]), 60);
        assert_eq!(read(&r, "Lbl-PollyState", "Caption"), "Ready: 118 characters.");
        assert!(p.root.join("data/cache/speech.mp3").exists());
    });

    phase("EC2", "two servers listed with vCPU and memory", &p, &|p| {
        let r = run_form(p, &json!([click("Btn-Ec2Reload"), wait(2000), get("Lbl-Ec2State", "Caption"), get("Dg-Ec2", "Rows")]), 60);
        assert_eq!(read(&r, "Lbl-Ec2State", "Caption"), "2 server(s) in us-east-1");
        let rows = read(&r, "Dg-Ec2", "Rows");
        assert!(rows.contains("t3.micro\t2\t1 GiB") && rows.contains("m5.large\t2\t8 GiB"), "{rows}");
    });

    phase("Rekognition", "a picture sent; two boxes placed over it", &p, &|p| {
        let file = p.root.join("people.jpg");
        std::fs::write(&file, picture(600, 400)).unwrap();
        let r = run_form(
            p,
            &json!([set("Drp-Rek", "DroppedFiles", file.to_str().unwrap()), json!({"event": {"control": "Drp-Rek", "name": "onFilesDropped"}}), click("Btn-Recognize"), wait(2500), get("Lbl-RekDetails", "Caption"), get("Shp-Box1", "Visible"), get("Shp-Box1", "Width"), get("Shp-Box3", "Visible")]),
            60,
        );
        let details = read(&r, "Lbl-RekDetails", "Caption");
        assert!(details.contains("Ada Lovelace") && details.contains("Charles Babbage"), "{details}");
        assert_eq!(read(&r, "Shp-Box1", "Visible"), "true");
        assert_eq!(read(&r, "Shp-Box3", "Visible"), "false");
        // 600 x 400 in a 480 x 420 picture box: scale 0.8, the face is half the picture wide.
        assert_eq!(read(&r, "Shp-Box1", "Width"), "240");
    });

    // Operator, 2026-10-18: the example runs on any machine, with any account. Boxes left
    // empty mean the machine's own AWS sign-in (`aws configure`, IAM Identity Center ...);
    // Connect used to stop at "both keys are needed" and never reached AWS.
    phase("DynamoDB", "a new pet: Connect (machine sign-in) finds the bucket; photo put, item stored", &p, &|p| {
        let r = run_form(
            p,
            &json!([click("Btn-Connect"), wait(2500), get("Lbl-Conn", "Caption"), get("Txt-Bucket", "Text"), click("Btn-NewPet"), wait(3500), get("Lbl-DdbState", "Caption")]),
            60,
        );
        assert!(read(&r, "Lbl-Conn", "Caption").starts_with("Connected as arn:aws:iam::123456789012:user/demo"), "{r}");
        assert_eq!(read(&r, "Txt-Bucket", "Text"), "powercloud-photos-1c2fa620-982a-11e3-aff7-50e2416294e0", "Connect did not read the stack's outputs: {}", read(&r, "Lbl-Conn", "Caption"));
        let log = hosted_log(p);
        let put = log.lines().find(|l| l.contains("operation_name='PutObject'")).unwrap_or("");
        assert!(put.contains("powercloud-photos-1c2fa620-982a-11e3-aff7-50e2416294e0"), "PutObject did not name the bucket: {put}");
        assert!(log.contains("PutObject") && log.contains("photos/"), "no photo was put in S3");
        assert!(log.contains("Milo") && log.contains("Poodle"), "the item was not stored");
        let made: Vec<_> = std::fs::read_dir(p.root.join("data/cache")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with("-400.jpg")).collect();
        let fitted = made.iter().find(|n| image::image_dimensions(p.root.join("data/cache").join(n)).is_ok_and(|d| d == (400, 400)));
        assert!(fitted.is_some(), "no 400 x 400 photo among {made:?} ({})", shown(&r));
    });

    phase("DynamoDB", "edit, view and delete a pet through their popups", &p, &|p| {
        let click_cell = |row: &str, column: &str| {
            json!([set("Dg-Pets", "ClickedRow", row), set("Dg-Pets", "ClickedColumn", column), json!({"event": {"control": "Dg-Pets", "name": "onCellClick"}}), wait(2500)])
        };
        let mut s = vec![click("Btn-Reload"), wait(1500)];
        for (row, col) in [("1", "7"), ("1", "6"), ("2", "8")] {
            s.extend(click_cell(row, col).as_array().unwrap().clone());
        }
        let r = run_form(p, &Value::Array(s), 90);
        let out = shown(&r);
        assert!(out.contains("EDIT-SEEN 20261001000001|Rex"), "the edit popup was not told which pet: {out}");
        assert!(out.contains("VIEW-SEEN Rex|data/cache/20261001000001-400.jpg"), "the view popup was not given the pet and its photo: {out}");
        assert!(out.contains("CONFIRM-SEEN Delete this pet and its photo?"), "{out}");
        let log = hosted_log(p);
        assert!(log.contains("Rexy"), "the edited name was not stored");
        assert!(log.contains("photos/20261001000001.jpg"), "an edit without a new photo lost the photo key");
        assert!(log.contains("DeleteItem") && log.contains("20261001000002"), "the pet was not deleted");
        assert!(log.contains("DeleteObject"), "the photo was not deleted");
        assert!(p.root.join("data/cache/20261001000001-400.jpg").exists());
    });

    phase("EC2", "a switch turned on: server started, Working window up, closed when running", &p, &|p| {
        let r = run_form(
            p,
            &json!([click("Btn-Ec2Reload"), wait(1500), set("Dg-Ec2", "EditedRow", "1"), set("Dg-Ec2", "EditedColumn", "7"), set("Dg-Ec2", "PreviousValue", "0"), set("Dg-Ec2", "EditedValue", "1"),
                json!({"event": {"control": "Dg-Ec2", "name": "onCellEdited"}}), wait(5500), get("Lbl-Ec2State", "Caption"), get("Dg-Ec2", "Enabled")]),
            60,
        );
        assert!(hosted_log(p).contains("StartInstances") && hosted_log(p).contains("i-0a1"), "the server was not started");
        assert_eq!(r["windows"].as_array().map(Vec::len), Some(0), "the Working window is still open: {}", r["windows"]);
        assert_eq!(read(&r, "Dg-Ec2", "Enabled"), "true");
        assert_eq!(read(&r, "Lbl-Ec2State", "Caption"), "2 server(s) in us-east-1");
    });

    assert_eq!(p.photo_hits.load(Ordering::SeqCst), 0, "the application fetched a temporary link itself, outside MCP");
    println!("\nspec 092 — PowerCloud, tab by tab, against the fake servers\n{}\n  total hosted calls: {}   (no secret reached any output: {KEY_ID}, the secret and the token were planted)", summary.join("\n"), calls_to(&p));
}

/// Operator, 2026-10-18: a run after the one that deployed the stack sent PutObject with an
/// empty bucket name. With no stack and no bucket typed, a pet with a photo is refused with
/// what to do, and nothing goes to S3.
#[test]
fn without_a_stack_or_a_bucket_a_photo_is_refused_and_nothing_is_sent() {
    let p = project("no-stack");
    let path = p.root.join("fake-hosted.json");
    let mut hosted: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let missing = envelope(json!({"Status": "error", "Error": "ValidationError: Stack with id powercloud does not exist"}));
    hosted["answers_when"].as_array_mut().unwrap().insert(0, json!({"tool": "aws___run_script", "contains": r#"operation_name=\"DescribeStacks\""#, "answer": missing}));
    std::fs::write(&path, hosted.to_string()).unwrap();
    let r = run_form(&p, &json!([click("Btn-NewPet"), wait(3500), get("Lbl-DdbState", "Caption")]), 60);
    assert_eq!(
        read(&r, "Lbl-DdbState", "Caption"),
        "No photo bucket yet: deploy the stack on the Settings tab, or type the bucket's name there.",
        "{r}"
    );
    let log = hosted_log(&p);
    assert!(log.contains("DescribeStacks"), "the stack was not asked for its bucket");
    assert!(!log.contains("operation_name='PutObject'"), "a PutObject went out without a bucket");
}

/// Keys made to run the demo are not allowed to deploy it: the Settings tab says what to do,
/// rather than only AWS's refusal (operator, 2026-10-18).
#[test]
fn keys_that_cannot_deploy_are_told_how_to_deploy() {
    let p = project("no-deploy");
    let path = p.root.join("fake-hosted.json");
    let mut hosted: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let refused = envelope(json!({"Status": "error", "Error": "AccessDenied: User: arn:aws:iam::123456789012:user/demo is not authorized to perform: cloudformation:CreateStack"}));
    hosted["answers_when"].as_array_mut().unwrap().insert(0, json!({"tool": "aws___run_script", "contains": r#"operation_name=\"CreateStack\""#, "answer": refused}));
    std::fs::write(&path, hosted.to_string()).unwrap();
    let r = run_form(&p, &json!([click("Btn-Deploy"), wait(2500), get("Lbl-StackState", "Caption")]), 60);
    assert_eq!(
        read(&r, "Lbl-StackState", "Caption"),
        "These keys cannot deploy the stack: deploy with an administrator's keys, or Save template… and run it; then Connect.",
        "{r}"
    );
}

// ---- spec 092, amendment A1: the six languages ----------------------------------------------------

/// Picking a language in CMB-LANGUAGE, as the user does: the position, then the event.
fn pick(language: usize) -> Vec<Value> {
    vec![set("Cmb-Language", "SelectedIndex", &language.to_string()), json!({"event": {"control": "Cmb-Language", "name": "onSelectedIndexChanged"}}), wait(200)]
}

/// The value a `read` step of a script answered.
fn step_value(report: &Value, step: usize) -> String {
    report["steps"][step]["value"].as_str().unwrap_or_else(|| panic!("step {step} read nothing: {}", report["steps"][step])).to_owned()
}

/// Every designed text the language picker changes: the main form's captions, hints, tab
/// titles, list of voices and its eight status lines.
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

/// The language picker shows every text of the main form in each of the six languages, and
/// English comes back exactly as it was designed.
#[test]
fn powercloud_shows_every_text_of_its_main_form_in_each_of_the_six_languages() {
    let t = Instant::now();
    let p = project("languages");
    let n = LANGUAGE_TEXTS.len();
    let reads = || LANGUAGE_TEXTS.iter().map(|(c, pr)| get(c, pr)).collect::<Vec<_>>();
    // Block 0: English as the form opens. Blocks 1 to 5: pt es fr jp cn. Block 6: English again.
    let mut steps = reads();
    for language in 2..=6 {
        steps.extend(pick(language));
        steps.extend(reads());
    }
    steps.extend(pick(1));
    steps.extend(reads());
    let r = run_form(&p, &Value::Array(steps), 60);
    let block = |b: usize| -> Vec<String> {
        let first = if b == 0 { 0 } else { n + (b - 1) * (n + 3) + 3 };
        (0..n).map(|i| step_value(&r, first + i)).collect()
    };
    let english = block(0);
    assert!(english.iter().all(|v| !v.is_empty()), "an empty text in English: {english:?}");
    assert_eq!(block(6), english, "English did not come back as it was designed");

    let names = ["pt", "es", "fr", "jp", "cn"];
    let (mut checked, mut changed) = (0, 0);
    for b in 1..=5 {
        let texts = block(b);
        for (i, ((control, property), value)) in LANGUAGE_TEXTS.iter().zip(&texts).enumerate() {
            checked += 1;
            assert!(!value.is_empty(), "{control}::{property} is empty in {}", names[b - 1]);
            assert!(value != &english[i], "{control}::{property} is still English in {}: {value:?}", names[b - 1]);
            changed += 1;
        }
    }
    let probes = [
        ("pt", "Conectar", "Não conectado"),
        ("es", "Conectar", "No conectado"),
        ("fr", "Se connecter", "Non connecté"),
        ("jp", "接続", "未接続"),
        ("cn", "连接", "未连接"),
    ];
    let index = |control: &str, property: &str| LANGUAGE_TEXTS.iter().position(|(c, pr)| *c == control && *pr == property).unwrap();
    for (b, (name, connect, conn)) in probes.iter().enumerate() {
        let texts = block(b + 1);
        assert_eq!(&texts[index("Btn-Connect", "Caption")], connect, "{name}");
        assert_eq!(&texts[index("Lbl-Conn", "Caption")], conn, "{name}");
    }
    let tabs = |b: usize| block(b)[index("Tab-Main", "Tabs")].clone();
    assert_eq!(tabs(0), "Settings\nDynamoDB\nS3\nLambda\nPolly\nEC2\nRekognition");
    assert_eq!(tabs(1), "Configurações\nDynamoDB\nS3\nLambda\nPolly\nEC2\nRekognition");
    assert_eq!(read_voices(&block(5), index("Cmb-Lang", "Items")), 6, "the voices lost an entry in Chinese");
    println!(
        "\nspec 092 amendment A1 — six languages on the main form\n  {n} texts x 6 languages read back; {checked} translated texts checked, {changed} differ from English;\n  English after a round trip identical to the design; {:.1} s",
        t.elapsed().as_secs_f32()
    );
}

fn read_voices(texts: &[String], at: usize) -> usize {
    texts[at].lines().count()
}

/// A status line is written again in the language picked after it was shown: the message and
/// what went into it are remembered, not the text.
#[test]
fn a_status_line_is_written_again_when_the_language_changes() {
    let t = Instant::now();
    let p = project("status");
    let mut steps = vec![set("Txt-KeyId", "Text", KEY_ID), set("Txt-Secret", "Text", SECRET), set("Txt-Token", "Text", TOKEN), click("Btn-Connect"), wait(2500), get("Lbl-Conn", "Caption")];
    let at = |steps: &Vec<Value>| steps.len() - 1;
    let mut found = Vec::new();
    found.push(("Lbl-Conn en", at(&steps)));
    for language in [3, 5, 1] {
        steps.extend(pick(language));
        steps.push(get("Lbl-Conn", "Caption"));
        found.push(("Lbl-Conn", at(&steps)));
    }
    steps.extend([click("Btn-Reload"), wait(1500), get("Lbl-DdbState", "Caption")]);
    found.push(("Lbl-DdbState en", at(&steps)));
    for language in [4, 6] {
        steps.extend(pick(language));
        steps.push(get("Lbl-DdbState", "Caption"));
        found.push(("Lbl-DdbState", at(&steps)));
    }
    steps.extend(pick(1));
    steps.extend([click("Btn-Ec2Reload"), wait(2000), get("Lbl-Ec2State", "Caption")]);
    found.push(("Lbl-Ec2State en", at(&steps)));
    steps.extend(pick(2));
    steps.push(get("Lbl-Ec2State", "Caption"));
    found.push(("Lbl-Ec2State pt", at(&steps)));
    let r = run_form(&p, &Value::Array(steps), 90);
    let said: Vec<String> = found.iter().map(|(_, i)| step_value(&r, *i)).collect();
    let arn = "arn:aws:iam::123456789012:user/demo";
    assert_eq!(said[0], format!("Connected as {arn}"));
    assert_eq!(said[1], format!("Conectado como {arn}"));
    assert_eq!(said[2], format!("{arn} として接続しました"));
    assert_eq!(said[3], said[0], "English did not come back");
    assert_eq!(said[4], "2 pet(s) in the table.");
    assert_eq!(said[5], "2 animal(aux) dans la table.");
    assert_eq!(said[6], "表中有 2 只宠物。");
    assert_eq!(said[7], "2 server(s) in us-east-1");
    assert_eq!(said[8], "2 servidor(es) em us-east-1");
    println!("\nspec 092 amendment A1 — status lines rewritten after a change of language\n  {} lines read in 3 tabs and 6 languages, all as expected; {:.1} s", said.len(), t.elapsed().as_secs_f32());
}

/// The main form tells its popups the language (a property of its own) and asks the confirm
/// popup its question already translated.
#[test]
fn the_popups_are_told_the_language_and_the_question_arrives_translated() {
    let p = project("popups");
    let mut steps = pick(2);
    steps.extend([click("Btn-Reload"), wait(1500)]);
    steps.extend([set("Dg-Pets", "ClickedRow", "2"), set("Dg-Pets", "ClickedColumn", "8"), json!({"event": {"control": "Dg-Pets", "name": "onCellClick"}}), wait(2500)]);
    let r = run_form(&p, &Value::Array(steps), 90);
    let out = shown(&r);
    assert!(out.contains("LANG-SEEN 2"), "the popup was not told the language: {out}");
    assert!(out.contains("CONFIRM-SEEN Excluir este animal e a foto dele?"), "the question did not arrive translated: {out}");
}

/// A popup run on its own has no opener, so what it asks the opener for is answered here: the
/// language, the pet's age, nothing else. Everything else is the popup's own program.
fn run_popup(p: &Project, stem: &str, language: usize, steps: &Value) -> Value {
    let original = std::fs::read_to_string(example().join("forms").join(format!("{stem}.cfrm"))).unwrap();
    let mut out = String::new();
    for line in original.lines() {
        if line.contains("super::GetProperty(") {
            let value = if line.contains("\"UiLanguage\"") {
                language.to_string()
            } else if line.contains("\"PetAge\"") {
                "7".to_owned()
            } else {
                String::new()
            };
            out.push_str(&format!("           SET     WS-TEXT                       TO    \"{value}\"\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    let form_path = p.root.join("forms").join(format!("{stem}.cfrm"));
    std::fs::write(&form_path, out).unwrap();
    let form = cobolt_forms::load_form(&form_path).unwrap();
    std::fs::write(p.root.join("generated").join(format!("{stem}.cbl")), cobolt_codegen::generate(&form)).unwrap();
    let script = p.root.join("popup-steps.json");
    std::fs::write(&script, steps.to_string()).unwrap();
    let output = Command::new(rcrun())
        .args(["run-form", &format!("forms/{stem}.cfrm"), &format!("generated/{stem}.cbl"), "--designer", "--headless", script.to_str().unwrap(), "--headless-limit", "30"])
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(output.status.success(), "run-form of {stem} failed:\n{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
    stdout
        .lines()
        .find_map(|l| l.strip_prefix("@RUN-FORM-RESULT "))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"))
}

/// Each popup, told each of the six languages, shows its own texts in it; English is what it
/// was designed with.
#[test]
fn each_popup_shows_its_texts_in_the_language_it_is_told() {
    let t = Instant::now();
    let p = project("popup-texts");
    let confirm = [("Yes", "No"), ("Sim", "Não"), ("Sí", "No"), ("Oui", "Non"), ("はい", "いいえ"), ("是", "否")];
    let view = [("Close", "Age 7"), ("Fechar", "Idade 7"), ("Cerrar", "Edad 7"), ("Fermer", "Âge 7"), ("閉じる", "年齢 7"), ("关闭", "年龄 7")];
    let pet = [
        ("Pet", "Save", "Cancel", "A pet needs a name."),
        ("Animal", "Salvar", "Cancelar", "Um animal precisa de um nome."),
        ("Mascota", "Guardar", "Cancelar", "Una mascota necesita un nombre."),
        ("Animal", "Enregistrer", "Annuler", "Un animal doit avoir un nom."),
        ("ペット", "保存", "キャンセル", "ペットには名前が必要です。"),
        ("宠物", "保存", "取消", "宠物必须有名称。"),
    ];
    for language in 1..=6 {
        let want = language - 1;
        let r = run_popup(&p, "confirm-form", language, &json!([get("Btn-Yes", "Caption"), get("Btn-No", "Caption")]));
        assert_eq!((read(&r, "Btn-Yes", "Caption"), read(&r, "Btn-No", "Caption")), (confirm[want].0.to_owned(), confirm[want].1.to_owned()), "confirm-form, language {language}");
        let r = run_popup(&p, "view-form", language, &json!([get("Btn-Close", "Caption"), get("Lbl-Age", "Caption")]));
        assert_eq!((read(&r, "Btn-Close", "Caption"), read(&r, "Lbl-Age", "Caption")), (view[want].0.to_owned(), view[want].1.to_owned()), "view-form, language {language}");
        let r = run_popup(&p, "pet-form", language, &json!([get("Lbl-Title", "Caption"), get("Btn-Save", "Caption"), get("Btn-Cancel", "Caption"), click("Btn-Save"), get("Lbl-Note", "Caption")]));
        let got = (read(&r, "Lbl-Title", "Caption"), read(&r, "Btn-Save", "Caption"), read(&r, "Btn-Cancel", "Caption"), read(&r, "Lbl-Note", "Caption"));
        assert_eq!(got, (pet[want].0.to_owned(), pet[want].1.to_owned(), pet[want].2.to_owned(), pet[want].3.to_owned()), "pet-form, language {language}");
    }
    println!("\nspec 092 amendment A1 — the popups, told each language\n  3 popups x 6 languages = 18 runs, each texts as expected; {:.1} s", t.elapsed().as_secs_f32());
}

/// Spec 092 AC8 (R28) — the CloudFormation template: its resources, parameters and outputs,
/// a private encrypted bucket with the lifecycle rule, a function small enough to be
/// written inline, and no statement that hands out administrator rights.
#[test]
fn the_cloudformation_template_has_what_the_example_needs_and_no_wildcard_administrator() {
    let path = example().join("cloudformation/powercloud.yaml");
    let text = std::fs::read_to_string(&path).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("the template is not YAML: {e}"));
    let section = |name: &str| doc.get(name).and_then(serde_yaml::Value::as_mapping).unwrap_or_else(|| panic!("no {name}")).clone();
    let (resources, parameters, outputs) = (section("Resources"), section("Parameters"), section("Outputs"));
    let kind = |r: &str| resources.get(r).and_then(|v| v.get("Type")).and_then(serde_yaml::Value::as_str).unwrap_or_else(|| panic!("no resource {r}")).to_owned();
    for (resource, want) in [
        ("PhotoBucket", "AWS::S3::Bucket"),
        ("PhotoBucketPolicy", "AWS::S3::BucketPolicy"),
        ("PresignRole", "AWS::IAM::Role"),
        ("PresignFunction", "AWS::Lambda::Function"),
        ("PetsTable", "AWS::DynamoDB::Table"),
        ("DemoInstance", "AWS::EC2::Instance"),
        ("OperatorPolicy", "AWS::IAM::ManagedPolicy"),
    ] {
        assert_eq!(kind(resource), want);
    }
    for parameter in ["BucketPrefix", "FunctionName", "TableName", "CreateTable", "CreateDemoInstance", "UrlLifetimeSeconds", "FunctionRoleArn"] {
        assert!(parameters.contains_key(parameter), "no parameter {parameter}");
    }
    // A role handed in (the powercloud-deployer stack's) replaces the stack's own, and then the
    // stack writes no IAM at all: the deployer stack holds the role and the run policy.
    assert_eq!(resources["PresignRole"]["Condition"].as_str(), Some("MakeIam"));
    assert_eq!(resources["OperatorPolicy"]["Condition"].as_str(), Some("MakeIam"));
    assert!(resources["PresignFunction"]["Properties"]["Role"].get("Fn::If").is_some(), "the function's role is not chosen");
    for output in ["BucketName", "FunctionName", "TableName", "OperatorPolicyArn", "DemoInstanceId"] {
        assert!(outputs.contains_key(output), "no output {output}");
    }

    // A private, encrypted bucket that clears abandoned uploads.
    let bucket = &resources["PhotoBucket"]["Properties"];
    // Its name is public: no account id and no region in it (operator, 2026-10-18), but the
    // stack's whole UUID, which only all of is sure to be unique.
    let name = serde_yaml::to_string(&bucket["BucketName"]).unwrap();
    assert!(!name.contains("AccountId") && !name.contains("AWS::Region"), "the bucket name carries the account or the region: {name}");
    assert!(name.contains("AWS::StackId") && name.contains("Fn::Select") && name.contains("- 2"), "the bucket name is not the stack's UUID: {name}");
    for flag in ["BlockPublicAcls", "BlockPublicPolicy", "IgnorePublicAcls", "RestrictPublicBuckets"] {
        assert_eq!(bucket["PublicAccessBlockConfiguration"][flag].as_bool(), Some(true), "{flag}");
    }
    assert!(bucket["BucketEncryption"]["ServerSideEncryptionConfiguration"][0]["ServerSideEncryptionByDefault"]["SSEAlgorithm"].as_str().is_some());
    assert_eq!(bucket["LifecycleConfiguration"]["Rules"][0]["AbortIncompleteMultipartUpload"]["DaysAfterInitiation"].as_i64(), Some(1));

    // The function is inline code, under CloudFormation's 4 KB limit, without tabs.
    let code = resources["PresignFunction"]["Properties"]["Code"]["ZipFile"].as_str().expect("inline code");
    assert!(code.len() < 4096, "{} bytes of inline code", code.len());
    assert!(!code.contains('\t'), "a tab in the inline code");

    // No Allow statement is a wildcard administrator.
    let mut statements = 0;
    let mut allows = 0;
    fn walk(v: &serde_yaml::Value, statements: &mut usize, allows: &mut usize) {
        match v {
            serde_yaml::Value::Mapping(m) => {
                if let Some(effect) = m.get("Effect").and_then(serde_yaml::Value::as_str) {
                    *statements += 1;
                    if effect == "Allow" {
                        *allows += 1;
                        let actions: Vec<String> = match m.get("Action") {
                            Some(serde_yaml::Value::String(s)) => vec![s.clone()],
                            Some(serde_yaml::Value::Sequence(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
                            _ => Vec::new(),
                        };
                        for a in &actions {
                            assert!(a != "*" && a != "*:*" && !a.ends_with(":*"), "a wildcard action: {a}");
                        }
                    }
                }
                m.values().for_each(|x| walk(x, statements, allows));
            }
            serde_yaml::Value::Sequence(s) => s.iter().for_each(|x| walk(x, statements, allows)),
            _ => {}
        }
    }
    walk(&serde_yaml::Value::Mapping(resources.clone()), &mut statements, &mut allows);
    assert!(statements >= 10 && allows >= 9, "{statements} statements, {allows} allows");
    println!("spec 092 AC8 — template: {} resources, {} parameters, {} outputs, {statements} policy statements ({allows} Allow, none a wildcard administrator), {} bytes of inline code", resources.len(), parameters.len(), outputs.len(), code.len());
}

/// Operator, 2026-10-18: "update the policy so the demo can deploy the stack". The deployer
/// template gives one user what deploying the PowerCloud stack takes - and never the right to
/// create a role, a user or a key, or to attach a policy: whoever may create a role and hand it
/// to a function can give that function any right, and through it themselves.
#[test]
fn the_deployer_lets_a_user_deploy_the_stack_without_any_way_to_raise_its_own_rights() {
    let text = std::fs::read_to_string(example().join("cloudformation/powercloud-deployer.yaml")).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("the deployer template is not YAML: {e}"));
    let resources = &doc["Resources"];
    assert_eq!(resources["FunctionRole"]["Type"].as_str(), Some("AWS::IAM::Role"));
    assert_eq!(resources["DeployerPolicy"]["Type"].as_str(), Some("AWS::IAM::ManagedPolicy"));
    assert!(doc["Outputs"].get("FunctionRoleArn").is_some(), "the Deploy button reads FunctionRoleArn");

    let strings = |v: &serde_yaml::Value| -> Vec<String> {
        match v {
            serde_yaml::Value::String(s) => vec![s.clone()],
            serde_yaml::Value::Sequence(a) => a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
            _ => Vec::new(),
        }
    };
    let statements = resources["DeployerPolicy"]["Properties"]["PolicyDocument"]["Statement"].as_sequence().expect("statements").clone();
    let allowed: Vec<String> = statements.iter().filter(|s| s["Effect"].as_str() == Some("Allow")).flat_map(|s| strings(&s["Action"])).collect();

    // What deploying the stack's resources takes.
    for need in ["cloudformation:CreateStack", "cloudformation:DeleteStack", "s3:CreateBucket", "s3:PutBucket*", "lambda:CreateFunction", "iam:PassRole", "dynamodb:CreateTable"] {
        assert!(allowed.iter().any(|a| a == need), "the deployer cannot {need}");
    }
    // Never a way to more rights: no IAM but handing over and reading the one role - no role,
    // user, key or policy made, and no policy version written (a stack update rewriting a policy
    // attached to the user would be one).
    for a in &allowed {
        assert!(a != "*" && !a.ends_with(":*"), "a wildcard action: {a}");
        assert!(!a.starts_with("iam:") || a == "iam:PassRole" || a == "iam:GetRole", "the deployer may {a}");
        assert!(!a.starts_with("sts:"), "the deployer may {a}");
    }

    // Running the demo: the run policy grants what the stack's OperatorPolicy grants, no more.
    let actions_of = |policy: &serde_yaml::Value| -> std::collections::BTreeSet<String> {
        policy["Properties"]["PolicyDocument"]["Statement"].as_sequence().unwrap().iter().flat_map(|s| strings(&s["Action"])).collect()
    };
    let main: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(example().join("cloudformation/powercloud.yaml")).unwrap()).unwrap();
    let run = actions_of(&resources["RunPolicy"]);
    assert_eq!(run, actions_of(&main["Resources"]["OperatorPolicy"]), "RunPolicy and the stack's OperatorPolicy grant different actions");
    assert!(run.iter().all(|a| !a.starts_with("iam:")), "the run policy writes IAM");
    // Only the one role, and only to Lambda.
    let pass = statements.iter().find(|s| strings(&s["Action"]).iter().any(|a| a == "iam:PassRole")).unwrap();
    assert!(pass["Resource"].get("Fn::GetAtt").is_some(), "PassRole is not limited to FunctionRole: {:?}", pass["Resource"]);
    assert_eq!(pass["Condition"]["StringEquals"]["iam:PassedToService"].as_str(), Some("lambda.amazonaws.com"));
    // Every Allow names its resources, but the two that read a template only.
    for s in statements.iter().filter(|s| s["Effect"].as_str() == Some("Allow")) {
        if s["Resource"].as_str() == Some("*") {
            assert!(strings(&s["Action"]).iter().all(|a| a == "cloudformation:ValidateTemplate" || a == "cloudformation:GetTemplateSummary"), "{s:?}");
        }
    }
    // The deployer stack holds the user's rights: the user may not change or remove it.
    let deny = statements.iter().find(|s| s["Effect"].as_str() == Some("Deny")).expect("no Deny");
    assert_eq!(deny["Resource"]["Ref"].as_str(), Some("AWS::StackId"));
    for a in ["cloudformation:UpdateStack", "cloudformation:DeleteStack", "cloudformation:ExecuteChangeSet"] {
        assert!(strings(&deny["Action"]).iter().any(|x| x == a), "the deployer stack is not protected from {a}");
    }
    // The function's role reads photos and writes its own logs, nothing else.
    let role_actions: Vec<String> = resources["FunctionRole"]["Properties"]["Policies"][0]["PolicyDocument"]["Statement"]
        .as_sequence()
        .unwrap()
        .iter()
        .flat_map(|s| strings(&s["Action"]))
        .collect();
    assert_eq!(role_actions, ["s3:GetObject", "logs:CreateLogGroup", "logs:CreateLogStream", "logs:PutLogEvents"]);
    assert!(resources["FunctionRole"]["Properties"].get("ManagedPolicyArns").is_none(), "a managed policy widens the role");
}

/// The Deploy button hands the stack the role the powercloud-deployer stack made, so keys that
/// may deploy but not create a role can deploy; without that stack it hands an empty one.
#[test]
fn deploy_hands_the_stack_the_role_the_deployer_made() {
    let p = project("deployer");
    let path = p.root.join("fake-hosted.json");
    let mut hosted: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let role = "arn:aws:iam::123456789012:role/powercloud-deployer-FunctionRole-ABC123";
    let deployer = json!({"StackName": "powercloud-deployer", "StackStatus": "CREATE_COMPLETE", "Outputs": [
        {"OutputKey": "FunctionRoleArn", "OutputValue": role}, {"OutputKey": "DeployerPolicyArn", "OutputValue": "arn:aws:iam::123456789012:policy/x"}]});
    let answer = envelope(json!({"Answer": {"Stacks": [deployer.clone()]}, "Rows": [deployer]}));
    hosted["answers_when"].as_array_mut().unwrap().insert(0, json!({"tool": "aws___run_script", "contains": r#"StackName\\\":\\\"powercloud-deployer"#, "answer": answer}));
    std::fs::write(&path, hosted.to_string()).unwrap();
    run_form(&p, &json!([click("Btn-Deploy"), wait(2500)]), 60);
    let log = hosted_log(&p);
    assert!(log.lines().any(|l| l.contains("powercloud-deployer") && l.contains("DescribeStacks") && !l.contains("CreateStack")), "the deployer stack was not asked");
    let create = log.lines().find(|l| l.contains(r#"operation_name=\"CreateStack\""#)).expect("no CreateStack");
    assert!(create.contains("FunctionRoleArn") && create.contains(role), "the role was not handed to the stack");
}

/// Spec 092 R29 — the example holds no credential: no AWS access key id, no secret key, no
/// session token and no private key in any file of the project.
#[test]
fn the_example_holds_no_credential() {
    fn walk(dir: &Path, hits: &mut Vec<String>, files: &mut usize) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                walk(&path, hits, files);
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue }; // images and other binaries
            *files += 1;
            let bytes = text.as_bytes();
            for i in 0..bytes.len().saturating_sub(20) {
                let head = &bytes[i..i + 4];
                if (head == b"AKIA" || head == b"ASIA") && bytes[i + 4..i + 20].iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
                    hits.push(format!("{}: an access key id", path.display()));
                }
            }
            for needle in ["aws_secret_access_key", "-----BEGIN", "aws_session_token"] {
                if text.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()) {
                    hits.push(format!("{}: {needle}", path.display()));
                }
            }
        }
    }
    let (mut hits, mut files) = (Vec::new(), 0);
    walk(&example(), &mut hits, &mut files);
    assert!(hits.is_empty(), "credentials in the example: {hits:?}");
    println!("spec 092 R29 — {files} text files of the example scanned, no credential found");
}
