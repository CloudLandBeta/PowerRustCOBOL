// SPDX-License-Identifier: Apache-2.0

pub mod bert_embedder;
pub mod grace;
pub mod knowledge_store;
pub mod project_knowledge;
pub mod rig_transport;
pub mod sandbox;
pub mod specialist;

#[cfg(feature = "local-retrieval")]
pub mod embedding;
#[cfg(feature = "local-retrieval")]
pub mod retrieval;

pub use specialist::{compose_system_prompt, route_specialist, MeshRequest, Specialist};
