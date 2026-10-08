// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The AWS controls, as data (spec 078).
//!
//! Every AWS control shares one shape — a connection, `Mode`, the timeouts,
//! `AllowWrite`, `Verbose`, an asynchronous lifecycle, a row set — and differs
//! only in its own inputs, its completion events and its tile. This table is
//! where those differences live, so the model, the painter, the toolbox and
//! the runtime read one description instead of each keeping a copy. What each
//! operation does on AWS is the route table's (`cobolt-runtime`'s
//! `aws/routes.toml`), not this file's.

use crate::model::ControlType;

/// A design-time property's default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seed {
    Str(&'static str),
    Int(i64),
    Bool(bool),
}

/// One AWS control.
#[derive(Debug, Clone, Copy)]
pub struct AwsControl {
    /// The type's name, as `ControlType::as_str` spells it.
    pub name: &'static str,
    /// Every event it raises: its operations' completion events, the primary
    /// one first (the event a double-click on the control binds), then
    /// [`SHARED_EVENTS`] — whole, because the model hands out static lists.
    pub events: &'static [&'static str],
    /// Its own design-time properties, beyond the shared set.
    pub seeds: &'static [(&'static str, Seed)],
    /// Every run-time answer: [`SHARED_RUNTIME`], then its own.
    pub runtime: &'static [&'static str],
    /// The property the canvas card shows as its caption, and the caption
    /// when that property is empty.
    pub caption: (&'static str, &'static str),
    /// The hand-drawn tile (amendment A5).
    pub icon_svg: &'static str,
}

/// The design-time properties every AWS control has.
pub const SHARED_SEEDS: &[(&str, Seed)] = &[
    // The project's AWS connection, by name: a profile NAME and a region —
    // never a key (R11, R12).
    ("Connection", Seed::Str("")),
    // Async I/O (spec 032), as RestClient and WebSearch.
    ("Mode", Seed::Str("Async")),
    ("Busy", Seed::Bool(false)),
    ("TimeoutMs", Seed::Int(30_000)),
    // A server's first start downloads packages (amendment A3).
    ("StartTimeoutMs", Seed::Int(120_000)),
    // Read-only unless the developer opts in (R25).
    ("AllowWrite", Seed::Bool(false)),
    // Narrates each call into the output, credentials masked (R27).
    ("Verbose", Seed::Bool(false)),
];

/// The run-time answers every AWS control has: the answer's text, the whole
/// JSON, the row set's size, and the failure text.
pub const SHARED_RUNTIME: &[&str] = &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError"];

/// The lifecycle events every AWS control raises after its own (R19).
pub const SHARED_EVENTS: &[&str] = &["onComplete", "onError", "onTimeout", "onCancelled"];

/// Every AWS control, in toolbox order.
pub const CONTROLS: &[AwsControl] = &[
    AwsControl {
        name: "AwsLambda",
        events: &["onInvoked", "onFunctionsListed", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[("FunctionName", Seed::Str(""))],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError", "FunctionError"],
        caption: ("FunctionName", "Lambda"),
        icon_svg: include_str!("../assets/aws/AwsLambda.svg"),
    },
    AwsControl {
        name: "AwsMcp",
        events: &["onToolResult", "onToolsListed", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            // The route-table server whose tools `Call` reaches.
            ("ServerId", Seed::Str("")),
            ("ToolName", Seed::Str("")),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError"],
        caption: ("ServerId", "MCP"),
        icon_svg: include_str!("../assets/aws/AwsMcp.svg"),
    },
    AwsControl {
        name: "AwsKnowledgeBase",
        events: &["onQueried", "onKnowledgeBasesListed", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("KnowledgeBaseId", Seed::Str("")),
            ("MaxResults", Seed::Int(10)),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError"],
        caption: ("KnowledgeBaseId", "Knowledge Base"),
        icon_svg: include_str!("../assets/aws/AwsKnowledgeBase.svg"),
    },
    AwsControl {
        name: "AwsAgentCore",
        events: &["onInvoked", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("RuntimeArn", Seed::Str("")),
            ("SessionId", Seed::Str("")),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError", "SessionId"],
        caption: ("RuntimeArn", "AgentCore"),
        icon_svg: include_str!("../assets/aws/AwsAgentCore.svg"),
    },
    AwsControl {
        name: "AwsAgentMemory",
        events: &["onEventRecorded", "onRetrieved", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("MemoryId", Seed::Str("")),
            ("ActorId", Seed::Str("")),
            ("SessionId", Seed::Str("")),
            ("Namespace", Seed::Str("")),
            ("TopK", Seed::Int(10)),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError", "EventId"],
        caption: ("MemoryId", "Memory"),
        icon_svg: include_str!("../assets/aws/AwsAgentMemory.svg"),
    },
    AwsControl {
        name: "AwsS3Tables",
        events: &["onTablesListed", "onQueried", "onRowsAppended", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("TableBucketArn", Seed::Str("")),
            ("Namespace", Seed::Str("")),
            ("TableName", Seed::Str("")),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError", "RowsAppended"],
        caption: ("Namespace", "S3 Tables"),
        icon_svg: include_str!("../assets/aws/AwsS3Tables.svg"),
    },
    AwsControl {
        name: "AwsGlue",
        events: &["onJobStarted", "onJobRun", "onCrawlerStarted", "onTableSchema", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("JobName", Seed::Str("")),
            ("JobRunId", Seed::Str("")),
            ("CrawlerName", Seed::Str("")),
            ("DatabaseName", Seed::Str("")),
            ("TableName", Seed::Str("")),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError", "JobRunId", "State"],
        caption: ("JobName", "Glue"),
        icon_svg: include_str!("../assets/aws/AwsGlue.svg"),
    },
    AwsControl {
        name: "AwsDynamoDB",
        events: &["onItem", "onQueried", "onScanned", "onItemPut", "onItemUpdated", "onItemDeleted", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("TableName", Seed::Str("")),
            ("IndexName", Seed::Str("")),
            ("Limit", Seed::Int(100)),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError", "Found"],
        caption: ("TableName", "DynamoDB"),
        icon_svg: include_str!("../assets/aws/AwsDynamoDB.svg"),
    },
    AwsControl {
        name: "AwsS3",
        events: &["onListed", "onObject", "onObjectPut", "onObjectDeleted", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("Bucket", Seed::Str("")),
            ("MaxKeys", Seed::Int(1000)),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError", "ContentType"],
        caption: ("Bucket", "S3"),
        icon_svg: include_str!("../assets/aws/AwsS3.svg"),
    },
    AwsControl {
        name: "AwsS3Vectors",
        events: &["onVectorsQueried", "onVectorsPut", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("VectorBucketName", Seed::Str("")),
            ("IndexName", Seed::Str("")),
            ("TopK", Seed::Int(5)),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError"],
        caption: ("IndexName", "S3 Vectors"),
        icon_svg: include_str!("../assets/aws/AwsS3Vectors.svg"),
    },
    AwsControl {
        name: "AwsRekognition",
        events: &["onLabels", "onTextDetected", "onFaces", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("MinConfidence", Seed::Int(70)),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError"],
        caption: ("", "Rekognition"),
        icon_svg: include_str!("../assets/aws/AwsRekognition.svg"),
    },
    AwsControl {
        name: "AwsPolly",
        events: &["onSynthesized", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("VoiceId", Seed::Str("Joanna")),
            ("OutputFormat", Seed::Str("mp3")),
            ("Engine", Seed::Str("neural")),
            ("OutputFile", Seed::Str("")),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError", "ContentType", "Characters", "SavedFile"],
        caption: ("VoiceId", "Polly"),
        icon_svg: include_str!("../assets/aws/AwsPolly.svg"),
    },
    AwsControl {
        name: "AwsComprehend",
        events: &["onSentiment", "onEntities", "onKeyPhrases", "onLanguage", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("LanguageCode", Seed::Str("en")),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError", "Sentiment", "Language"],
        caption: ("", "Comprehend"),
        icon_svg: include_str!("../assets/aws/AwsComprehend.svg"),
    },
    AwsControl {
        name: "AwsTextract",
        events: &["onTextDetected", "onDocumentAnalyzed", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[

        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError"],
        caption: ("", "Textract"),
        icon_svg: include_str!("../assets/aws/AwsTextract.svg"),
    },
    AwsControl {
        name: "AwsEC2",
        events: &["onDescribed", "onInstancesStarted", "onInstancesStopped", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("InstanceIds", Seed::Str("")),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError"],
        caption: ("InstanceIds", "EC2"),
        icon_svg: include_str!("../assets/aws/AwsEC2.svg"),
    },
    AwsControl {
        name: "AwsCognito",
        events: &["onSignedUp", "onConfirmed", "onSignedIn", "onAttribute", "onSignedOut", "onComplete", "onError", "onTimeout", "onCancelled"],
        seeds: &[
            ("ClientId", Seed::Str("")),
        ],
        runtime: &["ResponseBody", "ResultJson", "RowCount", "Busy", "LastError", "SignedIn", "UserName", "Confirmed", "Challenge"],
        caption: ("ClientId", "Cognito"),
        icon_svg: include_str!("../assets/aws/AwsCognito.svg"),
    },
];

/// The AWS control of this type, if it is one.
pub fn get(ct: &ControlType) -> Option<&'static AwsControl> {
    by_name(ct.as_str())
}

/// The AWS control of this name, if it is one.
pub fn by_name(name: &str) -> Option<&'static AwsControl> {
    CONTROLS.iter().find(|c| c.name == name)
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Every AWS control type has one entry, and every entry one type.
    #[test]
    fn the_catalogue_and_the_control_types_agree() {
        let aws_types: Vec<&str> = ControlType::ALL
            .iter()
            .filter(|t| t.is_aws())
            .map(|t| t.as_str())
            .collect();
        let names: Vec<&str> = CONTROLS.iter().map(|c| c.name).collect();
        assert_eq!(aws_types.len(), names.len(), "{aws_types:?} vs {names:?}");
        for n in &names {
            assert!(aws_types.contains(n), "{n} has no ControlType");
            assert!(!by_name(n).unwrap().events.is_empty(), "{n} has no completion event");
            let c = by_name(n).unwrap();
            assert!(c.icon_svg.contains("<svg"), "{n} has no tile");
            assert!(c.events.ends_with(SHARED_EVENTS), "{n}: the shared lifecycle ends its events");
            assert!(c.runtime.starts_with(SHARED_RUNTIME), "{n}: the shared answers start its runtime list");
        }
    }
}
