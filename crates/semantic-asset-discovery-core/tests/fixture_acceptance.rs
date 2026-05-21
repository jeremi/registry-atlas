use semantic_asset_discovery_core::{
    analyze_artifacts, AnalyzeInput, AnalyzeOptions, ArtifactKind, DiscoveryEvidence,
    FetchedArtifact, SemanticAssetKind,
};
use std::fs;
use std::path::{Path, PathBuf};

const FIXTURE_ROOT: &str = "../../fixtures/semantic-asset-discovery";

#[test]
fn accepts_catalogue_and_semic_standard_fixtures() {
    let report = analyze_fixture_bundle(vec![
        ("dcat-ap/catalog.jsonld", "application/ld+json"),
        ("breg-dcat-ap/catalog.jsonld", "application/ld+json"),
        ("prof/profile.jsonld", "application/ld+json"),
        ("semic-shacl-turtle/person-shapes.ttl", "text/turtle"),
        ("json-schema/person.schema.json", "application/schema+json"),
        ("openapi/openapi.json", "application/json"),
        ("ogc-records/landing.json", "application/json"),
        ("ogc-features/landing.json", "application/json"),
        ("ogc-features/collections.json", "application/json"),
        (
            "registry-relay-standards/catalog.jsonld",
            "application/ld+json",
        ),
    ]);

    assert!(report
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == ArtifactKind::DcatCatalog));
    assert!(report
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == ArtifactKind::ProfProfile));
    assert!(report
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == ArtifactKind::Shacl));
    assert!(report
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == ArtifactKind::JsonSchema));
    assert!(report
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == ArtifactKind::OpenApi));
    assert!(report
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == ArtifactKind::OgcRecords));
    assert!(report
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == ArtifactKind::OgcFeatures));
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::Catalog));
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::Dataset));
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::DataService));
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::ShapeGraph));
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::ApiDescription));
    assert!(report
        .profiles
        .iter()
        .any(|profile| profile.base_standard_iri.is_some()));
    assert!(report.findings.iter().any(|finding| matches!(
        finding.evidence.as_ref(),
        Some(DiscoveryEvidence::SchemaProperty {
            ref property_path,
            ref property_name,
            ..
        }) if property_path == "name" && property_name == "name"
    )));
    assert!(report.findings.iter().any(|finding| matches!(
        finding.evidence.as_ref(),
        Some(DiscoveryEvidence::OpenApiOperation {
            ref path,
            ref method,
            ref operation_id,
            ..
        }) if path == "/persons/{id}"
            && method == "get"
            && operation_id.as_deref() == Some("getPerson")
    )));
    assert!(report.findings.iter().any(|finding| matches!(
        finding.evidence.as_ref(),
        Some(DiscoveryEvidence::OgcCollection {
            ref collection_id,
            ref title,
            ..
        }) if collection_id == "community-services"
            && title.as_deref() == Some("Community services")
    )));
    assert!(report.findings.iter().any(|finding| matches!(
        finding.evidence.as_ref(),
        Some(DiscoveryEvidence::ShaclProperty {
            ref shape,
            ref path,
            ref predicate,
            ..
        }) if shape == "https://example.org/shapes/person"
            && path == "http://xmlns.com/foaf/0.1/name"
            && predicate == "sh:path"
    )));
    assert_eq!(report.summary.parse_error_count, 0);
}

#[test]
fn accepts_semantic_model_package_fixtures_without_dcat() {
    let report = analyze_fixture_bundle(vec![
        (
            "publicschema-package/semantic-asset-package.v1.toml",
            "application/toml",
        ),
        ("publicschema-package/model.linkml.yaml", "application/yaml"),
        ("publicschema-package/context.jsonld", "application/ld+json"),
        ("publicschema-package/shapes.ttl", "text/turtle"),
        ("publicschema-package/concepts.ttl", "text/turtle"),
        ("publicschema-package/alignments.ttl", "text/turtle"),
    ]);

    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::SemanticModelPackage));
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::Class));
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::Property));
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::ConceptScheme));
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::Alignment));
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::Crosswalk));
    assert!(report
        .next_fetches
        .iter()
        .any(|candidate| candidate.reason.contains("linkml")
            && candidate.url == "https://w3id.org/linkml/types"));
    assert_eq!(report.summary.parse_error_count, 0);
}

fn analyze_fixture_bundle(
    fixtures: Vec<(&str, &str)>,
) -> semantic_asset_discovery_core::DiscoveryReport {
    let artifacts = fixtures
        .into_iter()
        .map(|(relative, media_type)| fetched_artifact(relative, media_type))
        .collect();
    analyze_artifacts(AnalyzeInput {
        entry_url: "https://example.org/entry".to_string(),
        analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
        artifacts,
        options: AnalyzeOptions::default(),
    })
    .expect("fixtures should analyze")
}

fn fetched_artifact(relative: &str, media_type: &str) -> FetchedArtifact {
    let path = fixture_path(relative);
    FetchedArtifact {
        url: format!("https://example.org/{relative}"),
        final_url: None,
        status: 200,
        media_type: Some(media_type.to_string()),
        request_accept: None,
        redirect_chain: Vec::new(),
        headers: Vec::new(),
        body: fs::read(path).expect("fixture file exists"),
        fetched_at: "2026-05-19T00:00:00Z".to_string(),
        depth: 0,
        discovered_from: None,
        discovered_by: None,
    }
}

fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_ROOT)
        .join(relative)
}
