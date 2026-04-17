//! API module - HTTP client, SSE parsing, and provider support

mod client;
mod providers;
mod sse;

pub use client::ApiClient;
pub use providers::{LlmConfig, LlmProvider, resolve_model_alias, get_model_aliases, is_model_alias};
pub use sse::SseEvent;
