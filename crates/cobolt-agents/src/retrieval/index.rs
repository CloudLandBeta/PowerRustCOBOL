// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index as TantivyIndex, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

pub struct LexicalIndex {
    index: TantivyIndex,
    reader: IndexReader,
    schema: Schema,
}

impl LexicalIndex {
    pub fn new() -> Result<Self, tantivy::TantivyError> {
        let mut schema_builder = Schema::builder();
        let _text_field = schema_builder.add_text_field("text", TEXT | STORED);
        let _id_field = schema_builder.add_text_field("id", STRING | STORED);
        let schema = schema_builder.build();

        let index = TantivyIndex::create_in_ram(schema.clone());
        let mut index_writer: IndexWriter = index.writer(15_000_000)?;

        index_writer.commit()?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;

        Ok(Self {
            index,
            reader,
            schema,
        })
    }

    pub fn expand_query(query: &str) -> String {
        let mut synonyms = HashMap::new();
        synonyms.insert("keyed file", "INDEXED");
        synonyms.insert("keyed", "INDEXED");
        synonyms.insert("grid", "DataGrid");
        synonyms.insert("table", "DataGrid");
        synonyms.insert("array", "table");
        synonyms.insert("call", "::");
        synonyms.insert("method", "::");
        synonyms.insert("window", "Form");
        synonyms.insert("screen", "Form");
        synonyms.insert("component", "Control");

        let mut expanded = String::new();
        for word in query.split_whitespace() {
            expanded.push_str(word);
            expanded.push(' ');
            if let Some(syn) = synonyms.get(word.to_lowercase().as_str()) {
                expanded.push_str(syn);
                expanded.push(' ');
            }
        }
        expanded
    }

    pub fn add_document(&self, id: &str, text: &str) -> Result<(), tantivy::TantivyError> {
        let mut index_writer: IndexWriter = self.index.writer(15_000_000)?;
        let text_field = self.schema.get_field("text").unwrap();
        let id_field = self.schema.get_field("id").unwrap();

        let mut doc = TantivyDocument::default();
        doc.add_text(id_field, id);
        doc.add_text(text_field, text);

        index_writer.add_document(doc)?;
        index_writer.commit()?;
        Ok(())
    }

    pub fn search(
        &self,
        query_str: &str,
        top_n: usize,
    ) -> Result<Vec<(f32, String, String)>, tantivy::TantivyError> {
        let _ = self.reader.reload();
        let text_field = self.schema.get_field("text").unwrap();
        let id_field = self.schema.get_field("id").unwrap();

        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![text_field]);

        let expanded_query = Self::expand_query(query_str);

        let query = match query_parser.parse_query(&expanded_query) {
            Ok(q) => q,
            Err(_) => return Ok(Vec::new()), // If invalid query syntax, return empty
        };

        let top_docs = searcher.search(&query, &TopDocs::with_limit(top_n))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            let id = match retrieved_doc.get_first(id_field) {
                Some(tantivy::schema::OwnedValue::Str(s)) => s.to_string(),
                _ => String::new(),
            };
            let text = match retrieved_doc.get_first(text_field) {
                Some(tantivy::schema::OwnedValue::Str(s)) => s.to_string(),
                _ => String::new(),
            };
            results.push((score, id, text));
        }

        Ok(results)
    }
}
