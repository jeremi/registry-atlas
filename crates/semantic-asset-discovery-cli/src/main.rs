use chrono::Utc;
use clap::{Parser, Subcommand};
use semantic_asset_discovery::{Credentials, DiscoveryClient, DiscoveryPolicy};
use semantic_asset_discovery_core::{
    analyze_artifacts, AnalyzeInput, AnalyzeOptions, DiscoveryEvidence, DiscoveryReport,
    FetchedArtifact, RelationClaim, SemanticAsset, SemanticRelation, ServiceGraph,
    LEGACY_REPORT_SCHEMA_VERSION_V1, REPORT_SCHEMA_VERSION,
};
use serde::Serialize;
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
    ServiceView {
        #[arg(value_name = "PUBLIC_SERVICE_IRI")]
        service_iri: String,
        #[arg(long, value_name = "REPORT_JSON", conflicts_with = "bundle")]
        report: Option<PathBuf>,
        #[arg(long, value_name = "BUNDLE_JSON", conflicts_with = "report")]
        bundle: Option<PathBuf>,
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
        Command::ServiceView {
            service_iri,
            report,
            bundle,
        } => {
            let report = read_service_view_report(report, bundle)?;
            let view = build_service_view(&report, &service_iri)?;
            println!("{}", serde_json::to_string_pretty(&view)?);
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
    if schema_version != REPORT_SCHEMA_VERSION && schema_version != LEGACY_REPORT_SCHEMA_VERSION_V1
    {
        return Err(AppError::Validation(format!(
            "{} uses unsupported schema_version `{schema_version}`",
            path.display()
        )));
    }
    let report: DiscoveryReport = serde_json::from_value(value)?;
    if report.schema_version.0 == REPORT_SCHEMA_VERSION {
        ServiceGraph::from_report(&report)?;
    }
    Ok(())
}

fn read_service_view_report(
    report: Option<PathBuf>,
    bundle: Option<PathBuf>,
) -> Result<DiscoveryReport, AppError> {
    match (report, bundle) {
        (Some(report), None) => {
            let text = fs::read_to_string(report)?;
            let report: DiscoveryReport = serde_json::from_str(&text)?;
            if report.schema_version.0 != REPORT_SCHEMA_VERSION {
                return Err(AppError::Validation(format!(
                    "unsupported schema_version `{}`",
                    report.schema_version.0
                )));
            }
            Ok(report)
        }
        (None, Some(bundle)) => {
            let text = fs::read_to_string(bundle)?;
            let input: AnalyzeInput = serde_json::from_str(&text)?;
            Ok(analyze_artifacts(input)?)
        }
        (None, None) => Err(AppError::Cli(
            "service-view requires either --report or --bundle".to_string(),
        )),
        (Some(_), Some(_)) => Err(AppError::Cli(
            "service-view accepts either --report or --bundle, not both".to_string(),
        )),
    }
}

fn build_service_view(
    report: &DiscoveryReport,
    service_iri: &str,
) -> Result<ServiceFirstView, AppError> {
    let graph = ServiceGraph::from_report(report)?;
    let service = graph.public_service(service_iri)?;
    let service_id = service.id().to_string();
    let channels = service
        .channels()
        .into_iter()
        .map(|channel| PathAssetView {
            asset: asset_ref(&graph, channel.asset),
            relations: relation_refs(channel.relations()),
            source_evidence_refs: evidence_refs(channel.relations(), channel.claims()),
        })
        .collect();
    let requirements = service
        .requirements()
        .into_iter()
        .map(|requirement| RequirementOutput {
            asset: asset_ref(&graph, requirement.asset),
            relations: relation_refs(requirement.relations()),
            evidence_options: requirement
                .evidence_options()
                .into_iter()
                .map(|option| EvidenceOptionOutput {
                    asset: asset_ref(&graph, option.asset),
                    relations: relation_refs(option.relations()),
                    evidence_types: option
                        .evidence_types()
                        .into_iter()
                        .map(|evidence_type| PathAssetView {
                            asset: asset_ref(&graph, evidence_type.asset),
                            relations: relation_refs(evidence_type.relations()),
                            source_evidence_refs: evidence_refs(
                                evidence_type.relations(),
                                evidence_type.claims(),
                            ),
                        })
                        .collect(),
                    missing_evidence_types: option
                        .missing_evidence_types()
                        .into_iter()
                        .map(|evidence_type| asset_ref(&graph, evidence_type.asset))
                        .collect(),
                    satisfiable: option.is_satisfiable(),
                    source_evidence_refs: evidence_refs(option.relations(), option.claims()),
                })
                .collect(),
            accepted_evidence_types: requirement
                .accepted_evidence_types()
                .into_iter()
                .map(|evidence_type| PathAssetView {
                    asset: asset_ref(&graph, evidence_type.asset),
                    relations: relation_refs(evidence_type.relations()),
                    source_evidence_refs: evidence_refs(
                        evidence_type.relations(),
                        evidence_type.claims(),
                    ),
                })
                .collect(),
            source_evidence_refs: evidence_refs(requirement.relations(), requirement.claims()),
        })
        .collect();
    let accepted_evidence_types = service
        .accepted_evidence_types()
        .into_iter()
        .map(|evidence_type| PathAssetView {
            asset: asset_ref(&graph, evidence_type.asset),
            relations: relation_refs(evidence_type.relations()),
            source_evidence_refs: evidence_refs(evidence_type.relations(), evidence_type.claims()),
        })
        .collect();
    let providers = service
        .evidence_providers()
        .into_iter()
        .map(|provider| PathAssetView {
            asset: asset_ref(&graph, provider.asset),
            relations: relation_refs(provider.relations()),
            source_evidence_refs: evidence_refs(provider.relations(), provider.claims()),
        })
        .collect();
    let forms = service
        .forms()
        .into_iter()
        .map(|form| PathAssetView {
            asset: asset_ref(&graph, form.asset),
            relations: relation_refs(form.relations()),
            source_evidence_refs: evidence_refs(form.relations(), form.claims()),
        })
        .collect();
    let routes = graph
        .routes_for_service(&service_id)
        .into_iter()
        .map(|route| RouteOutput {
            kind: route_kind_name(route.route_kind).to_string(),
            service: asset_ref(&graph, route.service),
            target: asset_ref(&graph, route.target),
            relations: relation_refs(route.relations()),
            source_evidence_refs: evidence_refs(route.relations(), route.claims()),
        })
        .collect();

    Ok(ServiceFirstView {
        schema_version: "semantic-asset-discovery.service-view.v1",
        source_report_schema_version: report.schema_version.0.clone(),
        service: ServiceOutput {
            asset: asset_ref(&graph, service.asset),
            channels,
        },
        requirements,
        accepted_evidence_types,
        providers,
        routes,
        forms,
        gaps: service
            .gaps()
            .into_iter()
            .map(|gap| GapOutput {
                asset_id: gap.asset_id,
                predicate: gap.predicate,
                message: gap.message,
            })
            .collect(),
    })
}

#[derive(Debug, Serialize)]
struct ServiceFirstView {
    schema_version: &'static str,
    source_report_schema_version: String,
    service: ServiceOutput,
    requirements: Vec<RequirementOutput>,
    accepted_evidence_types: Vec<PathAssetView>,
    providers: Vec<PathAssetView>,
    routes: Vec<RouteOutput>,
    forms: Vec<PathAssetView>,
    gaps: Vec<GapOutput>,
}

#[derive(Debug, Serialize)]
struct ServiceOutput {
    asset: AssetRef,
    channels: Vec<PathAssetView>,
}

#[derive(Debug, Serialize)]
struct RequirementOutput {
    asset: AssetRef,
    relations: Vec<RelationRef>,
    evidence_options: Vec<EvidenceOptionOutput>,
    accepted_evidence_types: Vec<PathAssetView>,
    source_evidence_refs: Vec<SourceEvidenceRef>,
}

#[derive(Debug, Serialize)]
struct EvidenceOptionOutput {
    asset: AssetRef,
    relations: Vec<RelationRef>,
    evidence_types: Vec<PathAssetView>,
    missing_evidence_types: Vec<AssetRef>,
    satisfiable: bool,
    source_evidence_refs: Vec<SourceEvidenceRef>,
}

#[derive(Debug, Serialize)]
struct PathAssetView {
    asset: AssetRef,
    relations: Vec<RelationRef>,
    source_evidence_refs: Vec<SourceEvidenceRef>,
}

#[derive(Debug, Serialize)]
struct RouteOutput {
    kind: String,
    service: AssetRef,
    target: AssetRef,
    relations: Vec<RelationRef>,
    source_evidence_refs: Vec<SourceEvidenceRef>,
}

#[derive(Debug, Serialize)]
struct AssetRef {
    id: String,
    kind: String,
    iri: Option<String>,
    title: Option<String>,
    description: Option<String>,
    endpoint_url: Option<String>,
    endpoint_relation_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct RelationRef {
    id: String,
    predicate: String,
}

#[derive(Debug, Serialize)]
struct SourceEvidenceRef {
    relation_id: String,
    claim_id: String,
    asserted_by_artifact_id: String,
    location: Option<String>,
    evidence: DiscoveryEvidence,
}

#[derive(Debug, Serialize)]
struct GapOutput {
    asset_id: String,
    predicate: String,
    message: String,
}

fn asset_ref(graph: &ServiceGraph<'_>, asset: &SemanticAsset) -> AssetRef {
    let endpoint = graph.endpoint_url_for_asset(&asset.id);
    AssetRef {
        id: asset.id.clone(),
        kind: asset_kind_name(asset),
        iri: asset.uri.clone(),
        title: asset.title.clone(),
        description: asset.description.clone(),
        endpoint_url: endpoint.map(|endpoint| endpoint.url.to_string()),
        endpoint_relation_id: endpoint.map(|endpoint| endpoint.relation_id.to_string()),
    }
}

fn asset_kind_name(asset: &SemanticAsset) -> String {
    serde_json::to_value(&asset.kind)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| format!("{:?}", asset.kind))
}

fn relation_refs(relations: &[&SemanticRelation]) -> Vec<RelationRef> {
    relations
        .iter()
        .map(|relation| RelationRef {
            id: relation.id.clone(),
            predicate: relation.predicate.clone(),
        })
        .collect()
}

fn evidence_refs(
    relations: &[&SemanticRelation],
    claims: Vec<&RelationClaim>,
) -> Vec<SourceEvidenceRef> {
    let relation_ids = relations
        .iter()
        .map(|relation| relation.id.as_str())
        .collect::<Vec<_>>();
    claims
        .into_iter()
        .filter(|claim| relation_ids.contains(&claim.relation_id.as_str()))
        .map(|claim| SourceEvidenceRef {
            relation_id: claim.relation_id.clone(),
            claim_id: claim.id.clone(),
            asserted_by_artifact_id: claim.asserted_by_artifact_id.clone(),
            location: claim.evidence.location(),
            evidence: claim.evidence.clone(),
        })
        .collect()
}

fn route_kind_name(kind: semantic_asset_discovery_core::ServiceRouteKind) -> &'static str {
    match kind {
        semantic_asset_discovery_core::ServiceRouteKind::EvidenceType => "evidence_type",
        semantic_asset_discovery_core::ServiceRouteKind::EvidenceProvider => "evidence_provider",
        semantic_asset_discovery_core::ServiceRouteKind::SupportingDataService => {
            "supporting_data_service"
        }
        semantic_asset_discovery_core::ServiceRouteKind::EvidenceAccessService => {
            "evidence_access_service"
        }
        semantic_asset_discovery_core::ServiceRouteKind::Form => "form",
    }
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
    ServiceGraph(#[from] semantic_asset_discovery_core::ServiceGraphError),
    #[error(transparent)]
    Discovery(#[from] semantic_asset_discovery::DiscoveryError),
}
