// SPDX-License-Identifier: Apache-2.0

use cobolt_agents::orchestrator::{route_specialist, MeshRequest, Orchestrator};
use cobolt_agents::retrieval::index::LexicalIndex;
use std::sync::Mutex;

fn probe_request(prompt: &str) -> MeshRequest {
    MeshRequest {
        provider: "openai".into(),
        model: "test-model".into(),
        api_key: String::new(),
        // Unreachable on purpose: the request must fail fast AFTER routing
        // and URL resolution have been logged, so tests stay offline.
        endpoint: "http://127.0.0.1:1/v1".into(),
        specialist: None,
        system_prompt: String::new(),
        skills: String::new(),
        context: String::new(),
        history: vec![],
        user_prompt: prompt.into(),
        temperature: 0.0,
        max_tokens: 16,
        verbose: false,
    }
}

/// Routing decisions are observable through the on_log callback — the first
/// line names the specialist. The network call then fails (offline test).
async fn routed_specialist(prompt: &str) -> String {
    let orch = Orchestrator::new();
    let lines: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let on_log = |line: String| lines.lock().unwrap().push(line);
    let _ = orch
        .handle_request(&probe_request(prompt), &on_log, &|_: &str| {})
        .await;
    let lines = lines.into_inner().unwrap();
    lines
        .iter()
        .find(|l| l.starts_with("routing"))
        .cloned()
        .unwrap_or_default()
}

#[tokio::test]
async fn orchestrator_routes_by_domain() {
    assert!(routed_specialist("I need a new UI form for login")
        .await
        .contains("FormsDesigner"));
    assert!(
        routed_specialist("Please bind the click event to my button")
            .await
            .contains("EventBinder")
    );
    assert!(routed_specialist("Write a function to calculate tax")
        .await
        .contains("CodeGenerator"));
}

#[test]
fn router_recognizes_power_rust_cobol_languages() {
    let form_requests = [
        "Add a button to Tab1 of TabControl-1",
        "Añade un botón a la Tab1 del TabControl-1",
        "Adicione um botão na aba Tab1 do TabControl-1",
        "TabControl-1 の Tab1 にボタンを追加",
        "向 TabControl-1 的 Tab1 添加按钮",
        "Ajouter un bouton a l'onglet Tab1 du TabControl-1",
    ];
    for prompt in form_requests {
        assert_eq!(route_specialist(prompt), "FormsDesigner", "{prompt}");
    }

    let event_requests = [
        "Bind the click event to my button",
        "Vincula el evento clic a mi botón",
        "Ligue o evento clique ao meu botão",
        "ボタンのクリックイベントをバインド",
        "绑定按钮的点击事件",
        "Associer l'evenement clic a mon bouton",
    ];
    for prompt in event_requests {
        assert_eq!(route_specialist(prompt), "EventBinder", "{prompt}");
    }
}

#[tokio::test]
async fn ollama_cloud_wrong_host_is_healed_and_openai_wire_chosen() {
    let orch = Orchestrator::new();
    let lines: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let on_log = |line: String| lines.lock().unwrap().push(line);
    let mut req = probe_request("hello");
    req.provider = "ollama_cloud".into();
    // The wrong host previously shipped as the provider default — it must be
    // healed to ollama.com before the request is attempted.
    req.endpoint = "https://api.ollama.com/v1/chat/completions".into();
    req.api_key = "test-key".into();
    let _ = orch.handle_request(&req, &on_log, &|_: &str| {}).await;
    let lines = lines.into_inner().unwrap();
    let post = lines
        .iter()
        .find(|l| l.starts_with("POST"))
        .expect("URL resolution must be logged");
    assert!(
        post.contains("https://ollama.com/v1/chat/completions"),
        "wrong host must heal to ollama.com: {post}"
    );
    assert!(post.contains("openai wire format"), "{post}");
}

#[tokio::test]
async fn local_ollama_native_wire_chosen_from_api_suffix() {
    let orch = Orchestrator::new();
    let lines: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let on_log = |line: String| lines.lock().unwrap().push(line);
    let mut req = probe_request("hello");
    req.provider = "ollama".into();
    req.endpoint = "http://127.0.0.1:1/api".into();
    let _ = orch.handle_request(&req, &on_log, &|_: &str| {}).await;
    let lines = lines.into_inner().unwrap();
    let post = lines
        .iter()
        .find(|l| l.starts_with("POST"))
        .expect("logged");
    assert!(post.contains("/api/chat"), "{post}");
    assert!(post.contains("ollama-native wire format"), "{post}");
}

#[test]
fn test_lexical_index_synonyms() {
    let index = LexicalIndex::new().expect("Failed to create index");

    index
        .add_document("doc1", "INDEXED file operations")
        .unwrap();
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
