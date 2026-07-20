// SPDX-License-Identifier: Apache-2.0
//! Live probe for the orchestrator wire paths.
//!
//! ```text
//! cargo run -p cobolt-agents --example live_probe -- <provider> <endpoint> <model> [api_key]
//! # local native:  … -- ollama http://localhost:11434/api gpt-oss:20b
//! # ollama cloud:  … -- ollama_cloud https://ollama.com/v1 gpt-oss:120b $OLLAMA_API_KEY
//! ```
//! Prints every on_log line (exactly what the IDE's Agentic AI log shows),
//! then the reply and the connection-log trace.

use cobolt_agents::{MeshRequest, Orchestrator};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (provider, endpoint, model) = match (args.get(1), args.get(2), args.get(3)) {
        (Some(p), Some(e), Some(m)) => (p.clone(), e.clone(), m.clone()),
        _ => {
            eprintln!("usage: live_probe <provider> <endpoint> <model> [api_key]");
            std::process::exit(2);
        }
    };
    let api_key = args.get(4).cloned().unwrap_or_default();

    let req = MeshRequest {
        provider,
        model,
        api_key,
        endpoint,
        endpoint_user_edited: false,
        specialist: None,
        system_prompt: "You are a terse assistant. Answer in one short sentence.".into(),
        skills: String::new(),
        context: String::new(),
        history: vec![],
        user_prompt: "Say the word COBOL and nothing else.".into(),
        temperature: 0.0,
        max_tokens: 30,
        verbose: false,
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let orch = Orchestrator::new();
        let on_log = |line: String| println!("[log] {line}");
        let on_chunk = |chunk: &str| print!("{chunk}");
        match orch.handle_request(&req, &on_log, &on_chunk).await {
            Ok((reply, trace)) => {
                println!("[reply] {reply}");
                println!("---- trace ----\n{trace}");
            }
            Err(e) => {
                eprintln!("[error] {e}");
                std::process::exit(1);
            }
        }
    });
}
