use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Model {
    #[default]
    #[serde(rename = "claude-sonnet-5-5")]
    Sonnet,
    #[serde(rename = "claude-opus-5-5")]
    Opus,
}

impl Model {
    pub fn id(self) -> &'static str {
        match self {
            Model::Sonnet => "claude-sonnet-5-5",
            Model::Opus => "claude-opus-5-5",
        }
    }

    /// Dollars per million tokens of input, output, cache reads and 5-minute cache writes.
    fn prices(self) -> [f64; 4] {
        match self {
            Model::Sonnet => [2.0, 10.0, 0.2, 2.5],
            Model::Opus => [4.0, 20.0, 0.2, 5.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    // Anthropic's starting point for multistep tool use on Sonnet 5.5. Opus 5.5 defaults to it.
    #[default]
    Medium,
    High,
}

impl Effort {
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Config {
    pub model: Model,
    pub effort: Effort,
}

/// Token counts as the API reports them. Input excludes cached tokens.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

impl Usage {
    pub fn add(&mut self, other: Usage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
    }

    /// At the model's list prices. A refusal served by a fallback model bills at that model's.
    pub fn dollars(&self, model: Model) -> f64 {
        let [input, output, read, write] = model.prices();
        (self.input_tokens as f64 * input
            + self.output_tokens as f64 * output
            + self.cache_read_tokens as f64 * read
            + self.cache_write_tokens as f64 * write)
            / 1_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_values_are_the_model_ids() {
        let c: Config =
            serde_json::from_str(r#"{"model":"claude-opus-5-5","effort":"low"}"#).unwrap();
        assert_eq!(c.model, Model::Opus);
        assert_eq!(c.effort, Effort::Low);
        assert_eq!(
            serde_json::to_value(Model::Sonnet).unwrap(),
            "claude-sonnet-5-5"
        );
    }

    #[test]
    fn cost_counts_cache_reads_at_their_own_price() {
        let u = Usage {
            input_tokens: 1_000,
            output_tokens: 1_000,
            cache_read_tokens: 100_000,
            cache_write_tokens: 0,
        };
        assert!((u.dollars(Model::Sonnet) - (0.002 + 0.01 + 0.02)).abs() < 1e-9);
    }
}
