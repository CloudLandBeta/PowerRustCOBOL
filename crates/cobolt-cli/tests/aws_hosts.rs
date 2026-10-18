// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 078 T-A16 (R28; AC6, AC11, AC16): the same COBOL programs on the
//! three hosts. `tests/cobol/aws/forms` drive every Delivery A operation of
//! `AwsLambda` and `AwsMcp` against the fake MCP server, through a project
//! whose AWS connection routes both servers to it, and report a tally:
//!
//! - under `rcrun run-form` (Run Form),
//! - as embedded child forms, loaded into an application shell's ContentPane,
//! - in a built binary, run headless (`--ignored`: it compiles one), which
//!   also checks that no AWS server outlives the application (AC6).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

/// Spec 103: the loopback stand-in for the hosted endpoint, shared with `cobolt-runtime`'s tests.
#[path = "../../cobolt-runtime/tests/support/http_fake.rs"]
mod http_fake;

struct Project {
    root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

/// The fake MCP server is `cobolt-runtime`'s test binary: built on demand,
/// beside `rcrun` in the same target folder.
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
    assert!(path.exists(), "no fake MCP server at {}", path.display());
    path
}

/// A project holding the three forms, their generated programs, and one AWS
/// connection whose `lambda` and `fake` servers are the fake.
fn project(tag: &str) -> Project {
    project_with(tag, None)
}

/// [`project`], with the hosted server reached over HTTPS at `hosted_url` (spec 103) instead of
/// being the fake program.
fn project_with(tag: &str, hosted_url: Option<&str>) -> Project {
    let root = std::env::temp_dir().join(format!("prc-078-hosts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["forms", "generated", "src", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/aws/forms");
    let mut forms = Vec::new();
    for entry in std::fs::read_dir(&fixtures).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        std::fs::copy(entry.path(), root.join("forms").join(&name)).unwrap();
        if let Some(stem) = name.strip_suffix(".cfrm") {
            let form = cobolt_forms::load_form(&entry.path()).unwrap();
            std::fs::write(root.join("generated").join(format!("{stem}.cbl")), cobolt_codegen::generate(&form)).unwrap();
            forms.push(stem.to_owned());
        }
    }
    let reply = r#"Function app-f returned: [{"id":1,"name":"Ana"},{"id":2,"name":"Bo"}]"#;
    let script = json!({
        "tools": [
            {"name": "app_f", "inputSchema": {"type": "object", "properties": {"parameters": {"type": "object"}}}},
            {"name": "read_thing", "inputSchema": {"type": "object"}, "annotations": {"readOnlyHint": true}},
            {"name": "write_thing", "inputSchema": {"type": "object"}}
        ],
        "answers": {"app_f": {"content": [{"type": "text", "text": reply}], "isError": false}},
        "pid_file": root.join("pids.txt").to_string_lossy(),
    });
    std::fs::write(root.join("fake.json"), script.to_string()).unwrap();
    // The Delivery B servers answer from a script of their own, so the
    // Lambda server still lists exactly its three tools.
    let script_b = json!({
        "tools": [
            {"name": "QueryKnowledgeBases", "inputSchema": {"type": "object"}},
            {"name": "ListKnowledgeBases", "inputSchema": {"type": "object"}},
            {"name": "invoke_agent_runtime", "inputSchema": {"type": "object"}},
            {"name": "memory_create_event", "inputSchema": {"type": "object"}},
            {"name": "memory_retrieve_records", "inputSchema": {"type": "object"}},
            {"name": "list_tables", "inputSchema": {"type": "object"}},
            {"name": "query_database", "inputSchema": {"type": "object"}},
            {"name": "append_rows_to_table", "inputSchema": {"type": "object"}},
            {"name": "manage_aws_glue_jobs", "inputSchema": {"type": "object"}},
            {"name": "manage_aws_glue_crawlers", "inputSchema": {"type": "object"}},
            {"name": "manage_aws_glue_tables", "inputSchema": {"type": "object"}},
            {"name": "aws___run_script", "inputSchema": {"type": "object", "properties": {"code": {"type": "string"}}, "required": ["code"]}}
        ],
        "answers": delivery_b_answers(),
        "answers_when": hosted_answers(),
        "pid_file": root.join("pids.txt").to_string_lossy(),
    });
    std::fs::write(root.join("fake-b.json"), script_b.to_string()).unwrap();
    let fake = fake_mcp();
    let server = |id: &str| {
        if let (true, Some(url)) = (id == "hosted", hosted_url) {
            return format!(
                "[servers.hosted]\ntransport = 'https'\nurl = '{url}'\nservice = 'aws-mcp'\nsigning_region = 'us-east-1'\n\
                 metadata = {{ AWS_REGION = \"{{Connection.Region|or:us-east-1}}\" }}\n"
            );
        }
        let script = if matches!(id, "lambda" | "fake") { "fake.json" } else { "fake-b.json" };
        format!("[servers.{id}]\ncommand = '{}'\nargs = ['{}']\n", fake.display(), root.join(script).display())
    };
    let routes: String = ["lambda", "fake", "bedrock-kb", "agentcore", "s3tables", "dataprocessing", "hosted"].map(server).concat();
    let listed = |dir: &str, ext: &str| forms.iter().map(|f| format!("\"{dir}/{f}.{ext}\"")).collect::<Vec<_>>().join(", ");
    let manifest = format!(
        "[project]\nname = \"AwsHosts\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [{}]\ngenerated = [{}]\n\n\
         [[integrations.aws_connections]]\nid = \"aws-hosts\"\nname = \"Demo\"\nprofile = \"demo\"\nregion = \"eu-west-1\"\n\
         routes_override = '''\n{routes}'''\n",
        listed("forms", "cfrm"),
        listed("generated", "cbl"),
    );
    std::fs::write(root.join("awshosts.project.toml"), manifest).unwrap();
    std::fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    Project { root }
}

/// What the fake answers each tool with: the shapes each real server's
/// source produces (see the route fixtures' provenance).
fn delivery_b_answers() -> Value {
    let text = |t: &str| json!({"content": [{"type": "text", "text": t}], "isError": false});
    let two = |m: &str, d: &str| json!({"content": [{"type": "text", "text": m}, {"type": "text", "text": d}], "isError": false});
    json!({
        "QueryKnowledgeBases": text(concat!(
            r#"{"content": {"type": "TEXT", "text": "Refunds take 14 days."}, "location": {"type": "S3", "s3Location": {"uri": "s3://docs/refunds.pdf"}}, "score": 0.82}"#,
            "\n\n",
            r#"{"content": {"type": "TEXT", "text": "Keep the receipt."}, "location": {"type": "WEB", "webLocation": {"url": "https://example.com/faq"}}, "score": 0.41}"#
        )),
        "ListKnowledgeBases": text(r#"{"KB12345678": {"name": "Support docs", "description": "FAQ", "type": "VECTOR", "data_sources": []}}"#),
        "invoke_agent_runtime": text(r#"{"status": "success", "runtime_session_id": "sess-0123456789abcdef0123456789abcdef", "response_body": "Hi! How can I help?", "message": "ok"}"#),
        "memory_create_event": text(r#"{"status": "success", "message": "Event created", "event": {"eventId": "ev-77"}}"#),
        "memory_retrieve_records": text(r#"{"status": "success", "message": "2 records", "memory_records": [{"memoryRecordId": "r1", "content": {"text": "Likes tea"}, "score": 0.91}, {"memoryRecordId": "r2", "content": {"text": "Lives in Porto"}, "score": 0.33}]}"#),
        "list_tables": text(r#"{"tables": [{"namespace": ["sales"], "name": "orders", "table_arn": "arn:t/1"}, {"namespace": ["ops"], "name": "events", "table_arn": "arn:t/2"}, {"namespace": ["sales"], "name": "refunds", "table_arn": "arn:t/3"}], "total_count": 3}"#),
        "query_database": text(r#"{"columns": ["order_id", "total"], "rows": [[1, 19.9], [2, 5.0]]}"#),
        "append_rows_to_table": text(r#"{"status": "success", "rows_appended": 2}"#),
        "manage_aws_glue_jobs": two("Successfully processed the job run", r#"{"job_name": "nightly-etl", "job_run_id": "jr_abc", "job_run_details": {"Id": "jr_abc", "JobRunState": "SUCCEEDED"}}"#),
        "manage_aws_glue_crawlers": two("Successfully started crawler raw-crawler", r#"{"crawler_name": "raw-crawler", "operation": "start"}"#),
        "manage_aws_glue_tables": two("Successfully retrieved table", r#"{"database_name": "sales", "table_name": "orders", "storage_descriptor": {"Columns": [{"Name": "order_id", "Type": "bigint"}, {"Name": "total", "Type": "double"}]}}"#),
    })
}

/// What the hosted server's `aws___run_script` answers, by the AWS operation
/// its script calls: the envelope AWS's own code reads, as text.
fn hosted_answers() -> Value {
    let env = |v: Value| {
        let text = json!({"status": "success", "stdout": "", "stderr": "", "return_value": v}).to_string();
        json!({"content": [{"type": "text", "text": text}], "isError": false})
    };
    let rules = [
        // 1.91.210: the operations the AWS Labs servers (uvx) used to answer, each by its exact
        // AWS operation, ahead of the looser rules below (AppendRows' GetTableMetadata
        // before Query's StartQueryExecution, both before Glue's GetTable).
        ("operation_name='Invoke'", json!({"Payload": r#"[{"id":1,"name":"Ana"},{"id":2,"name":"Bo"}]"#, "FunctionError": ""})),
        (
            "operation_name='ListFunctions'",
            json!({"Functions": [{"Name": "app_f", "Description": "", "ReadOnly": "0"}, {"Name": "read_thing", "Description": "", "ReadOnly": "0"}, {"Name": "write_thing", "Description": "", "ReadOnly": "0"}], "Names": "app_f\nread_thing\nwrite_thing"}),
        ),
        (
            "operation_name='Retrieve'",
            json!({"Passages": [
                {"content": {"type": "TEXT", "text": "Refunds take 14 days."}, "location": {"type": "S3", "s3Location": {"uri": "s3://docs/refunds.pdf"}}, "score": 0.82},
                {"content": {"type": "TEXT", "text": "Keep the receipt."}, "location": {"type": "WEB", "webLocation": {"url": "https://example.com/faq"}}, "score": 0.41}], "Text": "two passages"}),
        ),
        ("operation_name='ListKnowledgeBases'", json!({"KnowledgeBases": {"KB12345678": {"name": "Support docs", "description": "FAQ", "type": "VECTOR"}}})),
        (
            "operation_name='InvokeAgentRuntime'",
            json!({"response_body": "Hi! How can I help?", "runtime_session_id": "sess-0123456789abcdef0123456789abcdef", "content_type": "application/json"}),
        ),
        ("operation_name='CreateEvent'", json!({"event": {"eventId": "ev-77"}})),
        (
            "operation_name='RetrieveMemoryRecords'",
            json!({"memory_records": [{"memoryRecordId": "r1", "content": {"text": "Likes tea"}, "score": 0.91}, {"memoryRecordId": "r2", "content": {"text": "Lives in Porto"}, "score": 0.33}]}),
        ),
        (
            "operation_name='ListTableBuckets'",
            json!({"tables": [{"namespace": ["sales"], "name": "orders", "table_arn": "arn:t/1"}, {"namespace": ["ops"], "name": "events", "table_arn": "arn:t/2"}, {"namespace": ["sales"], "name": "refunds", "table_arn": "arn:t/3"}]}),
        ),
        ("operation_name='GetTableMetadata'", json!({"rows_appended": 2})),
        ("operation_name='StartQueryExecution'", json!({"columns": ["order_id", "total"], "rows": [["1", "19.9"], ["2", "5.0"]]})),
        ("operation_name='StartJobRun'", json!({"job_name": "nightly-etl", "job_run_id": "jr_abc"})),
        ("operation_name='GetJobRun'", json!({"job_name": "nightly-etl", "job_run_id": "jr_abc", "job_run_details": {"Id": "jr_abc", "JobRunState": "SUCCEEDED"}})),
        ("operation_name='StartCrawler'", json!({"crawler_name": "raw-crawler", "started": "1"})),
        (
            "operation_name='GetTable'",
            json!({"database_name": "sales", "table_name": "orders", "table_definition": {"StorageDescriptor": {"Columns": [{"Name": "order_id", "Type": "bigint"}, {"Name": "total", "Type": "double"}]}}}),
        ),
        ("GetItem", json!({"Items": [{"id": 42, "customer": "Ana"}], "Found": "1"})),
        ("Scan", json!({"Items": [{"id": 1}, {"id": 2}], "Count": 2})),
        ("'Query'", json!({"Items": [{"id": 1}, {"id": 2}], "Count": 2})),
        ("PutItem", json!({"Done": "1"})),
        ("ListObjectsV2", json!({"Contents": [{"Key": "a.txt", "Size": 5, "LastModified": "2026-10-07 10:00:00+00:00"}]})),
        ("GetObject", json!({"Base64": "aGVsbG8=", "Text": "hello", "ContentType": "text/plain"})),
        ("QueryVectors", json!({"vectors": [{"key": "doc-7", "distance": 0.12, "metadata": {}}]})),
        ("DetectLabels", json!({"Labels": [{"Name": "Dog", "Confidence": 98.1}, {"Name": "Pet", "Confidence": 97.0}]})),
        ("operation_name='DetectText'", json!({"TextDetections": [{"DetectedText": "GOOD BOY", "Type": "LINE", "Confidence": 99}]})),
        ("DetectFaces", json!({"FaceDetails": [{"Confidence": 99.9, "AgeRange": {"Low": 2, "High": 6}, "BoundingBox": {"Left": 0.1, "Top": 0.2, "Width": 0.3, "Height": 0.4}}]})),
        (
            "RecognizeCelebrities",
            json!({"CelebrityFaces": [{"Name": "Ada Lovelace", "Id": "1a2B3c", "MatchConfidence": 99.5, "Urls": "www.wikidata.org/wiki/Q7259 www.imdb.com/name/nm0000001", "Face": {"BoundingBox": {"Left": 0.25, "Top": 0.125, "Width": 0.5, "Height": 0.625}}}], "UnrecognizedFaces": []}),
        ),
        ("ListBuckets", json!({"Buckets": [{"Name": "photos", "CreationDate": "2026-10-01T09:30:00+00:00"}, {"Name": "reports", "CreationDate": "2025-02-14T18:00:00+00:00"}]})),
        ("SynthesizeSpeech", json!({"Base64": "SUQzBAA=", "ContentType": "audio/mpeg", "Characters": 5})),
        ("DetectSentiment", json!({"Sentiment": "POSITIVE", "Scores": [{"Positive": 0.97}]})),
        ("DetectEntities", json!({"Entities": [{"Text": "Ana", "Type": "PERSON", "Score": 0.99}, {"Text": "Porto", "Type": "LOCATION", "Score": 0.98}]})),
        ("DetectDominantLanguage", json!({"Languages": [{"LanguageCode": "pt", "Score": 0.98}]})),
        ("DetectKeyPhrases", json!({"KeyPhrases": [{"Text": "the new ticket form", "Score": 0.99}]})),
        ("DetectDocumentText", json!({"Lines": [{"Text": "Name: Ana", "Confidence": 99, "Page": 1}, {"Text": "Total: 42", "Confidence": 98, "Page": 1}]})),
        ("AnalyzeDocument", json!({"Rows": [{"Kind": "FIELD", "Key": "Name:", "Value": "Ana"}, {"Kind": "CELL", "Value": "42", "Table": 1, "Row": 1, "Column": 1}]})),
        ("DescribeInstances", json!({"Instances": [{"InstanceId": "i-1", "Name": "web", "State": "running"}]})),
        ("StartInstances", json!({"Instances": [{"InstanceId": "i-1", "State": "pending", "Previous": "stopped"}]})),
        ("InitiateAuth", json!({"SignedIn": "1", "UserName": "ana", "Challenge": "", "Secret": {"AccessToken": "demo-access-token", "IdToken": "demo-id-token"}})),
        ("GetUser", json!({"Value": "ana@example.com"})),
        ("GlobalSignOut", json!({"SignedIn": "0", "UserName": ""})),
    ];
    let mut answers: Vec<Value> = rules
        .into_iter()
        .map(|(op, v)| {
            let contains = if op.starts_with('\'') || op.starts_with("operation_name") { op.to_owned() } else { format!("'{op}'") };
            let contains = if contains.starts_with("operation_name") { contains } else { format!("operation_name={contains}") };
            json!({"tool": "aws___run_script", "contains": contains, "answer": env(v)})
        })
        .collect();
    // AwsMcp.AwsCall (spec 092): the operation arrives as a quoted LITERAL, so
    // its script says `operation_name="DescribeStacks"` — `\"` in the JSON
    // text the fake matches — which no rule above can match. The stack that
    // does not exist comes first: AWS answers it with an error code.
    let stack = json!({"StackName": "powercloud", "StackStatus": "CREATE_COMPLETE"});
    for (contains, v) in [
        ("no-such-stack", json!({"Status": "error", "Error": "ValidationException: Stack with id no-such-stack does not exist"})),
        (r#"operation_name=\"DescribeStacks\""#, json!({"Answer": {"Stacks": [stack]}, "Rows": [stack]})),
    ] {
        answers.push(json!({"tool": "aws___run_script", "contains": contains, "answer": env(v)}));
    }
    Value::Array(answers)
}

/// The steps that run one demo; `open` loads it into the shell's pane first.
fn steps(open: Option<&str>) -> Value {
    let mut s = Vec::new();
    if let Some(form) = open {
        s.push(json!({"open_form": form}));
    }
    s.push(json!({"event": {"control": "BTN-RUN", "name": "onClick"}}));
    s.push(json!({"wait_ms": 2500}));
    s.push(json!({"event": {"control": "BTN-SUMMARY", "name": "onClick"}}));
    s.push(json!({"read": {"control": "LBL-RESULT", "property": "Caption"}}));
    Value::Array(s)
}

/// The demo's tally, and its summary block, from a headless run's stdout.
fn outcome(stdout: &str) -> (String, Vec<String>) {
    let report: Value = stdout
        .lines()
        .find_map(|l| l.strip_prefix("@RUN-FORM-RESULT "))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"));
    let tally = report["steps"]
        .as_array()
        .and_then(|s| s.iter().find_map(|st| st["value"].as_str().map(str::to_owned)))
        .unwrap_or_default();
    let block = stdout
        .lines()
        .skip_while(|l| !l.starts_with("===="))
        .take_while(|l| !l.starts_with("@RUN-FORM-RESULT"))
        .map(|l| l.trim_end().to_owned())
        .collect();
    (tally.trim().to_owned(), block)
}

fn run_form(p: &Project, form: &str, program: &str, script: &Value) -> String {
    run_form_env(p, form, program, script, &[])
}

/// [`run_form`] with these variables set for `rcrun`; the host's own AWS keys and profile are never passed on.
fn run_form_env(p: &Project, form: &str, program: &str, script: &Value, env: &[(&str, String)]) -> String {
    let script_path = p.root.join(format!("steps-{}.json", form.replace('/', "-")));
    std::fs::write(&script_path, script.to_string()).unwrap();
    let mut command = Command::new(rcrun());
    for name in ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN", "AWS_PROFILE"] {
        command.env_remove(name);
    }
    for (k, v) in env {
        command.env(k, v);
    }
    let out = command
        .args([
            "run-form",
            form,
            program,
            "--designer",
            "--headless",
            script_path.to_str().unwrap(),
            "--headless-limit",
            "60",
        ])
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form {form} failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout
}

const DEMOS: [(&str, &str); 16] = [
    ("aws-lambda-demo", "PASS 009 FAIL 000"),
    ("aws-mcp-demo", "PASS 014 FAIL 000"),
    ("aws-knowledge-base-demo", "PASS 008 FAIL 000"),
    ("aws-agent-core-demo", "PASS 006 FAIL 000"),
    ("aws-agent-memory-demo", "PASS 007 FAIL 000"),
    ("aws-s3-tables-demo", "PASS 010 FAIL 000"),
    ("aws-glue-demo", "PASS 009 FAIL 000"),
    ("aws-dynamodb-demo", "PASS 009 FAIL 000"),
    ("aws-s3-demo", "PASS 010 FAIL 000"),
    ("aws-s3-vectors-demo", "PASS 006 FAIL 000"),
    ("aws-rekognition-demo", "PASS 013 FAIL 000"),
    ("aws-polly-demo", "PASS 006 FAIL 000"),
    ("aws-comprehend-demo", "PASS 007 FAIL 000"),
    ("aws-textract-demo", "PASS 007 FAIL 000"),
    ("aws-ec2-demo", "PASS 006 FAIL 000"),
    ("aws-cognito-demo", "PASS 009 FAIL 000"),
];

#[test]
fn the_demos_pass_under_run_form_and_as_embedded_child_forms() {
    let p = project("rf");
    let mut table = Vec::new();
    for (demo, want) in DEMOS {
        let direct = outcome(&run_form(&p, &format!("forms/{demo}.cfrm"), &format!("generated/{demo}.cbl"), &steps(None)));
        let embedded = outcome(&run_form(&p, "forms/aws-shell.cfrm", "generated/aws-shell.cbl", &steps(Some(demo))));
        println!("\n  {demo} under Run Form:\n    {}", direct.1.join("\n    "));
        assert_eq!(direct.0, want, "{demo} under Run Form");
        assert_eq!(embedded.0, want, "{demo} as an embedded child form");
        table.push(format!("{demo}: Run Form {} · embedded {}", direct.0, embedded.0));
    }
    println!("\n078 T-A16 — {}", table.join("; "));
}

/// The demos that reach the hosted AWS MCP Server — every one since 1.91.210, when the six that
/// used the AWS Labs servers (uvx) moved there too.
const HOSTED_DEMOS: [&str; 16] = [
    "aws-lambda-demo",
    "aws-knowledge-base-demo",
    "aws-agent-core-demo",
    "aws-agent-memory-demo",
    "aws-s3-tables-demo",
    "aws-glue-demo",
    "aws-mcp-demo",
    "aws-dynamodb-demo",
    "aws-s3-demo",
    "aws-s3-vectors-demo",
    "aws-rekognition-demo",
    "aws-polly-demo",
    "aws-comprehend-demo",
    "aws-textract-demo",
    "aws-ec2-demo",
    "aws-cognito-demo",
];

/// Spec 103 AC9/AC11: the hosted demos, reaching the hosted server over HTTPS — signed by the runtime
/// itself, with nothing on the PATH to start and the sign-in coming from a credentials file — give the
/// tallies they give against the fake program, under Run Form and as embedded child forms; and the
/// requests the endpoint received prove what ran: every one signed correctly, every hosted operation of
/// the route table asked for, every session ended.
#[test]
fn the_hosted_demos_pass_over_https_with_nothing_on_the_path() {
    use http_fake::HttpFake;

    let fake = HttpFake::start(json!({
        "tools": [{"name": "aws___run_script", "inputSchema": {"type": "object", "properties": {"code": {"type": "string"}}, "required": ["code"]}}],
        "answers_when": hosted_answers(),
    }));
    let p = project_with("https", Some(&fake.url));

    // The sign-in: one profile, "demo", in a credentials file of its own. The keys are made up.
    let home = p.root.join("home");
    std::fs::create_dir_all(home.join(".aws")).unwrap();
    std::fs::write(home.join(".aws/config"), "").unwrap();
    const SECRET: &str = "demo-secret-EXAMPLE-not-real";
    std::fs::write(
        home.join(".aws/credentials"),
        format!("[demo]\naws_access_key_id = AKIDDEMOEXAMPLE\naws_secret_access_key = {SECRET}\n"),
    )
    .unwrap();
    let nothing = p.root.join("empty-path");
    std::fs::create_dir_all(&nothing).unwrap();
    let env = [
        ("PATH", nothing.to_string_lossy().into_owned()),
        ("HOME", home.to_string_lossy().into_owned()),
        ("AWS_CONFIG_FILE", home.join(".aws/config").to_string_lossy().into_owned()),
        ("AWS_SHARED_CREDENTIALS_FILE", home.join(".aws/credentials").to_string_lossy().into_owned()),
        ("AWS_EC2_METADATA_DISABLED", "true".to_owned()),
    ];

    let started = std::time::Instant::now();
    let mut table = Vec::new();
    for demo in HOSTED_DEMOS {
        let want = DEMOS.iter().find(|(d, _)| *d == demo).unwrap().1;
        let go = |form: &str, program: &str, open: Option<&str>| outcome(&run_form_env(&p, form, program, &steps(open), &env)).0;
        let direct = go(&format!("forms/{demo}.cfrm"), &format!("generated/{demo}.cbl"), None);
        let embedded = go("forms/aws-shell.cfrm", "generated/aws-shell.cbl", Some(demo));
        assert_eq!(direct, want, "{demo} under Run Form, over https");
        assert_eq!(embedded, want, "{demo} as an embedded child form, over https");
        table.push(format!("{demo}: {direct}"));
    }
    let took = started.elapsed();

    // What the endpoint received.
    let requests = fake.requests();
    let posts: Vec<_> = requests.iter().filter(|r| r.method == "POST").collect();
    assert!(!posts.is_empty(), "the demos reached the endpoint");
    for r in &posts {
        let auth = support_verify(r, SECRET);
        assert_eq!(auth, "AKIDDEMOEXAMPLE");
    }
    // The route table's own hosted operations, each asked for at least once.
    let scripts: Vec<String> = posts
        .iter()
        .filter_map(|r| r.json())
        .filter(|m| m["method"] == "tools/call")
        .map(|m| m["params"]["arguments"]["code"].as_str().unwrap_or_default().to_owned())
        .collect();
    let routes = cobolt_runtime::aws::routes::Routes::shipped();
    let (mut seen, mut missing, mut hosted_ops) = (0, Vec::new(), 0);
    for (name, op) in &routes.ops {
        if op.server != "hosted" {
            continue;
        }
        hosted_ops += 1;
        let template = format!("{:?}", op.input);
        let names: Vec<&str> = template.split("operation_name='").skip(1).filter_map(|t| t.split('\'').next()).collect();
        let asked = if names.is_empty() {
            // `AwsMcp.AwsCall` names its operation from COBOL, as a quoted literal.
            scripts.iter().any(|c| c.contains("operation_name=\""))
        } else {
            names.iter().any(|n| scripts.iter().any(|c| c.contains(&format!("operation_name='{n}'"))))
        };
        if asked {
            seen += 1;
        } else {
            missing.push(name.clone());
        }
    }
    let sessions = fake.sessions_opened();
    fake.wait_for(|f| f.count_http("DELETE") as u64 == sessions, std::time::Duration::from_secs(3));
    let ended = fake.count_http("DELETE");
    println!(
        "\n  103 AC9/AC11 — hosted demos over HTTPS, PATH empty, sign-in from a credentials file:\n    {}\n    {} s for {} runs; \
         {} POSTs received, all {} verified with an independent SigV4 check; {seen} of {hosted_ops} hosted operations asked for (the other {} are writes the demos see refused); \
         {sessions} sessions opened, {ended} ended with a DELETE",
        table.join("\n    "),
        took.as_secs(),
        HOSTED_DEMOS.len() * 2,
        posts.len(),
        posts.len(),
        hosted_ops - seen,
    );
    assert_eq!(hosted_ops, 47, "the route table sends 47 operations to the hosted server");
    // The demos run with AllowWrite off, so eight write operations are refused before anything is sent;
    // `cobolt-runtime`'s `aws_https` runs those eight with AllowWrite on. Any OTHER operation going
    // unasked is a gap.
    let refused_in_the_demos = [
        "AwsCognito.Confirm",
        "AwsCognito.SignUp",
        "AwsDynamoDB.DeleteItem",
        "AwsDynamoDB.UpdateItem",
        "AwsEC2.Stop",
        "AwsS3.DeleteObject",
        "AwsS3.PutObject",
        "AwsS3Vectors.PutVectors",
    ];
    missing.sort();
    assert_eq!(missing, refused_in_the_demos, "hosted operations the demos did not ask for");
    assert_eq!(ended as u64, sessions, "every session an application opened it also ended");
}

/// The access key id a logged request was signed with, after checking the signature with `secret`.
fn support_verify(r: &http_fake::Logged, secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::{Digest, Sha256};
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let mac = |key: &[u8], data: &[u8]| {
        let mut m = Hmac::<Sha256>::new_from_slice(key).unwrap();
        m.update(data);
        m.finalize().into_bytes().to_vec()
    };
    let auth = r.header("authorization").expect("a signed request");
    let rest = auth.strip_prefix("AWS4-HMAC-SHA256 ").expect("SigV4");
    let field = |name: &str| rest.split(',').map(str::trim).find_map(|p| p.strip_prefix(name)).unwrap_or_else(|| panic!("no {name}")).to_owned();
    let scope: Vec<String> = field("Credential=").split('/').map(str::to_owned).collect();
    let signed: Vec<String> = field("SignedHeaders=").split(';').map(str::to_owned).collect();
    let stamp = r.header("x-amz-date").unwrap().to_owned();
    let headers: String = signed
        .iter()
        .map(|n| format!("{n}:{}\n", r.header(n).unwrap_or_else(|| panic!("signed header {n} missing")).split_whitespace().collect::<Vec<_>>().join(" ")))
        .collect();
    let (path, query) = r.path.split_once('?').unwrap_or((r.path.as_str(), ""));
    let canonical = format!("{}\n{path}\n{query}\n{headers}\n{}\n{}", r.method, signed.join(";"), hex(&Sha256::digest(&r.body)));
    let to_sign = format!("AWS4-HMAC-SHA256\n{stamp}\n{}/{}/{}/aws4_request\n{}", scope[1], scope[2], scope[3], hex(&Sha256::digest(canonical.as_bytes())));
    let mut key = mac(format!("AWS4{secret}").as_bytes(), scope[1].as_bytes());
    for part in [scope[2].as_str(), scope[3].as_str(), "aws4_request"] {
        key = mac(&key, part.as_bytes());
    }
    assert_eq!(hex(&mac(&key, to_sign.as_bytes())), field("Signature="), "{} {} does not verify", r.method, r.path);
    scope[0].clone()
}

/// AC6/AC16 — the built binary gives the same tallies, and leaves no AWS
/// server running behind it.
#[test]
#[ignore = "builds a real binary; run with --ignored"]
fn the_demos_pass_in_a_built_binary_and_leave_no_server_behind() {
    let p = project("bin");
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    // The unoptimised build: this test proves behaviour, not speed, and a
    // release build of the whole application would be minutes of compiling.
    let opts = cobolt_compiler::BuildOptions { verbose: false, workspace_root: Some(ws), debug: true, ..Default::default() };
    let t = std::time::Instant::now();
    let built = cobolt_compiler::build_project(&p.root.join("awshosts.project.toml"), &opts)
        .unwrap_or_else(|e| panic!("the build failed: {e}"));
    let build_s = t.elapsed().as_secs_f32();
    let mut table = Vec::new();
    for (demo, want) in DEMOS {
        let script = p.root.join(format!("bin-{demo}.json"));
        std::fs::write(&script, steps(Some(demo)).to_string()).unwrap();
        let out = Command::new(&built.binary_path)
            .env("PRC_HEADLESS_SCRIPT", &script)
            .env("PRC_HEADLESS_LIMIT", "60")
            .current_dir(&p.root)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "{demo}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr));
        let (tally, block) = outcome(&stdout);
        println!("\n  {demo} in the built binary:\n    {}", block.join("\n    "));
        assert_eq!(tally, want, "{demo} in the built binary");
        table.push(format!("{demo}: {tally}"));
    }
    // AC6: every server the binary started has ended with it.
    std::thread::sleep(std::time::Duration::from_millis(500));
    let pids = std::fs::read_to_string(p.root.join("pids.txt")).unwrap_or_default();
    let alive: Vec<&str> = pids
        .lines()
        .filter(|pid| Command::new("kill").args(["-0", pid]).output().is_ok_and(|o| o.status.success()))
        .collect();
    println!("\n078 T-A16 built binary (build {build_s:.0} s) — {}; servers started {}, still running {}", table.join("; "), pids.lines().count(), alive.len());
    assert!(alive.is_empty(), "AWS servers outlived the binary: {alive:?}");
}

/// Spec 078 AC19 — the Guide's "Calling AWS" handlers compile as the designer
/// would compile them: each `PROGRAM-ID. CONTROL--EVENT.` example becomes that
/// control's event handler on a form, and the generated program must parse
/// and pass the semantic analyser.
#[test]
fn the_guides_aws_examples_compile() {
    let guide = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/developers-guide-en.md")).unwrap();
    let start = guide.find("### Calling AWS").expect("the Calling AWS section");
    let end = start + guide[start..].find("\n## ").expect("the section ends");
    let section = &guide[start..end];
    let mut form = cobolt_forms::Form::new("AWS-GUIDE", "AWS guide", 640, 480);
    for (id, ct) in [
        ("LAMBDA-1", cobolt_forms::ControlType::AwsLambda),
        ("MCP-1", cobolt_forms::ControlType::AwsMcp),
        ("LBL-QUOTE", cobolt_forms::ControlType::Label),
        ("TXT-OUT", cobolt_forms::ControlType::TextBox),
        ("BTN-QUOTE", cobolt_forms::ControlType::Button),
        ("BTN-TOOLS", cobolt_forms::ControlType::Button),
        ("BEDROCK-1", cobolt_forms::ControlType::AwsKnowledgeBase),
        ("CORE-1", cobolt_forms::ControlType::AwsAgentCore),
        ("MEMORY-1", cobolt_forms::ControlType::AwsAgentMemory),
        ("TABLES-1", cobolt_forms::ControlType::AwsS3Tables),
        ("GLUE-1", cobolt_forms::ControlType::AwsGlue),
        ("TXT-QUESTION", cobolt_forms::ControlType::TextBox),
        ("LBL-STATE", cobolt_forms::ControlType::Label),
        ("BTN-ASK", cobolt_forms::ControlType::Button),
        ("BTN-CHAT", cobolt_forms::ControlType::Button),
        ("BTN-SALES", cobolt_forms::ControlType::Button),
        ("BTN-ETL", cobolt_forms::ControlType::Button),
        ("BTN-CHECK", cobolt_forms::ControlType::Button),
        ("ORDERS-1", cobolt_forms::ControlType::AwsDynamoDB),
        ("VISION-1", cobolt_forms::ControlType::AwsRekognition),
        ("USERS-1", cobolt_forms::ControlType::AwsCognito),
        ("TXT-ID", cobolt_forms::ControlType::TextBox),
        ("TXT-USER", cobolt_forms::ControlType::TextBox),
        ("TXT-PASSWORD", cobolt_forms::ControlType::TextBox),
        ("BTN-FIND", cobolt_forms::ControlType::Button),
        ("BTN-PHOTO", cobolt_forms::ControlType::Button),
        ("BTN-LOGIN", cobolt_forms::ControlType::Button),
        // Spec 092: celebrities and their boxes, and AwsCall.
        ("BTN-WHO", cobolt_forms::ControlType::Button),
        ("PIC-PHOTO", cobolt_forms::ControlType::PictureBox),
        ("SHP-FACE", cobolt_forms::ControlType::Shape),
        ("BTN-STACK", cobolt_forms::ControlType::Button),
        ("CLOUD-1", cobolt_forms::ControlType::AwsMcp),
        // Spec 092 A6: keys typed in at run time.
        ("BTN-CONNECT", cobolt_forms::ControlType::Button),
        ("TXT-KEY-ID", cobolt_forms::ControlType::TextBox),
        ("TXT-SECRET", cobolt_forms::ControlType::TextBox),
        ("TXT-TOKEN", cobolt_forms::ControlType::TextBox),
    ] {
        form.controls.push(cobolt_forms::Control::new(id, ct, 0, 0));
    }
    let mut handlers = Vec::new();
    for block in section.split("```cobol").skip(1).map(|b| &b[..b.find("```").unwrap()]) {
        for part in block.split("PROGRAM-ID.").skip(1) {
            let (name, body) = part.split_once('\n').unwrap();
            let name = name.trim().trim_end_matches('.');
            let (ctrl, ev) = name.split_once("--").unwrap();
            let event = match ev {
                "ONCLICK" => "onClick",
                "ONINVOKED" => "onInvoked",
                "ONTOOLRESULT" => "onToolResult",
                "ONQUERIED" => "onQueried",
                "ONJOBSTARTED" => "onJobStarted",
                "ONJOBRUN" => "onJobRun",
                "ONITEM" => "onItem",
                "ONLABELS" => "onLabels",
                "ONSIGNEDIN" => "onSignedIn",
                "ONCELEBRITIES" => "onCelebrities",
                "ONAWSCALLED" => "onAwsCalled",
                "ONERROR" => "onError",
                other => panic!("an example handler for an unexpected event: {other}"),
            };
            let c = form.controls.iter_mut().find(|c| c.id == ctrl).unwrap_or_else(|| panic!("no control {ctrl}"));
            c.events.push(cobolt_forms::EventBinding {
                event: event.to_owned(),
                paragraph: name.to_owned(),
                code: format!("       ENVIRONMENT DIVISION.\n{}", body.trim_end()),
            });
            handlers.push(name.to_owned());
        }
    }
    assert_eq!(handlers.len(), 26, "{handlers:?}");
    let src = cobolt_codegen::generate(&form);
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the Guide's AWS examples do not parse:\n{}", errors.join("\n"));
    let program = parsed.program.expect("a program");
    let sem: Vec<String> = cobolt_semantic::analyze(&program).errors().map(|d| d.message.clone()).collect();
    assert!(sem.is_empty(), "the Guide's AWS examples fail the semantic analyser: {sem:?}");
    println!("078 AC19: {} Guide handlers compile: {}", handlers.len(), handlers.join(", "));
}
