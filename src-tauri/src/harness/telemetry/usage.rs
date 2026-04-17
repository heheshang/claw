//! Usage tracking for API calls

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCallRecord {
    pub timestamp: DateTime<Utc>,
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
    pub duration_ms: u64,
}

pub struct UsageTracker {
    session_path: PathBuf,
    in_memory_calls: Vec<ApiCallRecord>,
}

impl UsageTracker {
    pub fn new(session_path: PathBuf) -> Self {
        // Ensure directory exists
        if let Some(parent) = session_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        Self {
            session_path,
            in_memory_calls: Vec::new(),
        }
    }

    pub fn add_record(&mut self, record: &ApiCallRecord) -> Result<(), String> {
        // Append to file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.session_path)
            .map_err(|e| format!("File error: {}", e))?;

        let json = serde_json::to_string(record)
            .map_err(|e| format!("Serialize error: {}", e))?;
        writeln!(file, "{}", json)
            .map_err(|e| format!("Write error: {}", e))?;

        self.in_memory_calls.push(record.clone());
        Ok(())
    }

    pub fn get_stats(&self) -> Result<UsageStats, String> {
        let records = self.load_records()?;

        let total_calls = records.len() as u64;
        let total_input_tokens: u64 = records.iter().map(|r| r.input_tokens).sum();
        let total_output_tokens: u64 = records.iter().map(|r| r.output_tokens).sum();
        let total_tokens = total_input_tokens + total_output_tokens;
        let total_cost: f64 = records.iter().map(|r| r.cost_usd).sum();

        let first_call = records.first().map(|r| r.timestamp);
        let last_call = records.last().map(|r| r.timestamp);

        Ok(UsageStats {
            total_calls,
            total_input_tokens,
            total_output_tokens,
            total_tokens,
            total_cost_usd: total_cost,
            first_call,
            last_call,
        })
    }

    fn load_records(&self) -> Result<Vec<ApiCallRecord>, String> {
        if !self.session_path.exists() {
            return Ok(Vec::new());
        }

        let file = OpenOptions::new()
            .read(true)
            .open(&self.session_path)
            .map_err(|e| format!("File error: {}", e))?;

        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Read error: {}", e))?;
            if !line.trim().is_empty() {
                let record = serde_json::from_str(&line)
                    .map_err(|e| format!("Parse error: {}", e))?;
                records.push(record);
            }
        }

        Ok(records)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    pub total_calls: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub first_call: Option<DateTime<Utc>>,
    pub last_call: Option<DateTime<Utc>>,
}
