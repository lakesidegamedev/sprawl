//! Corpus statistics and the realism score: how far a town's metrics sit from real towns.

use crate::metrics::{Metrics, NAMES, percentile};
use serde_json::{Value, json};

#[derive(Clone, Debug)]
pub struct MetricStats {
    pub p05: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p95: f64,
    pub mean: f64,
    pub std: f64,
}

#[derive(Clone, Debug)]
pub struct CorpusStats {
    pub n_towns: usize,
    pub metrics: Vec<MetricStats>,
}

pub fn stats_of(values: &[f64]) -> MetricStats {
    let v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    let n = v.len().max(1) as f64;
    let mean = v.iter().sum::<f64>() / n;
    let std = (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n).sqrt();
    MetricStats {
        p05: percentile(&v, 0.05),
        p25: percentile(&v, 0.25),
        p50: percentile(&v, 0.5),
        p75: percentile(&v, 0.75),
        p95: percentile(&v, 0.95),
        mean,
        std,
    }
}

impl CorpusStats {
    pub fn from_towns(all: &[Metrics]) -> CorpusStats {
        let metrics = (0..NAMES.len())
            .map(|i| stats_of(&all.iter().map(|m| m.values[i]).collect::<Vec<_>>()))
            .collect();
        CorpusStats {
            n_towns: all.len(),
            metrics,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut m = serde_json::Map::new();
        for (name, s) in NAMES.iter().zip(&self.metrics) {
            m.insert(
                (*name).to_string(),
                json!({"p05": s.p05, "p25": s.p25, "p50": s.p50, "p75": s.p75, "p95": s.p95,
                       "mean": s.mean, "std": s.std}),
            );
        }
        json!({
            "attribution": "Derived from OpenStreetMap data, (c) OpenStreetMap contributors, ODbL. Aggregated statistics only.",
            "n_towns": self.n_towns,
            "metrics": Value::Object(m),
        })
    }

    pub fn from_json(v: &Value) -> Option<CorpusStats> {
        let m = v.get("metrics")?;
        let mut metrics = Vec::new();
        for name in NAMES {
            let s = m.get(name)?;
            let f = |k: &str| s.get(k).and_then(|x| x.as_f64());
            metrics.push(MetricStats {
                p05: f("p05")?,
                p25: f("p25")?,
                p50: f("p50")?,
                p75: f("p75")?,
                p95: f("p95")?,
                mean: f("mean")?,
                std: f("std")?,
            });
        }
        Some(CorpusStats {
            n_towns: v.get("n_towns")?.as_u64()? as usize,
            metrics,
        })
    }
}

/// Smallest spread a metric is judged against, so a metric that is nearly constant in real towns
/// (such as the highway share) does not dominate the score.
fn floor(name: &str) -> f64 {
    if name.starts_with("share_") || name == "giant_component_share" {
        0.02
    } else if name == "orientation_entropy" || name == "circuity" {
        0.03
    } else {
        0.0
    }
}

#[derive(Clone, Debug)]
pub struct MetricScore {
    pub name: &'static str,
    pub value: f64,
    pub median: f64,
    pub z: f64,
    pub inside_p05_p95: bool,
}

#[derive(Clone, Debug)]
pub struct Score {
    /// Mean of min(|z|, 4) over metrics. 0 is a perfectly typical real town; lower is better.
    pub distance: f64,
    pub per_metric: Vec<MetricScore>,
}

pub const Z_CLIP: f64 = 4.0;

pub fn score(m: &Metrics, corpus: &CorpusStats) -> Score {
    let mut per = Vec::new();
    let mut sum = 0.0;
    for (i, name) in NAMES.iter().enumerate() {
        let s = &corpus.metrics[i];
        let v = m.values[i];
        let sigma = ((s.p75 - s.p25) / 1.349)
            .max(0.05 * s.p50.abs())
            .max(floor(name))
            .max(1e-9);
        let z = if v.is_finite() {
            (v - s.p50) / sigma
        } else {
            Z_CLIP
        };
        sum += z.abs().min(Z_CLIP);
        per.push(MetricScore {
            name,
            value: v,
            median: s.p50,
            z,
            inside_p05_p95: v >= s.p05 && v <= s.p95,
        });
    }
    Score {
        distance: sum / NAMES.len() as f64,
        per_metric: per,
    }
}
