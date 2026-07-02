/// oxigenai — OxigenAI 源内 unified CLI + HTTP server
///
/// Subcommands:
///   oxigenai query <text>      Generate a legal report (default when no subcommand given)
///   oxigenai serve             Start the HTTP API server
///   oxigenai compile           Compile law articles to Legalis DSL
///   oxigenai simulate          Run a population-level policy simulation
///   oxigenai formalize         Evaluate statutes against user facts
///   oxigenai predict           Predict a likely judicial ruling from case law
///
/// Examples:
///   oxigenai "個人情報保護法の適用範囲は？"          # bare invocation = query
///   oxigenai query "労働基準法 時間外労働"
///   oxigenai serve                                   # PORT=8081 oxigenai serve
///   oxigenai compile --query "労働基準法"
///   oxigenai compile --xml law.xml --dsl-only
///   oxigenai simulate "労働基準法" --population 500
///   oxigenai formalize "無期転換" --age 35 --attr employment_type=fixed_term --attr years_employed=6
///   oxigenai predict "5年勤続の有期社員を経営不振で解雇できるか" --facts "解雇回避努力なし"
use axum::{
    Router,
    http::{HeaderName, Method},
    routing::{get, post},
};
use clap::{Parser, Subcommand};
use legalis_dsl::format_statutes;
use legalis_sim::DemographicProfile;
use oxigenai::config::AppConfig;
use oxigenai::handlers::report::AppState;
use oxigenai::models::law::FullArticle;
use oxigenai::services::bq_retriever::BigQueryRetriever;
use oxigenai::services::gemini_client::GeminiService;
use oxigenai::services::pipeline::{PipelineContext, ReportOptions, generate_law_report};
use oxigenai::verifier::compiler::CompilerService;
use oxigenai::verifier::dsl_bridge::StatuteBridge;
use oxigenai::verifier::formalize::{FormalizeService, UserFacts};
use oxigenai::verifier::integration::LegalVerifier;
use oxigenai::verifier::jurisdiction::MultiJurisdictionMatcher;
use oxigenai::verifier::jurisprudence::CaseLawPredictor;
use oxigenai::verifier::simulator::{SimulationConfig, SimulatorService, profile_by_name};
use oxigenai::verifier::translate_check::compare_statutes;
use std::collections::HashMap;
use std::io::Read as _;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

// ─── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "oxigenai",
    version,
    about = "OxigenAI 源内 — 日本法令AI (Legalis-RS + OxiZ SMT)",
    long_about = "Japanese legal AI powered by Legalis-RS and OxiZ SMT solver.\n\
                  Run without a subcommand to use as a query CLI (same as `query`).",
    subcommand_required = true,
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a Japanese legal report
    #[command(alias = "q")]
    Query {
        /// Legal query text, or "-" to read from stdin
        query: String,
        /// Jurisdiction code for statute resolution (JP, EU, US)
        #[arg(long, short = 'j', default_value = "JP")]
        jurisdiction: String,
        /// Output raw JSON ({outputs, usageMetadata})
        #[arg(long)]
        json: bool,
        /// Suppress usage/cost metadata printed to stderr
        #[arg(long)]
        no_usage: bool,
    },

    /// Start the HTTP API server (POST /, /compile, /simulate, /formalize, /predict-ruling, GET /health)
    #[command(alias = "server")]
    Serve {
        /// TCP port to listen on (overrides PORT env var, default 8080)
        #[arg(long, short = 'p')]
        port: Option<u16>,
    },

    /// Compile law articles to Legalis DSL text
    ///
    /// Mode A: --xml <file>   Parse e-Gov XML → Statute → DSL (no network needed)
    /// Mode B: --query <text> Embed query → BQ search → domain match → DSL
    Compile {
        /// e-Gov XML file to compile (Mode A)
        #[arg(long, group = "input")]
        xml: Option<PathBuf>,
        /// Query to find law articles from BigQuery (Mode B)
        #[arg(long, group = "input")]
        query: Option<String>,
        /// Jurisdiction code for Mode B statute resolution (JP, EU, US)
        #[arg(long, short = 'j', default_value = "JP")]
        jurisdiction: String,
        /// Print only the raw DSL text (default: human-readable + DSL)
        #[arg(long)]
        dsl_only: bool,
        /// Output full JSON response
        #[arg(long, conflicts_with = "dsl_only")]
        json: bool,
    },

    /// Run a population-level policy simulation (legalis-sim)
    ///
    /// Finds statutes for the query via -j/--jurisdiction (which independently
    /// selects the applicable statutes), generates a demographic population from
    /// the requested --profile (jp_2024, us_2024, or eu_2024; default jp_2024,
    /// resolved via profile_by_name — unrecognized names are rejected with an
    /// error), runs SimEngine, and returns SimulationMetrics + Markdown summary.
    /// jurisdiction and profile are intentionally independent axes: jurisdiction
    /// never derives the profile, and vice versa.
    #[command(alias = "sim")]
    Simulate {
        /// Legal query to find statutes
        query: String,
        /// Jurisdiction code for statute resolution (JP, EU, US)
        #[arg(long, short = 'j', default_value = "JP")]
        jurisdiction: String,
        /// Number of simulated agents (max 10,000)
        #[arg(long, default_value_t = 1000)]
        population: usize,
        /// Demographic profile for population generation (jp_2024, us_2024, eu_2024)
        #[arg(long, default_value = "jp_2024")]
        profile: String,
        /// Output raw JSON
        #[arg(long)]
        json: bool,
    },

    /// Evaluate statutes against user-supplied facts (EntailmentEngine)
    ///
    /// Example: oxigenai formalize "無期転換" --age 35 --attr employment_type=fixed_term --attr years_employed=6
    #[command(alias = "eval")]
    Formalize {
        /// Legal query to find statutes
        query: String,
        /// Jurisdiction code for statute resolution (JP, EU, US)
        #[arg(long, short = 'j', default_value = "JP")]
        jurisdiction: String,
        /// Applicant age in years
        #[arg(long)]
        age: Option<u32>,
        /// Annual income in JPY
        #[arg(long)]
        income: Option<u64>,
        /// Key=value attribute pairs (repeatable, e.g. --attr employment_type=fixed_term)
        #[arg(long = "attr", value_name = "KEY=VALUE")]
        attrs: Vec<String>,
        /// Free-form description of the situation (Japanese)
        #[arg(long, default_value = "")]
        description: String,
        /// Output raw JSON
        #[arg(long)]
        json: bool,
    },

    /// Predict a likely judicial ruling from case law (生成的法解釈)
    ///
    /// Searches a curated corpus of real Japanese landmark precedents, then uses
    /// Gemini web-grounded retrieval + deterministic synthesis to predict a
    /// holding (結論), reasoning grounded in the precedents (判例の射程), and a
    /// confidence assessment.
    ///
    /// Example: oxigenai predict "5年勤続の有期社員を経営不振で解雇できるか" --facts "解雇回避努力なし"
    #[command(alias = "pred")]
    Predict {
        /// Legal question / fact pattern to predict a ruling for
        query: String,
        /// Jurisdiction code for related-statute selection (JP, EU, US)
        #[arg(long, short = 'j', default_value = "JP")]
        jurisdiction: String,
        /// Free-form description of the facts (事実関係, Japanese)
        #[arg(long, default_value = "")]
        facts: String,
        /// Output raw JSON
        #[arg(long)]
        json: bool,
    },

    /// Verify a statute and its translation carry the same legal meaning
    ///
    /// Compiles BOTH e-Gov XML documents to Legalis DSL and compares the
    /// resulting statute sets structurally (statute count, effect-type multiset,
    /// recursive precondition-kind multiset). Fully offline — no network.
    ///
    /// Example: oxigenai translate-check --source ja.xml --target en.xml
    #[command(name = "translate-check", alias = "txcheck")]
    TranslateCheck {
        /// Source-language e-Gov XML file (e.g. Japanese original)
        #[arg(long)]
        source: PathBuf,
        /// Target-language e-Gov XML file (e.g. English translation)
        #[arg(long)]
        target: PathBuf,
        /// Advisory source language tag
        #[arg(long, default_value = "ja")]
        source_lang: String,
        /// Advisory target language tag
        #[arg(long, default_value = "en")]
        target_lang: String,
        /// Output raw JSON
        #[arg(long)]
        json: bool,
    },
}

// ─── Entry point ───────────────────────────────────────────────────────────────

/// Parse CLI args, injecting "query" as default subcommand for bare invocations.
///
/// `oxigenai "クエリ文"` → treated as `oxigenai query "クエリ文"`
fn parse_cli_with_default_query() -> Cli {
    let raw: Vec<String> = std::env::args().collect();
    let known = [
        "serve",
        "server",
        "query",
        "q",
        "compile",
        "simulate",
        "sim",
        "formalize",
        "eval",
        "predict",
        "pred",
        "translate-check",
        "txcheck",
        "help",
        "--help",
        "-h",
        "--version",
        "-V",
    ];
    let first = raw.get(1).map(|s| s.as_str()).unwrap_or("");
    // If first arg is not a known subcommand/flag and is not empty → inject "query"
    if !first.is_empty() && !first.starts_with('-') && !known.contains(&first) {
        let mut injected = raw[..1].to_vec();
        injected.push("query".to_string());
        injected.extend_from_slice(&raw[1..]);
        Cli::parse_from(injected)
    } else {
        Cli::parse()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cli = parse_cli_with_default_query();

    let is_server = matches!(cli.command, Commands::Serve { .. });
    init_tracing(is_server);

    match cli.command {
        Commands::Serve { port } => run_serve(port).await,

        Commands::Query {
            query,
            jurisdiction,
            json,
            no_usage,
        } => {
            let q = resolve_stdin(&query)?;
            let ctx = build_context().await?;
            run_query(&q, &jurisdiction, json, no_usage, &ctx).await
        }

        Commands::Compile {
            xml,
            query,
            jurisdiction,
            dsl_only,
            json,
        } => run_compile(xml, query, &jurisdiction, dsl_only, json).await,

        Commands::Simulate {
            query,
            jurisdiction,
            population,
            profile,
            json,
        } => {
            let ctx = build_context().await?;
            run_simulate(
                &query,
                &jurisdiction,
                population.clamp(1, 10_000),
                &profile,
                json,
                &ctx,
            )
            .await
        }

        Commands::Formalize {
            query,
            jurisdiction,
            age,
            income,
            attrs,
            description,
            json,
        } => {
            let ctx = build_context().await?;
            run_formalize(
                &query,
                &jurisdiction,
                age,
                income,
                &attrs,
                &description,
                json,
                &ctx,
            )
            .await
        }

        Commands::Predict {
            query,
            jurisdiction,
            facts,
            json,
        } => {
            let ctx = build_context().await?;
            run_predict(&query, &jurisdiction, &facts, json, &ctx).await
        }

        Commands::TranslateCheck {
            source,
            target,
            source_lang,
            target_lang,
            json,
        } => run_translate_check(&source, &target, &source_lang, &target_lang, json),
    }
}

// ─── Tracing ───────────────────────────────────────────────────────────────────

fn init_tracing(server_mode: bool) {
    if server_mode {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .init();
    } else {
        // CLI: tracing → stderr so stdout stays clean for pipeline output
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
            )
            .with_writer(std::io::stderr)
            .init();
    }
}

// ─── Shared helpers ────────────────────────────────────────────────────────────

/// Build a `PipelineContext` from environment variables.
async fn build_context() -> anyhow::Result<PipelineContext> {
    let config = Arc::new(AppConfig::from_env()?);
    let gemini = Arc::new(GeminiService::new(config.clone()).await?);
    let bq = Arc::new(BigQueryRetriever::from_config(&config).await?);
    let verifier = Arc::new(LegalVerifier::new());
    Ok(PipelineContext {
        gemini,
        bq,
        config,
        verifier,
    })
}

/// Resolve query text — reads from stdin when the value is "-".
fn resolve_stdin(query: &str) -> anyhow::Result<String> {
    if query == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        let trimmed = buf.trim().to_string();
        anyhow::ensure!(
            !trimmed.is_empty(),
            "stdinが空です — クエリを入力してください"
        );
        Ok(trimmed)
    } else {
        Ok(query.to_string())
    }
}

/// Embed query → BQ search → convert to `Vec<FullArticle>`.
async fn fetch_articles(query: &str, ctx: &PipelineContext) -> anyhow::Result<Vec<FullArticle>> {
    let embeddings = ctx
        .gemini
        .embed_texts(std::slice::from_ref(&query.to_string()))
        .await?;
    let articles = ctx.bq.get_articles_by_nearest_law(&embeddings).await?;
    let full: Vec<FullArticle> = articles
        .iter()
        .filter_map(|a| {
            let content = a.content.as_ref().or(a.article_summary.as_ref())?;
            Some(FullArticle {
                law_id: a.law_id.clone(),
                title: a.law_title.clone(),
                content: content.clone(),
                unique_anchor: a.unique_anchor.clone(),
                anchor: None,
                url: FullArticle::build_egov_url(&a.law_id, None),
            })
        })
        .collect();
    Ok(full)
}

// ─── serve ─────────────────────────────────────────────────────────────────────

async fn run_serve(port_flag: Option<u16>) -> anyhow::Result<()> {
    info!("OxigenAI starting up...");
    info!("Legalis-RS + OxiZ SMT powered 源内 Rust Edition");

    let config = Arc::new(AppConfig::from_env()?);
    info!(
        "Config: project={}, model={}, bq={}",
        config.project_id, config.model_id, config.bq_dataset_id
    );

    let gemini = Arc::new(GeminiService::new(config.clone()).await?);
    let bq = Arc::new(BigQueryRetriever::from_config(&config).await?);
    let verifier = Arc::new(LegalVerifier::new());

    let ctx: AppState = Arc::new(PipelineContext {
        gemini,
        bq,
        config,
        verifier,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::POST, Method::OPTIONS, Method::GET])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("x-api-key"),
        ])
        .expose_headers([HeaderName::from_static("content-type")]);

    let app = Router::new()
        // Root: GET serves the self-contained WebUI, POST is the report API.
        .route(
            "/",
            get(oxigenai::handlers::web::index).post(oxigenai::handlers::report::generate_report),
        )
        .route("/ui", get(oxigenai::handlers::web::index))
        .route("/health", get(oxigenai::handlers::report::health_check))
        .route(
            "/jurisdictions",
            get(oxigenai::handlers::jurisdictions::list_jurisdictions),
        )
        .route("/compile", post(oxigenai::handlers::compile::compile))
        .route("/simulate", post(oxigenai::handlers::simulate::simulate))
        .route("/formalize", post(oxigenai::handlers::formalize::formalize))
        .route(
            "/predict-ruling",
            post(oxigenai::handlers::predict::predict_ruling),
        )
        .route(
            "/translate-check",
            post(oxigenai::handlers::translate::translate_check),
        )
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(ctx);

    let port = port_flag
        .map(|p| p.to_string())
        .or_else(|| std::env::var("PORT").ok())
        .unwrap_or_else(|| "8080".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("OxigenAI server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

// ─── query ─────────────────────────────────────────────────────────────────────

async fn run_query(
    query: &str,
    jurisdiction: &str,
    json: bool,
    no_usage: bool,
    ctx: &PipelineContext,
) -> anyhow::Result<()> {
    let options = ReportOptions::new(jurisdiction);
    let (report, usage) = generate_law_report(query, ctx, &options).await?;

    if json {
        let resp = serde_json::json!({
            "outputs": report,
            "usageMetadata": usage,
        });
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        println!("{}", report);

        if !no_usage && !usage.is_empty() {
            eprintln!("\n--- 使用量 ---");
            for entry in &usage {
                let input = entry.tokens.get("promptTokenCount").copied().unwrap_or(0);
                let output = entry
                    .tokens
                    .get("candidatesTokenCount")
                    .copied()
                    .unwrap_or(0);
                let cost = entry
                    .estimated_cost_info
                    .as_ref()
                    .map(|c| c.estimated_cost)
                    .unwrap_or(0.0);
                eprintln!(
                    "  {}: requests={} input={} output={} cost=${:.4}",
                    entry.model_version, entry.request_count, input, output, cost
                );
            }
        }
    }

    Ok(())
}

// ─── compile ───────────────────────────────────────────────────────────────────

async fn run_compile(
    xml: Option<PathBuf>,
    query: Option<String>,
    jurisdiction: &str,
    dsl_only: bool,
    json: bool,
) -> anyhow::Result<()> {
    let result = match (xml, query) {
        (Some(path), _) => {
            // Mode A parses e-Gov XML natively; jurisdiction does not apply here.
            let xml_str = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("ファイル読み込みエラー: {e}"))?;
            CompilerService::compile_xml(&xml_str)
                .map_err(|e| anyhow::anyhow!("コンパイルエラー: {e}"))?
        }
        (None, Some(q)) => {
            let ctx = build_context().await?;
            let articles = fetch_articles(&q, &ctx).await?;
            CompilerService::compile_articles_for(&articles, jurisdiction)
        }
        (None, None) => {
            anyhow::bail!("--xml または --query のどちらかを指定してください");
        }
    };

    if json {
        let resp = serde_json::json!({
            "law_title": result.law_title,
            "law_num": result.law_num,
            "dsl_text": result.dsl_text,
            "statutes": result.statutes.iter().map(|s| serde_json::json!({
                "id": s.id,
                "title": s.title,
                "dsl": s.dsl,
                "source": s.source,
            })).collect::<Vec<_>>(),
            "article_count": result.article_count,
            "statute_count": result.statutes.len(),
            "warnings": result.warnings,
        });
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    if dsl_only {
        println!("{}", result.dsl_text);
        return Ok(());
    }

    // Human-readable output: metadata to stderr, DSL to stdout
    if !result.law_title.is_empty() {
        eprintln!("法令: {} {}", result.law_title, result.law_num);
    }
    eprintln!(
        "条文: {}件 → {}件 形式化",
        result.article_count,
        result.statutes.len()
    );
    for w in &result.warnings {
        eprintln!("⚠️  {}", w);
    }
    println!("{}", result.dsl_text);

    Ok(())
}

// ─── simulate ──────────────────────────────────────────────────────────────────

/// Resolves the requested demographic profile name to a `DemographicProfile`.
///
/// Mirrors `crate::handlers::simulate::resolve_profile` for the CLI path:
/// jurisdiction (`-j/--jurisdiction`) independently selects which statutes
/// apply, while `--profile` independently selects the demographic distribution
/// used for population generation — one is never derived from the other.
/// Returns a CLI-facing error listing the valid profile names when
/// `profile_name` does not match a known profile (see
/// `oxigenai::verifier::simulator::profile_by_name`).
fn resolve_profile(profile_name: &str) -> anyhow::Result<DemographicProfile> {
    profile_by_name(profile_name).ok_or_else(|| {
        anyhow::anyhow!(
            "不明なプロファイルです: '{profile_name}'。\
             有効な値は jp_2024, us_2024, eu_2024 のいずれかです。"
        )
    })
}

async fn run_simulate(
    query: &str,
    jurisdiction: &str,
    population: usize,
    profile: &str,
    json: bool,
    ctx: &PipelineContext,
) -> anyhow::Result<()> {
    let demographic_profile = resolve_profile(profile)?;

    let articles = fetch_articles(query, ctx).await?;
    let article_statutes = StatuteBridge::convert_articles_domain_only_for(&articles, jurisdiction);

    let mut seen = std::collections::HashSet::new();
    let statutes: Vec<_> = article_statutes
        .into_iter()
        .filter_map(|a| {
            if seen.insert(a.statute.id.clone()) {
                Some(a.statute)
            } else {
                None
            }
        })
        .collect();

    anyhow::ensure!(
        !statutes.is_empty(),
        "シミュレーション対象の条文が見つかりませんでした。クエリを変更してください。"
    );

    eprintln!(
        "条文: {}件 | 人口: {}人 — シミュレーション実行中...",
        statutes.len(),
        population
    );

    let config = SimulationConfig {
        population_size: population,
        profile: demographic_profile,
    };

    let result = SimulatorService::run(statutes, &config)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if json {
        let resp = serde_json::json!({
            "query": query,
            "population_size": result.population_size,
            "statute_count": result.statute_count,
            "total_applications": result.metrics.total_applications,
            "deterministic_count": result.metrics.deterministic_count,
            "discretion_count": result.metrics.discretion_count,
            "void_count": result.metrics.void_count,
            "deterministic_ratio": result.metrics.deterministic_ratio(),
            "discretion_ratio": result.metrics.discretion_ratio(),
            "statute_details": result.statute_details,
            "markdown_summary": result.markdown_summary,
        });
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        println!("{}", result.markdown_summary);
        eprintln!(
            "決定論的適用率: {:.1}%",
            result.metrics.deterministic_ratio() * 100.0
        );
    }

    Ok(())
}

// ─── formalize ─────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn run_formalize(
    query: &str,
    jurisdiction: &str,
    age: Option<u32>,
    income: Option<u64>,
    attrs: &[String],
    description: &str,
    json: bool,
    ctx: &PipelineContext,
) -> anyhow::Result<()> {
    // Parse --attr KEY=VALUE pairs
    let mut attributes: HashMap<String, String> = HashMap::new();
    for attr in attrs {
        if let Some((k, v)) = attr.split_once('=') {
            attributes.insert(k.to_string(), v.to_string());
        } else {
            eprintln!("⚠️ 無効な属性形式 (KEY=VALUE が必要): {attr}");
        }
    }

    let facts = UserFacts {
        age,
        income,
        attributes,
        description: if description.is_empty() {
            "CLIクエリ".to_string()
        } else {
            description.to_string()
        },
    };

    let articles = fetch_articles(query, ctx).await?;
    let service = FormalizeService::new();
    let evaluations = service.evaluate_for(&articles, &facts, jurisdiction);

    if json {
        let applicable = evaluations.iter().filter(|e| e.applies).count();
        let resp = serde_json::json!({
            "query": query,
            "evaluations": evaluations,
            "total_evaluated": evaluations.len(),
            "applicable_count": applicable,
        });
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    // Human-readable output
    let applicable: Vec<_> = evaluations.iter().filter(|e| e.applies).collect();
    let non_applicable: Vec<_> = evaluations.iter().filter(|e| !e.applies).collect();

    println!("=== 法令適用評価: {} ===\n", query);
    println!(
        "適用条文: {}件 / 評価: {}件\n",
        applicable.len(),
        evaluations.len()
    );

    if !applicable.is_empty() {
        println!("【適用される条文】");
        for eval in &applicable {
            println!("  ✅ {} — {}", eval.statute_id, eval.statute_title);
            if let Some(effect) = &eval.effect {
                println!("     効果: {}", effect);
            }
            println!("     {}", eval.explanation);
        }
        println!();
    }

    if !non_applicable.is_empty() {
        println!("【適用されない / 裁量的判断が必要な条文】");
        for eval in &non_applicable {
            let icon = if eval.result_type == "void" {
                "❌"
            } else {
                "⚖️ "
            };
            println!("  {} {} — {}", icon, eval.statute_id, eval.statute_title);
            println!("     {}", eval.explanation);
        }
        println!();
    }

    if evaluations.is_empty() {
        println!("評価対象の条文が見つかりませんでした。クエリを変更してください。");
    }

    Ok(())
}

// ─── predict ───────────────────────────────────────────────────────────────────

async fn run_predict(
    query: &str,
    jurisdiction: &str,
    facts: &str,
    json: bool,
    ctx: &PipelineContext,
) -> anyhow::Result<()> {
    // Fold any supplied fact pattern into the query for grounded synthesis.
    let effective_query = if facts.trim().is_empty() {
        query.to_string()
    } else {
        format!("{}\n\n【事実関係】\n{}", query, facts.trim())
    };

    eprintln!("判例コーパスを検索し、判決を予測しています...");

    let articles = fetch_articles(query, ctx).await?;

    // Select related statutes for the chosen jurisdiction via the multi-jurisdiction
    // bridge. This does not alter the (case-law-driven) prediction on stdout; it is
    // surfaced as discovery metadata on stderr.
    let registry = MultiJurisdictionMatcher::new();
    let code = registry.normalize_code(jurisdiction);
    let related_statutes = StatuteBridge::convert_articles_domain_only_for(&articles, &code);
    info!(
        "predict: jurisdiction={}, {} related statutes selected",
        code,
        related_statutes.len()
    );

    let predictor = CaseLawPredictor::new();
    let prediction = predictor
        .predict_ruling(&effective_query, &articles, &ctx.gemini)
        .await
        .map_err(|e| anyhow::anyhow!("判決予測に失敗しました: {e}"))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&prediction)?);
        return Ok(());
    }

    // Human-readable Markdown to stdout; metadata/usage to stderr.
    println!("{}", prediction.markdown_summary);

    eprintln!(
        "\n適用法域: {} | 関連法令: {}件 | 推定法分野: {} | 参照判例: {}件 | 確信度: {} ({:.0}%)",
        code,
        related_statutes.len(),
        prediction.inferred_legal_area,
        prediction.cited_precedents.len(),
        prediction.confidence_label,
        prediction.confidence * 100.0
    );

    if !prediction.usage.is_empty() {
        eprintln!("--- 使用量 ---");
        for entry in &prediction.usage {
            let input = entry.tokens.get("promptTokenCount").copied().unwrap_or(0);
            let output = entry
                .tokens
                .get("candidatesTokenCount")
                .copied()
                .unwrap_or(0);
            let cost = entry
                .estimated_cost_info
                .as_ref()
                .map(|c| c.estimated_cost)
                .unwrap_or(0.0);
            eprintln!(
                "  {}: requests={} input={} output={} cost=${:.4}",
                entry.model_version, entry.request_count, input, output, cost
            );
        }
    }

    Ok(())
}

// ─── translate-check ─────────────────────────────────────────────────────────

/// Compile two e-Gov XML documents and compare their statute structures.
///
/// Fully offline: reads both files, compiles each via
/// [`CompilerService::compile_xml_to_statutes`], and reports structural
/// equivalence (no network, no machine translation).
fn run_translate_check(
    source: &PathBuf,
    target: &PathBuf,
    source_lang: &str,
    target_lang: &str,
    json: bool,
) -> anyhow::Result<()> {
    let source_xml = std::fs::read_to_string(source)
        .map_err(|e| anyhow::anyhow!("源文ファイル読み込みエラー: {e}"))?;
    let target_xml = std::fs::read_to_string(target)
        .map_err(|e| anyhow::anyhow!("訳文ファイル読み込みエラー: {e}"))?;

    let source_statutes = CompilerService::compile_xml_to_statutes(&source_xml)
        .map_err(|e| anyhow::anyhow!("源文のコンパイルエラー: {e}"))?;
    let target_statutes = CompilerService::compile_xml_to_statutes(&target_xml)
        .map_err(|e| anyhow::anyhow!("訳文のコンパイルエラー: {e}"))?;

    let comparison = compare_statutes(&source_statutes, &target_statutes);

    if json {
        let resp = serde_json::json!({
            "equivalent": comparison.equivalent,
            "score": comparison.score,
            "count_score": comparison.count_score,
            "effect_score": comparison.effect_score,
            "condition_score": comparison.condition_score,
            "divergences": comparison.divergences,
            "source_signature": comparison.source_signature,
            "target_signature": comparison.target_signature,
            "source_lang": source_lang,
            "target_lang": target_lang,
            "source_dsl": format_statutes(&source_statutes),
            "target_dsl": format_statutes(&target_statutes),
        });
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    let verdict = if comparison.equivalent {
        "✅ 構造的に等価"
    } else {
        "⚠️  構造的差異あり"
    };
    println!("=== 翻訳整合性チェック ({source_lang} → {target_lang}) ===\n");
    println!("{verdict}（総合スコア {:.1}%）", comparison.score * 100.0);
    println!(
        "  条文数 一致度: {:.0}% | 法的効果 一致度: {:.0}% | 適用条件 一致度: {:.0}%",
        comparison.count_score * 100.0,
        comparison.effect_score * 100.0,
        comparison.condition_score * 100.0
    );

    if comparison.divergences.is_empty() {
        println!("\n差異は検出されませんでした。両文書は同じ法的構造を持ちます。");
    } else {
        println!("\n【検出された差異】");
        for divergence in &comparison.divergences {
            println!("  • {}", divergence.detail);
        }
    }

    Ok(())
}

// ─── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_profile_known_values_ok() {
        assert!(resolve_profile("jp_2024").is_ok());
        assert!(resolve_profile("us_2024").is_ok());
        assert!(resolve_profile("eu_2024").is_ok());
    }

    #[test]
    fn test_resolve_profile_unknown_returns_error() {
        let err = resolve_profile("bogus_profile").expect_err("unknown profile must be rejected");
        let message = err.to_string();
        assert!(message.contains("jp_2024"));
        assert!(message.contains("us_2024"));
        assert!(message.contains("eu_2024"));
    }

    #[test]
    fn test_simulate_profile_flag_defaults_to_jp_2024() {
        let cli = Cli::try_parse_from(["oxigenai", "simulate", "query text"])
            .expect("simulate subcommand should parse with defaults");
        match cli.command {
            Commands::Simulate { profile, .. } => assert_eq!(profile, "jp_2024"),
            _ => panic!("expected Simulate subcommand"),
        }
    }

    #[test]
    fn test_simulate_profile_flag_overridable() {
        let cli =
            Cli::try_parse_from(["oxigenai", "simulate", "query text", "--profile", "us_2024"])
                .expect("simulate subcommand should parse with --profile override");
        match cli.command {
            Commands::Simulate { profile, .. } => assert_eq!(profile, "us_2024"),
            _ => panic!("expected Simulate subcommand"),
        }
    }

    #[test]
    fn test_simulate_jurisdiction_flag_independent_of_profile() {
        // -j/--jurisdiction (statute selection) and --profile (demographics) must
        // be settable independently, with neither derived from the other.
        let cli = Cli::try_parse_from([
            "oxigenai",
            "simulate",
            "query text",
            "--jurisdiction",
            "US",
            "--profile",
            "eu_2024",
        ])
        .expect("jurisdiction and profile should be independently settable");
        match cli.command {
            Commands::Simulate {
                jurisdiction,
                profile,
                ..
            } => {
                assert_eq!(jurisdiction, "US");
                assert_eq!(profile, "eu_2024");
            }
            _ => panic!("expected Simulate subcommand"),
        }
    }
}
