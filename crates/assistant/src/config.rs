use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    #[default]
    Anthropic,
    OpenRouter,
    /// Any server that speaks OpenAI's chat completions, such as DeepSeek, LM Studio or Ollama.
    Custom,
}

impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic",
            Provider::OpenRouter => "OpenRouter",
            Provider::Custom => "The model's server",
        }
    }
}

/// A Claude model, for the Anthropic provider.
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

    pub fn label(self) -> &'static str {
        match self {
            Model::Sonnet => "Sonnet 5.5",
            Model::Opus => "Opus 5.5",
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub provider: Provider,
    pub model: Model,
    /// Sent to Anthropic and OpenRouter. A custom server gets none, many reject the field.
    pub effort: Effort,
    /// An OpenRouter model id such as "vendor/model".
    pub router_model: String,
    /// The custom server's base URL, the part before /chat/completions.
    pub custom_url: String,
    pub custom_model: String,
}

impl Config {
    /// The model as the player picked it, to tell answers from different models apart.
    pub fn model_label(&self) -> String {
        match self.provider {
            Provider::Anthropic => self.model.label().to_string(),
            Provider::OpenRouter => self.router_model.trim().to_string(),
            Provider::Custom => self.custom_model.trim().to_string(),
        }
    }

    /// What the player still has to fill in, apart from the key.
    pub fn missing(&self) -> Option<&'static str> {
        match self.provider {
            Provider::OpenRouter if self.router_model.trim().is_empty() => {
                Some("Pick an OpenRouter model in the settings.")
            }
            Provider::Custom
                if self.custom_url.trim().is_empty() || self.custom_model.trim().is_empty() =>
            {
                Some("Set the server's address and model in the settings.")
            }
            _ => None,
        }
    }
}

/// Token counts as the API reports them. Input excludes cached tokens.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    /// Dollars, from list prices or as the provider reported them. None when nobody said.
    pub cost: Option<f64>,
}

impl Usage {
    pub fn add(&mut self, other: Usage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
        self.cost = match (self.cost, other.cost) {
            (Some(a), Some(b)) => Some(a + b),
            (a, b) => a.or(b),
        };
    }

    /// At the model's list prices. A refusal served by a fallback model bills at that model's.
    pub fn priced(mut self, model: Model) -> Usage {
        let [input, output, read, write] = model.prices();
        self.cost = Some(
            (self.input_tokens as f64 * input
                + self.output_tokens as f64 * output
                + self.cache_read_tokens as f64 * read
                + self.cache_write_tokens as f64 * write)
                / 1_000_000.0,
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_from_before_providers_still_load() {
        let c: Config =
            serde_json::from_str(r#"{"model":"claude-opus-5-5","effort":"low"}"#).unwrap();
        assert_eq!(c.provider, Provider::Anthropic);
        assert_eq!(c.model, Model::Opus);
        assert_eq!(c.effort, Effort::Low);
        assert_eq!(
            serde_json::to_value(Model::Sonnet).unwrap(),
            "claude-sonnet-5-5"
        );
        assert_eq!(
            serde_json::to_value(Provider::OpenRouter).unwrap(),
            "openRouter"
        );
    }

    #[test]
    fn says_what_is_missing() {
        let mut c = Config {
            provider: Provider::OpenRouter,
            ..Default::default()
        };
        assert!(c.missing().unwrap().contains("OpenRouter model"));
        c.router_model = " vendor/model ".into();
        assert_eq!(c.missing(), None);
        assert_eq!(c.model_label(), "vendor/model");
        c.provider = Provider::Custom;
        c.custom_url = "http://localhost:11434/v1".into();
        assert!(c.missing().is_some());
    }

    #[test]
    fn cost_counts_cache_reads_at_their_own_price() {
        let u = Usage {
            input_tokens: 1_000,
            output_tokens: 1_000,
            cache_read_tokens: 100_000,
            ..Default::default()
        }
        .priced(Model::Sonnet);
        assert!((u.cost.unwrap() - (0.002 + 0.01 + 0.02)).abs() < 1e-9);
    }

    #[test]
    fn unknown_cost_stays_unknown() {
        let mut u = Usage::default();
        u.add(Usage::default());
        assert_eq!(u.cost, None);
        u.add(Usage {
            cost: Some(0.01),
            ..Default::default()
        });
        assert_eq!(u.cost, Some(0.01));
    }
}
