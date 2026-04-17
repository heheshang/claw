//! Background heartbeat task - inspired by zeroclaw's heartbeat module
//!
//! Provides periodic health checks and optional LLM-driven task execution.

use crate::harness::daemon::{spawn_periodic_task, DaemonState};
use crate::harness::{build_system_prompt, get_tool_definitions, ApiMessage, ContentBlock, MessageRole};
use anyhow::Result;
use log::info;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Heartbeat configuration
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Enable heartbeat task
    pub enabled: bool,
    /// Interval in seconds between heartbeat ticks
    pub interval_secs: u64,
    /// Optional message to send on each heartbeat
    pub message: Option<String>,
    /// Enable LLM-driven decision making (two-phase mode)
    pub two_phase: bool,
    /// Default temperature for LLM calls
    pub default_temperature: f64,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: 300, // 5 minutes
            message: Some("heartbeat".to_string()),
            two_phase: false,
            default_temperature: 0.7,
        }
    }
}

/// Heartbeat metrics
#[derive(Debug, Default)]
pub struct HeartbeatMetrics {
    pub consecutive_failures: u64,
    pub last_tick_at: Option<chrono::DateTime<chrono::Utc>>,
    pub tick_count: u64,
    pub total_success_time_ms: u64,
    pub total_failure_time_ms: u64,
}

/// Heartbeat engine
pub struct HeartbeatEngine {
    config: HeartbeatConfig,
    metrics: Arc<Mutex<HeartbeatMetrics>>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: String,
}

impl HeartbeatEngine {
    pub fn new(
        config: HeartbeatConfig,
        api_key: Option<String>,
        base_url: Option<String>,
        model: String,
    ) -> Self {
        Self {
            config,
            metrics: Arc::new(Mutex::new(HeartbeatMetrics::default())),
            api_key,
            base_url,
            model,
        }
    }

    /// Record a successful heartbeat tick
    pub async fn record_success(&self, elapsed_ms: f64) {
        let mut m = self.metrics.lock().await;
        m.consecutive_failures = 0;
        m.last_tick_at = Some(chrono::Utc::now());
        m.tick_count += 1;
        m.total_success_time_ms += elapsed_ms as u64;
    }

    /// Record a failed heartbeat tick
    pub async fn record_failure(&self, elapsed_ms: f64) {
        let mut m = self.metrics.lock().await;
        m.consecutive_failures += 1;
        m.last_tick_at = Some(chrono::Utc::now());
        m.total_failure_time_ms += elapsed_ms as u64;
    }

    /// Get current metrics snapshot
    pub async fn metrics(&self) -> HeartbeatMetrics {
        self.metrics.lock().await.clone()
    }

    /// Run a single heartbeat tick
    pub async fn tick(&self) -> Result<String, String> {
        let start = Instant::now();
        let tick_duration_ms = start.elapsed().as_millis() as f64;

        // Prepare the heartbeat message
        let message = self
            .config
            .message
            .clone()
            .unwrap_or_else(|| "heartbeat tick".to_string());

        info!("[HEARTBEAT] Ticking: {}", message);

        // Build API request
        let api_key = self.api_key.clone().ok_or_else(|| "API key not configured".to_string())?;
        let base_url = self
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com".to_string());

        let system_prompt = build_system_prompt();
        let tools = get_tool_definitions();

        let messages = vec![ApiMessage {
            role: MessageRole::User,
            content: message.clone(),
        }];

        let client = reqwest::Client::new();
        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "temperature": self.config.default_temperature,
            "system": system_prompt,
            "tools": tools,
            "messages": messages
        });

        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .header("x-api-key", &api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let elapsed = start.elapsed().as_millis() as f64;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            self.record_failure(elapsed).await;
            return Err(format!("API error {}: {}", status, text));
        }

        #[derive(Deserialize)]
        struct ApiResponse {
            content: Vec<ContentBlock>,
        }

        let api_response: ApiResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let text = api_response
            .content
            .iter()
            .find(|c| c.block_type == "text")
            .and_then(|c| c.text.clone())
            .unwrap_or_default();

        self.record_success(tick_duration_ms).await;

        info!("[HEARTBEAT] Tick completed: {} chars", text.len());
        Ok(text)
    }
}

/// Run the heartbeat worker loop
pub async fn run_heartbeat_worker(
    config: HeartbeatConfig,
    api_key: Option<String>,
    base_url: Option<String>,
    model: String,
    daemon_state: Arc<DaemonState>,
) -> Result<(), String> {
    if !config.enabled {
        info!("[HEARTBEAT] Heartbeat disabled, skipping");
        return Ok(());
    }

    let engine = HeartbeatEngine::new(config.clone(), api_key, base_url, model);

    let engine_clone = Arc::new(engine);
    let daemon_clone = daemon_state.clone();

    // Spawn the periodic task
    let handle = spawn_periodic_task(
        "heartbeat",
        daemon_state,
        config.interval_secs.max(60), // Min 60 seconds
        move || {
            let eng = engine_clone.clone();
            async move {
                eng.tick().await
            }
        },
    );

    // Wait for shutdown
    daemon_clone.mark_ok("heartbeat").await;

    // Keep running until shutdown is requested
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        if daemon_clone.shutdown_requested.load(Ordering::SeqCst) > 0 {
            info!("[HEARTBEAT] Shutting down heartbeat worker");
            break;
        }
    }

    handle.abort();

    Ok(())
}

use chrono::Utc;
use serde::Deserialize;
use tokio::sync::atomic::{AtomicU64, Ordering};
