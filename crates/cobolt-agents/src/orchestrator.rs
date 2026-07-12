// SPDX-License-Identifier: Apache-2.0

use crate::specialist::Specialist;
use tracing::{info, instrument};

pub struct Orchestrator {
    specialists: Vec<Specialist>,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            specialists: vec![
                Specialist::new("FormsDesigner"),
                Specialist::new("CodeGenerator"),
                Specialist::new("EventBinder"),
            ]
        }
    }

    #[instrument(skip(self, request))]
    pub async fn handle_request(&self, request: &str) -> Result<String, String> {
        info!("Orchestrator received request: {}", request);
        
        let lower_req = request.to_lowercase();
        let target_name = if lower_req.contains("form") || lower_req.contains("ui") || lower_req.contains("screen") {
            "FormsDesigner"
        } else if lower_req.contains("event") || lower_req.contains("click") || lower_req.contains("bind") {
            "EventBinder"
        } else {
            "CodeGenerator"
        };
        
        let specialist = self.specialists.iter().find(|s| s.name == target_name).unwrap();
        
        info!(specialist = %specialist.name, "Routing request to specialist");
        
        // Simulating the agent interaction and retrieval injection
        let response = format!("Mesh Orchestrator delegated task to {}. Vector DB and Lexical Index context injected.", specialist.name);
        
        Ok(response)
    }
}
