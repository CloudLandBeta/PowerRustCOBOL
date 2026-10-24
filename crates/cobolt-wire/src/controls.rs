// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The class C catalogue (WASM-005 R13, R16, R17; plan §1.3, D1).
//!
//! On the web a class C control keeps its COBOL API, but its work runs on the
//! backend: every operation is a remote call to a program the Build generates.
//! This table is the one place both ends read to agree on that call — one
//! [`Operation`] per *(control type, operation)*: its parameter and result
//! layouts, whether the caller waits, and the binding kinds it serves. The
//! frontend's lowering, the player, the interpreter and the backend generator
//! all take names and layouts from here, never from a copy of their own.
//!
//! **R17 is a property of the type.** A parameter is typed by a [`Role`], and
//! the enum has no variant for SQL text, a path, a URL or a command: a row
//! cannot be written that lets the browser choose what the backend runs. What
//! the backend runs — the query, the file, the endpoint — is fixed at design
//! time in the generated program; the browser sends keys, values and a row
//! window.
//!
//! `CobolTable` bindings have no row: a COBOL table stays in the browser (Q8).

use crate::layout::{validate, Item, Kind, LayoutError, Occurs};

/// What a parameter or a result member *is*. Closed on purpose: nothing here
/// can carry a statement, a path, a URL or a command (R17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A key value, matched against the design-time key of the source.
    Key,
    /// A cell or field value, stored or shown as data, never interpreted.
    Value,
    /// The value of a bound parameter of the design-time query.
    Parameter,
    /// The first row of the window, from 0.
    RowOffset,
    /// The most rows the window may hold.
    RowCount,
    /// How many occurrences a table holds (its `DEPENDING ON` item), or a total.
    Count,
    /// The rows of a window.
    Rows,
    /// The changes of a save.
    Changes,
    /// What a change does: one of [`ACTIONS`].
    Action,
    /// A status code in the source's terms (a FILE STATUS, `CommitStatus`).
    Status,
    /// A human-readable message.
    Message,
    /// The name of one of the control's own properties an answer sets.
    Property,
}

/// The words a [`Role::Action`] member may hold.
pub const ACTIONS: &[&str] = &["UPDATE", "INSERT", "DELETE"];

/// The shape of a field on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// `PIC X(n)`.
    Text(usize),
    /// `PIC 9(digits)` (unsigned, integer).
    Count(u16),
    /// A table of up to `max` groups of `of`, its length in the sibling
    /// declared just before it, `counter`.
    Table { max: usize, counter: &'static str, of: &'static [Field] },
}

/// One member of a parameter or result group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    pub role: Role,
    pub shape: Shape,
}

/// The binding kinds a DataGrid operation serves. `CobolTable` is not one:
/// it never leaves the browser (Q8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    IndexedFile,
    Sql,
    RestApi,
    AgentAi,
}

impl BindingKind {
    /// The kind's spelling in a form (`_BindingKind`, `BindingSourceKind`).
    pub fn word(self) -> &'static str {
        match self {
            BindingKind::IndexedFile => "IndexedFile",
            BindingKind::Sql => "Sql",
            BindingKind::RestApi => "RestApi",
            BindingKind::AgentAi => "AgentAi",
        }
    }
}

/// One row of the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operation {
    /// The control type, as a form names it (`DataGrid`).
    pub control: &'static str,
    /// The operation, upper case; the last part of the endpoint name.
    pub operation: &'static str,
    /// The control's existing COBOL method (or host action) this serves, so
    /// the developer's API is unchanged (R12).
    pub method: &'static str,
    /// The caller waits for the answer.
    pub suspends: bool,
    /// The binding kinds served; empty for a control's own method.
    pub serves: &'static [BindingKind],
    /// The members of the parameter group, in order.
    pub params: &'static [Field],
    /// The members of the result group, in order.
    pub result: &'static [Field],
    /// The control's properties that hold a connector or a secret: on a web
    /// form they must be empty, the value living on the backend (R14).
    pub secrets: &'static [&'static str],
    /// The arguments of the COBOL call (positions from 1) that are **not**
    /// sent: they are fixed when the Build generates the backend program, so
    /// the browser can never choose them (R17). Every other argument travels,
    /// in order, in the parameter group's `ARGS`.
    pub fixed: &'static [(u8, Fixed)],
}

/// What an argument fixed at Build is. None of these ever travels from the
/// browser: the remote-lowering pass takes the call's literal and compiles it
/// into the backend program, and refuses a call whose argument is not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixed {
    /// SQL text, or an expression language that is evaluated (DynamoDB's
    /// condition and update expressions).
    Statement,
    /// What is reached: a REST resource path, a function, tool, job or index
    /// name.
    Resource,
    /// A local file: on the web the bytes move through a backend slot (R15);
    /// the backend names the file, the browser never does.
    File,
    /// A connection string: never on the web; the backend's configuration
    /// named by `BackendConfig` stands in for it (R14).
    Connector,
}

/// The most rows one window carries.
pub const MAX_WINDOW_ROWS: usize = 100;
/// The most columns a grid row carries.
pub const MAX_CELLS: usize = 32;
/// The widest cell, in bytes.
pub const CELL_BYTES: usize = 128;
/// The most bound parameters of a design-time query.
pub const MAX_PARAMS: usize = 16;
/// The most changes one save carries.
pub const MAX_CHANGES: usize = 100;

const CELL: &[Field] = &[Field { name: "CELL-TEXT", role: Role::Value, shape: Shape::Text(CELL_BYTES) }];
const KEY_CELL: &[Field] = &[Field { name: "KEY-TEXT", role: Role::Key, shape: Shape::Text(CELL_BYTES) }];
const PARAM: &[Field] = &[Field { name: "PARAM-VALUE", role: Role::Parameter, shape: Shape::Text(256) }];

const ROW: &[Field] = &[
    Field { name: "CELL-COUNT", role: Role::Count, shape: Shape::Count(3) },
    Field { name: "CELLS", role: Role::Value, shape: Shape::Table { max: MAX_CELLS, counter: "CELL-COUNT", of: CELL } },
];

const CHANGE: &[Field] = &[
    Field { name: "CHANGE-ACTION", role: Role::Action, shape: Shape::Text(6) },
    Field { name: "KEY-COUNT", role: Role::Count, shape: Shape::Count(3) },
    Field { name: "KEYS", role: Role::Key, shape: Shape::Table { max: MAX_CELLS, counter: "KEY-COUNT", of: KEY_CELL } },
    Field { name: "CELL-COUNT", role: Role::Count, shape: Shape::Count(3) },
    Field { name: "CELLS", role: Role::Value, shape: Shape::Table { max: MAX_CELLS, counter: "CELL-COUNT", of: CELL } },
];

const GRID_KINDS: &[BindingKind] = &[BindingKind::IndexedFile, BindingKind::Sql, BindingKind::RestApi, BindingKind::AgentAi];

/// The catalogue. Rows are added per control; the DataGrid's come first.
pub const CATALOGUE: &[Operation] = &[
    Operation {
        control: "DataGrid",
        operation: "READ",
        method: "RefreshBinding",
        suspends: true,
        serves: GRID_KINDS,
        params: &[
            Field { name: "ROW-OFFSET", role: Role::RowOffset, shape: Shape::Count(9) },
            Field { name: "ROW-COUNT", role: Role::RowCount, shape: Shape::Count(4) },
            Field { name: "PARAM-COUNT", role: Role::Count, shape: Shape::Count(2) },
            Field { name: "PARAMS", role: Role::Parameter, shape: Shape::Table { max: MAX_PARAMS, counter: "PARAM-COUNT", of: PARAM } },
        ],
        // The read's own outcome first: a source that cannot be read (FILE
        // STATUS 35, a refused CONNECT) is not an empty source.
        result: &[
            Field { name: "READ-STATUS", role: Role::Status, shape: Shape::Text(2) },
            Field { name: "READ-MESSAGE", role: Role::Message, shape: Shape::Text(256) },
            Field { name: "ROW-TOTAL", role: Role::Count, shape: Shape::Count(9) },
            Field { name: "ROWS-RETURNED", role: Role::Count, shape: Shape::Count(4) },
            Field { name: "ROWS", role: Role::Rows, shape: Shape::Table { max: MAX_WINDOW_ROWS, counter: "ROWS-RETURNED", of: ROW } },
        ],
        secrets: &[],
        fixed: &[],
    },
    Operation {
        control: "DataGrid",
        operation: "SAVE",
        method: "Save",
        suspends: true,
        serves: GRID_KINDS,
        params: &[
            Field { name: "CHANGE-COUNT", role: Role::Count, shape: Shape::Count(4) },
            Field { name: "CHANGES", role: Role::Changes, shape: Shape::Table { max: MAX_CHANGES, counter: "CHANGE-COUNT", of: CHANGE } },
        ],
        result: &[
            Field { name: "COMMIT-STATUS", role: Role::Status, shape: Shape::Text(2) },
            Field { name: "COMMIT-MESSAGE", role: Role::Message, shape: Shape::Text(256) },
            Field { name: "FAILED-CHANGE", role: Role::Count, shape: Shape::Count(4) },
        ],
        secrets: &[],
        fixed: &[],
    },
    // ── IndexedFile: the facade codegen emits for the control (`<id>-<OP>`).
    //    Stateless (O4): every call carries the key and the record it starts from.
    ix("OPEN"),
    ix("START"),
    ix("READ-NEXT"),
    ix("READ-PREVIOUS"),
    ix("READ-FIRST"),
    ix("READ-LAST"),
    ix("READ-INVALID"),
    ix("WRITE"),
    ix("REWRITE"),
    ix("DELETE"),
    ix("COMMIT"),
    ix("ROLLBACK"),
    ix("CLOSE"),
    // ── SqlDatabase: the statement is the call's literal, fixed at Build.
    m("SqlDatabase", "OPEN", "Open", SQL_SECRETS, &[(1, Fixed::Connector)]),
    m("SqlDatabase", "QUERY", "Query", SQL_SECRETS, &[(1, Fixed::Statement)]),
    m("SqlDatabase", "EXECUTE", "Execute", SQL_SECRETS, &[(1, Fixed::Statement)]),
    m("SqlDatabase", "CLOSE", "Close", SQL_SECRETS, &[]),
    // ── RestClient: the resource is fixed at Build, the base URL and the
    //    token are the backend's; a body travels as data.
    m("RestClient", "GET", "Get", &["AuthToken"], &[(1, Fixed::Resource)]),
    m("RestClient", "POST", "Post", &["AuthToken"], &[(1, Fixed::Resource)]),
    m("RestClient", "PUT", "Put", &["AuthToken"], &[(1, Fixed::Resource)]),
    m("RestClient", "DELETE", "Delete", &["AuthToken"], &[(1, Fixed::Resource)]),
    // ── AgentObject, KnowledgeBase, WebSearch: questions and documents are data.
    m("AgentObject", "ASK", "Ask", &["AgentAPIKey"], &[]),
    m("KnowledgeBase", "SEARCH", "Search", &["EmbeddingAPIKey"], &[]),
    m("KnowledgeBase", "ADDDOCUMENT", "AddDocument", &["EmbeddingAPIKey"], &[]),
    m("KnowledgeBase", "UPDATEDOCUMENT", "UpdateDocument", &["EmbeddingAPIKey"], &[]),
    m("KnowledgeBase", "DELETEDOCUMENT", "DeleteDocument", &["EmbeddingAPIKey"], &[]),
    m("KnowledgeBase", "GETDOCUMENT", "GetDocument", &["EmbeddingAPIKey"], &[]),
    m("KnowledgeBase", "LISTDOCUMENTS", "ListDocuments", &["EmbeddingAPIKey"], &[]),
    m("WebSearch", "SEARCH", "Search", &["ApiKey"], &[]),
    // ── FileDropZone and Viewer: files through the backend's slots (R15).
    m("FileDropZone", "COMMITFILES", "CommitFiles", &[], &[]),
    m("Viewer", "SAVEAS", "SaveAs", &[], &[(1, Fixed::File)]),
    m("Viewer", "SAVEASPDF", "SaveAsPdf", &[], &[(1, Fixed::File)]),
    // ── Maps: the data operations (its tiles stay class B).
    m("Maps", "GEOCODE", "Geocode", &[], &[]),
    m("Maps", "REVERSEGEOCODE", "ReverseGeocode", &[], &[]),
    m("Maps", "DIRECTIONS", "Directions", &[], &[]),
    m("Maps", "DISTANCEMATRIX", "DistanceMatrix", &[], &[]),
    m("Maps", "PLACESSEARCH", "PlacesSearch", &[], &[]),
    m("Maps", "TRACEROAD", "TraceRoad", &[], &[]),
    // ── The AWS controls: one row per route of `routes.toml` (runtime). The
    //    control holds the NAME of a connection (`Connection`), no key.
    m("AwsDynamoDB", "GETITEM", "GetItem", &[], &[]),
    m("AwsDynamoDB", "QUERY", "Query", &[], &[(1, Fixed::Statement)]),
    m("AwsDynamoDB", "SCAN", "Scan", &[], &[]),
    m("AwsDynamoDB", "PUTITEM", "PutItem", &[], &[]),
    m("AwsDynamoDB", "UPDATEITEM", "UpdateItem", &[], &[(2, Fixed::Statement)]),
    m("AwsDynamoDB", "DELETEITEM", "DeleteItem", &[], &[]),
    m("AwsS3", "LIST", "List", &[], &[]),
    m("AwsS3", "GETOBJECT", "GetObject", &[], &[(2, Fixed::File)]),
    m("AwsS3", "PUTOBJECT", "PutObject", &[], &[(3, Fixed::File)]),
    m("AwsS3", "DELETEOBJECT", "DeleteObject", &[], &[]),
    m("AwsS3", "LISTBUCKETS", "ListBuckets", &[], &[]),
    m("AwsS3Vectors", "QUERYVECTORS", "QueryVectors", &[], &[(1, Fixed::Resource)]),
    m("AwsS3Vectors", "PUTVECTORS", "PutVectors", &[], &[(1, Fixed::Resource)]),
    m("AwsRekognition", "DETECTLABELS", "DetectLabels", &[], &[(1, Fixed::File)]),
    m("AwsRekognition", "DETECTTEXT", "DetectText", &[], &[(1, Fixed::File)]),
    m("AwsRekognition", "DETECTFACES", "DetectFaces", &[], &[(1, Fixed::File)]),
    m("AwsRekognition", "RECOGNIZECELEBRITIES", "RecognizeCelebrities", &[], &[(1, Fixed::File)]),
    m("AwsPolly", "SYNTHESIZE", "Synthesize", &[], &[(4, Fixed::File)]),
    m("AwsComprehend", "DETECTSENTIMENT", "DetectSentiment", &[], &[]),
    m("AwsComprehend", "DETECTENTITIES", "DetectEntities", &[], &[]),
    m("AwsComprehend", "DETECTKEYPHRASES", "DetectKeyPhrases", &[], &[]),
    m("AwsComprehend", "DETECTLANGUAGE", "DetectLanguage", &[], &[]),
    m("AwsTextract", "DETECTTEXT", "DetectText", &[], &[(1, Fixed::File)]),
    m("AwsTextract", "ANALYZEDOCUMENT", "AnalyzeDocument", &[], &[(1, Fixed::File)]),
    m("AwsEC2", "DESCRIBE", "Describe", &[], &[(1, Fixed::Resource)]),
    m("AwsEC2", "START", "Start", &[], &[(1, Fixed::Resource)]),
    m("AwsEC2", "STOP", "Stop", &[], &[(1, Fixed::Resource)]),
    m("AwsCognito", "SIGNUP", "SignUp", &[], &[]),
    m("AwsCognito", "CONFIRM", "Confirm", &[], &[]),
    m("AwsCognito", "SIGNIN", "SignIn", &[], &[]),
    m("AwsCognito", "GETATTRIBUTE", "GetAttribute", &[], &[]),
    m("AwsCognito", "SIGNOUT", "SignOut", &[], &[]),
    m("AwsLambda", "INVOKE", "Invoke", &[], &[(1, Fixed::Resource)]),
    m("AwsLambda", "LISTFUNCTIONS", "ListFunctions", &[], &[]),
    m("AwsKnowledgeBase", "QUERY", "Query", &[], &[(1, Fixed::Resource)]),
    m("AwsKnowledgeBase", "LISTKNOWLEDGEBASES", "ListKnowledgeBases", &[], &[]),
    m("AwsAgentCore", "INVOKE", "Invoke", &[], &[(1, Fixed::Resource)]),
    m("AwsAgentMemory", "RECORDEVENT", "RecordEvent", &[], &[(1, Fixed::Resource)]),
    m("AwsAgentMemory", "RETRIEVE", "Retrieve", &[], &[(1, Fixed::Resource), (2, Fixed::Resource)]),
    m("AwsS3Tables", "LISTTABLES", "ListTables", &[], &[]),
    m("AwsS3Tables", "QUERY", "Query", &[], &[(1, Fixed::Statement)]),
    m("AwsS3Tables", "APPENDROWS", "AppendRows", &[], &[(1, Fixed::Resource)]),
    m("AwsGlue", "STARTJOBRUN", "StartJobRun", &[], &[(1, Fixed::Resource)]),
    m("AwsGlue", "GETJOBRUN", "GetJobRun", &[], &[(1, Fixed::Resource)]),
    m("AwsGlue", "STARTCRAWLER", "StartCrawler", &[], &[(1, Fixed::Resource)]),
    m("AwsGlue", "GETTABLESCHEMA", "GetTableSchema", &[], &[(1, Fixed::Resource), (2, Fixed::Resource)]),
    m("AwsMcp", "CALL", "Call", &[], &[(1, Fixed::Resource)]),
    m("AwsMcp", "LISTTOOLS", "ListTools", &[], &[]),
    m("AwsMcp", "AWSCALL", "AwsCall", &[], &[(1, Fixed::Resource), (2, Fixed::Resource)]),
];

const SQL_SECRETS: &[&str] = &["ConnectionString"];

/// The most arguments a method call sends.
pub const MAX_ARGS: usize = 8;
/// The widest argument or answer, in bytes.
pub const ARG_BYTES: usize = 2048;
/// The most properties one answer sets.
pub const MAX_PROPS: usize = 32;

const ARG: &[Field] = &[Field { name: "ARG-VALUE", role: Role::Value, shape: Shape::Text(ARG_BYTES) }];
const PROP: &[Field] = &[
    Field { name: "PROP-NAME", role: Role::Property, shape: Shape::Text(64) },
    Field { name: "PROP-VALUE", role: Role::Value, shape: Shape::Text(ARG_BYTES) },
];

/// A method's parameters: the arguments that are not fixed at Build, in order.
const METHOD_PARAMS: &[Field] = &[
    Field { name: "ARG-COUNT", role: Role::Count, shape: Shape::Count(2) },
    Field { name: "ARGS", role: Role::Value, shape: Shape::Table { max: MAX_ARGS, counter: "ARG-COUNT", of: ARG } },
];

/// A method's answer: its value, the properties it set (`LastError`,
/// `StatusCode`, a provider's fields), and the rows of a tabular answer.
const METHOD_RESULT: &[Field] = &[
    Field { name: "RESULT-VALUE", role: Role::Value, shape: Shape::Text(ARG_BYTES) },
    Field { name: "LAST-ERROR", role: Role::Message, shape: Shape::Text(512) },
    Field { name: "PROP-COUNT", role: Role::Count, shape: Shape::Count(2) },
    Field { name: "PROPS", role: Role::Value, shape: Shape::Table { max: MAX_PROPS, counter: "PROP-COUNT", of: PROP } },
    Field { name: "ROWS-RETURNED", role: Role::Count, shape: Shape::Count(4) },
    Field { name: "ROWS", role: Role::Rows, shape: Shape::Table { max: MAX_WINDOW_ROWS, counter: "ROWS-RETURNED", of: ROW } },
];

/// An IndexedFile call: the key and the record it starts from…
const IX_PARAMS: &[Field] = &[
    Field { name: "KEY-VALUE", role: Role::Key, shape: Shape::Text(256) },
    Field { name: "RECORD-IMAGE", role: Role::Value, shape: Shape::Text(4096) },
];

/// …and what the file did: its FILE STATUS and the record now current.
const IX_RESULT: &[Field] = &[
    Field { name: "FILE-STATUS", role: Role::Status, shape: Shape::Text(2) },
    Field { name: "RECORD-IMAGE", role: Role::Value, shape: Shape::Text(4096) },
    Field { name: "AT-END", role: Role::Count, shape: Shape::Count(1) },
];

const fn m(
    control: &'static str,
    operation: &'static str,
    method: &'static str,
    secrets: &'static [&'static str],
    fixed: &'static [(u8, Fixed)],
) -> Operation {
    Operation { control, operation, method, suspends: true, serves: &[], params: METHOD_PARAMS, result: METHOD_RESULT, secrets, fixed }
}

const fn ix(operation: &'static str) -> Operation {
    Operation {
        control: "IndexedFile",
        operation,
        method: operation,
        suspends: true,
        serves: &[],
        params: IX_PARAMS,
        result: IX_RESULT,
        secrets: &[],
        fixed: &[],
    }
}

/// The secret-bearing properties of a control type (R14), from its rows.
pub fn secrets_of(control: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for op in of_control(control) {
        for s in op.secrets {
            if !out.contains(s) {
                out.push(s);
            }
        }
    }
    out
}

/// Every row of the catalogue.
pub fn rows() -> &'static [Operation] {
    CATALOGUE
}

/// The row of `control`'s `operation` (any case), if there is one.
pub fn find(control: &str, operation: &str) -> Option<&'static Operation> {
    CATALOGUE
        .iter()
        .find(|o| o.control.eq_ignore_ascii_case(control) && o.operation.eq_ignore_ascii_case(operation))
}

/// The rows of one control type.
pub fn of_control(control: &str) -> impl Iterator<Item = &'static Operation> + '_ {
    CATALOGUE.iter().filter(move |o| o.control.eq_ignore_ascii_case(control))
}

/// The row of a DataGrid operation serving binding kind `kind` (any case;
/// `CobolTable` has none).
pub fn for_binding(kind: &str, operation: &str) -> Option<&'static Operation> {
    CATALOGUE.iter().find(|o| {
        o.operation.eq_ignore_ascii_case(operation) && o.serves.iter().any(|k| k.word().eq_ignore_ascii_case(kind))
    })
}

impl Operation {
    /// The parameter group, as the wire lays it out: `<NAME>-PARAMS`.
    pub fn param_layout(&self) -> Item {
        group(&format!("{}-{}-PARAMS", self.control.to_ascii_uppercase(), self.operation), self.params)
    }

    /// The result group: `<NAME>-RESULT`.
    pub fn result_layout(&self) -> Item {
        group(&format!("{}-{}-RESULT", self.control.to_ascii_uppercase(), self.operation), self.result)
    }

    /// Both layouts pass the mapping's layout check.
    pub fn check(&self) -> Result<(), LayoutError> {
        validate(&self.param_layout())?;
        validate(&self.result_layout())
    }

    /// Every field of the parameters, tables walked, in order.
    pub fn all_params(&self) -> Vec<&'static Field> {
        let mut out = Vec::new();
        walk(self.params, &mut out);
        out
    }
}

fn walk(fields: &'static [Field], out: &mut Vec<&'static Field>) {
    for f in fields {
        out.push(f);
        if let Shape::Table { of, .. } = f.shape {
            walk(of, out);
        }
    }
}

fn group(name: &str, fields: &[Field]) -> Item {
    Item {
        name: name.to_string(),
        filler: false,
        redefines: false,
        occurs: None,
        kind: Kind::Group { children: fields.iter().map(item).collect() },
        value: None,
    }
}

fn item(f: &Field) -> Item {
    let (occurs, kind) = match f.shape {
        Shape::Text(len) => (None, Kind::Alphanumeric { len, justified: false }),
        Shape::Count(digits) => (None, Kind::Numeric { digits, scale: 0, signed: false }),
        Shape::Table { max, counter, of } => (
            Some(Occurs { min: 0, max, depending_on: Some(counter.to_string()) }),
            Kind::Group { children: of.iter().map(item).collect() },
        ),
    };
    Item { name: f.name.to_string(), filler: false, redefines: false, occurs, kind, value: None }
}

// ── endpoint names ──────────────────────────────────────────────────────────

/// The name of the generated endpoint for `control` on `form` doing
/// `operation`: `RC-<len>-<FORM>-<len>-<CONTROL>-<OPERATION>`, upper case
/// (COBOL names ignore case).
///
/// Each part is preceded by its length, so the name is collision-free: two
/// distinct *(form, control)* pairs never meet in one name, whatever hyphens
/// their names hold (`A-B`/`C` and `A`/`B-C` differ). It is deterministic —
/// a pure function of its inputs — and every character is one a COBOL name
/// may hold. A part that is empty or holds anything but letters, digits and
/// hyphens is refused, as is an operation the catalogue does not list.
pub fn endpoint_name(form: &str, control: &str, operation: &str) -> Result<String, String> {
    let form = cobol_word(form, "form")?;
    let control = cobol_word(control, "control")?;
    let op = operation.trim().to_ascii_uppercase();
    if !CATALOGUE.iter().any(|o| o.operation == op) {
        return Err(format!("\"{operation}\" is not an operation of the class C catalogue"));
    }
    Ok(format!("RC-{}-{form}-{}-{control}-{op}", form.len(), control.len()))
}

fn cobol_word(text: &str, what: &str) -> Result<String, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err(format!("the {what} name is empty"));
    }
    if !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("the {what} name \"{t}\" holds a character a COBOL name cannot (letters, digits and hyphens only)"));
    }
    Ok(t.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::encode;
    use crate::node::Node;

    #[test]
    fn every_row_passes_the_layout_check() {
        for op in rows() {
            op.check().unwrap_or_else(|e| panic!("{} {}: {e}", op.control, op.operation));
            // And the mapping can write an empty group of it.
            let layout = op.param_layout();
            encode(&layout, &Node::initial(&layout)).unwrap_or_else(|e| panic!("{} {}: {e}", op.control, op.operation));
        }
    }

    #[test]
    fn the_grid_rows_serve_the_four_backend_binding_kinds_and_not_cobol_tables() {
        for kind in ["IndexedFile", "Sql", "RestApi", "AgentAi"] {
            for op in ["READ", "SAVE"] {
                let row = for_binding(kind, op).unwrap_or_else(|| panic!("no {op} row for {kind}"));
                assert_eq!(row.control, "DataGrid");
                assert!(row.suspends);
            }
        }
        assert!(for_binding("CobolTable", "READ").is_none(), "a COBOL table stays in the browser (Q8)");
        assert!(for_binding("CobolTable", "SAVE").is_none());
        assert!(rows().iter().all(|o| !o.serves.iter().any(|k| k.word().eq_ignore_ascii_case("CobolTable"))));
    }

    #[test]
    fn the_grid_read_carries_a_row_window_and_bound_parameters_only() {
        let read = find("datagrid", "read").unwrap();
        let roles: Vec<Role> = read.params.iter().map(|f| f.role).collect();
        assert_eq!(roles, vec![Role::RowOffset, Role::RowCount, Role::Count, Role::Parameter]);
    }

    #[test]
    fn no_parameter_can_carry_sql_a_path_a_url_or_a_command() {
        // The roles a parameter may have. `Role` has no other variant, so this
        // walk is the R17 check: adding a variant that could carry a statement
        // fails to compile here until it is put in one of these arms.
        for op in rows() {
            for f in op.all_params() {
                let ok = match f.role {
                    Role::Key | Role::Value | Role::Parameter | Role::RowOffset | Role::RowCount | Role::Count => true,
                    Role::Rows | Role::Changes | Role::Action | Role::Status | Role::Message | Role::Property => true,
                };
                assert!(ok);
                let name = f.name.to_ascii_uppercase();
                for word in ["SQL", "QUERY", "PATH", "URL", "COMMAND", "STATEMENT", "FILE-NAME"] {
                    assert!(!name.contains(word), "{} {}: parameter {} looks like it carries a {word}", op.control, op.operation, f.name);
                }
            }
        }
    }

    #[test]
    fn each_row_is_listed_once_and_its_fixed_arguments_are_sound() {
        let mut seen = std::collections::HashSet::new();
        for op in rows() {
            assert!(seen.insert((op.control, op.operation)), "{} {} is listed twice", op.control, op.operation);
            assert_eq!(op.operation, op.operation.to_ascii_uppercase());
            // A trailing `-<digits>` is kept free for a call site's number.
            let tail = op.operation.rsplit('-').next().unwrap_or("");
            assert!(tail.is_empty() || !tail.chars().all(|c| c.is_ascii_digit()), "{} {}", op.control, op.operation);
            let mut at = std::collections::HashSet::new();
            for (pos, _) in op.fixed {
                assert!((1..=MAX_ARGS as u8).contains(pos), "{} {}: position {pos}", op.control, op.operation);
                assert!(at.insert(*pos), "{} {}: position {pos} fixed twice", op.control, op.operation);
            }
        }
        println!("catalogue: {} rows over {} control types", rows().len(), {
            let mut c: Vec<&str> = rows().iter().map(|o| o.control).collect();
            c.dedup();
            c.len()
        });
    }

    #[test]
    fn the_secret_bearing_properties_are_named_per_control() {
        assert_eq!(secrets_of("SqlDatabase"), vec!["ConnectionString"]);
        assert_eq!(secrets_of("AgentObject"), vec!["AgentAPIKey"]);
        assert_eq!(secrets_of("RestClient"), vec!["AuthToken"]);
        assert_eq!(secrets_of("KnowledgeBase"), vec!["EmbeddingAPIKey"]);
        assert_eq!(secrets_of("WebSearch"), vec!["ApiKey"]);
        assert!(secrets_of("AwsS3").is_empty(), "an AWS control holds a connection's name, never a key");
        // The connection string never travels: `Open`'s argument is fixed.
        assert_eq!(find("SqlDatabase", "OPEN").unwrap().fixed, &[(1, Fixed::Connector)]);
        assert_eq!(find("SqlDatabase", "QUERY").unwrap().fixed, &[(1, Fixed::Statement)]);
        assert_eq!(find("RestClient", "GET").unwrap().fixed, &[(1, Fixed::Resource)]);
    }

    #[test]
    fn the_endpoint_name_is_stable_and_distinct() {
        let a = endpoint_name("Orders", "Grid1", "READ").unwrap();
        assert_eq!(a, "RC-6-ORDERS-5-GRID1-READ");
        assert_eq!(a, endpoint_name("ORDERS", "grid1", "read").unwrap(), "COBOL names ignore case");
        // Hyphens cannot make two pairs meet.
        let x = endpoint_name("A-B", "C", "READ").unwrap();
        let y = endpoint_name("A", "B-C", "READ").unwrap();
        assert_ne!(x, y);
        let mut all = std::collections::HashSet::new();
        for form in ["F", "F-1", "F1", "ORDERS"] {
            for control in ["G", "G-1", "1-G", "GRID"] {
                for op in ["READ", "SAVE"] {
                    assert!(all.insert(endpoint_name(form, control, op).unwrap()), "{form} {control} {op} collided");
                }
            }
        }
        assert!(endpoint_name("", "G", "READ").is_err());
        assert!(endpoint_name("F", "G H", "READ").is_err());
        assert!(endpoint_name("F", "G", "DROP").is_err());
    }
}
