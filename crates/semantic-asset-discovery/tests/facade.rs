use semantic_asset_discovery::semantic_asset_discovery_core::{
    ArtifactKind, ArtifactStatus, DiscoveredArtifact, DiscoveredLink, DiscoveryEvidence,
    DiscoveryReport, DiscoverySummary, HeaderPair, LinkConfidence, ProfileClaim, SchemaVersion,
    SemanticAsset, SemanticAssetKind, SourceHint, StandardClaim,
};
use semantic_asset_discovery::{
    ConditionStatus, DiscoveryClient, DiscoveryError, DiscoveryFetcher, DiscoveryPolicy,
    DiscoveryRequest, DiscoveryRun, DiscoveryRunEnvelope, FetchRequest, FetchResponse,
    FetchSummary, RejectedFetch,
};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

#[test]
fn discovery_request_defaults_are_safe_and_serializable() {
    let request: DiscoveryRequest =
        serde_json::from_str(r#"{"entry_url":"https://publisher.example/catalog"}"#)
            .expect("request should deserialize with defaults");

    assert_eq!(
        request.policy,
        semantic_asset_discovery::DiscoveryPolicyName::PublicWeb
    );
    assert_eq!(request.max_depth, 2);
    assert_eq!(request.max_fetches, 50);
    assert_eq!(request.max_body_bytes, 8_388_608);
    assert_eq!(request.max_total_bytes, 67_108_864);
    assert_eq!(request.max_concurrent_fetches, 8);
    assert_eq!(request.timeout_ms, 10_000);
    assert_eq!(request.total_timeout_ms, 120_000);
    assert_eq!(request.accepted_schemes, ["http", "https"]);
    assert!(request.user_agent.is_some());

    let value = serde_json::to_value(&request).expect("request should serialize");
    assert!(value
        .get("allowed_origins")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    assert!(value.get("credentials").is_none());
}

#[test]
fn entry_policy_rejection_returns_fetch_rejected_and_redacts_url() {
    let error = run_async(async {
        DiscoveryClient::new()
            .discover("https://user:pass@127.0.0.1/catalog?api_key=secret")
            .await
    })
    .expect_err("embedded credentials must be rejected before fetch");

    match error {
        DiscoveryError::FetchRejected {
            reason_code,
            rejected,
            ..
        } => {
            assert_eq!(reason_code, "policy.embedded_credentials");
            assert_eq!(rejected.reason_code, "policy.embedded_credentials");
            assert!(!rejected.url.contains("user:pass"));
            assert!(!rejected.url.contains("secret"));
            assert!(rejected.url.contains("api_key=REDACTED"));
            assert!(!rejected.credential_sent);
        }
        other => panic!("expected FetchRejected, got {other:?}"),
    }
}

#[test]
fn public_policy_blocks_ipv4_mapped_ipv6_loopback() {
    let error = run_async(async {
        DiscoveryClient::new()
            .discover("http://[::ffff:127.0.0.1]/metadata")
            .await
    })
    .expect_err("IPv4-mapped loopback must be rejected before fetch");

    match error {
        DiscoveryError::FetchRejected { reason_code, .. } => {
            assert_eq!(reason_code, "policy.private_network_blocked");
        }
        other => panic!("expected FetchRejected, got {other:?}"),
    }
}

#[test]
fn follow_up_body_limit_returns_partial_run_with_rejected_fetch() {
    let fetcher = Arc::new(MockFetcher::new([
        (
            "http://127.0.0.1:48120/catalog",
            FetchResponse {
                url: "http://127.0.0.1:48120/catalog".to_string(),
                status: 200,
                headers: vec![
                    header("content-type", "text/html"),
                    header("link", r#"</big>; rel="describedby"; type="application/json""#),
                    header("set-cookie", "session=secret"),
                ],
                body: br#"<html><head><link rel="describedby" href="/big" type="application/json"></head></html>"#
                    .to_vec(),
            },
        ),
        (
            "http://127.0.0.1:48120/big",
            FetchResponse {
                url: "http://127.0.0.1:48120/big".to_string(),
                status: 200,
                headers: vec![header("content-type", "application/json")],
                body: vec![b'x'; 128],
            },
        ),
    ]));
    let client = DiscoveryClient::builder()
        .policy(DiscoveryPolicy::local_development())
        .fetcher(fetcher.clone())
        .max_depth(1)
        .max_body_bytes(96)
        .build()
        .expect("client should build");

    let run = run_async(async { client.discover("http://127.0.0.1:48120/catalog").await })
        .expect("follow-up body limit should preserve partial run");

    assert_eq!(run.fetched().fetched_count, 1);
    assert_eq!(run.rejected_fetches().len(), 1);
    assert_eq!(
        run.rejected_fetches()[0].reason_code,
        "limit.body_too_large"
    );
    assert_eq!(
        run.conditions().has_no_blocking_fetch_failures().status(),
        ConditionStatus::Warning
    );
    assert!(fetcher.requested("http://127.0.0.1:48120/big"));
}

#[test]
fn secret_discovered_urls_do_not_escape_facade_reports() {
    let fetcher = Arc::new(MockFetcher::new([(
        "http://127.0.0.1:48121/catalog",
        FetchResponse {
            url: "http://127.0.0.1:48121/catalog".to_string(),
            status: 200,
            headers: vec![header("content-type", "application/ld+json")],
            body: br#"{"@context":{"dcat":"http://www.w3.org/ns/dcat#"},"@id":"https://example.test/catalog","@type":"dcat:Catalog","dcat:dataset":"https://user:pass@example.test/dataset.jsonld?api_key=secret&ok=true"}"#.to_vec(),
        },
    )]));
    let run = run_async(async {
        DiscoveryClient::builder()
            .policy(DiscoveryPolicy::local_development())
            .fetcher(fetcher)
            .max_depth(1)
            .build()
            .expect("client should build")
            .discover("http://127.0.0.1:48121/catalog")
            .await
    })
    .expect("run should finish with rejected link finding");

    let serialized = serde_json::to_string(&run.into_envelope()).expect("envelope serializes");
    assert!(!serialized.contains("user:pass"));
    assert!(!serialized.contains("secret"));
    assert!(serialized.contains("api_key=REDACTED"));
}

#[test]
fn views_project_registry_substrate_graph_evidence_and_conditions() {
    let run = sample_run(Vec::new());
    let first_dataset = run.registry().datasets().next();

    assert_eq!(first_dataset.unwrap().title(), Some("Population dataset"));
    assert_eq!(run.registry().catalogues().count(), 1);
    assert_eq!(run.registry().services().count(), 1);
    assert_eq!(run.registry().semantic_models().count(), 1);
    assert!(run.substrate().catalogue().has_dcat());
    assert_eq!(run.substrate().exchange().openapi_specs().count(), 1);

    let evidence: Vec<_> = run.evidence().for_asset("asset-dataset").collect();
    assert_eq!(evidence[0].term(), "dct:title");
    assert_eq!(
        evidence[0].source_url(),
        Some("https://publisher.example/catalog.jsonld")
    );

    let edges: Vec<_> = run.graph().outgoing("artifact-catalog").collect();
    assert_eq!(edges[0].rel(), Some("describedby"));
    assert_eq!(
        edges[0].target_id_or_url(),
        "https://publisher.example/openapi.json"
    );

    let conditions = run.conditions();
    assert!(conditions.has_machine_readable_entry().is_true());
    assert!(conditions.has_registerable_asset().is_true());
    assert!(conditions.has_access_method().is_true());
    assert!(conditions.has_semantic_constraints().is_true());
    assert!(conditions.has_declared_profile().is_true());
    assert!(conditions.has_policy_signal().is_true());
    assert!(conditions.has_trust_signal().is_true());
    assert!(conditions.has_no_blocking_fetch_failures().is_true());
    assert_eq!(conditions.all().len(), 10);
}

#[test]
fn conditions_warn_for_optional_rejected_follow_up() {
    let run = sample_run(vec![RejectedFetch {
        id: "rejected-1".to_string(),
        url: "https://publisher.example/schema.json".to_string(),
        reason_code: "auth.required".to_string(),
        discovered_from: Some("https://publisher.example/catalog.jsonld".to_string()),
        credential_sent: false,
    }]);

    assert_eq!(
        run.conditions().has_no_blocking_fetch_failures().status(),
        ConditionStatus::Warning
    );
}

#[test]
fn offline_bundle_analyzes_local_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("openapi.json");
    std::fs::write(
        &path,
        r#"{"openapi":"3.1.0","info":{"title":"Example API","version":"1.0.0"},"paths":{}}"#,
    )
    .expect("fixture write");

    let run = semantic_asset_discovery::DiscoveryBundle::new("https://publisher.example/")
        .add_file(&path)
        .expect("file should be accepted")
        .analyze()
        .expect("bundle should analyze");

    assert_eq!(run.fetched().fetched_count, 1);
    assert_eq!(run.registry().services().count(), 1);
    assert!(run.rejected_fetches().is_empty());
}

fn sample_run(rejected_fetches: Vec<RejectedFetch>) -> DiscoveryRun {
    DiscoveryRun::from_envelope(DiscoveryRunEnvelope {
        report: sample_report(),
        fetched: FetchSummary {
            entry_url: "https://publisher.example/catalog.jsonld".to_string(),
            fetched_count: 2,
            rejected_count: rejected_fetches.len() as u64,
            redirect_count: 0,
            total_decompressed_bytes: 1024,
            max_total_bytes: 67_108_864,
            max_concurrent_fetches: 8,
            total_elapsed_ms: 12,
        },
        rejected_fetches,
    })
}

fn sample_report() -> DiscoveryReport {
    DiscoveryReport {
        schema_version: SchemaVersion::default(),
        run_id: "run-1".to_string(),
        entry_url: "https://publisher.example/catalog.jsonld".to_string(),
        analyzed_at: "2026-05-20T00:00:00Z".to_string(),
        summary: DiscoverySummary {
            artifact_count: 2,
            asset_count: 6,
            standard_count: 1,
            profile_count: 1,
            failed_artifact_count: 0,
            unsupported_artifact_count: 0,
            parse_error_count: 0,
            next_fetch_count: 0,
            truncated: false,
        },
        artifacts: vec![
            DiscoveredArtifact {
                id: "artifact-catalog".to_string(),
                url: "https://publisher.example/catalog.jsonld".to_string(),
                final_url: None,
                kind: ArtifactKind::DcatCatalog,
                status: ArtifactStatus::Fetched,
                media_type: Some("application/ld+json".to_string()),
                http_status: Some(200),
                title: Some("Example catalogue".to_string()),
                description: None,
                discovered_from: None,
                discovered_by: None,
                byte_length: Some(512),
                hash: None,
                error: None,
                analyzed_at: "2026-05-20T00:00:00Z".to_string(),
            },
            DiscoveredArtifact {
                id: "artifact-openapi".to_string(),
                url: "https://publisher.example/openapi.json".to_string(),
                final_url: None,
                kind: ArtifactKind::OpenApi,
                status: ArtifactStatus::Fetched,
                media_type: Some("application/json".to_string()),
                http_status: Some(200),
                title: Some("Population API".to_string()),
                description: None,
                discovered_from: Some("https://publisher.example/catalog.jsonld".to_string()),
                discovered_by: None,
                byte_length: Some(512),
                hash: None,
                error: None,
                analyzed_at: "2026-05-20T00:00:00Z".to_string(),
            },
        ],
        assets: vec![
            asset(
                "asset-catalog",
                SemanticAssetKind::Catalog,
                "artifact-catalog",
                "Example catalogue",
                Some("https://publisher.example/catalog"),
            ),
            asset(
                "asset-dataset",
                SemanticAssetKind::Dataset,
                "artifact-catalog",
                "Population dataset",
                Some("https://publisher.example/datasets/population"),
            ),
            asset(
                "asset-api",
                SemanticAssetKind::ApiDescription,
                "artifact-openapi",
                "Population API",
                Some("https://publisher.example/openapi.json"),
            ),
            asset(
                "asset-shape",
                SemanticAssetKind::ShapeGraph,
                "artifact-catalog",
                "Population shape",
                Some("https://publisher.example/shapes/population"),
            ),
            asset(
                "asset-policy",
                SemanticAssetKind::Policy,
                "artifact-catalog",
                "Access policy",
                Some("https://publisher.example/policies/access"),
            ),
            asset(
                "asset-trust",
                SemanticAssetKind::TrustArtifact,
                "artifact-catalog",
                "Issuer metadata",
                Some("did:web:publisher.example"),
            ),
        ],
        links: vec![DiscoveredLink {
            id: "link-1".to_string(),
            from_artifact_id: Some("artifact-catalog".to_string()),
            from_url: "https://publisher.example/catalog.jsonld".to_string(),
            to_url: "https://publisher.example/openapi.json".to_string(),
            rel: Some("describedby".to_string()),
            predicate: None,
            role: Some("application/json".to_string()),
            confidence: LinkConfidence::Declared,
            discovered_by: DiscoveryEvidence::JsonPointer {
                artifact_id: Some("artifact-catalog".to_string()),
                pointer: "/dataset/0".to_string(),
                value: Some("https://publisher.example/openapi.json".to_string()),
            },
        }],
        standards: vec![StandardClaim {
            id: "standard-dcat".to_string(),
            iri: "https://www.w3.org/ns/dcat".to_string(),
            label: Some("DCAT".to_string()),
            version: None,
            claimed_by_artifact_id: "artifact-catalog".to_string(),
            evidence: DiscoveryEvidence::JsonLdPredicate {
                artifact_id: Some("artifact-catalog".to_string()),
                predicate: "dct:conformsTo".to_string(),
                pointer: Some("/conformsTo".to_string()),
                value: Some("https://www.w3.org/ns/dcat".to_string()),
            },
        }],
        profiles: vec![ProfileClaim {
            id: "profile-dcat-ap".to_string(),
            iri: "https://semiceu.github.io/DCAT-AP/releases/3.0.0/".to_string(),
            label: Some("DCAT-AP".to_string()),
            version: Some("3.0.0".to_string()),
            base_standard_iri: Some("https://www.w3.org/ns/dcat".to_string()),
            claimed_by_artifact_id: "artifact-catalog".to_string(),
            evidence: DiscoveryEvidence::JsonLdPredicate {
                artifact_id: Some("artifact-catalog".to_string()),
                predicate: "dct:conformsTo".to_string(),
                pointer: Some("/conformsTo".to_string()),
                value: Some("DCAT-AP".to_string()),
            },
        }],
        findings: Vec::new(),
        next_fetches: Vec::new(),
    }
}

fn asset(
    id: &str,
    kind: SemanticAssetKind,
    artifact_id: &str,
    title: &str,
    uri: Option<&str>,
) -> SemanticAsset {
    let predicate = match kind {
        SemanticAssetKind::Policy => "access_rights",
        SemanticAssetKind::TrustArtifact => "issuer",
        _ => "dct:title",
    };
    SemanticAsset {
        id: id.to_string(),
        kind,
        artifact_id: artifact_id.to_string(),
        uri: uri.map(str::to_string),
        title: Some(title.to_string()),
        description: None,
        publisher: Some("Example Publisher".to_string()),
        endpoint_url: if id == "asset-api" {
            Some("https://publisher.example/api".to_string())
        } else {
            None
        },
        conforms_to: if id == "asset-dataset" {
            vec!["https://semiceu.github.io/DCAT-AP/releases/3.0.0/".to_string()]
        } else {
            Vec::new()
        },
        source_hints: vec![SourceHint {
            label: title.to_string(),
            predicate: Some(predicate.to_string()),
            path: Some("/title".to_string()),
            artifact_id: artifact_id.to_string(),
        }],
        raw_refs: Vec::new(),
    }
}

fn header(name: &str, value: &str) -> HeaderPair {
    HeaderPair {
        name: name.to_string(),
        value: value.to_string(),
    }
}

fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("test runtime")
        .block_on(future)
}

struct MockFetcher {
    responses: HashMap<String, FetchResponse>,
    requested: Mutex<Vec<String>>,
}

impl MockFetcher {
    fn new<const N: usize>(responses: [(&str, FetchResponse); N]) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(url, response)| (url.to_string(), response))
                .collect(),
            requested: Mutex::new(Vec::new()),
        }
    }

    fn requested(&self, url: &str) -> bool {
        self.requested
            .lock()
            .unwrap()
            .iter()
            .any(|value| value == url)
    }
}

impl DiscoveryFetcher for MockFetcher {
    fn fetch<'a>(
        &'a self,
        request: FetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<FetchResponse, semantic_asset_discovery::FetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.requested.lock().unwrap().push(request.url.clone());
            self.responses
                .get(&request.url)
                .cloned()
                .ok_or_else(|| semantic_asset_discovery::FetchError::new("missing mock response"))
        })
    }
}
