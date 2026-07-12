// SPDX-License-Identifier: Apache-2.0

use cobolt_agents::orchestrator::Orchestrator;
use cobolt_agents::retrieval::index::LexicalIndex;

#[tokio::test]
async fn test_orchestrator_fan_out() {
    let orchestrator = Orchestrator::new();
    
    // Test FormsDesigner routing
    let form_res = orchestrator.handle_request("I need a new UI form for login").await.unwrap();
    assert!(form_res.contains("FormsDesigner"));
    
    // Test EventBinder routing
    let event_res = orchestrator.handle_request("Please bind the click event to my button").await.unwrap();
    assert!(event_res.contains("EventBinder"));
    
    // Test CodeGenerator routing
    let code_res = orchestrator.handle_request("Write a function to calculate tax").await.unwrap();
    assert!(code_res.contains("CodeGenerator"));
}

#[test]
fn test_lexical_index_synonyms() {
    let index = LexicalIndex::new().expect("Failed to create index");
    
    index.add_document("doc1", "INDEXED file operations").unwrap();
    index.add_document("doc2", "DataGrid UI component").unwrap();
    index.add_document("doc3", "Unrelated text").unwrap();
    
    // Search for "keyed" which should expand to "INDEXED"
    let results = index.search("keyed file", 10).unwrap();
    assert!(!results.is_empty(), "Should find doc1 via synonym");
    assert_eq!(results[0].1, "doc1");
    
    // Search for "grid" which should expand to "DataGrid"
    let results2 = index.search("grid", 10).unwrap();
    assert!(!results2.is_empty(), "Should find doc2 via synonym");
    assert_eq!(results2[0].1, "doc2");
}
