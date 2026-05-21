use chrono::Utc;
use semantic_asset_discovery_core::{
    analyze_artifacts, AnalyzeInput, AnalyzeOptions, ArtifactKind, ArtifactStatus,
    DiscoveredArtifact, DiscoveredLink, DiscoveryEvidence, DiscoveryReport, FetchedArtifact,
    HeaderPair, ProfileClaim, SemanticAsset, SemanticAssetKind, StandardClaim,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::future::Future;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use url::Url;

pub use semantic_asset_discovery_core;

const DEFAULT_ACCEPT: &str =
    "application/ld+json, application/json, text/turtle, text/html;q=0.8, application/yaml;q=0.7";
const DEFAULT_MAX_DEPTH: u32 = 2;
const DEFAULT_MAX_FETCHES: u64 = 50;
const DEFAULT_MAX_BODY_BYTES: u64 = 8_388_608;
const DEFAULT_MAX_TOTAL_BYTES: u64 = 67_108_864;
const DEFAULT_MAX_CONCURRENT_FETCHES: u64 = 8;
const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_TOTAL_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_MAX_REDIRECTS: u32 = 10;

const SAFE_RESPONSE_HEADERS: &[&str] = &[
    "content-type",
    "content-length",
    "etag",
    "last-modified",
    "link",
    "location",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryPolicyName {
    PublicWeb,
    LocalDevelopment,
    #[serde(other)]
    Unknown,
}

impl Default for DiscoveryPolicyName {
    fn default() -> Self {
        Self::PublicWeb
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryRequest {
    #[serde(default)]
    pub entry_url: String,
    #[serde(default)]
    pub policy: DiscoveryPolicyName,
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default = "default_max_fetches")]
    pub max_fetches: u64,
    #[serde(default = "default_max_body_bytes")]
    pub max_body_bytes: u64,
    #[serde(default = "default_max_total_bytes")]
    pub max_total_bytes: u64,
    #[serde(default = "default_max_concurrent_fetches")]
    pub max_concurrent_fetches: u64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_total_timeout_ms")]
    pub total_timeout_ms: u64,
    #[serde(default = "default_user_agent_option")]
    pub user_agent: Option<String>,
    #[serde(default = "default_accepted_schemes")]
    pub accepted_schemes: Vec<String>,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

impl DiscoveryRequest {
    pub fn new(entry_url: impl Into<String>) -> Self {
        Self {
            entry_url: entry_url.into(),
            ..Self::default()
        }
    }
}

impl Default for DiscoveryRequest {
    fn default() -> Self {
        Self {
            entry_url: String::new(),
            policy: DiscoveryPolicyName::PublicWeb,
            max_depth: DEFAULT_MAX_DEPTH,
            max_fetches: DEFAULT_MAX_FETCHES,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_concurrent_fetches: DEFAULT_MAX_CONCURRENT_FETCHES,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            total_timeout_ms: DEFAULT_TOTAL_TIMEOUT_MS,
            user_agent: Some(default_user_agent()),
            accepted_schemes: default_accepted_schemes(),
            allowed_origins: Vec::new(),
        }
    }
}

fn default_max_depth() -> u32 {
    DEFAULT_MAX_DEPTH
}

fn default_max_fetches() -> u64 {
    DEFAULT_MAX_FETCHES
}

fn default_max_body_bytes() -> u64 {
    DEFAULT_MAX_BODY_BYTES
}

fn default_max_total_bytes() -> u64 {
    DEFAULT_MAX_TOTAL_BYTES
}

fn default_max_concurrent_fetches() -> u64 {
    DEFAULT_MAX_CONCURRENT_FETCHES
}

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

fn default_total_timeout_ms() -> u64 {
    DEFAULT_TOTAL_TIMEOUT_MS
}

fn default_accepted_schemes() -> Vec<String> {
    vec!["http".to_string(), "https".to_string()]
}

fn default_user_agent() -> String {
    format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

fn default_user_agent_option() -> Option<String> {
    Some(default_user_agent())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveryRunEnvelope {
    pub report: DiscoveryReport,
    pub fetched: FetchSummary,
    #[serde(default)]
    pub rejected_fetches: Vec<RejectedFetch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FetchSummary {
    pub entry_url: String,
    pub fetched_count: u64,
    pub rejected_count: u64,
    pub redirect_count: u64,
    pub total_decompressed_bytes: u64,
    pub max_total_bytes: u64,
    pub max_concurrent_fetches: u64,
    pub total_elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RejectedFetch {
    pub id: String,
    pub url: String,
    pub reason_code: String,
    pub discovered_from: Option<String>,
    pub credential_sent: bool,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DiscoveryError {
    #[error("invalid URL: {url}")]
    InvalidUrl {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("invalid discovery policy: {message}")]
    InvalidPolicy { message: String },
    #[error("fetch failed for {url}: {message}")]
    FetchFailed {
        url: String,
        message: String,
        rejected: Option<Box<RejectedFetch>>,
        #[source]
        source: Option<Box<dyn Error + Send + Sync>>,
    },
    #[error("fetch rejected for {url}: {reason_code}")]
    FetchRejected {
        url: String,
        reason_code: String,
        rejected: Box<RejectedFetch>,
    },
    #[error("body too large for {url}: {actual_bytes} > {limit_bytes}")]
    BodyTooLarge {
        url: String,
        actual_bytes: u64,
        limit_bytes: u64,
    },
    #[error("too many redirects for {url}: {limit}")]
    TooManyRedirects { url: String, limit: u32 },
    #[error("core analysis failed")]
    CoreAnalyze {
        #[source]
        source: semantic_asset_discovery_core::AnalyzeError,
    },
    #[error("internal discovery invariant failed: {message}")]
    Internal { message: String },
}

#[derive(Debug, Clone)]
pub struct DiscoveryPolicy {
    name: DiscoveryPolicyName,
    allow_private_network: bool,
    allow_http_downgrade: bool,
    max_redirects: u32,
}

impl DiscoveryPolicy {
    pub fn public_web() -> Self {
        Self {
            name: DiscoveryPolicyName::PublicWeb,
            allow_private_network: false,
            allow_http_downgrade: false,
            max_redirects: DEFAULT_MAX_REDIRECTS,
        }
    }

    pub fn local_development() -> Self {
        Self {
            name: DiscoveryPolicyName::LocalDevelopment,
            allow_private_network: true,
            allow_http_downgrade: true,
            max_redirects: DEFAULT_MAX_REDIRECTS,
        }
    }

    pub fn name(&self) -> DiscoveryPolicyName {
        self.name
    }
}

impl Default for DiscoveryPolicy {
    fn default() -> Self {
        Self::public_web()
    }
}

#[derive(Debug, Clone)]
pub struct Credentials {
    header: Option<HeaderPair>,
    allowed_origins: Vec<String>,
}

impl Credentials {
    pub fn none() -> Self {
        Self {
            header: None,
            allowed_origins: Vec::new(),
        }
    }

    pub fn bearer(token: impl Into<String>) -> Self {
        Self {
            header: Some(HeaderPair {
                name: "authorization".to_string(),
                value: format!("Bearer {}", token.into()),
            }),
            allowed_origins: Vec::new(),
        }
    }

    pub fn api_key_header(name: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            header: Some(HeaderPair {
                name: name.into(),
                value: token.into(),
            }),
            allowed_origins: Vec::new(),
        }
    }

    pub fn same_origin_only(self) -> Self {
        self
    }

    pub fn allowed_origins<I, S>(mut self, origins: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_origins = origins.into_iter().map(Into::into).collect();
        self
    }
}

impl Default for Credentials {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone)]
pub struct FetchRequest {
    pub url: String,
    pub headers: Vec<HeaderPair>,
    pub timeout: Duration,
    pub resolved_addrs: Vec<SocketAddr>,
    pub max_body_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct FetchResponse {
    pub url: String,
    pub status: u16,
    pub headers: Vec<HeaderPair>,
    pub body: Vec<u8>,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct FetchError {
    pub message: String,
}

impl FetchError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait DiscoveryFetcher: Send + Sync {
    fn fetch<'a>(
        &'a self,
        request: FetchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<FetchResponse, FetchError>> + Send + 'a>>;
}

#[derive(Clone)]
struct ReqwestDiscoveryFetcher;

impl ReqwestDiscoveryFetcher {
    fn new() -> Result<Self, DiscoveryError> {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| DiscoveryError::InvalidPolicy {
                message: error.to_string(),
            })?;
        Ok(Self)
    }
}

impl DiscoveryFetcher for ReqwestDiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        request: FetchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<FetchResponse, FetchError>> + Send + 'a>> {
        Box::pin(async move {
            let parsed_url =
                Url::parse(&request.url).map_err(|error| FetchError::new(error.to_string()))?;
            let domain = parsed_url
                .host_str()
                .ok_or_else(|| FetchError::new("request URL has no host"))?
                .to_string();
            let mut client_builder =
                reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
            if !request.resolved_addrs.is_empty() {
                client_builder = client_builder.resolve_to_addrs(&domain, &request.resolved_addrs);
            }
            let client = client_builder
                .build()
                .map_err(|error| FetchError::new(error.to_string()))?;
            let mut builder = client.get(parsed_url).timeout(request.timeout);
            for header in &request.headers {
                builder = builder.header(&header.name, &header.value);
            }
            let mut response = builder
                .send()
                .await
                .map_err(|error| FetchError::new(error.to_string()))?;
            let url = response.url().to_string();
            let status = response.status().as_u16();
            let headers = response
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    value.to_str().ok().map(|value| HeaderPair {
                        name: name.to_string(),
                        value: value.to_string(),
                    })
                })
                .collect();
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| FetchError::new(error.to_string()))?
            {
                if body.len() as u64 + chunk.len() as u64 > request.max_body_bytes {
                    return Err(FetchError::new("response body exceeded fetch limit"));
                }
                body.extend_from_slice(&chunk);
            }
            Ok(FetchResponse {
                url,
                status,
                headers,
                body,
            })
        })
    }
}

#[derive(Clone)]
pub struct DiscoveryClient {
    config: ClientConfig,
    fetcher: Arc<dyn DiscoveryFetcher>,
}

#[derive(Clone)]
struct ClientConfig {
    policy: DiscoveryPolicy,
    max_depth: u32,
    max_fetches: u64,
    max_body_bytes: u64,
    max_total_bytes: u64,
    max_concurrent_fetches: u64,
    timeout: Duration,
    total_timeout: Duration,
    user_agent: String,
    accepted_schemes: Vec<String>,
    credentials: Credentials,
}

pub struct DiscoveryClientBuilder {
    config: ClientConfig,
    fetcher: Option<Arc<dyn DiscoveryFetcher>>,
}

impl DiscoveryClient {
    pub fn new() -> Self {
        Self::builder()
            .build()
            .expect("default discovery client configuration is valid")
    }

    pub fn builder() -> DiscoveryClientBuilder {
        DiscoveryClientBuilder {
            config: ClientConfig::default(),
            fetcher: None,
        }
    }

    pub async fn discover(
        &self,
        entry_url: impl AsRef<str>,
    ) -> Result<DiscoveryRun, DiscoveryError> {
        let request = self.request_for(entry_url.as_ref());
        self.discover_request(request).await
    }

    pub async fn discover_request(
        &self,
        request: DiscoveryRequest,
    ) -> Result<DiscoveryRun, DiscoveryError> {
        let entry_url = parse_url(&request.entry_url)?;
        let start = Instant::now();
        let mut artifacts = Vec::new();
        let mut rejected_fetches = Vec::new();
        let mut queue = VecDeque::from([QueuedFetch {
            url: entry_url.to_string(),
            depth: 0,
            discovered_from: None,
            discovered_by: None,
        }]);
        let mut seen = HashSet::new();
        let mut redirect_count = 0_u64;
        let mut total_decompressed_bytes = 0_u64;

        while let Some(queued) = queue.pop_front() {
            if start.elapsed() >= Duration::from_millis(request.total_timeout_ms) {
                let rejected = rejected_fetch(
                    &queued.url,
                    "limit.total_timeout",
                    queued.discovered_from.clone(),
                    false,
                );
                if artifacts.is_empty() {
                    return Err(DiscoveryError::FetchFailed {
                        url: rejected.url.clone(),
                        message: "total discovery timeout elapsed".to_string(),
                        rejected: Some(Box::new(rejected)),
                        source: None,
                    });
                }
                rejected_fetches.push(rejected);
                continue;
            }
            if artifacts.len() as u64 >= request.max_fetches || queued.depth > request.max_depth {
                continue;
            }
            if !seen.insert(queued.url.clone()) {
                continue;
            }

            match self
                .fetch_artifact(
                    &request,
                    &entry_url,
                    queued.clone(),
                    &mut redirect_count,
                    &mut total_decompressed_bytes,
                )
                .await
            {
                Ok(artifact) => {
                    artifacts.push(artifact);
                    let report =
                        self.analyze(&request.entry_url, artifacts.clone(), request.max_fetches)?;
                    for candidate in report.next_fetches {
                        if queued.depth < request.max_depth
                            && !seen.contains(&candidate.url)
                            && (artifacts.len() as u64) + (queue.len() as u64) < request.max_fetches
                        {
                            queue.push_back(QueuedFetch {
                                url: candidate.url,
                                depth: queued.depth + 1,
                                discovered_from: Some(candidate.discovered_from),
                                discovered_by: Some(candidate.discovered_by),
                            });
                        }
                    }
                }
                Err(FetchOutcome::Rejected(rejected)) => {
                    if queued.depth == 0 && artifacts.is_empty() {
                        return Err(DiscoveryError::FetchRejected {
                            url: rejected.url.clone(),
                            reason_code: rejected.reason_code.clone(),
                            rejected: Box::new(rejected),
                        });
                    }
                    rejected_fetches.push(rejected);
                }
                Err(FetchOutcome::Failed {
                    rejected,
                    message,
                    source,
                }) => {
                    if queued.depth == 0 && artifacts.is_empty() {
                        return Err(DiscoveryError::FetchFailed {
                            url: rejected.url.clone(),
                            message,
                            rejected: Some(Box::new(rejected)),
                            source,
                        });
                    }
                    rejected_fetches.push(rejected);
                }
                Err(FetchOutcome::BodyTooLarge {
                    url,
                    actual_bytes,
                    limit_bytes,
                    rejected,
                }) => {
                    if queued.depth == 0 && artifacts.is_empty() {
                        return Err(DiscoveryError::BodyTooLarge {
                            url,
                            actual_bytes,
                            limit_bytes,
                        });
                    }
                    rejected_fetches.push(rejected);
                }
                Err(FetchOutcome::TooManyRedirects { url, rejected }) => {
                    if queued.depth == 0 && artifacts.is_empty() {
                        return Err(DiscoveryError::TooManyRedirects {
                            url,
                            limit: request_policy(&request).max_redirects,
                        });
                    }
                    rejected_fetches.push(rejected);
                }
            }
        }

        let report = self.analyze(&request.entry_url, artifacts, request.max_fetches)?;
        let fetched = FetchSummary {
            entry_url: request.entry_url,
            fetched_count: report.summary.artifact_count,
            rejected_count: rejected_fetches.len() as u64,
            redirect_count,
            total_decompressed_bytes,
            max_total_bytes: request.max_total_bytes,
            max_concurrent_fetches: request.max_concurrent_fetches,
            total_elapsed_ms: start.elapsed().as_millis() as u64,
        };
        Ok(DiscoveryRun {
            report,
            fetched,
            rejected_fetches,
        })
    }

    fn request_for(&self, entry_url: &str) -> DiscoveryRequest {
        DiscoveryRequest {
            entry_url: entry_url.to_string(),
            policy: self.config.policy.name(),
            max_depth: self.config.max_depth,
            max_fetches: self.config.max_fetches,
            max_body_bytes: self.config.max_body_bytes,
            max_total_bytes: self.config.max_total_bytes,
            max_concurrent_fetches: self.config.max_concurrent_fetches,
            timeout_ms: self.config.timeout.as_millis() as u64,
            total_timeout_ms: self.config.total_timeout.as_millis() as u64,
            user_agent: Some(self.config.user_agent.clone()),
            accepted_schemes: self.config.accepted_schemes.clone(),
            allowed_origins: self.config.credentials.allowed_origins.clone(),
        }
    }

    async fn fetch_artifact(
        &self,
        request: &DiscoveryRequest,
        entry_url: &Url,
        queued: QueuedFetch,
        redirect_count: &mut u64,
        total_decompressed_bytes: &mut u64,
    ) -> Result<FetchedArtifact, FetchOutcome> {
        let policy = request_policy(request);
        let mut current = queued.url.clone();
        let mut redirect_chain = Vec::new();
        let mut redirects = 0_u32;

        loop {
            let parsed = match Url::parse(&current) {
                Ok(parsed) => parsed,
                Err(_) => {
                    return Err(FetchOutcome::Rejected(rejected_fetch(
                        &current,
                        "policy.invalid_url",
                        queued.discovered_from.clone(),
                        false,
                    )));
                }
            };
            if let Err(reason_code) = validate_url_policy(&parsed, request, &policy) {
                return Err(FetchOutcome::Rejected(rejected_fetch(
                    &current,
                    reason_code,
                    queued.discovered_from.clone(),
                    false,
                )));
            }
            let resolved_addrs = match resolve_url_addrs(&parsed, &policy) {
                Ok(resolved_addrs) => resolved_addrs,
                Err(reason_code) => {
                    return Err(FetchOutcome::Rejected(rejected_fetch(
                        &current,
                        reason_code,
                        queued.discovered_from.clone(),
                        false,
                    )));
                }
            };

            let credential_sent = credentials_allowed(&self.config.credentials, entry_url, &parsed);
            let mut headers = vec![
                HeaderPair {
                    name: "accept".to_string(),
                    value: DEFAULT_ACCEPT.to_string(),
                },
                HeaderPair {
                    name: "user-agent".to_string(),
                    value: request
                        .user_agent
                        .clone()
                        .unwrap_or_else(default_user_agent),
                },
            ];
            if credential_sent {
                if let Some(header) = self.config.credentials.header.clone() {
                    headers.push(header);
                }
            }

            let response = self
                .fetcher
                .fetch(FetchRequest {
                    url: current.clone(),
                    headers,
                    timeout: Duration::from_millis(request.timeout_ms),
                    resolved_addrs,
                    max_body_bytes: request.max_body_bytes,
                })
                .await
                .map_err(|error| FetchOutcome::Failed {
                    rejected: rejected_fetch(
                        &current,
                        "fetch.failed",
                        queued.discovered_from.clone(),
                        credential_sent,
                    ),
                    message: error.message,
                    source: None,
                })?;

            let final_url = match Url::parse(&response.url) {
                Ok(url) => url,
                Err(_) => {
                    return Err(FetchOutcome::Rejected(rejected_fetch(
                        &response.url,
                        "policy.invalid_final_url",
                        queued.discovered_from.clone(),
                        credential_sent,
                    )));
                }
            };
            if let Err(reason_code) = validate_url_policy(&final_url, request, &policy) {
                return Err(FetchOutcome::Rejected(rejected_fetch(
                    final_url.as_str(),
                    reason_code,
                    queued.discovered_from.clone(),
                    credential_sent,
                )));
            }
            if let Err(reason_code) = resolve_url_addrs(&final_url, &policy) {
                return Err(FetchOutcome::Rejected(rejected_fetch(
                    final_url.as_str(),
                    reason_code,
                    queued.discovered_from.clone(),
                    credential_sent,
                )));
            }

            if is_redirect(response.status) {
                redirects += 1;
                *redirect_count += 1;
                if redirects > policy.max_redirects {
                    let rejected = rejected_fetch(
                        final_url.as_str(),
                        "policy.too_many_redirects",
                        queued.discovered_from.clone(),
                        credential_sent,
                    );
                    return Err(FetchOutcome::TooManyRedirects {
                        url: rejected.url.clone(),
                        rejected,
                    });
                }
                let location = header_value(&response.headers, "location").ok_or_else(|| {
                    FetchOutcome::Rejected(rejected_fetch(
                        final_url.as_str(),
                        "policy.redirect_missing_location",
                        queued.discovered_from.clone(),
                        credential_sent,
                    ))
                })?;
                let next = final_url.join(location).map_err(|_| {
                    FetchOutcome::Rejected(rejected_fetch(
                        location,
                        "policy.invalid_redirect",
                        queued.discovered_from.clone(),
                        credential_sent,
                    ))
                })?;
                if !policy.allow_http_downgrade
                    && final_url.scheme() == "https"
                    && next.scheme() == "http"
                {
                    return Err(FetchOutcome::Rejected(rejected_fetch(
                        next.as_str(),
                        "policy.https_downgrade",
                        queued.discovered_from.clone(),
                        false,
                    )));
                }
                redirect_chain.push(redact_url(final_url.as_str()));
                current = next.to_string();
                continue;
            }

            if response.status == 401 || response.status == 403 {
                return Err(FetchOutcome::Rejected(rejected_fetch(
                    final_url.as_str(),
                    if credential_sent {
                        "auth.rejected"
                    } else {
                        "auth.required"
                    },
                    queued.discovered_from.clone(),
                    credential_sent,
                )));
            }

            let actual_bytes = response.body.len() as u64;
            if actual_bytes > request.max_body_bytes {
                let rejected = rejected_fetch(
                    final_url.as_str(),
                    "limit.body_too_large",
                    queued.discovered_from.clone(),
                    credential_sent,
                );
                return Err(FetchOutcome::BodyTooLarge {
                    url: rejected.url.clone(),
                    actual_bytes,
                    limit_bytes: request.max_body_bytes,
                    rejected,
                });
            }
            if total_decompressed_bytes.saturating_add(actual_bytes) > request.max_total_bytes {
                let rejected = rejected_fetch(
                    final_url.as_str(),
                    "limit.total_bytes",
                    queued.discovered_from.clone(),
                    credential_sent,
                );
                return Err(FetchOutcome::BodyTooLarge {
                    url: rejected.url.clone(),
                    actual_bytes: total_decompressed_bytes.saturating_add(actual_bytes),
                    limit_bytes: request.max_total_bytes,
                    rejected,
                });
            }
            *total_decompressed_bytes += actual_bytes;

            let sanitized_headers = sanitize_headers(&response.headers);
            let media_type = header_value(&sanitized_headers, "content-type").map(str::to_string);
            return Ok(FetchedArtifact {
                url: redact_url(&queued.url),
                final_url: Some(redact_url(final_url.as_str())),
                status: response.status,
                media_type,
                request_accept: Some(DEFAULT_ACCEPT.to_string()),
                redirect_chain,
                headers: sanitized_headers,
                body: response.body,
                fetched_at: Utc::now().to_rfc3339(),
                depth: queued.depth as u8,
                discovered_from: queued.discovered_from.map(|value| redact_url(&value)),
                discovered_by: queued.discovered_by,
            });
        }
    }

    fn analyze(
        &self,
        entry_url: &str,
        artifacts: Vec<FetchedArtifact>,
        max_next_fetches: u64,
    ) -> Result<DiscoveryReport, DiscoveryError> {
        analyze_artifacts(AnalyzeInput {
            entry_url: redact_url(entry_url),
            analyzed_at: None,
            artifacts,
            options: AnalyzeOptions {
                max_next_fetches,
                accepted_schemes: self.config.accepted_schemes.clone(),
                ..AnalyzeOptions::default()
            },
        })
        .map_err(|source| DiscoveryError::CoreAnalyze { source })
    }
}

impl Default for DiscoveryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveryClientBuilder {
    pub fn policy(mut self, policy: DiscoveryPolicy) -> Self {
        self.config.policy = policy;
        self
    }

    pub fn credentials(mut self, credentials: Credentials) -> Self {
        self.config.credentials = credentials;
        self
    }

    pub fn max_depth(mut self, max_depth: u32) -> Self {
        self.config.max_depth = max_depth;
        self
    }

    pub fn max_fetches(mut self, max_fetches: u64) -> Self {
        self.config.max_fetches = max_fetches;
        self
    }

    pub fn max_body_bytes(mut self, max_body_bytes: u64) -> Self {
        self.config.max_body_bytes = max_body_bytes;
        self
    }

    pub fn max_total_bytes(mut self, max_total_bytes: u64) -> Self {
        self.config.max_total_bytes = max_total_bytes;
        self
    }

    pub fn max_concurrent_fetches(mut self, max_concurrent_fetches: u64) -> Self {
        self.config.max_concurrent_fetches = max_concurrent_fetches;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    pub fn total_timeout(mut self, total_timeout: Duration) -> Self {
        self.config.total_timeout = total_timeout;
        self
    }

    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.config.user_agent = user_agent.into();
        self
    }

    pub fn accepted_schemes<I, S>(mut self, schemes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.accepted_schemes = schemes.into_iter().map(Into::into).collect();
        self
    }

    pub fn fetcher(mut self, fetcher: Arc<dyn DiscoveryFetcher>) -> Self {
        self.fetcher = Some(fetcher);
        self
    }

    pub fn build(self) -> Result<DiscoveryClient, DiscoveryError> {
        validate_config(&self.config)?;
        let fetcher = match self.fetcher {
            Some(fetcher) => fetcher,
            None => Arc::new(ReqwestDiscoveryFetcher::new()?),
        };
        Ok(DiscoveryClient {
            config: self.config,
            fetcher,
        })
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            policy: DiscoveryPolicy::public_web(),
            max_depth: DEFAULT_MAX_DEPTH,
            max_fetches: DEFAULT_MAX_FETCHES,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_concurrent_fetches: DEFAULT_MAX_CONCURRENT_FETCHES,
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
            total_timeout: Duration::from_millis(DEFAULT_TOTAL_TIMEOUT_MS),
            user_agent: default_user_agent(),
            accepted_schemes: default_accepted_schemes(),
            credentials: Credentials::none(),
        }
    }
}

#[derive(Debug, Clone)]
struct QueuedFetch {
    url: String,
    depth: u32,
    discovered_from: Option<String>,
    discovered_by: Option<DiscoveryEvidence>,
}

enum FetchOutcome {
    Rejected(RejectedFetch),
    Failed {
        rejected: RejectedFetch,
        message: String,
        source: Option<Box<dyn Error + Send + Sync>>,
    },
    BodyTooLarge {
        url: String,
        actual_bytes: u64,
        limit_bytes: u64,
        rejected: RejectedFetch,
    },
    TooManyRedirects {
        url: String,
        rejected: RejectedFetch,
    },
}

#[derive(Debug, Clone)]
pub struct DiscoveryBundle {
    entry_url: String,
    artifacts: Vec<FetchedArtifact>,
}

impl DiscoveryBundle {
    pub fn new(entry_url: impl Into<String>) -> Self {
        Self {
            entry_url: entry_url.into(),
            artifacts: Vec::new(),
        }
    }

    pub fn add_file(mut self, path: impl AsRef<Path>) -> Result<Self, DiscoveryError> {
        let path = path.as_ref();
        let body = std::fs::read(path).map_err(|error| DiscoveryError::FetchFailed {
            url: path.display().to_string(),
            message: error.to_string(),
            rejected: None,
            source: Some(Box::new(error)),
        })?;
        let url = file_artifact_url(&self.entry_url, path);
        self.artifacts.push(FetchedArtifact {
            url,
            final_url: None,
            status: 200,
            media_type: media_type_for_path(path),
            request_accept: None,
            redirect_chain: Vec::new(),
            headers: Vec::new(),
            body,
            fetched_at: Utc::now().to_rfc3339(),
            depth: 0,
            discovered_from: None,
            discovered_by: None,
        });
        Ok(self)
    }

    pub fn add_artifact(mut self, artifact: FetchedArtifact) -> Self {
        self.artifacts.push(artifact);
        self
    }

    pub fn analyze(self) -> Result<DiscoveryRun, DiscoveryError> {
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: self.entry_url.clone(),
            analyzed_at: None,
            artifacts: self.artifacts,
            options: AnalyzeOptions::default(),
        })
        .map_err(|source| DiscoveryError::CoreAnalyze { source })?;
        let fetched = FetchSummary {
            entry_url: self.entry_url,
            fetched_count: report.summary.artifact_count,
            rejected_count: 0,
            redirect_count: 0,
            total_decompressed_bytes: report
                .artifacts
                .iter()
                .filter_map(|artifact| artifact.byte_length)
                .sum(),
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_concurrent_fetches: 1,
            total_elapsed_ms: 0,
        };
        Ok(DiscoveryRun {
            report,
            fetched,
            rejected_fetches: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveryRun {
    report: DiscoveryReport,
    fetched: FetchSummary,
    rejected_fetches: Vec<RejectedFetch>,
}

impl DiscoveryRun {
    pub fn report(&self) -> &DiscoveryReport {
        &self.report
    }

    pub fn into_report(self) -> DiscoveryReport {
        self.report
    }

    pub fn fetched(&self) -> &FetchSummary {
        &self.fetched
    }

    pub fn rejected_fetches(&self) -> &[RejectedFetch] {
        &self.rejected_fetches
    }

    pub fn into_envelope(self) -> DiscoveryRunEnvelope {
        DiscoveryRunEnvelope {
            report: self.report,
            fetched: self.fetched,
            rejected_fetches: self.rejected_fetches,
        }
    }

    pub fn from_envelope(envelope: DiscoveryRunEnvelope) -> Self {
        Self {
            report: envelope.report,
            fetched: envelope.fetched,
            rejected_fetches: envelope.rejected_fetches,
        }
    }

    pub fn registry(&self) -> RegistryView<'_> {
        RegistryView { run: self }
    }

    pub fn substrate(&self) -> SubstrateView<'_> {
        SubstrateView { run: self }
    }

    pub fn graph(&self) -> GraphView<'_> {
        GraphView { run: self }
    }

    pub fn evidence(&self) -> EvidenceView<'_> {
        EvidenceView { run: self }
    }

    pub fn conditions(&self) -> ConditionView<'_> {
        ConditionView { run: self }
    }
}

impl From<DiscoveryRunEnvelope> for DiscoveryRun {
    fn from(envelope: DiscoveryRunEnvelope) -> Self {
        Self::from_envelope(envelope)
    }
}

impl From<DiscoveryRun> for DiscoveryRunEnvelope {
    fn from(run: DiscoveryRun) -> Self {
        run.into_envelope()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RegistryView<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> RegistryView<'r> {
    pub fn catalogues(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        self.assets_by_kinds(&[SemanticAssetKind::Catalog])
    }

    pub fn datasets(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        self.assets_by_kinds(&[
            SemanticAssetKind::Dataset,
            SemanticAssetKind::RecordCollection,
            SemanticAssetKind::FeatureCollection,
        ])
    }

    pub fn services(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        self.assets_by_kinds(&[
            SemanticAssetKind::DataService,
            SemanticAssetKind::ApiDescription,
        ])
    }

    pub fn distributions(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        self.assets_by_kinds(&[SemanticAssetKind::Distribution])
    }

    pub fn profiles(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        self.assets_by_kinds(&[SemanticAssetKind::Profile])
    }

    pub fn semantic_models(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        self.assets_by_kinds(&[
            SemanticAssetKind::SemanticModelPackage,
            SemanticAssetKind::ShapeGraph,
            SemanticAssetKind::ConceptScheme,
            SemanticAssetKind::Vocabulary,
            SemanticAssetKind::VocabularyTerm,
            SemanticAssetKind::Class,
            SemanticAssetKind::Property,
            SemanticAssetKind::Alignment,
            SemanticAssetKind::Crosswalk,
        ])
    }

    pub fn registerable_assets(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        self.run
            .report
            .assets
            .iter()
            .filter(|asset| is_registerable(asset))
            .map(|asset| RegistryAsset {
                run: self.run,
                asset,
            })
    }

    fn assets_by_kinds(
        &self,
        kinds: &'static [SemanticAssetKind],
    ) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        self.run
            .report
            .assets
            .iter()
            .filter(move |asset| kinds.contains(&asset.kind))
            .map(|asset| RegistryAsset {
                run: self.run,
                asset,
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RegistryAsset<'r> {
    run: &'r DiscoveryRun,
    asset: &'r SemanticAsset,
}

impl<'r> RegistryAsset<'r> {
    pub fn id(&self) -> &str {
        &self.asset.id
    }

    pub fn kind(&self) -> SemanticAssetKind {
        self.asset.kind.clone()
    }

    pub fn uri(&self) -> Option<&str> {
        self.asset.uri.as_deref()
    }

    pub fn title(&self) -> Option<&str> {
        self.asset.title.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.asset.description.as_deref()
    }

    pub fn publisher(&self) -> Option<&str> {
        self.asset.publisher.as_deref()
    }

    pub fn source_url(&self) -> Option<&str> {
        source_url_for_artifact(self.run.report(), &self.asset.artifact_id)
    }

    pub fn access_methods(&self) -> AccessMethodsView<'r> {
        AccessMethodsView {
            run: self.run,
            asset: Some(self.asset),
        }
    }

    pub fn semantics(&self) -> SemanticsFacet<'r> {
        SemanticsFacet { run: self.run }
    }

    pub fn policy(&self) -> PolicyFacet<'r> {
        PolicyFacet { run: self.run }
    }

    pub fn trust(&self) -> TrustFacet<'r> {
        TrustFacet { run: self.run }
    }

    pub fn claims(&self) -> ClaimsView<'r> {
        ClaimsView {
            run: self.run,
            asset: Some(self.asset),
        }
    }

    pub fn evidence(&self) -> impl Iterator<Item = EvidenceItem<'r>> + 'r {
        self.run.evidence().for_asset(self.id())
    }

    pub fn conditions(&self) -> impl Iterator<Item = Condition<'r>> + 'r {
        self.run.conditions().all().into_iter()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AccessMethodsView<'r> {
    run: &'r DiscoveryRun,
    asset: Option<&'r SemanticAsset>,
}

impl<'r> AccessMethodsView<'r> {
    pub fn all(&self) -> impl Iterator<Item = AccessMethod<'r>> + 'r {
        let run = self.run;
        let selected_id = self.asset.map(|asset| asset.id.as_str());
        run.report
            .assets
            .iter()
            .filter(move |asset| {
                matches!(
                    asset.kind,
                    SemanticAssetKind::DataService
                        | SemanticAssetKind::ApiDescription
                        | SemanticAssetKind::Distribution
                ) || asset.endpoint_url.is_some()
                    || (selected_id == Some(asset.id.as_str()) && asset.endpoint_url.is_some())
            })
            .map(move |asset| AccessMethod { run, asset })
    }

    pub fn api_descriptions(&self) -> impl Iterator<Item = AccessMethod<'r>> + 'r {
        self.all()
            .filter(|method| method.asset.kind == SemanticAssetKind::ApiDescription)
    }

    pub fn distributions(&self) -> impl Iterator<Item = AccessMethod<'r>> + 'r {
        self.all()
            .filter(|method| method.asset.kind == SemanticAssetKind::Distribution)
    }

    pub fn human_processes(&self) -> impl Iterator<Item = AccessMethod<'r>> + 'r {
        std::iter::empty()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AccessMethod<'r> {
    run: &'r DiscoveryRun,
    asset: &'r SemanticAsset,
}

impl<'r> AccessMethod<'r> {
    pub fn url(&self) -> Option<&str> {
        self.asset
            .endpoint_url
            .as_deref()
            .or(self.asset.uri.as_deref())
            .or_else(|| source_url_for_artifact(self.run.report(), &self.asset.artifact_id))
    }

    pub fn source_url(&self) -> Option<&str> {
        source_url_for_artifact(self.run.report(), &self.asset.artifact_id)
    }

    pub fn kind(&self) -> SemanticAssetKind {
        self.asset.kind.clone()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SemanticsFacet<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> SemanticsFacet<'r> {
    pub fn constraints(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        registry_assets(self.run).filter(|asset| {
            matches!(
                asset.asset.kind,
                SemanticAssetKind::ShapeGraph
                    | SemanticAssetKind::SemanticModelPackage
                    | SemanticAssetKind::Alignment
                    | SemanticAssetKind::Crosswalk
            )
        })
    }

    pub fn classes(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        registry_assets(self.run).filter(|asset| asset.asset.kind == SemanticAssetKind::Class)
    }

    pub fn properties(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        registry_assets(self.run).filter(|asset| asset.asset.kind == SemanticAssetKind::Property)
    }

    pub fn vocabularies(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        registry_assets(self.run).filter(|asset| {
            matches!(
                asset.asset.kind,
                SemanticAssetKind::Vocabulary
                    | SemanticAssetKind::VocabularyTerm
                    | SemanticAssetKind::ConceptScheme
            )
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PolicyFacet<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> PolicyFacet<'r> {
    pub fn access_rights(&self) -> impl Iterator<Item = PolicySignal<'r>> + 'r {
        policy_signals(self.run).filter(|signal| signal.term().contains("access"))
    }

    pub fn rights_statements(&self) -> impl Iterator<Item = PolicySignal<'r>> + 'r {
        policy_signals(self.run).filter(|signal| signal.term().contains("rights"))
    }

    pub fn policy_artifacts(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        registry_assets(self.run).filter(|asset| asset.asset.kind == SemanticAssetKind::Policy)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PolicySignal<'r> {
    asset: &'r SemanticAsset,
    hint: &'r semantic_asset_discovery_core::SourceHint,
}

impl<'r> PolicySignal<'r> {
    pub fn term(&self) -> &str {
        self.hint.predicate.as_deref().unwrap_or(&self.hint.label)
    }

    pub fn asset_id(&self) -> &str {
        &self.asset.id
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrustFacet<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> TrustFacet<'r> {
    pub fn issuers(&self) -> impl Iterator<Item = TrustSignal<'r>> + 'r {
        trust_signals(self.run).filter(|signal| signal.term().contains("issuer"))
    }

    pub fn verifiers(&self) -> impl Iterator<Item = TrustSignal<'r>> + 'r {
        trust_signals(self.run).filter(|signal| signal.term().contains("verifier"))
    }

    pub fn trust_artifacts(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        registry_assets(self.run)
            .filter(|asset| asset.asset.kind == SemanticAssetKind::TrustArtifact)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrustSignal<'r> {
    asset: &'r SemanticAsset,
    hint: &'r semantic_asset_discovery_core::SourceHint,
}

impl<'r> TrustSignal<'r> {
    pub fn term(&self) -> &str {
        self.hint.predicate.as_deref().unwrap_or(&self.hint.label)
    }

    pub fn asset_id(&self) -> &str {
        &self.asset.id
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClaimsView<'r> {
    run: &'r DiscoveryRun,
    asset: Option<&'r SemanticAsset>,
}

impl<'r> ClaimsView<'r> {
    pub fn standards(&self) -> impl Iterator<Item = StandardClaimView<'r>> + 'r {
        let run = self.run;
        let artifact_id = self.asset.map(|asset| asset.artifact_id.as_str());
        run.report
            .standards
            .iter()
            .filter(move |claim| {
                artifact_id.is_none_or(|artifact_id| claim.claimed_by_artifact_id == artifact_id)
            })
            .map(move |claim| StandardClaimView { claim })
    }

    pub fn profiles(&self) -> impl Iterator<Item = ProfileClaimView<'r>> + 'r {
        let run = self.run;
        let artifact_id = self.asset.map(|asset| asset.artifact_id.as_str());
        run.report
            .profiles
            .iter()
            .filter(move |claim| {
                artifact_id.is_none_or(|artifact_id| claim.claimed_by_artifact_id == artifact_id)
            })
            .map(move |claim| ProfileClaimView { claim })
    }

    pub fn conforms_to(&self) -> impl Iterator<Item = &'r str> + 'r {
        self.asset
            .into_iter()
            .flat_map(|asset| asset.conforms_to.iter().map(String::as_str))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StandardClaimView<'r> {
    claim: &'r StandardClaim,
}

impl<'r> StandardClaimView<'r> {
    pub fn iri(&self) -> &str {
        &self.claim.iri
    }

    pub fn label(&self) -> Option<&str> {
        self.claim.label.as_deref()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileClaimView<'r> {
    claim: &'r ProfileClaim,
}

impl<'r> ProfileClaimView<'r> {
    pub fn iri(&self) -> &str {
        &self.claim.iri
    }

    pub fn label(&self) -> Option<&str> {
        self.claim.label.as_deref()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SubstrateView<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> SubstrateView<'r> {
    pub fn catalogue(&self) -> CatalogueLayer<'r> {
        CatalogueLayer { run: self.run }
    }

    pub fn semantics(&self) -> SemanticsLayer<'r> {
        SemanticsLayer { run: self.run }
    }

    pub fn trust(&self) -> TrustLayer<'r> {
        TrustLayer { run: self.run }
    }

    pub fn policy(&self) -> PolicyLayer<'r> {
        PolicyLayer { run: self.run }
    }

    pub fn runtime_auth(&self) -> RuntimeAuthLayer<'r> {
        RuntimeAuthLayer { run: self.run }
    }

    pub fn exchange(&self) -> ExchangeLayer<'r> {
        ExchangeLayer { run: self.run }
    }

    pub fn profiles(&self) -> ProfileLayer<'r> {
        ProfileLayer { run: self.run }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CatalogueLayer<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> CatalogueLayer<'r> {
    pub fn has_dcat(&self) -> bool {
        self.run.report.artifacts.iter().any(|artifact| {
            matches!(
                artifact.kind,
                ArtifactKind::DcatCatalog | ArtifactKind::DcatProfileCatalog
            )
        }) || self
            .run
            .report
            .standards
            .iter()
            .any(|claim| claim.iri.to_ascii_lowercase().contains("dcat"))
    }

    pub fn catalogues(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        self.run.registry().catalogues()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SemanticsLayer<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> SemanticsLayer<'r> {
    pub fn constraints(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        SemanticsFacet { run: self.run }.constraints()
    }

    pub fn parser_support(&self) -> ConditionStatus {
        ConditionStatus::True
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrustLayer<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> TrustLayer<'r> {
    pub fn trust_artifacts(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        self.run.registry().registerable_assets().filter(|asset| {
            matches!(
                asset.kind(),
                SemanticAssetKind::TrustArtifact
                    | SemanticAssetKind::PrivacyBasis
                    | SemanticAssetKind::LifecycleStatus
            )
        })
    }

    pub fn parser_support(&self) -> ConditionStatus {
        ConditionStatus::Unknown
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PolicyLayer<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> PolicyLayer<'r> {
    pub fn policy_artifacts(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        PolicyFacet { run: self.run }.policy_artifacts()
    }

    pub fn parser_support(&self) -> ConditionStatus {
        ConditionStatus::Unknown
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeAuthLayer<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> RuntimeAuthLayer<'r> {
    pub fn parser_support(&self) -> ConditionStatus {
        let _ = self.run;
        ConditionStatus::Unknown
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExchangeLayer<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> ExchangeLayer<'r> {
    pub fn openapi_specs(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        registry_assets(self.run).filter(|asset| asset.kind() == SemanticAssetKind::ApiDescription)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileLayer<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> ProfileLayer<'r> {
    pub fn claims(&self) -> impl Iterator<Item = ProfileClaimView<'r>> + 'r {
        ClaimsView {
            run: self.run,
            asset: None,
        }
        .profiles()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GraphView<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> GraphView<'r> {
    pub fn artifacts(&self) -> impl Iterator<Item = ArtifactNode<'r>> + 'r {
        self.run
            .report
            .artifacts
            .iter()
            .map(|artifact| ArtifactNode { artifact })
    }

    pub fn artifact(&self, id: &str) -> Option<ArtifactNode<'r>> {
        self.run
            .report
            .artifacts
            .iter()
            .find(|artifact| artifact.id == id)
            .map(|artifact| ArtifactNode { artifact })
    }

    pub fn assets(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r {
        registry_assets(self.run)
    }

    pub fn asset(&self, id: &str) -> Option<RegistryAsset<'r>> {
        self.run
            .report
            .assets
            .iter()
            .find(|asset| asset.id == id)
            .map(|asset| RegistryAsset {
                run: self.run,
                asset,
            })
    }

    pub fn outgoing(&self, id: &str) -> impl Iterator<Item = GraphEdge<'r>> + 'r {
        let run = self.run;
        let id = id.to_string();
        run.report
            .links
            .iter()
            .filter(move |link| {
                link.from_artifact_id.as_deref() == Some(id.as_str()) || link.from_url == id
            })
            .map(move |link| GraphEdge { link })
    }

    pub fn standards(&self) -> impl Iterator<Item = StandardClaimView<'r>> + 'r {
        self.run
            .report
            .standards
            .iter()
            .map(|claim| StandardClaimView { claim })
    }

    pub fn profiles(&self) -> impl Iterator<Item = ProfileClaimView<'r>> + 'r {
        self.run
            .report
            .profiles
            .iter()
            .map(|claim| ProfileClaimView { claim })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ArtifactNode<'r> {
    artifact: &'r DiscoveredArtifact,
}

impl<'r> ArtifactNode<'r> {
    pub fn id(&self) -> &str {
        &self.artifact.id
    }

    pub fn url(&self) -> &str {
        &self.artifact.url
    }

    pub fn kind(&self) -> ArtifactKind {
        self.artifact.kind.clone()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GraphEdge<'r> {
    link: &'r DiscoveredLink,
}

impl<'r> GraphEdge<'r> {
    pub fn rel(&self) -> Option<&str> {
        self.link.rel.as_deref()
    }

    pub fn target_id_or_url(&self) -> &str {
        &self.link.to_url
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EvidenceView<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> EvidenceView<'r> {
    pub fn for_asset(&self, asset_id: &str) -> impl Iterator<Item = EvidenceItem<'r>> + 'r {
        let run = self.run;
        let asset_id = asset_id.to_string();
        run.report
            .assets
            .iter()
            .filter(move |asset| asset.id == asset_id)
            .flat_map(move |asset| {
                asset.source_hints.iter().map(move |hint| EvidenceItem {
                    run,
                    artifact_id: &hint.artifact_id,
                    term: hint.predicate.as_deref().unwrap_or(&hint.label),
                    value: hint.path.as_deref(),
                })
            })
    }

    pub fn for_condition(
        &self,
        condition_name: &str,
    ) -> impl Iterator<Item = EvidenceItem<'r>> + 'r {
        let _ = condition_name;
        std::iter::empty()
    }

    pub fn for_standard(&self, claim_id: &str) -> impl Iterator<Item = EvidenceItem<'r>> + 'r {
        let run = self.run;
        let claim_id = claim_id.to_string();
        run.report
            .standards
            .iter()
            .filter(move |claim| claim.id == claim_id)
            .map(move |claim| {
                EvidenceItem::from_evidence(run, &claim.claimed_by_artifact_id, &claim.evidence)
            })
    }

    pub fn for_profile(&self, claim_id: &str) -> impl Iterator<Item = EvidenceItem<'r>> + 'r {
        let run = self.run;
        let claim_id = claim_id.to_string();
        run.report
            .profiles
            .iter()
            .filter(move |claim| claim.id == claim_id)
            .map(move |claim| {
                EvidenceItem::from_evidence(run, &claim.claimed_by_artifact_id, &claim.evidence)
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EvidenceItem<'r> {
    run: &'r DiscoveryRun,
    artifact_id: &'r str,
    term: &'r str,
    value: Option<&'r str>,
}

impl<'r> EvidenceItem<'r> {
    fn from_evidence(
        run: &'r DiscoveryRun,
        artifact_id: &'r str,
        evidence: &'r DiscoveryEvidence,
    ) -> Self {
        match evidence {
            DiscoveryEvidence::HttpHeader {
                header_name, value, ..
            } => Self {
                run,
                artifact_id,
                term: header_name,
                value: value.as_deref(),
            },
            DiscoveryEvidence::JsonLdPredicate {
                predicate, value, ..
            } => Self {
                run,
                artifact_id,
                term: predicate,
                value: value.as_deref(),
            },
            DiscoveryEvidence::JsonPointer { pointer, value, .. } => Self {
                run,
                artifact_id,
                term: pointer,
                value: value.as_deref(),
            },
            DiscoveryEvidence::SchemaProperty {
                property_path,
                value,
                ..
            } => Self {
                run,
                artifact_id,
                term: property_path,
                value: value.as_deref(),
            },
            DiscoveryEvidence::ShaclProperty { path, value, .. } => Self {
                run,
                artifact_id,
                term: path,
                value: value.as_deref(),
            },
            DiscoveryEvidence::OpenApiOperation {
                operation_id,
                summary,
                path,
                ..
            } => Self {
                run,
                artifact_id,
                term: operation_id.as_deref().unwrap_or(path),
                value: summary.as_deref(),
            },
            DiscoveryEvidence::OgcCollection {
                collection_id,
                title,
                ..
            } => Self {
                run,
                artifact_id,
                term: collection_id,
                value: title.as_deref(),
            },
            DiscoveryEvidence::HtmlLink { rel, href, .. } => Self {
                run,
                artifact_id,
                term: rel,
                value: Some(href),
            },
            DiscoveryEvidence::UrlPattern { pattern, value, .. } => Self {
                run,
                artifact_id,
                term: pattern,
                value: Some(value),
            },
            DiscoveryEvidence::ContentSniff {
                detector, marker, ..
            } => Self {
                run,
                artifact_id,
                term: detector,
                value: Some(marker),
            },
            DiscoveryEvidence::HostPolicy { policy, value, .. } => Self {
                run,
                artifact_id,
                term: policy,
                value: value.as_deref(),
            },
        }
    }

    pub fn term(&self) -> &str {
        self.term
    }

    pub fn value(&self) -> Option<&str> {
        self.value
    }

    pub fn source_url(&self) -> Option<&str> {
        source_url_for_artifact(self.run.report(), self.artifact_id)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConditionStatus {
    True,
    False,
    Unknown,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRef<'r> {
    pub artifact_id: Option<&'r str>,
    pub asset_id: Option<&'r str>,
    pub url: Option<&'r str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition<'r> {
    pub name: &'r str,
    pub status: ConditionStatus,
    pub reason: &'r str,
    pub message: &'r str,
    pub evidence: Vec<EvidenceRef<'r>>,
}

impl<'r> Condition<'r> {
    pub fn name(&self) -> &str {
        self.name
    }

    pub fn status(&self) -> ConditionStatus {
        self.status
    }

    pub fn is_true(&self) -> bool {
        self.status == ConditionStatus::True
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConditionView<'r> {
    run: &'r DiscoveryRun,
}

impl<'r> ConditionView<'r> {
    pub fn all(&self) -> Vec<Condition<'r>> {
        vec![
            self.has_machine_readable_entry(),
            self.has_registerable_asset(),
            self.has_stable_identity(),
            self.has_human_label(),
            self.has_access_method(),
            self.has_semantic_constraints(),
            self.has_declared_profile(),
            self.has_policy_signal(),
            self.has_trust_signal(),
            self.has_no_blocking_fetch_failures(),
        ]
    }

    pub fn can_register_catalogue(&self) -> Condition<'r> {
        self.has_registerable_asset()
    }

    pub fn has_machine_readable_entry(&self) -> Condition<'r> {
        let recognized = self.run.report.artifacts.iter().any(|artifact| {
            artifact.status == ArtifactStatus::Fetched
                && !matches!(
                    artifact.kind,
                    ArtifactKind::Unknown | ArtifactKind::HtmlLandingPage
                )
        });
        condition(
            "HasMachineReadableEntry",
            bool_status(recognized),
            if recognized {
                "recognized_entry"
            } else {
                "no_recognized_entry"
            },
            if recognized {
                "The entry produced at least one recognized machine-readable artifact."
            } else {
                "No recognized machine-readable entry artifact was found."
            },
        )
    }

    pub fn has_registerable_asset(&self) -> Condition<'r> {
        let has_asset = self
            .run
            .registry()
            .registerable_assets()
            .any(|asset| asset.kind() != SemanticAssetKind::Distribution);
        condition(
            "HasRegisterableAsset",
            bool_status(has_asset),
            if has_asset {
                "registerable_asset_found"
            } else {
                "no_registerable_asset"
            },
            if has_asset {
                "At least one registry-displayable asset was discovered."
            } else {
                "No catalogue, dataset, service, profile, or semantic model was discovered."
            },
        )
    }

    pub fn has_stable_identity(&self) -> Condition<'r> {
        let assets: Vec<_> = self.run.registry().registerable_assets().collect();
        let status = if assets.is_empty() {
            ConditionStatus::False
        } else if assets.iter().all(|asset| has_stable_identity(asset.asset)) {
            ConditionStatus::True
        } else {
            ConditionStatus::Warning
        };
        condition(
            "HasStableIdentity",
            status,
            "stable_identity_check",
            "Registerable assets were checked for stable identifiers.",
        )
    }

    pub fn has_human_label(&self) -> Condition<'r> {
        let assets: Vec<_> = self.run.registry().registerable_assets().collect();
        let status = if assets.is_empty() {
            ConditionStatus::False
        } else if assets.iter().all(|asset| {
            asset.title().is_some()
                || asset
                    .asset
                    .source_hints
                    .iter()
                    .any(|hint| !hint.label.trim().is_empty())
        }) {
            ConditionStatus::True
        } else {
            ConditionStatus::Warning
        };
        condition(
            "HasHumanLabel",
            status,
            "human_label_check",
            "Registerable assets were checked for human-readable labels.",
        )
    }

    pub fn has_access_method(&self) -> Condition<'r> {
        let has_access = AccessMethodsView {
            run: self.run,
            asset: None,
        }
        .all()
        .next()
        .is_some();
        condition(
            "HasAccessMethod",
            bool_status(has_access),
            if has_access {
                "access_method_found"
            } else {
                "no_access_method"
            },
            if has_access {
                "At least one service, distribution, endpoint, or API description exists."
            } else {
                "No access method evidence was discovered."
            },
        )
    }

    pub fn has_semantic_constraints(&self) -> Condition<'r> {
        let has_constraints = self.run.report.assets.iter().any(|asset| {
            matches!(
                asset.kind,
                SemanticAssetKind::ShapeGraph
                    | SemanticAssetKind::SemanticModelPackage
                    | SemanticAssetKind::Class
                    | SemanticAssetKind::Property
                    | SemanticAssetKind::Vocabulary
                    | SemanticAssetKind::ConceptScheme
            )
        }) || self.run.report.artifacts.iter().any(|artifact| {
            matches!(
                artifact.kind,
                ArtifactKind::Shacl
                    | ArtifactKind::JsonSchema
                    | ArtifactKind::LinkMlSchema
                    | ArtifactKind::OwlOntology
            )
        });
        condition(
            "HasSemanticConstraints",
            bool_status(has_constraints),
            if has_constraints {
                "semantic_constraints_found"
            } else {
                "no_semantic_constraints"
            },
            if has_constraints {
                "Semantic structure or constraints were discovered."
            } else {
                "No semantic constraint evidence was discovered."
            },
        )
    }

    pub fn has_declared_profile(&self) -> Condition<'r> {
        let has_profile = !self.run.report.profiles.is_empty()
            || self
                .run
                .report
                .assets
                .iter()
                .any(|asset| !asset.conforms_to.is_empty());
        condition(
            "HasDeclaredProfile",
            bool_status(has_profile),
            "declared_profile_check",
            "Profile declarations were checked.",
        )
    }

    pub fn has_policy_signal(&self) -> Condition<'r> {
        let has_policy = policy_signals(self.run).next().is_some()
            || self
                .run
                .report
                .assets
                .iter()
                .any(|asset| asset.kind == SemanticAssetKind::Policy);
        condition(
            "HasPolicySignal",
            bool_status(has_policy),
            "policy_signal_check",
            "Policy and rights signals were checked.",
        )
    }

    pub fn has_trust_signal(&self) -> Condition<'r> {
        let has_trust = trust_signals(self.run).next().is_some()
            || self
                .run
                .report
                .assets
                .iter()
                .any(|asset| asset.kind == SemanticAssetKind::TrustArtifact)
            || self.run.report.artifacts.iter().any(|artifact| {
                matches!(
                    artifact.kind,
                    ArtifactKind::DidDocument | ArtifactKind::VerifiableCredential
                )
            });
        condition(
            "HasTrustSignal",
            bool_status(has_trust),
            "trust_signal_check",
            "Trust signals were checked.",
        )
    }

    pub fn has_no_blocking_fetch_failures(&self) -> Condition<'r> {
        let status = if self.run.rejected_fetches.is_empty() {
            ConditionStatus::True
        } else if self
            .run
            .rejected_fetches
            .iter()
            .any(|rejection| rejection.discovered_from.is_none())
        {
            ConditionStatus::False
        } else {
            ConditionStatus::Warning
        };
        condition(
            "HasNoBlockingFetchFailures",
            status,
            "fetch_failure_check",
            "Rejected fetches were checked for blocking discovery failures.",
        )
    }
}

fn condition<'r>(
    name: &'r str,
    status: ConditionStatus,
    reason: &'r str,
    message: &'r str,
) -> Condition<'r> {
    Condition {
        name,
        status,
        reason,
        message,
        evidence: Vec::new(),
    }
}

fn bool_status(value: bool) -> ConditionStatus {
    if value {
        ConditionStatus::True
    } else {
        ConditionStatus::False
    }
}

fn validate_config(config: &ClientConfig) -> Result<(), DiscoveryError> {
    if config.max_fetches == 0 {
        return Err(DiscoveryError::InvalidPolicy {
            message: "max_fetches must be greater than zero".to_string(),
        });
    }
    if config.max_body_bytes == 0 {
        return Err(DiscoveryError::InvalidPolicy {
            message: "max_body_bytes must be greater than zero".to_string(),
        });
    }
    if config.max_total_bytes == 0 {
        return Err(DiscoveryError::InvalidPolicy {
            message: "max_total_bytes must be greater than zero".to_string(),
        });
    }
    if config.max_concurrent_fetches == 0 {
        return Err(DiscoveryError::InvalidPolicy {
            message: "max_concurrent_fetches must be greater than zero".to_string(),
        });
    }
    if config.timeout.is_zero() || config.total_timeout.is_zero() {
        return Err(DiscoveryError::InvalidPolicy {
            message: "timeouts must be greater than zero".to_string(),
        });
    }
    if config.user_agent.trim().is_empty() {
        return Err(DiscoveryError::InvalidPolicy {
            message: "user_agent must not be empty".to_string(),
        });
    }
    if config.accepted_schemes.is_empty() {
        return Err(DiscoveryError::InvalidPolicy {
            message: "accepted_schemes must not be empty".to_string(),
        });
    }
    for origin in &config.credentials.allowed_origins {
        let parsed = Url::parse(origin).map_err(|error| DiscoveryError::InvalidUrl {
            url: origin.clone(),
            source: error,
        })?;
        if parsed.cannot_be_a_base() || parsed.host_str().is_none() {
            return Err(DiscoveryError::InvalidPolicy {
                message: format!("allowed origin `{origin}` is not an origin URL"),
            });
        }
    }
    if let Some(header) = &config.credentials.header {
        if header.name.trim().is_empty() || header.name.contains(['\r', '\n', ':']) {
            return Err(DiscoveryError::InvalidPolicy {
                message: "credential header name is invalid".to_string(),
            });
        }
    }
    Ok(())
}

fn request_policy(request: &DiscoveryRequest) -> DiscoveryPolicy {
    match request.policy {
        DiscoveryPolicyName::LocalDevelopment => DiscoveryPolicy::local_development(),
        DiscoveryPolicyName::PublicWeb | DiscoveryPolicyName::Unknown => {
            DiscoveryPolicy::public_web()
        }
    }
}

fn parse_url(value: &str) -> Result<Url, DiscoveryError> {
    Url::parse(value).map_err(|source| DiscoveryError::InvalidUrl {
        url: value.to_string(),
        source,
    })
}

fn validate_url_policy(
    url: &Url,
    request: &DiscoveryRequest,
    _policy: &DiscoveryPolicy,
) -> Result<(), &'static str> {
    if !request
        .accepted_schemes
        .iter()
        .any(|scheme| scheme.eq_ignore_ascii_case(url.scheme()))
    {
        return Err("policy.unsupported_scheme");
    }
    if !matches!(url.scheme(), "http" | "https") {
        return Err("policy.unsupported_scheme");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("policy.embedded_credentials");
    }
    Ok(())
}

fn resolve_url_addrs(url: &Url, policy: &DiscoveryPolicy) -> Result<Vec<SocketAddr>, &'static str> {
    let Some(host) = url.host() else {
        return Err("policy.missing_host");
    };
    let port = url.port_or_known_default().unwrap_or(80);
    let resolved: Vec<SocketAddr> = match host {
        url::Host::Ipv4(address) => vec![SocketAddr::new(IpAddr::V4(address), port)],
        url::Host::Ipv6(address) => vec![SocketAddr::new(IpAddr::V6(address), port)],
        url::Host::Domain(domain) => (domain, port)
            .to_socket_addrs()
            .map_err(|_| "policy.dns_failed")?
            .collect(),
    };
    if resolved.is_empty() {
        return Err("policy.dns_failed");
    }
    if !policy.allow_private_network && resolved.iter().any(|address| is_private_ip(address.ip())) {
        return Err("policy.private_network_blocked");
    }
    Ok(resolved)
}

fn is_private_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || address.is_unspecified()
                || address.is_multicast()
        }
        IpAddr::V6(address) => {
            address
                .to_ipv4_mapped()
                .is_some_and(|mapped| is_private_ip(IpAddr::V4(mapped)))
                || address.is_loopback()
                || address.is_unspecified()
                || address.is_multicast()
                || (address.segments()[0] & 0xfe00) == 0xfc00
                || (address.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

fn credentials_allowed(credentials: &Credentials, entry_url: &Url, url: &Url) -> bool {
    if credentials.header.is_none() {
        return false;
    }
    same_origin(entry_url, url)
        || credentials
            .allowed_origins
            .iter()
            .filter_map(|origin| Url::parse(origin).ok())
            .any(|origin| same_origin(&origin, url))
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn sanitize_headers(headers: &[HeaderPair]) -> Vec<HeaderPair> {
    headers
        .iter()
        .filter(|header| {
            SAFE_RESPONSE_HEADERS
                .iter()
                .any(|allowed| header.name.eq_ignore_ascii_case(allowed))
        })
        .map(|header| HeaderPair {
            name: header.name.to_ascii_lowercase(),
            value: header.value.clone(),
        })
        .collect()
}

fn header_value<'a>(headers: &'a [HeaderPair], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 300..=399)
}

fn rejected_fetch(
    url: &str,
    reason_code: impl Into<String>,
    discovered_from: Option<String>,
    credential_sent: bool,
) -> RejectedFetch {
    let redacted_url = redact_url(url);
    let reason_code = reason_code.into();
    RejectedFetch {
        id: stable_id("rejected", &[&redacted_url, &reason_code]),
        url: redacted_url,
        reason_code,
        discovered_from: discovered_from.map(|value| redact_url(&value)),
        credential_sent,
    }
}

fn redact_url(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return value.to_string();
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    let sensitive_keys = [
        "token",
        "access_token",
        "api_key",
        "apikey",
        "key",
        "secret",
        "password",
    ];
    if url.query().is_some() {
        let pairs: Vec<_> = url
            .query_pairs()
            .map(|(key, value)| {
                if sensitive_keys
                    .iter()
                    .any(|sensitive| key.eq_ignore_ascii_case(sensitive))
                {
                    (key.into_owned(), "REDACTED".to_string())
                } else {
                    (key.into_owned(), value.into_owned())
                }
            })
            .collect();
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    url.to_string()
}

fn stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("{prefix}-{hash:016x}")
}

fn file_artifact_url(entry_url: &str, path: &Path) -> String {
    if entry_url.ends_with('/') {
        format!(
            "{entry_url}{}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("artifact")
        )
    } else {
        entry_url.to_string()
    }
}

fn media_type_for_path(path: &Path) -> Option<String> {
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

fn registry_assets(run: &DiscoveryRun) -> impl Iterator<Item = RegistryAsset<'_>> {
    run.report
        .assets
        .iter()
        .map(|asset| RegistryAsset { run, asset })
}

fn policy_signals(run: &DiscoveryRun) -> impl Iterator<Item = PolicySignal<'_>> {
    run.report.assets.iter().flat_map(|asset| {
        asset
            .source_hints
            .iter()
            .filter(|hint| {
                let term = hint
                    .predicate
                    .as_deref()
                    .unwrap_or(&hint.label)
                    .to_ascii_lowercase();
                term.contains("policy") || term.contains("rights") || term.contains("access")
            })
            .map(move |hint| PolicySignal { asset, hint })
    })
}

fn trust_signals(run: &DiscoveryRun) -> impl Iterator<Item = TrustSignal<'_>> {
    run.report.assets.iter().flat_map(|asset| {
        asset
            .source_hints
            .iter()
            .filter(|hint| {
                let term = hint
                    .predicate
                    .as_deref()
                    .unwrap_or(&hint.label)
                    .to_ascii_lowercase();
                term.contains("trust")
                    || term.contains("issuer")
                    || term.contains("verifier")
                    || term.contains("did")
                    || term.contains("credential")
            })
            .map(move |hint| TrustSignal { asset, hint })
    })
}

fn is_registerable(asset: &SemanticAsset) -> bool {
    matches!(
        asset.kind,
        SemanticAssetKind::Catalog
            | SemanticAssetKind::Dataset
            | SemanticAssetKind::RecordCollection
            | SemanticAssetKind::FeatureCollection
            | SemanticAssetKind::DataService
            | SemanticAssetKind::ApiDescription
            | SemanticAssetKind::Distribution
            | SemanticAssetKind::Profile
            | SemanticAssetKind::SemanticModelPackage
            | SemanticAssetKind::ShapeGraph
            | SemanticAssetKind::ConceptScheme
            | SemanticAssetKind::Vocabulary
            | SemanticAssetKind::VocabularyTerm
            | SemanticAssetKind::Class
            | SemanticAssetKind::Property
            | SemanticAssetKind::Alignment
            | SemanticAssetKind::Crosswalk
    ) || matches!(
        asset.kind,
        SemanticAssetKind::Policy
            | SemanticAssetKind::QualityMeasurement
            | SemanticAssetKind::LifecycleStatus
            | SemanticAssetKind::PrivacyBasis
            | SemanticAssetKind::TrustArtifact
    ) && (asset.uri.is_some() || asset.title.is_some())
}

fn has_stable_identity(asset: &SemanticAsset) -> bool {
    asset.uri.as_deref().is_some_and(|uri| {
        uri.starts_with("http://")
            || uri.starts_with("https://")
            || uri.starts_with("urn:")
            || uri.starts_with("did:")
    }) || (!asset.id.is_empty() && !asset.artifact_id.is_empty())
}

fn source_url_for_artifact<'r>(report: &'r DiscoveryReport, artifact_id: &str) -> Option<&'r str> {
    report
        .artifacts
        .iter()
        .find(|artifact| artifact.id == artifact_id)
        .map(|artifact| artifact.final_url.as_deref().unwrap_or(&artifact.url))
}
