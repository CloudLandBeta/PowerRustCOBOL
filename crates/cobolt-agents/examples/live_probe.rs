// SPDX-License-Identifier: Apache-2.0
//! Live probe for the Rig chat transport (the mesh's single-agent wire).
//!
//! ```text
//! cargo run -p cobolt-agents --example live_probe -- <provider> <endpoint> <model> [api_key]
//! # local:         … -- ollama http://localhost:11434/v1 gpt-oss:20b
//! # ollama cloud:  … -- ollama_cloud https://ollama.com/v1 gpt-oss:120b $OLLAMA_API_KEY
//! # anthropic:     … -- anthropic https://api.anthropic.com/v1 claude-sonnet-5 $ANTHROPIC_API_KEY
//! ```
//! Streams text deltas as they arrive, then prints the reply and exact token
//! usage.

use cobolt_agents::rig_transport::{run_chat_blocking, ChatCall};

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

    let call = ChatCall {
        provider,
        model,
        api_key,
        endpoint,
        system_prompt: "You are a terse assistant. Answer in one short sentence.".into(),
        skills: String::new(),
        history: vec![],
        user_prompt: "Say the word COBOL and nothing else.".into(),
        temperature: 0.0,
        max_tokens: 30,
    };

    let on_chunk = |chunk: &str| {
        print!("{chunk}");
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
    };
    match run_chat_blocking(&call, &on_chunk) {
        Ok(reply) => {
            println!();
            println!("[reply] {}", reply.text);
            println!(
                "[tokens] {} in / {} out",
                reply.input_tokens, reply.output_tokens
            );
        }
        Err(e) => {
            eprintln!("[error] {e}");
            std::process::exit(1);
        }
    }
}
