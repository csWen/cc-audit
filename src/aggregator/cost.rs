/// Pricing per million tokens (USD), based on Anthropic's published pricing.
///
/// See: https://docs.anthropic.com/en/docs/about-claude/models
pub struct ModelPricing {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_create_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

pub fn pricing_for_model(model: &str) -> ModelPricing {
    if model.contains("opus-4-6") {
        // Opus 4.6 pricing (3x cheaper than Opus 4.0/4.1)
        ModelPricing {
            input_per_mtok: 5.0,
            output_per_mtok: 25.0,
            cache_create_per_mtok: 6.25, // 1.25x input
            cache_read_per_mtok: 0.50,   // 0.1x input
        }
    } else if model.contains("opus") {
        // Opus 4.0 / 4.1 pricing
        ModelPricing {
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
            cache_create_per_mtok: 18.75, // 1.25x input
            cache_read_per_mtok: 1.5,     // 0.1x input
        }
    } else if model.contains("haiku") {
        ModelPricing {
            input_per_mtok: 0.80,
            output_per_mtok: 4.0,
            cache_create_per_mtok: 1.0,
            cache_read_per_mtok: 0.08,
        }
    } else {
        // Default to Sonnet pricing
        ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
            cache_create_per_mtok: 3.75,
            cache_read_per_mtok: 0.30,
        }
    }
}

pub fn estimate_cost(
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    cache_create_tokens: u64,
    cache_read_tokens: u64,
) -> f64 {
    let p = pricing_for_model(model);
    (input_tokens as f64 * p.input_per_mtok
        + output_tokens as f64 * p.output_per_mtok
        + cache_create_tokens as f64 * p.cache_create_per_mtok
        + cache_read_tokens as f64 * p.cache_read_per_mtok)
        / 1_000_000.0
}
