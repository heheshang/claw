//! Cost calculation for API calls

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
}

pub struct CostCalculator {
    // Pricing per 1M tokens (as of 2024)
    anthropic_pricing: AnthropicPricing,
    openai_pricing: OpenAIPricing,
}

#[derive(Debug, Clone)]
struct AnthropicPricing {
    // Model -> (input_cost_per_million, output_cost_per_million)
    opus: (f64, f64),
    sonnet: (f64, f64),
    haiku: (f64, f64),
}

#[derive(Debug, Clone)]
struct OpenAIPricing {
    gpt4: (f64, f64),
    gpt35: (f64, f64),
}

impl CostCalculator {
    pub fn new() -> Self {
        Self {
            anthropic_pricing: AnthropicPricing {
                // Claude 3.5 (2024 pricing)
                opus: (15.0, 75.0),      // $15/M input, $75/M output
                sonnet: (3.0, 15.0),     // $3/M input, $15/M output
                haiku: (0.25, 1.25),     // $0.25/M input, $1.25/M output
            },
            openai_pricing: OpenAIPricing {
                gpt4: (30.0, 60.0),       // GPT-4 8K context
                gpt35: (0.5, 1.5),       // GPT-3.5 Turbo
            },
        }
    }

    pub fn calculate(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> CostEstimate {
        match provider.to_lowercase().as_str() {
            "anthropic" => self.calculate_anthropic(model, input_tokens, output_tokens),
            "openai" => self.calculate_openai(model, input_tokens, output_tokens),
            _ => {
                // Default to anthropic pricing
                self.calculate_anthropic(model, input_tokens, output_tokens)
            }
        }
    }

    fn calculate_anthropic(
        &self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> CostEstimate {
        let (input_rate, output_rate) = if model.contains("opus") {
            (self.anthropic_pricing.opus.0, self.anthropic_pricing.opus.1)
        } else if model.contains("haiku") {
            (self.anthropic_pricing.haiku.0, self.anthropic_pricing.haiku.1)
        } else {
            // Default to sonnet
            (self.anthropic_pricing.sonnet.0, self.anthropic_pricing.sonnet.1)
        };

        let input_cost = (input_tokens as f64 / 1_000_000.0) * input_rate;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * output_rate;

        CostEstimate {
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
        }
    }

    fn calculate_openai(
        &self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> CostEstimate {
        let (input_rate, output_rate) = if model.contains("gpt-4") || model.contains("gpt4") {
            (self.openai_pricing.gpt4.0, self.openai_pricing.gpt4.1)
        } else {
            // Default to GPT-3.5
            (self.openai_pricing.gpt35.0, self.openai_pricing.gpt35.1)
        };

        let input_cost = (input_tokens as f64 / 1_000_000.0) * input_rate;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * output_rate;

        CostEstimate {
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
        }
    }

    pub fn get_summary(&self) -> CostSummary {
        CostSummary {
            total_cost: 0.0, // Calculated dynamically from records
        }
    }
}

impl Default for CostCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub total_cost: f64,
}
