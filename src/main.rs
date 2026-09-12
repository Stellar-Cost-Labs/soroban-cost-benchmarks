//! CLI entry point for soroban-cost-benchmarks.
//!
//! Subcommands:
//! - `rent-forecast` — Generate storage-rent projections per tier
//! - `wasm-metrics` — Analyze a compiled WASM binary
//! - `benchmark` — Run multi-scenario benchmarks
//! - `compare` — Compare two cost snapshots
//! - `pr-comment` — Post/update a cost summary PR comment

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use soroban_cost_benchmarks::benchmark;
use soroban_cost_benchmarks::compare;
use soroban_cost_benchmarks::live_config;
use soroban_cost_benchmarks::pr_comment;
use soroban_cost_benchmarks::rent_forecast;
use soroban_cost_benchmarks::wasm_metrics;

#[derive(Parser)]
#[command(
    name = "soroban-cost-benchmarks",
    about = "Soroban storage-rent forecasting, WASM metrics, and CI PR comment bot",
    version,
    long_about = "Built on soroban-cost-estimator for RPC simulation and config snapshots.\n\n\
        Forecasts storage-rent costs at 30/180/365-day horizons per storage tier,\n\
        captures WASM binary metrics, runs multi-scenario benchmarks, and posts\n\
        structured cost summaries directly into GitHub PRs."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Network to use (testnet, mainnet, futurenet).
    #[arg(long, default_value = "testnet", global = true)]
    network: String,

    /// RPC URL override.
    #[arg(long, global = true)]
    rpc_url: Option<String>,

    /// Permit placeholder (demo) rent rates when no live fetch or snapshot is
    /// available.
    ///
    /// Off by default on purpose: every forecast must come from real network
    /// data (live RPC fetch or --config-snapshot), so placeholder numbers can
    /// never be mistaken for network-derived ones.
    #[arg(long, global = true)]
    allow_demo: bool,

    /// Verbose output.
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate storage-rent projections per tier at 30/180/365-day horizons.
    RentForecast {
        /// Path to save the config snapshot (JSON). If not provided, fetches live.
        #[arg(long)]
        config_snapshot: Option<PathBuf>,

        /// Storage entries as "tier:size_bytes" pairs (e.g., "persistent:1024").
        /// Can be specified multiple times.
        #[arg(short = 'e', long = "entry")]
        entries: Vec<String>,

        /// Time horizons in days (default: 30, 180, 365).
        #[arg(long, value_delimiter = ',')]
        horizons: Option<Vec<u32>>,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Output as CSV.
        #[arg(long)]
        csv: bool,

        /// File path for output (default: stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Analyze a compiled WASM binary for static metrics.
    WasmMetrics {
        /// Path to the .wasm file.
        #[arg(short, long)]
        wasm: PathBuf,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// File path for output (default: stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run multi-scenario benchmarks.
    Benchmark {
        /// Path to a config snapshot file (JSON).
        #[arg(long)]
        config_snapshot: Option<PathBuf>,

        /// Custom storage entries for benchmarking.
        #[arg(short = 'e', long = "entry")]
        entries: Vec<String>,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// File path for output (default: stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Fetch `ConfigSetting*` ledger entries live and report their provenance.
    ///
    /// Reads the entries directly via `getLedgerEntries` and stamps the snapshot
    /// with the network's current ledger from `getLatestLedger`. This is a
    /// deliberate workaround for a known `soroban-cost-estimator` bug where
    /// `config snapshot` reports a stale ledger; the tracking issue is printed
    /// alongside the evidence.
    LiveConfig {
        /// Write the fetched config snapshot as JSON to this path.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },

    /// Compare two cost snapshots for regression detection.
    Compare {
        /// Path to baseline snapshot.
        #[arg(long)]
        baseline: PathBuf,

        /// Path to current snapshot.
        #[arg(long)]
        current: PathBuf,

        /// Maximum allowed rent increase percentage before CI fails.
        #[arg(long, default_value = "10.0")]
        threshold: f64,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Output as Markdown (for PR comments).
        #[arg(long)]
        markdown: bool,
    },

    /// Post or update a cost summary comment on a GitHub PR.
    PrComment {
        /// GitHub owner (org or user).
        #[arg(long)]
        owner: String,

        /// Repository name.
        #[arg(long)]
        repo: String,

        /// PR number.
        #[arg(long)]
        pr_number: u64,

        /// Path to a rent forecast JSON file.
        #[arg(long)]
        forecast: Option<PathBuf>,

        /// Path to a WASM metrics JSON file.
        #[arg(long)]
        wasm_metrics: Option<PathBuf>,

        /// Path to a comparison result JSON file.
        #[arg(long)]
        comparison: Option<PathBuf>,

        /// Render the comment and print it, without calling the GitHub API.
        ///
        /// No token is required and nothing is posted — useful for reviewing
        /// exactly what would be published.
        #[arg(long)]
        dry_run: bool,
    },

    /// Export rent forecast as JSON or CSV.
    Export {
        /// Path to rent forecast JSON file.
        #[arg(long)]
        forecast: PathBuf,

        /// Export format: json or csv.
        #[arg(long, default_value = "json")]
        format: String,

        /// Output file path.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .init();

    match cli.command {
        Commands::RentForecast {
            config_snapshot,
            entries,
            horizons,
            json,
            csv,
            output,
        } => {
            cmd_rent_forecast(
                &cli.network,
                cli.rpc_url.as_deref(),
                cli.allow_demo,
                config_snapshot,
                &entries,
                horizons,
                json,
                csv,
                output,
            )
            .await?;
        }
        Commands::WasmMetrics { wasm, json, output } => {
            cmd_wasm_metrics(&wasm, json, output)?;
        }
        Commands::Benchmark {
            config_snapshot,
            entries,
            json,
            output,
        } => {
            cmd_benchmark(
                &cli.network,
                cli.rpc_url.as_deref(),
                cli.allow_demo,
                config_snapshot,
                &entries,
                json,
                output,
            )
            .await?;
        }
        Commands::LiveConfig { out } => {
            cmd_live_config(&cli.network, cli.rpc_url.as_deref(), out).await?;
        }
        Commands::Compare {
            baseline,
            current,
            threshold,
            json,
            markdown,
        } => {
            cmd_compare(&baseline, &current, threshold, json, markdown)?;
        }
        Commands::PrComment {
            owner,
            repo,
            pr_number,
            forecast,
            wasm_metrics: wasm_path,
            comparison,
            dry_run,
        } => {
            cmd_pr_comment(
                &owner, &repo, pr_number, forecast, wasm_path, comparison, dry_run,
            )
            .await?;
        }
        Commands::Export {
            forecast,
            format,
            output,
        } => {
            cmd_export(&forecast, &format, output)?;
        }
    }

    Ok(())
}

/// Rent rates plus the provenance needed to label the resulting output.
struct ConfigInput {
    config: rent_forecast::RentConfig,
    timestamp: String,
    ledger: u32,
    source: String,
}

/// Load rent rates from a snapshot file, a live RPC fetch, or — only when
/// explicitly asked — demo placeholders.
///
/// Precedence: `--config-snapshot` > live fetch (default) > demo (`--allow-demo`).
/// A failed live fetch is a hard error unless `--allow-demo` was passed, so the
/// tool never silently downgrades to plausible-looking placeholder numbers.
///
/// The live path is a deliberate workaround for a known `soroban-cost-estimator`
/// bug (see [`soroban_cost_benchmarks::live_config`]).
async fn load_config(
    network: &str,
    rpc_url: Option<&str>,
    config_snapshot: Option<PathBuf>,
    allow_demo: bool,
    command: &str,
) -> Result<ConfigInput, Box<dyn std::error::Error>> {
    if let Some(path) = config_snapshot {
        let data = std::fs::read_to_string(&path)?;
        let snapshot: soroban_cost_estimator::config_snapshot::model::ConfigSnapshot =
            serde_json::from_str(&data)?;
        // A snapshot file has no LiveSorobanStateSizeWindow, so the state size
        // is unknown and the rate falls back to the protocol's floor.
        let config = rent_forecast::extract_rent_config(&snapshot, None)?;
        return Ok(ConfigInput {
            config,
            timestamp: snapshot.timestamp.clone(),
            ledger: snapshot.ledger,
            source: format!("snapshot:{}", path.display()),
        });
    }

    if !allow_demo {
        match live_config::fetch_live_config(network, rpc_url).await {
            Ok(live) => {
                let config = rent_forecast::extract_rent_config(
                    &live.snapshot,
                    live.evidence.soroban_state_size_bytes,
                )?;
                eprintln!(
                    "ℹ️  {command}: live config from {} — current ledger {} (upstream `config snapshot` would report {})",
                    live.evidence.rpc_url,
                    live.evidence.current_ledger,
                    live.evidence.max_last_modified_ledger,
                );
                return Ok(ConfigInput {
                    config,
                    timestamp: live.snapshot.timestamp.clone(),
                    ledger: live.snapshot.ledger,
                    source: live.evidence.source_tag(),
                });
            }
            Err(error) => {
                return Err(format!(
                    "live config fetch failed: {error}\n\
                     Refusing to fall back to placeholder rates. Options:\n\
                     \x20 --config-snapshot <file>  use a saved config snapshot\n\
                     \x20 --allow-demo               use placeholder rates (output tagged as demo)\n\
                     \x20 --rpc-url <url>            point at a different RPC endpoint"
                )
                .into());
            }
        }
    }

    eprintln!(
        "⚠️  --allow-demo: using placeholder rent rates. These are NOT network data \
         and must not be quoted as a real forecast."
    );
    Ok(ConfigInput {
        config: demo_rent_config(),
        timestamp: "demo-config".to_string(),
        ledger: 0,
        source: live_config::DEMO_CONFIG_SOURCE.to_string(),
    })
}

/// Placeholder rates, used only when `--allow-demo` is passed explicitly.
fn demo_rent_config() -> rent_forecast::RentConfig {
    rent_forecast::RentConfig {
        persistent_rent_rate_denominator: 4096,
        temp_rent_rate_denominator: 4096,
        // Flat 1267 stroops/1 KB curve: low == high, so interpolation is a
        // no-op and `target` is irrelevant to the result.
        rent_fee_1kb_low: 1_267,
        rent_fee_1kb_high: 1_267,
        rent_fee_growth_factor: 0,
        state_target_size_bytes: 1,
        soroban_state_size_bytes: Some(0),
    }
}

/// `live-config` command: fetch `ConfigSetting*` entries and report provenance.
async fn cmd_live_config(
    network: &str,
    rpc_url: Option<&str>,
    out: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let live = live_config::fetch_live_config(network, rpc_url).await?;

    if let Some(path) = out {
        let json = serde_json::to_string_pretty(&live.snapshot)?;
        std::fs::write(&path, json)?;
        println!("Snapshot written to: {}", path.display());
    }

    let config =
        rent_forecast::extract_rent_config(&live.snapshot, live.evidence.soroban_state_size_bytes)?;
    println!("{}", live_config::format_evidence(&live.evidence));
    println!(
        "\nEffective rent rate per 1 KB: {} stroops  [{}]",
        rent_forecast::effective_rent_rate_1kb(&config),
        rent_forecast::rate_basis(&config),
    );
    println!("\nRent rates extracted from this fetch:");
    println!(
        "  persistent_rent_rate_denominator: {}",
        config.persistent_rent_rate_denominator
    );
    println!(
        "  temp_rent_rate_denominator:       {}",
        config.temp_rent_rate_denominator
    );
    println!(
        "  rent_fee_1kb_low:                 {}",
        config.rent_fee_1kb_low
    );
    println!(
        "  rent_fee_1kb_high:                {}",
        config.rent_fee_1kb_high
    );
    println!(
        "  rent_fee_growth_factor:           {}",
        config.rent_fee_growth_factor
    );
    println!("\nWorkaround for: {}", live_config::UPSTREAM_ISSUE_URL);
    Ok(())
}

async fn cmd_rent_forecast(
    network: &str,
    rpc_url: Option<&str>,
    allow_demo: bool,
    config_snapshot: Option<PathBuf>,
    entries: &[String],
    horizons: Option<Vec<u32>>,
    json: bool,
    csv: bool,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let horizons = horizons.unwrap_or_else(|| vec![30, 180, 365]);

    // Parse entries
    let parsed_entries: Vec<(rent_forecast::StorageTier, u64)> = if entries.is_empty() {
        // Default: 1 KB persistent entry
        vec![(rent_forecast::StorageTier::Persistent, 1024)]
    } else {
        entries
            .iter()
            .map(|e| parse_entry(e))
            .collect::<Result<Vec<_>, _>>()?
    };

    let input = load_config(
        network,
        rpc_url,
        config_snapshot,
        allow_demo,
        "rent-forecast",
    )
    .await?;

    let forecast = rent_forecast::generate_forecast(
        &input.config,
        network,
        &input.timestamp,
        input.ledger,
        &input.source,
        &parsed_entries,
        &horizons,
    );
    output_forecast(&forecast, json, csv, output)?;

    Ok(())
}

fn output_forecast(
    forecast: &rent_forecast::RentForecast,
    json: bool,
    csv: bool,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = if json {
        serde_json::to_string_pretty(forecast)?
    } else if csv {
        let mut wtr = csv::Writer::from_writer(vec![]);
        for entry in &forecast.entries {
            wtr.serialize(entry)?;
        }
        String::from_utf8(wtr.into_inner()?)?
    } else {
        rent_forecast::format_forecast_table(forecast)
    };

    match output {
        Some(path) => std::fs::write(path, content)?,
        None => println!("{content}"),
    }
    Ok(())
}

fn cmd_wasm_metrics(
    wasm_path: &Path,
    json: bool,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let metrics = wasm_metrics::analyze_wasm(wasm_path)?;

    let content = if json {
        serde_json::to_string_pretty(&metrics)?
    } else {
        wasm_metrics::format_metrics_table(&metrics)
    };

    match output {
        Some(path) => std::fs::write(path, content)?,
        None => println!("{content}"),
    }
    Ok(())
}

async fn cmd_benchmark(
    network: &str,
    rpc_url: Option<&str>,
    allow_demo: bool,
    config_snapshot: Option<PathBuf>,
    entries: &[String],
    json: bool,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let scenarios = if entries.is_empty() {
        benchmark::standard_scenarios()
    } else {
        let parsed: Vec<(rent_forecast::StorageTier, u64)> = entries
            .iter()
            .map(|e| parse_entry(e))
            .collect::<Result<Vec<_>, _>>()?;
        vec![benchmark::BenchmarkScenario {
            name: "custom".to_string(),
            description: "User-defined storage footprint".to_string(),
            storage_entries: parsed,
        }]
    };

    let input = load_config(network, rpc_url, config_snapshot, allow_demo, "benchmark").await?;
    let config = input.config;
    let config_timestamp = input.timestamp;
    let config_source = input.source;

    let horizons = vec![30, 180, 365];
    let results: Vec<benchmark::BenchmarkResult> = scenarios
        .iter()
        .map(|scenario| {
            let rent_forecast_entries = benchmark::scenario_rent(&config, scenario, &horizons);
            benchmark::BenchmarkResult {
                scenario: scenario.clone(),
                metrics: benchmark::ResourceMetrics {
                    cpu_instructions: 0,
                    memory_bytes: 0,
                    read_entries: 0,
                    write_entries: 0,
                    read_bytes: 0,
                    write_bytes: 0,
                    tx_size_bytes: 0,
                    min_resource_fee: 0,
                },
                rent_forecast: rent_forecast_entries,
            }
        })
        .collect();

    let report = benchmark::BenchmarkReport {
        network: network.to_string(),
        config_timestamp,
        config_source,
        results,
        delta: None,
    };

    let content = if json {
        serde_json::to_string_pretty(&report)?
    } else {
        benchmark::format_report_table(&report)
    };

    match output {
        Some(path) => std::fs::write(path, content)?,
        None => println!("{content}"),
    }
    Ok(())
}

fn cmd_compare(
    baseline_path: &Path,
    current_path: &Path,
    threshold: f64,
    json: bool,
    _markdown: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline = compare::load_snapshot(baseline_path)?;
    let current = compare::load_snapshot(current_path)?;

    let thresholds = compare::ThresholdConfig {
        max_rent_increase_percent: threshold,
        max_wasm_size_increase_percent: 20.0,
    };

    let result = compare::compare(&baseline, &current, &thresholds);

    let content = if json {
        serde_json::to_string_pretty(&result)?
    } else {
        compare::format_comparison_markdown(&result)
    };

    println!("{content}");

    if result.should_fail_ci {
        std::process::exit(1);
    }

    Ok(())
}

async fn cmd_pr_comment(
    owner: &str,
    repo: &str,
    pr_number: u64,
    forecast_path: Option<PathBuf>,
    wasm_metrics_path: Option<PathBuf>,
    comparison_path: Option<PathBuf>,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let token = if dry_run {
        // No API call is made, so no credential is needed.
        String::new()
    } else {
        std::env::var("GITHUB_TOKEN")
            .map_err(|_| "GITHUB_TOKEN environment variable is required")?
    };

    let rent_forecast = if let Some(path) = forecast_path {
        let data = std::fs::read_to_string(&path)?;
        serde_json::from_str(&data)?
    } else {
        return Err("At least --forecast is required for pr-comment".into());
    };

    let wasm = if let Some(path) = wasm_metrics_path {
        let data = std::fs::read_to_string(&path)?;
        Some(serde_json::from_str(&data)?)
    } else {
        None
    };

    let comparison = if let Some(path) = comparison_path {
        let data = std::fs::read_to_string(&path)?;
        Some(serde_json::from_str(&data)?)
    } else {
        None
    };

    let summary = pr_comment::CostSummaryComment {
        title: "Soroban Cost Summary".to_string(),
        rent_forecast,
        wasm_metrics: wasm,
        comparison,
    };

    if dry_run {
        eprintln!(
            "[dry run] no GitHub API call made — printing the comment body that would be sent to {owner}/{repo}#{pr_number}"
        );
        println!("{}", pr_comment::render_comment_markdown(&summary));
        return Ok(());
    }

    let config = pr_comment::PrCommentConfig {
        owner: owner.to_string(),
        repo: repo.to_string(),
        pr_number,
        token,
    };

    let comment_id = pr_comment::post_or_update_comment(&config, &summary).await?;
    println!("Comment posted/updated: ID {comment_id}");
    Ok(())
}

fn cmd_export(
    forecast_path: &Path,
    format: &str,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read_to_string(forecast_path)?;
    let forecast: rent_forecast::RentForecast = serde_json::from_str(&data)?;

    let content = match format {
        "csv" => {
            let mut wtr = csv::Writer::from_writer(vec![]);
            for entry in &forecast.entries {
                wtr.serialize(entry)?;
            }
            String::from_utf8(wtr.into_inner()?)?
        }
        _ => serde_json::to_string_pretty(&forecast)?,
    };

    match output {
        Some(path) => std::fs::write(path, content)?,
        None => println!("{content}"),
    }
    Ok(())
}

/// Parse a "tier:size_bytes" entry string.
fn parse_entry(s: &str) -> Result<(rent_forecast::StorageTier, u64), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid entry format '{s}'. Expected 'tier:size_bytes'").into());
    }

    let tier = match parts[0].to_lowercase().as_str() {
        "instance" => rent_forecast::StorageTier::Instance,
        "persistent" => rent_forecast::StorageTier::Persistent,
        "temp" | "temporary" => rent_forecast::StorageTier::Temporary,
        other => {
            return Err(
                format!("Unknown tier '{other}'. Use: instance, persistent, temporary").into(),
            );
        }
    };

    let size: u64 = parts[1].parse()?;
    Ok((tier, size))
}
