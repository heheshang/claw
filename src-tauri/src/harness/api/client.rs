//! HTTP client for LLM API calls

use super::providers::LlmConfig;
use super::sse::{SseEvent, SseParserContext};
use futures::StreamExt;

#[derive(Clone)]
pub struct ApiClient {
    config: LlmConfig,
    http_client: reqwest::Client,
}

impl ApiClient {
    pub fn new(config: LlmConfig) -> Self {
        // Configure client with timeout and TLS settings
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .danger_accept_invalid_certs(false)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            config,
            http_client,
        }
    }

    /// Check if error is retryable (5xx errors, rate limit, network issues, overloaded)
    fn is_retryable_error(error: &str) -> bool {
        error.contains("500") ||
        error.contains("502") ||
        error.contains("503") ||
        error.contains("504") ||
        error.contains("529") ||
        error.contains("rate limit") ||
        error.contains("429") ||
        error.contains("overloaded") ||
        error.contains("timeout") ||
        error.contains("connection refused") ||
        error.contains("error sending request")
    }

    pub async fn stream_chat(
        &self,
        messages: Vec<crate::harness::runtime::ApiMessage>,
        tools: Vec<serde_json::Value>,
        model: &str,
    ) -> Result<Vec<SseEvent>, String> {
        let api_key = self.config.api_key.clone()
            .ok_or_else(|| "API key not configured".to_string())?;

        let base_url = self.config.base_url();
        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

        let retry_count = self.config.retry_count.unwrap_or(3);

        log::info!("[API] Request: url={}, model={}, api_key_first_10={}, retry_count={}",
            url, model, &api_key[..10.min(api_key.len())], retry_count);

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": self.config.max_tokens,
            "messages": messages,
            "stream": true,
            "tools": tools,
        });

        if let Some(temp) = self.config.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        log::info!("[API] Sending request to {}", url);

        let mut last_error = String::new();
        let max_attempts = retry_count + 1; // retry_count is number of retries, so max_attempts = retries + 1

        for attempt in 0..max_attempts {
            if attempt > 0 {
                let delay = std::time::Duration::from_secs(u64::from(attempt) * 2);
                log::info!("[API] Retry attempt {}/{} after {}s", attempt, max_attempts, delay.as_secs());
                tokio::time::sleep(delay).await;
            }

            match self.send_request(&url, &api_key, &body).await {
                Ok(events) => return Ok(events),
                Err(e) => {
                    last_error = e.clone();
                    log::warn!("[API] Attempt {}/{} failed: {}", attempt + 1, max_attempts, e);

                    if attempt < max_attempts - 1 && Self::is_retryable_error(&e) {
                        continue;
                    }
                    break;
                }
            }
        }

        log::error!("[API] All {} attempts exhausted. Last error: {}", max_attempts, last_error);
        Err(format!("API request failed after {} attempts: {}", max_attempts, last_error))
    }

    /// Stream chat with real-time callback for each SSE event
    /// This allows immediate emission of events instead of waiting for all events
    pub async fn stream_chat_streaming<F, Fut>(
        &self,
        messages: Vec<crate::harness::runtime::ApiMessage>,
        tools: Vec<serde_json::Value>,
        model: &str,
        callback: F,
    ) -> Result<(), String>
    where
        F: Fn(SseEvent) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let api_key = self.config.api_key.clone()
            .ok_or_else(|| "API key not configured".to_string())?;

        let base_url = self.config.base_url();
        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

        let retry_count = self.config.retry_count.unwrap_or(3);

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": self.config.max_tokens,
            "messages": messages,
            "stream": true,
            "tools": tools,
        });

        if let Some(temp) = self.config.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        log::info!("[API] Streaming request to {}", url);

        let mut last_error = String::new();
        let max_attempts = retry_count + 1;

        for attempt in 0..max_attempts {
            if attempt > 0 {
                let delay = std::time::Duration::from_secs(u64::from(attempt) * 2);
                log::info!("[API] Retry attempt {}/{} after {}s", attempt, max_attempts, delay.as_secs());
                tokio::time::sleep(delay).await;
            }

            match self.send_request_streaming(&url, &api_key, &body, &callback).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = e.clone();
                    log::warn!("[API] Streaming attempt {}/{} failed: {}", attempt + 1, max_attempts, e);

                    if attempt < max_attempts - 1 && Self::is_retryable_error(&e) {
                        continue;
                    }
                    break;
                }
            }
        }

        log::error!("[API] All {} streaming attempts exhausted. Last error: {}", max_attempts, last_error);
        Err(format!("API request failed after {} attempts: {}", max_attempts, last_error))
    }

    async fn send_request_streaming<F, Fut>(
        &self,
        url: &str,
        api_key: &str,
        body: &serde_json::Value,
        callback: &F,
    ) -> Result<(), String>
    where
        F: Fn(SseEvent) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let response = self.http_client
            .post(url)
            .header("content-type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .header("x-api-key", api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status, text));
        }

        let mut ctx = SseParserContext::default();
        let mut stop_received = false;
        let mut buffer = String::new();

        // Use streaming to process SSE events as they arrive
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            match item {
                Ok(bytes) => {
                    let chunk = String::from_utf8_lossy(&bytes).to_string();
                    buffer.push_str(&chunk);

                    // Process complete lines from buffer
                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].to_string();
                        buffer = buffer[pos + 1..].to_string();

                        if stop_received {
                            break;
                        }

                        let line_events = super::sse::parse_sse_line(&line, &mut ctx);
                        for event in line_events {
                            // Call the callback immediately for each event
                            callback(event.clone()).await;

                            match &event {
                                SseEvent::MessageStop => {
                                    stop_received = true;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("[API] Stream error: {}", e);
                    break;
                }
            }

            if stop_received {
                break;
            }
        }

        log::info!("[API] Streaming complete");
        Ok(())
    }

    async fn send_request(&self, url: &str, api_key: &str, body: &serde_json::Value) -> Result<Vec<SseEvent>, String> {
        let response = self.http_client
            .post(url)
            .header("content-type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .header("x-api-key", api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| {
                log::error!("[API] Request failed: {} | url: {}", e, url);
                if e.to_string().contains("error sending request") {
                    if std::env::var("HTTP_PROXY").is_ok() || std::env::var("HTTPS_PROXY").is_ok() {
                        log::error!("[API] Proxy might be configured but not used by reqwest");
                    }
                    format!("Request failed: {} (check network/proxy settings)", e)
                } else {
                    format!("Request failed: {}", e)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            log::error!("[API] Response error: status={}, body={}", status, text);
            return Err(format!("API error {}: {}", status, text));
        }

        let mut events = Vec::new();
        let mut ctx = SseParserContext::default();
        let mut stop_received = false;
        let mut buffer = String::new();

        // Use streaming to process SSE events as they arrive
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            match item {
                Ok(bytes) => {
                    // Process each chunk as it arrives
                    let chunk = String::from_utf8_lossy(&bytes).to_string();
                    buffer.push_str(&chunk);

                    // Process complete lines from buffer
                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].to_string();
                        buffer = buffer[pos + 1..].to_string();

                        if stop_received {
                            break;
                        }

                        let line_events = super::sse::parse_sse_line(&line, &mut ctx);
                        for event in line_events {
                            match &event {
                                SseEvent::MessageStop => {
                                    events.push(event.clone());
                                    stop_received = true;
                                    break;
                                }
                                _ => {
                                    events.push(event.clone());
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("[API] Stream error: {}", e);
                    break;
                }
            }

            // If we received MessageStop, we're done
            if stop_received {
                break;
            }
        }

        log::info!("[API] Stream complete, {} events", events.len());
        Ok(events)
    }
}
