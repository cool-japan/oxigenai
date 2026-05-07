use legalis_core::{LegalEntity, Statute};
use legalis_sim::{
    DemographicProfile, Distribution, PopulationGenerator, SimEngine, SimulationMetrics,
    StatuteMetrics,
};
use serde::Serialize;
use std::collections::HashMap;
use thiserror::Error;
use tracing::debug;

/// Error from the simulator service.
#[derive(Debug, Error)]
pub enum SimulatorError {
    #[error("No statutes provided for simulation")]
    NoStatutes,
    #[error("Population is empty")]
    EmptyPopulation,
}

/// Japanese demographic profile based on 2024 census data.
///
/// - Age: Normal(48.4, 18.0) — median age ~48 in Japan's aging society
/// - Income: LogNormal(15.32, 0.65) — median income ~4.5M JPY (ln(4_500_000) ≈ 15.32)
/// - Employment type: 63% regular / 22% part-time / 15% fixed-term (厚労省 令和5年調査)
#[must_use]
pub fn jp_2024_profile() -> DemographicProfile {
    DemographicProfile {
        age_distribution: Distribution::Normal {
            mean: 48.4,
            std_dev: 18.0,
        },
        income_distribution: Distribution::LogNormal {
            mean: 15.32,
            std_dev: 0.65,
        },
        regions: vec![],
        custom_attributes: HashMap::new(),
    }
    .with_attribute(
        "employment_type_code",
        Distribution::Discrete {
            // (sampled value, probability)
            values: vec![
                (0.0, 0.63), // 0 → "regular" (正規雇用)
                (1.0, 0.22), // 1 → "part_time" (パート・アルバイト)
                (2.0, 0.15), // 2 → "fixed_term" (有期雇用)
            ],
        },
    )
}

/// Configuration for a simulation run.
pub struct SimulationConfig {
    /// Number of simulated agents (default: 1000)
    pub population_size: usize,
    /// Demographic profile for population generation
    pub profile: DemographicProfile,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            population_size: 1000,
            profile: jp_2024_profile(),
        }
    }
}

/// Per-statute metrics enriched with the statute title.
#[derive(Debug, Clone, Serialize)]
pub struct StatuteDetail {
    pub id: String,
    pub title: String,
    pub total: usize,
    pub deterministic: usize,
    pub discretion: usize,
    pub void: usize,
    /// Fraction of agents where the statute applied deterministically (0.0–1.0).
    pub effectiveness: f64,
    /// Fraction of agents requiring judicial discretion (0.0–1.0).
    pub ambiguity: f64,
}

impl StatuteDetail {
    fn from_metrics(id: String, title: String, m: &StatuteMetrics) -> Self {
        Self {
            id,
            title,
            total: m.total,
            deterministic: m.deterministic,
            discretion: m.discretion,
            void: m.void,
            effectiveness: m.effectiveness(),
            ambiguity: m.ambiguity(),
        }
    }
}

/// Enriched simulation result with per-statute breakdown and Markdown summary.
pub struct SimulationResult {
    pub metrics: SimulationMetrics,
    pub population_size: usize,
    pub statute_count: usize,
    pub statute_details: Vec<StatuteDetail>,
    pub markdown_summary: String,
}

/// Service for running population-level policy simulations via legalis-sim.
pub struct SimulatorService;

impl SimulatorService {
    /// Run a simulation: generate a JP population, apply all statutes, collect metrics.
    ///
    /// # Steps
    /// 1. Generate population via `PopulationGenerator` with `jp_2024_profile()`
    /// 2. Post-process entities: convert numeric `employment_type_code` → string label,
    ///    derive `duration_months` for fixed-term workers
    /// 3. Run `SimEngine::run_simulation().await`
    /// 4. Build per-statute details and Markdown summary
    pub async fn run(
        statutes: Vec<Statute>,
        config: &SimulationConfig,
    ) -> Result<SimulationResult, SimulatorError> {
        if statutes.is_empty() {
            return Err(SimulatorError::NoStatutes);
        }
        if config.population_size == 0 {
            return Err(SimulatorError::EmptyPopulation);
        }

        debug!(
            "SimulatorService::run — {} statutes, {} agents",
            statutes.len(),
            config.population_size
        );

        // Generate population
        let generator = PopulationGenerator::new(config.profile.clone(), config.population_size);
        let mut raw_entities = generator.generate();

        if raw_entities.is_empty() {
            return Err(SimulatorError::EmptyPopulation);
        }

        // Post-process: resolve employment_type_code → string label + set duration_months
        for entity in raw_entities.iter_mut() {
            let code_str = entity
                .get_attribute("employment_type_code")
                .unwrap_or_else(|| "0".to_string());
            let code = code_str.parse::<f64>().unwrap_or(0.0).round() as u32;

            let et_label = match code {
                1 => "part_time",
                2 => "fixed_term",
                _ => "regular",
            };
            entity.set_attribute("employment_type", et_label.to_string());

            // Derive years_employed and duration_months from age for fixed-term workers
            // Assumption: fixed-term workers started employment at ~22
            if et_label == "fixed_term" {
                let age_str = entity
                    .get_attribute("age")
                    .unwrap_or_else(|| "30".to_string());
                let age = age_str.parse::<f64>().unwrap_or(30.0).max(22.0);
                let years = (age - 22.0) as u32;
                let months = years * 12;
                entity.set_attribute("years_employed", years.to_string());
                entity.set_attribute("duration_months", months.to_string());
            }
        }

        // Box entities for SimEngine
        let population: Vec<Box<dyn LegalEntity>> = raw_entities
            .into_iter()
            .map(|e| Box::new(e) as Box<dyn LegalEntity>)
            .collect();

        debug!("Running SimEngine with {} agents", population.len());
        let engine = SimEngine::new(statutes.clone(), population);
        let metrics = engine.run_simulation().await;

        // Build statute details
        let statute_details: Vec<StatuteDetail> = statutes
            .iter()
            .filter_map(|s| {
                let m = metrics.statute_metrics.get(&s.id)?;
                Some(StatuteDetail::from_metrics(
                    s.id.clone(),
                    s.title.clone(),
                    m,
                ))
            })
            .collect();

        let markdown_summary =
            build_markdown_summary(&metrics, &statute_details, config.population_size);

        Ok(SimulationResult {
            metrics,
            population_size: config.population_size,
            statute_count: statutes.len(),
            statute_details,
            markdown_summary,
        })
    }
}

/// Build a Markdown summary from simulation metrics.
fn build_markdown_summary(
    metrics: &SimulationMetrics,
    details: &[StatuteDetail],
    population_size: usize,
) -> String {
    let mut md = String::from("## 政策シミュレーション結果\n\n");

    md.push_str(&format!(
        "**対象人口:** {}人 | **対象条文:** {}件 | **総適用試行:** {}件\n\n",
        population_size,
        details.len(),
        metrics.total_applications,
    ));

    // Overall impact table
    md.push_str("### 全体影響度\n\n");
    md.push_str("| 分類 | 件数 | 割合 |\n|------|------|------|\n");

    let total = metrics.total_applications.max(1);
    md.push_str(&format!(
        "| ✅ 決定論的適用 | {} | {:.1}% |\n",
        metrics.deterministic_count,
        metrics.deterministic_count as f64 / total as f64 * 100.0
    ));
    md.push_str(&format!(
        "| ⚖️ 裁量的判断 | {} | {:.1}% |\n",
        metrics.discretion_count,
        metrics.discretion_count as f64 / total as f64 * 100.0
    ));
    md.push_str(&format!(
        "| ❌ 論理矛盾 | {} | {:.1}% |\n\n",
        metrics.void_count,
        metrics.void_count as f64 / total as f64 * 100.0
    ));

    // Quality assessment
    let det_ratio = metrics.deterministic_ratio();
    let (grade, comment) = if det_ratio >= 0.85 {
        ('A', "法令の適用が高度に決定論的です。")
    } else if det_ratio >= 0.70 {
        ('B', "法令の適用は概ね決定論的です。")
    } else if det_ratio >= 0.55 {
        ('C', "法令の適用に一定の裁量が伴います。")
    } else if det_ratio >= 0.40 {
        ('D', "法令の適用に大きな裁量が伴います。")
    } else {
        ('F', "法令の適用のほとんどが裁量的です。")
    };
    md.push_str(&format!("**品質評価: {}** — {}\n\n", grade, comment));

    // Per-statute breakdown
    if !details.is_empty() {
        md.push_str("### 条文別影響分析\n\n");
        md.push_str("| 条文ID | タイトル | 適用率 | 曖昧度 | 総計 |\n");
        md.push_str("|--------|----------|--------|--------|------|\n");
        for d in details {
            md.push_str(&format!(
                "| {} | {} | {:.1}% | {:.1}% | {} |\n",
                d.id,
                d.title.chars().take(30).collect::<String>(),
                d.effectiveness * 100.0,
                d.ambiguity * 100.0,
                d.total,
            ));
        }
        md.push('\n');
    }

    md.push_str("*Powered by Legalis-Sim ECS Engine + OxiZ SMT Solver*\n");
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jp_2024_profile_distributions() {
        let profile = jp_2024_profile();
        // Verify age distribution is Normal with JP census values
        assert!(matches!(
            profile.age_distribution,
            Distribution::Normal { mean, .. } if (mean - 48.4).abs() < 0.1
        ));
        // Verify income is LogNormal
        assert!(matches!(
            profile.income_distribution,
            Distribution::LogNormal { .. }
        ));
        // Verify employment_type_code custom attribute exists
        assert!(
            profile
                .custom_attributes
                .contains_key("employment_type_code")
        );
    }

    #[test]
    fn test_simulation_config_default() {
        let config = SimulationConfig::default();
        assert_eq!(config.population_size, 1000);
    }

    #[test]
    fn test_simulator_error_no_statutes() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = SimulationConfig {
            population_size: 10,
            profile: jp_2024_profile(),
        };
        let result = rt.block_on(SimulatorService::run(vec![], &config));
        assert!(matches!(result, Err(SimulatorError::NoStatutes)));
    }

    #[test]
    fn test_simulator_error_empty_population() {
        use legalis_jp::reasoning::lsa_article_32_working_hours;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = SimulationConfig {
            population_size: 0,
            profile: jp_2024_profile(),
        };
        let result = rt.block_on(SimulatorService::run(
            vec![lsa_article_32_working_hours()],
            &config,
        ));
        assert!(matches!(result, Err(SimulatorError::EmptyPopulation)));
    }

    #[test]
    fn test_simulator_run_labor_statutes() {
        use legalis_jp::reasoning::{
            lca_article_18_indefinite_conversion, lsa_article_32_working_hours,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let statutes = vec![
            lsa_article_32_working_hours(),
            lca_article_18_indefinite_conversion(),
        ];
        let config = SimulationConfig {
            population_size: 100,
            profile: jp_2024_profile(),
        };
        let result = rt
            .block_on(SimulatorService::run(statutes, &config))
            .unwrap();
        assert_eq!(result.population_size, 100);
        assert_eq!(result.statute_count, 2);
        // total_applications = population_size * statute_count
        assert_eq!(result.metrics.total_applications, 200);
        // Markdown summary should mention 100 and 2
        assert!(result.markdown_summary.contains("100"));
        assert!(result.markdown_summary.contains("Legalis-Sim"));
    }

    #[test]
    fn test_build_markdown_summary_empty_details() {
        let metrics = SimulationMetrics {
            total_applications: 1000,
            deterministic_count: 900,
            discretion_count: 80,
            void_count: 20,
            statute_metrics: HashMap::new(),
            discretion_agents: vec![],
        };
        let md = build_markdown_summary(&metrics, &[], 100);
        assert!(md.contains("政策シミュレーション結果"));
        assert!(md.contains("900"));
        assert!(md.contains("Legalis-Sim"));
    }

    #[test]
    fn test_statute_detail_from_metrics() {
        let m = StatuteMetrics {
            total: 100,
            deterministic: 80,
            discretion: 15,
            void: 5,
        };
        let detail =
            StatuteDetail::from_metrics("LSA_Art32".to_string(), "労働時間".to_string(), &m);
        assert_eq!(detail.id, "LSA_Art32");
        assert!((detail.effectiveness - 0.80).abs() < 0.01);
        assert!((detail.ambiguity - 0.15).abs() < 0.01);
    }
}
