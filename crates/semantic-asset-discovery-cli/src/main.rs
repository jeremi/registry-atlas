use chrono::Utc;
use clap::{Parser, Subcommand};
use semantic_asset_discovery::{Credentials, DiscoveryClient, DiscoveryPolicy};
use semantic_asset_discovery_core::{
    analyze_artifacts, AnalyzeInput, AnalyzeOptions, DiscoveryReport, FetchedArtifact,
    REPORT_SCHEMA_VERSION,
};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(name = "semantic-asset-discovery")]
#[command(about = "Analyze and harvest semantic metadata artifacts without owning publication.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Analyze {
        #[arg(long)]
        entry_url: String,
        #[arg(value_name = "ARTIFACT")]
        artifacts: Vec<PathBuf>,
    },
    AnalyzeBundle {
        #[arg(value_name = "BUNDLE_JSON")]
        bundle: Option<PathBuf>,
    },
    Harvest {
        url: String,
        #[arg(long, default_value_t = 2)]
        max_depth: u32,
        #[arg(long, default_value_t = 50)]
        max_fetches: u64,
        #[arg(long, default_value_t = 8_388_608)]
        max_body_bytes: u64,
        #[arg(long, default_value_t = 67_108_864)]
        max_total_bytes: u64,
        #[arg(long, default_value_t = 8)]
        max_concurrent_fetches: u64,
        #[arg(long, default_value_t = 10_000)]
        timeout_ms: u64,
        #[arg(long, default_value_t = 120_000)]
        total_timeout_ms: u64,
        #[arg(long)]
        allow_private_network: bool,
        #[arg(long, value_name = "ENV")]
        bearer_token_env: Option<String>,
    },
    ValidateReport {
        #[arg(value_name = "REPORT_JSON")]
        reports: Vec<PathBuf>,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(AppError::Cli(message)) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
        Err(AppError::Validation(message)) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), AppError> {
    let cli = Cli::parse();
    match cli.command {
        Command::Analyze {
            entry_url,
            artifacts,
        } => {
            let input = AnalyzeInput {
                entry_url: entry_url.clone(),
                analyzed_at: None,
                artifacts: artifacts
                    .into_iter()
                    .map(|path| fetched_from_file(&entry_url, path))
                    .collect::<Result<Vec<_>, _>>()?,
                options: AnalyzeOptions::default(),
            };
            print_report(input)?;
        }
        Command::AnalyzeBundle { bundle } => {
            let text = read_optional_path(bundle)?;
            let input: AnalyzeInput = serde_json::from_str(&text)?;
            print_report(input)?;
        }
        Command::Harvest {
            url,
            max_depth,
            max_fetches,
            max_body_bytes,
            max_total_bytes,
            max_concurrent_fetches,
            timeout_ms,
            total_timeout_ms,
            allow_private_network,
            bearer_token_env,
        } => {
            let envelope = harvest(
                &url,
                HarvestOptions {
                    max_depth,
                    max_fetches,
                    max_body_bytes,
                    max_total_bytes,
                    max_concurrent_fetches,
                    timeout_ms,
                    total_timeout_ms,
                    allow_private_network,
                    bearer_token_env,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
        Command::ValidateReport { reports } => {
            if reports.is_empty() {
                return Err(AppError::Cli(
                    "validate-report requires at least one report path".to_string(),
                ));
            }
            for path in reports {
                validate_report(path)?;
            }
        }
    }
    Ok(())
}

fn print_report(input: AnalyzeInput) -> Result<(), AppError> {
    let report = analyze_artifacts(input)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn fetched_from_file(entry_url: &str, path: PathBuf) -> Result<FetchedArtifact, AppError> {
    let body = fs::read(&path)?;
    let url = if entry_url.ends_with('/') {
        format!(
            "{entry_url}{}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("artifact")
        )
    } else {
        entry_url.to_string()
    };
    Ok(FetchedArtifact {
        url,
        final_url: None,
        status: 200,
        media_type: media_type_for_path(&path),
        request_accept: None,
        redirect_chain: Vec::new(),
        headers: Vec::new(),
        body,
        fetched_at: Utc::now().to_rfc3339(),
        depth: 0,
        discovered_from: None,
        discovered_by: None,
    })
}

fn read_optional_path(path: Option<PathBuf>) -> Result<String, AppError> {
    match path {
        Some(path) => Ok(fs::read_to_string(path)?),
        None => {
            let mut input = String::new();
            io::stdin().read_to_string(&mut input)?;
            Ok(input)
        }
    }
}

struct HarvestOptions {
    max_depth: u32,
    max_fetches: u64,
    max_body_bytes: u64,
    max_total_bytes: u64,
    max_concurrent_fetches: u64,
    timeout_ms: u64,
    total_timeout_ms: u64,
    allow_private_network: bool,
    bearer_token_env: Option<String>,
}

fn harvest(
    url: &str,
    options: HarvestOptions,
) -> Result<semantic_asset_discovery::DiscoveryRunEnvelope, AppError> {
    let policy = if options.allow_private_network {
        DiscoveryPolicy::local_development()
    } else {
        DiscoveryPolicy::public_web()
    };
    let credentials = match &options.bearer_token_env {
        Some(name) => {
            let token = env::var(name)
                .map_err(|_| AppError::Cli(format!("environment variable `{name}` is not set")))?;
            if token.trim().is_empty() {
                return Err(AppError::Cli(format!(
                    "environment variable `{name}` is empty"
                )));
            }
            Credentials::bearer(token).same_origin_only()
        }
        None => Credentials::none(),
    };
    let client = DiscoveryClient::builder()
        .policy(policy)
        .credentials(credentials)
        .max_depth(options.max_depth)
        .max_fetches(options.max_fetches)
        .max_body_bytes(options.max_body_bytes)
        .max_total_bytes(options.max_total_bytes)
        .max_concurrent_fetches(options.max_concurrent_fetches)
        .timeout(Duration::from_millis(options.timeout_ms))
        .total_timeout(Duration::from_millis(options.total_timeout_ms))
        .build()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(runtime.block_on(client.discover(url))?.into_envelope())
}

fn validate_report(path: PathBuf) -> Result<(), AppError> {
    let text = fs::read_to_string(&path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let schema_version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AppError::Cli(format!("{} has no schema_version", path.display())))?;
    if schema_version != REPORT_SCHEMA_VERSION {
        return Err(AppError::Validation(format!(
            "{} uses unsupported schema_version `{schema_version}`",
            path.display()
        )));
    }
    let _report: DiscoveryReport = serde_json::from_value(value)?;
    Ok(())
}

fn media_type_for_path(path: &std::path::Path) -> Option<String> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "json" => Some("application/json".to_string()),
        "jsonld" => Some("application/ld+json".to_string()),
        "ttl" => Some("text/turtle".to_string()),
        "yaml" | "yml" => Some("application/yaml".to_string()),
        "toml" => Some("application/toml".to_string()),
        "html" | "htm" => Some("text/html".to_string()),
        _ => None,
    }
}

#[derive(Debug, Error)]
enum AppError {
    #[error("{0}")]
    Cli(String),
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Analyze(#[from] semantic_asset_discovery_core::AnalyzeError),
    #[error(transparent)]
    Discovery(#[from] semantic_asset_discovery::DiscoveryError),
}
