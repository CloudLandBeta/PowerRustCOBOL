// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

use std::path::Path;
use rig_sqlite::{SqliteVectorStore, SqliteVectorStoreTable, Column, ColumnValue};
use rig_core::embeddings::EmbeddingModel;
use rig_core::OneOrMany;
use tokio_rusqlite::Connection;
use serde::{Deserialize, Serialize};
use crate::embedding::LocalEmbeddingModel;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ControlMetadata {
    pub control_type: String,
    pub properties: Vec<String>,
    pub events: Vec<String>,
}

impl SqliteVectorStoreTable for ControlMetadata {
    fn name() -> &'static str {
        "controls_index"
    }

    fn schema() -> Vec<Column> {
        vec![
            Column::new("control_type", "TEXT PRIMARY KEY"),
            Column::new("properties", "TEXT"),
            Column::new("events", "TEXT"),
        ]
    }

    fn id(&self) -> String {
        self.control_type.clone()
    }

    fn column_values(&self) -> Vec<(&'static str, Box<dyn ColumnValue>)> {
        vec![
            ("control_type", Box::new(self.control_type.clone())),
            ("properties", Box::new(serde_json::to_string(&self.properties).unwrap())),
            ("events", Box::new(serde_json::to_string(&self.events).unwrap())),
        ]
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LanguageFeatureMetadata {
    pub feature_name: String,
    pub description: String,
    pub syntax_examples: Vec<String>,
}

impl SqliteVectorStoreTable for LanguageFeatureMetadata {
    fn name() -> &'static str {
        "rustcobol_features_index"
    }

    fn schema() -> Vec<Column> {
        vec![
            Column::new("feature_name", "TEXT PRIMARY KEY"),
            Column::new("description", "TEXT"),
            Column::new("syntax_examples", "TEXT"),
        ]
    }

    fn id(&self) -> String {
        self.feature_name.clone()
    }

    fn column_values(&self) -> Vec<(&'static str, Box<dyn ColumnValue>)> {
        vec![
            ("feature_name", Box::new(self.feature_name.clone())),
            ("description", Box::new(self.description.clone())),
            ("syntax_examples", Box::new(serde_json::to_string(&self.syntax_examples).unwrap())),
        ]
    }
}

pub async fn init_vector_stores(
    db_path: impl AsRef<Path>,
    model: LocalEmbeddingModel,
) -> Result<(
    SqliteVectorStore<LocalEmbeddingModel, ControlMetadata>,
    SqliteVectorStore<LocalEmbeddingModel, LanguageFeatureMetadata>
), Box<dyn std::error::Error + Send + Sync>> {
    
    // Safety: we must initialize the sqlite-vec extension.
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ())));
    }
    
    let conn = Connection::open(db_path).await?;
    
    // 1. Controls Index
    let count_controls: i64 = {
        let conn_clone = conn.clone();
        conn_clone.call(|c| {
            c.query_row("SELECT COUNT(*) FROM controls_index", [], |row| row.get(0)).or(Ok(0))
        }).await?
    };
    
    let controls_store = SqliteVectorStore::<LocalEmbeddingModel, ControlMetadata>::new(
        conn.clone(),
        &model
    ).await?;
    
    if count_controls == 0 {
        populate_controls(&controls_store, &model).await?;
    }

    // 2. Language Features Index
    let count_features: i64 = {
        let conn_clone = conn.clone();
        conn_clone.call(|c| {
            c.query_row("SELECT COUNT(*) FROM rustcobol_features_index", [], |row| row.get(0)).or(Ok(0))
        }).await?
    };
    
    let features_store = SqliteVectorStore::<LocalEmbeddingModel, LanguageFeatureMetadata>::new(
        conn.clone(),
        &model
    ).await?;
    
    if count_features == 0 {
        populate_language_features(&features_store, &model).await?;
    }
    
    Ok((controls_store, features_store))
}

const CONTROLS: &[&str] = &[
    "Button", "TextBox", "Label", "CheckBox", "RadioButton", "ListBox", "ComboBox", "GroupBox", "Panel", "TabControl",
    "DataGrid", "PictureBox", "ProgressBar", "MenuBar", "ToolBar", "StatusBar", "Line", "DateTimePicker", "NumericUpDown",
    "TreeView", "Splitter", "Timer", "Shape", "Animator", "AgentObject", "RestClient", "SqlDatabase", "Slider",
    "BarChart", "LineChart", "PieChart", "AreaChart", "ScatterChart", "DonutChart"
];

async fn populate_controls(
    store: &SqliteVectorStore<LocalEmbeddingModel, ControlMetadata>,
    model: &LocalEmbeddingModel
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use cobolt_forms::model::ControlType;

    let mut docs = Vec::new();
    let mut texts = Vec::new();

    for &ctrl in CONTROLS {
        let properties = cobolt_forms::model::property_names_for(ctrl);
        
        let ct = ControlType::from_str(ctrl);
        let events = ct.supported_events().iter().map(|s| s.to_string()).collect::<Vec<_>>();
        
        let description = format!(
            "Control Type: {}\nProperties: {}\nEvents: {}",
            ctrl,
            properties.join(", "),
            events.join(", ")
        );

        texts.push(description);
        docs.push(ControlMetadata {
            control_type: ctrl.to_string(),
            properties,
            events,
        });
    }

    let embeddings = model.embed_texts(texts).await?;
    
    let rows: Vec<_> = docs.into_iter().zip(embeddings).map(|(doc, emb)| {
        (doc, OneOrMany::one(emb))
    }).collect();

    store.add_rows(rows).await?;

    Ok(())
}

async fn populate_language_features(
    store: &SqliteVectorStore<LocalEmbeddingModel, LanguageFeatureMetadata>,
    model: &LocalEmbeddingModel
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let features = vec![
        LanguageFeatureMetadata {
            feature_name: "Object-Oriented Syntax / Method Invocation".into(),
            description: "RustCOBOL extends COBOL-85 with object-oriented capabilities similar to COBOL 2002/2014. It uses the `::` operator for method invocation and property access on objects. Do NOT use the legacy CALL verb when interacting with controls or objects. You must use `<object>::<method>()` or `<object>::<property>` syntax.".into(),
            syntax_examples: vec![
                "MOVE 1 TO my-checkbox::checked".into(),
                "INVOKE my-button::onClick()".into(),
                "my-datagrid::refreshBinding()".into(),
                "MOVE \"Hello\" TO my-textbox::text".into(),
            ],
        },
        LanguageFeatureMetadata {
            feature_name: "Events and Event Handlers".into(),
            description: "RustCOBOL binds events to paragraphs or sections. Controls declare their supported events, and you handle them by defining a section with the exact same name as the event prefixed by the control name or handling it via the IDE generated handlers.".into(),
            syntax_examples: vec![
                "my-button-onClick SECTION.".into(),
            ],
        }
    ];

    let mut docs = Vec::new();
    let mut texts = Vec::new();

    for feat in features {
        let description = format!(
            "Feature: {}\nDescription: {}\nExamples: {}",
            feat.feature_name,
            feat.description,
            feat.syntax_examples.join("\n")
        );
        texts.push(description);
        docs.push(feat);
    }

    let embeddings = model.embed_texts(texts).await?;
    
    let rows: Vec<_> = docs.into_iter().zip(embeddings).map(|(doc, emb)| {
        (doc, OneOrMany::one(emb))
    }).collect();

    store.add_rows(rows).await?;

    Ok(())
}
