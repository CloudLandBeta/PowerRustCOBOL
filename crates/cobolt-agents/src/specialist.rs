// SPDX-License-Identifier: Apache-2.0

pub struct Specialist {
    pub name: String,
    pub system_prompt: String,
}

impl Specialist {
    pub fn new(name: impl Into<String>) -> Self {
        let name_str = name.into();
        let prompt = match name_str.as_str() {
            "FormsDesigner" => "You are an expert pair programmer specializing in PowerRustCOBOL UI Forms design.",
            "EventBinder" => "You are an expert pair programmer specializing in PowerRustCOBOL Event bindings.",
            _ => "You are an expert pair programmer for PowerRustCOBOL.",
        }.to_string();
        
        Self { 
            name: name_str,
            system_prompt: prompt,
        }
    }
}
