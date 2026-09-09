//! Model pricing and cost estimation.
//!
//! Prices are per million tokens (USD), sourced from Anthropic's published
//! pricing page: https://platform.claude.com/docs/en/docs/about-claude/pricing
//!
//! ## Adding support for a new model
//!
//! Append an entry to [`PRICING_TABLE`] with a substring pattern that uniquely
//! identifies the model ID. Table order matters: the first matching pattern
//! wins, so list more specific patterns (e.g. `"opus-4-7"`) before broader
//! ones (e.g. `"opus-4"`). Unknown models emit a one-time warning to stderr
//! and contribute $0 to cost estimates — this makes pricing gaps visible
//! rather than silently mispriced.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    /// 5-minute cache write rate (1.25x input). Claude Code uses 5m TTL
    /// caches by default; 1h cache writes are not distinguishable from the
    /// `cache_creation_input_tokens` field alone.
    pub cache_create_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

// ── Pricing tiers ──
// Grouped by shared rate cards; multiple model patterns can point at the same
// tier. When Anthropic announces a new model at an existing price point, just
// add a pattern entry below without duplicating the numbers.

const OPUS_LATEST: ModelPricing = ModelPricing {
    input_per_mtok: 5.0,
    output_per_mtok: 25.0,
    cache_create_per_mtok: 6.25,
    cache_read_per_mtok: 0.50,
};

const OPUS_LEGACY: ModelPricing = ModelPricing {
    input_per_mtok: 15.0,
    output_per_mtok: 75.0,
    cache_create_per_mtok: 18.75,
    cache_read_per_mtok: 1.50,
};

const SONNET: ModelPricing = ModelPricing {
    input_per_mtok: 3.0,
    output_per_mtok: 15.0,
    cache_create_per_mtok: 3.75,
    cache_read_per_mtok: 0.30,
};

const HAIKU_4_PLUS: ModelPricing = ModelPricing {
    input_per_mtok: 1.0,
    output_per_mtok: 5.0,
    cache_create_per_mtok: 1.25,
    cache_read_per_mtok: 0.10,
};

const HAIKU_3_5: ModelPricing = ModelPricing {
    input_per_mtok: 0.80,
    output_per_mtok: 4.0,
    cache_create_per_mtok: 1.0,
    cache_read_per_mtok: 0.08,
};

const HAIKU_3: ModelPricing = ModelPricing {
    input_per_mtok: 0.25,
    output_per_mtok: 1.25,
    cache_create_per_mtok: 0.30,
    cache_read_per_mtok: 0.03,
};

const SONNET_5: ModelPricing = ModelPricing {
    input_per_mtok: 2.0,
    output_per_mtok: 10.0,
    cache_create_per_mtok: 2.5,
    cache_read_per_mtok: 0.20,
};

/// Fable 5 / Fable 5.1's shared base rate card. Fable 5.1 overrides cache
/// reads to $0.25/MTok (see [`FABLE_5_1`]) instead of the usual 0.1x-input
/// formula; Fable 5 itself still follows that formula.
const FABLE: ModelPricing = ModelPricing {
    input_per_mtok: 10.0,
    output_per_mtok: 50.0,
    cache_create_per_mtok: 12.5,
    cache_read_per_mtok: 1.0,
};

/// Fable 5.1 / Mythos 5.1: same input/output/cache-write rate as [`FABLE`],
/// but cache reads are priced at a flat $0.25/MTok rather than 0.1x input.
const FABLE_5_1: ModelPricing = ModelPricing {
    input_per_mtok: 10.0,
    output_per_mtok: 50.0,
    cache_create_per_mtok: 12.5,
    cache_read_per_mtok: 0.25,
};

/// Lookup table: (substring pattern, pricing tier). Checked in order —
/// first match wins. List more specific patterns (e.g. `"opus-4-7"`) before
/// broader ones (e.g. `"opus-4"`) so newer models don't fall through to a
/// legacy tier.
static PRICING_TABLE: &[(&str, ModelPricing)] = &[
    // Opus 4.5+ (current pricing tier, 3x cheaper than 4.0/4.1)
    ("opus-4-8", OPUS_LATEST),
    ("opus-4-7", OPUS_LATEST),
    ("opus-4-6", OPUS_LATEST),
    ("opus-4-5", OPUS_LATEST),
    ("opus-5", OPUS_LATEST),
    // Opus legacy (4.0, 4.1, 3.0 — all share the $15/$75 rate card)
    ("opus-4-1", OPUS_LEGACY),
    ("opus-4", OPUS_LEGACY),
    ("opus-3", OPUS_LEGACY),
    // Fable 5.1 / Mythos 5.1 — check before the broader "fable-5" pattern
    ("fable-5-1", FABLE_5_1),
    ("mythos-5-1", FABLE_5_1),
    ("fable-5", FABLE),
    // Sonnet 5 (new cheaper tier vs 4.6 and earlier)
    ("sonnet-5", SONNET_5),
    // Sonnet — all other versions (3.5/3.7/4/4.5/4.6) share the same rate card
    ("sonnet", SONNET),
    // Haiku — list newer (more expensive) tiers first
    ("haiku-4", HAIKU_4_PLUS),
    ("haiku-3-5", HAIKU_3_5),
    ("3-5-haiku", HAIKU_3_5),
    ("haiku", HAIKU_3),
];

pub fn pricing_for_model(model: &str) -> Option<ModelPricing> {
    PRICING_TABLE
        .iter()
        .find(|(pattern, _)| model.contains(pattern))
        .map(|(_, pricing)| *pricing)
}

pub fn estimate_cost(
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    cache_create_tokens: u64,
    cache_read_tokens: u64,
) -> f64 {
    let Some(p) = pricing_for_model(model) else {
        warn_unknown_model(model);
        return 0.0;
    };
    (input_tokens as f64 * p.input_per_mtok
        + output_tokens as f64 * p.output_per_mtok
        + cache_create_tokens as f64 * p.cache_create_per_mtok
        + cache_read_tokens as f64 * p.cache_read_per_mtok)
        / 1_000_000.0
}

fn warn_unknown_model(model: &str) {
    static WARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let warned = WARNED.get_or_init(|| Mutex::new(HashSet::new()));
    let mut guard = warned.lock().unwrap();
    if guard.insert(model.to_string()) {
        eprintln!(
            "warning: no pricing configured for model '{}' — cost will be reported as $0. \
             Add it to PRICING_TABLE in src/aggregator/cost.rs.",
            model
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_47_uses_latest_tier() {
        let p = pricing_for_model("claude-opus-4-7").unwrap();
        assert_eq!(p.input_per_mtok, 5.0);
        assert_eq!(p.cache_read_per_mtok, 0.50);
    }

    #[test]
    fn opus_47_with_suffix_matches() {
        let p = pricing_for_model("us.anthropic.claude-opus-4-7[1m]").unwrap();
        assert_eq!(p.input_per_mtok, 5.0);
    }

    #[test]
    fn opus_46_uses_latest_tier() {
        let p = pricing_for_model("claude-opus-4-6").unwrap();
        assert_eq!(p.input_per_mtok, 5.0);
    }

    #[test]
    fn opus_41_uses_legacy_tier() {
        let p = pricing_for_model("claude-opus-4-1-20250805").unwrap();
        assert_eq!(p.input_per_mtok, 15.0);
        assert_eq!(p.cache_read_per_mtok, 1.50);
    }

    #[test]
    fn opus_40_uses_legacy_tier() {
        let p = pricing_for_model("claude-opus-4-20250514").unwrap();
        assert_eq!(p.input_per_mtok, 15.0);
    }

    #[test]
    fn sonnet_versions_share_pricing() {
        let p = pricing_for_model("claude-sonnet-4-6").unwrap();
        assert_eq!(p.input_per_mtok, 3.0);
        let p = pricing_for_model("claude-3-5-sonnet-20241022").unwrap();
        assert_eq!(p.input_per_mtok, 3.0);
    }

    #[test]
    fn haiku_45_uses_new_tier() {
        let p = pricing_for_model("claude-haiku-4-5-20251001").unwrap();
        assert_eq!(p.input_per_mtok, 1.0);
    }

    #[test]
    fn haiku_35_uses_legacy_tier() {
        let p = pricing_for_model("claude-3-5-haiku-20241022").unwrap();
        assert_eq!(p.input_per_mtok, 0.80);
    }

    #[test]
    fn opus_5_uses_latest_tier() {
        let p = pricing_for_model("claude-opus-5").unwrap();
        assert_eq!(p.input_per_mtok, 5.0);
        assert_eq!(p.output_per_mtok, 25.0);
    }

    #[test]
    fn opus_48_uses_latest_tier_not_legacy() {
        let p = pricing_for_model("claude-opus-4-8").unwrap();
        assert_eq!(p.input_per_mtok, 5.0);
        assert_eq!(p.output_per_mtok, 25.0);
    }

    #[test]
    fn sonnet_5_uses_new_cheaper_tier() {
        let p = pricing_for_model("claude-sonnet-5").unwrap();
        assert_eq!(p.input_per_mtok, 2.0);
        assert_eq!(p.output_per_mtok, 10.0);
        // Sonnet 4.6 and earlier stay on the older $3/$15 rate card.
        let legacy = pricing_for_model("claude-sonnet-4-6").unwrap();
        assert_eq!(legacy.input_per_mtok, 3.0);
    }

    #[test]
    fn fable_5_1_has_flat_cache_read_rate() {
        let p = pricing_for_model("claude-fable-5-1").unwrap();
        assert_eq!(p.input_per_mtok, 10.0);
        assert_eq!(p.output_per_mtok, 50.0);
        assert_eq!(p.cache_read_per_mtok, 0.25);
    }

    #[test]
    fn mythos_5_1_matches_fable_5_1_pricing() {
        let p = pricing_for_model("claude-mythos-5-1").unwrap();
        assert_eq!(p.cache_read_per_mtok, 0.25);
    }

    #[test]
    fn fable_5_uses_standard_cache_read_formula() {
        let p = pricing_for_model("claude-fable-5").unwrap();
        assert_eq!(p.input_per_mtok, 10.0);
        assert_eq!(p.cache_read_per_mtok, 1.0);
    }

    #[test]
    fn haiku_45_still_matches_current_tier() {
        let p = pricing_for_model("claude-haiku-4-5").unwrap();
        assert_eq!(p.input_per_mtok, 1.0);
    }

    #[test]
    fn unknown_model_returns_none() {
        assert!(pricing_for_model("gpt-5").is_none());
        assert!(pricing_for_model("claude-opus-99").is_none());
    }

    #[test]
    fn estimate_cost_for_opus_47() {
        // Reproduce the reported session:
        // 85K input + 317K output + 1.4M cache_create + 17.4M cache_read
        // under the corrected Opus 4.7 pricing.
        let cost = estimate_cost("claude-opus-4-7", 85_000, 317_000, 1_400_000, 17_400_000);
        let expected =
            (85_000.0 * 5.0 + 317_000.0 * 25.0 + 1_400_000.0 * 6.25 + 17_400_000.0 * 0.50)
                / 1_000_000.0;
        assert!((cost - expected).abs() < 1e-6);
    }

    #[test]
    fn estimate_cost_unknown_model_is_zero() {
        assert_eq!(estimate_cost("unknown-model-xyz", 1000, 1000, 0, 0), 0.0);
    }
}
