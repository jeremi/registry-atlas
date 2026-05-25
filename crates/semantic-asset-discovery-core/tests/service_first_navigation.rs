use semantic_asset_discovery_core::{
    analyze_artifacts, AnalyzeInput, AnalyzeOptions, DiscoveryReport, FetchedArtifact,
    RelationEndpoint, SemanticAssetKind, ServiceGraph, ServiceGraphGap, REPORT_SCHEMA_VERSION,
};
use std::fs;
use std::path::{Path, PathBuf};

const FIXTURE: &str = "../../fixtures/service-first/health-linked-child-support.cpsv-ap.jsonld";
const SERVICE_IRI: &str = "https://demo.example.gov/services/health-linked-child-support";
const EVIDENCE_TYPE_IRI: &str = "https://demo.example.gov/evidence-types/civil-child-status";

#[test]
fn parses_service_first_assets_relations_and_claims() {
    let report = analyze_service_first_fixture();

    assert_eq!(report.schema_version.0, REPORT_SCHEMA_VERSION);
    assert!(report
        .assets
        .iter()
        .any(|asset| asset.kind == SemanticAssetKind::PublicService
            && asset.uri.as_deref() == Some(SERVICE_IRI)));
    for kind in [
        SemanticAssetKind::Channel,
        SemanticAssetKind::Requirement,
        SemanticAssetKind::EvidenceTypeList,
        SemanticAssetKind::EvidenceType,
        SemanticAssetKind::InformationConcept,
        SemanticAssetKind::EvidenceOffering,
        SemanticAssetKind::PublicOrganisation,
        SemanticAssetKind::PublicRegistryService,
        SemanticAssetKind::FormDefinition,
        SemanticAssetKind::FormField,
        SemanticAssetKind::Dataset,
        SemanticAssetKind::DataService,
    ] {
        assert!(
            report.assets.iter().any(|asset| asset.kind == kind),
            "missing asset kind {kind:?}"
        );
    }

    assert_eq!(
        report
            .assets
            .iter()
            .filter(|asset| asset.kind == SemanticAssetKind::Requirement)
            .count(),
        4,
        "cv:Requirement and cccev:Requirement aliases should canonicalize to one requirement asset"
    );
    for predicate in [
        "cv:hasChannel",
        "cv:hasCompetentAuthority",
        "cv:holdsRequirement",
        "cccev:hasConcept",
        "cccev:hasEvidenceTypeList",
        "cccev:specifiesEvidenceType",
        "dcat:service",
        "dcat:servesDataset",
        "dcat:endpointURL",
        "dcat:landingPage",
        "dcterms:conformsTo",
        "dcterms:hasPart",
        "registry_manifest:evidenceType",
        "registry_manifest:evidenceService",
        "registry_manifest:issuingAuthority",
        "registry_manifest:hasForm",
    ] {
        assert!(
            report
                .relations
                .iter()
                .any(|relation| relation.predicate == predicate),
            "missing relation predicate {predicate}"
        );
    }

    assert!(!report.relations.is_empty());
    assert!(
        report
            .relations
            .iter()
            .all(|relation| relation.predicate != "registry_manifest:accessKind"),
        "literal access-kind values must not become relation endpoints"
    );
    for relation in &report.relations {
        assert!(
            report
                .relation_claims
                .iter()
                .any(|claim| claim.relation_id == relation.id),
            "relation {} has no claim",
            relation.id
        );
    }
    assert!(report.relations.iter().any(|relation| matches!(
        (&relation.subject, &relation.object),
        (
            RelationEndpoint::Asset { asset_id: subject_id, .. },
            RelationEndpoint::Asset { asset_id: object_id, .. }
        ) if subject_id != object_id && relation.predicate == "cv:holdsRequirement"
    )));
}

#[test]
fn service_graph_navigates_public_service_requirements_evidence_and_routes() {
    let report = analyze_service_first_fixture();
    let graph = ServiceGraph::from_report(&report).expect("claimed relation graph");
    let service = graph.public_service(SERVICE_IRI).expect("service by IRI");

    assert_eq!(service.channels().len(), 1);
    assert_eq!(service.requirements().len(), 3);
    assert_eq!(service.accepted_evidence_types().len(), 3);
    assert_eq!(
        service.accepted_evidence_types()[0].asset.uri.as_deref(),
        Some(EVIDENCE_TYPE_IRI)
    );
    assert_eq!(service.evidence_providers().len(), 3);
    assert_eq!(service.data_services().len(), 1);
    assert_eq!(service.forms().len(), 1);
    assert!(service.gaps().is_empty());
    assert_eq!(service.projection().gaps.len(), 0);
    assert_eq!(service.projection().data_service_ids.len(), 1);

    let evidence_type = graph
        .evidence_type(EVIDENCE_TYPE_IRI)
        .expect("evidence type by IRI");
    assert_eq!(
        evidence_type.asset.title.as_deref(),
        Some("Civil child status evidence")
    );
    assert_eq!(evidence_type.evidence_offerings().len(), 1);
    assert_eq!(
        evidence_type.evidence_offerings()[0]
            .access_services()
            .len(),
        1
    );
    assert_eq!(evidence_type.public_services().len(), 1);

    let routes = graph.routes_for_service(service.id());
    assert!(routes.len() >= 7);
    assert!(routes.iter().any(|route| {
        route.route_kind == semantic_asset_discovery_core::ServiceRouteKind::SupportingDataService
    }));
    assert!(routes.iter().all(|route| !route.relations().is_empty()));
    assert!(routes.iter().all(|route| route
        .evidence()
        .iter()
        .all(|evidence| evidence.location().is_some())));
    let access_service = evidence_type.evidence_offerings()[0].access_services()[0].asset;
    assert!(graph.endpoint_url_for_asset(&access_service.id).is_some());
}

#[test]
fn service_graph_endpoint_projection_requires_endpoint_relation() {
    let mut report = analyze_service_first_fixture();
    let graph = ServiceGraph::from_report(&report).expect("graph builds");
    let service = graph.public_service(SERVICE_IRI).expect("service by IRI");
    let access_service =
        service.accepted_evidence_types()[0].evidence_offerings()[0].access_services()[0].asset;
    let access_service_id = access_service.id.clone();
    assert!(graph.endpoint_url_for_asset(&access_service_id).is_some());

    remove_relations_with_predicates(&mut report, &["dcat:endpointURL"]);
    let graph = ServiceGraph::from_report(&report).expect("graph builds without endpoint edges");
    let service = graph.public_service(SERVICE_IRI).expect("service by IRI");
    let access_service =
        service.accepted_evidence_types()[0].evidence_offerings()[0].access_services()[0].asset;

    assert!(
        graph.endpoint_url_for_asset(&access_service.id).is_none(),
        "endpoint hints must be projected from dcat:endpointURL relation evidence"
    );
}

#[test]
fn service_graph_resolves_reverse_edges_across_artifact_order() {
    let offering_first = r#"{
      "@context": {
        "dcat": "http://www.w3.org/ns/dcat#",
        "dcterms": "http://purl.org/dc/terms/",
        "registry_manifest": "https://registry-manifest.dev/ns/v1#"
      },
      "@id": "https://example.test/catalog",
      "@type": "dcat:Catalog",
      "@graph": [
        {
          "@id": "https://example.test/offerings/proof",
          "@type": "registry_manifest:EvidenceOffering",
          "registry_manifest:evidenceType": {"@id": "https://example.test/evidence-types/proof"},
          "registry_manifest:providedBy": {"@id": "https://example.test/providers/registry"}
        },
        {
          "@id": "https://example.test/providers/registry",
          "@type": "registry_manifest:EvidenceProvider",
          "dcterms:title": "Registry"
        }
      ]
    }"#;
    let evidence_later = r#"{
      "@context": {
        "cccev": "http://data.europa.eu/m8g/",
        "dcterms": "http://purl.org/dc/terms/"
      },
      "@id": "https://example.test/evidence-types/proof",
      "@type": "cccev:EvidenceType",
      "dcterms:title": "Proof"
    }"#;
    let report = analyze_artifacts(AnalyzeInput {
        entry_url: "https://example.test/metadata".to_string(),
        analyzed_at: Some("2026-05-25T00:00:00Z".to_string()),
        artifacts: vec![
            fetched(
                "https://example.test/offerings",
                "application/ld+json",
                offering_first,
            ),
            fetched(
                "https://example.test/evidence-types",
                "application/ld+json",
                evidence_later,
            ),
        ],
        options: AnalyzeOptions::default(),
    })
    .expect("ordered artifacts analyze");
    let graph = ServiceGraph::from_report(&report).expect("graph builds");
    let evidence_type = graph
        .evidence_type("https://example.test/evidence-types/proof")
        .expect("evidence type found");

    assert_eq!(evidence_type.evidence_offerings().len(), 1);
    assert_eq!(evidence_type.providers().len(), 1);
}

#[test]
fn service_graph_preserves_grouped_evidence_type_list_semantics() {
    let grouped = r#"{
      "@context": {
        "cccev": "http://data.europa.eu/m8g/",
        "cv": "http://data.europa.eu/m8g/",
        "cpsv": "http://purl.org/vocab/cpsv#",
        "dcat": "http://www.w3.org/ns/dcat#",
        "dcterms": "http://purl.org/dc/terms/",
        "registry_manifest": "https://registry-manifest.dev/ns/v1#"
      },
      "@id": "https://example.test/metadata/cpsv-ap",
      "@type": "dcat:Catalog",
      "@graph": [
        {
          "@id": "https://example.test/services/family-benefit",
          "@type": "cpsv:PublicService",
          "dcterms:title": "Family benefit",
          "cv:hasChannel": {"@id": "https://example.test/services/family-benefit/channels/online"},
          "cv:holdsRequirement": {"@id": "https://example.test/requirements/child-proof"}
        },
        {
          "@id": "https://example.test/services/family-benefit/channels/online",
          "@type": "cv:Channel",
          "dcterms:title": "Online"
        },
        {
          "@id": "https://example.test/requirements/child-proof",
          "@type": "cccev:Requirement",
          "dcterms:title": "Child proof",
          "cccev:hasEvidenceTypeList": [
            {"@id": "https://example.test/requirements/child-proof#birth-and-residence"},
            {"@id": "https://example.test/requirements/child-proof#national-card"}
          ]
        },
        {
          "@id": "https://example.test/requirements/child-proof#birth-and-residence",
          "@type": "cccev:EvidenceTypeList",
          "dcterms:title": "Birth certificate and proof of residence",
          "cccev:specifiesEvidenceType": [
            {"@id": "https://example.test/evidence-types/birth-certificate"},
            {"@id": "https://example.test/evidence-types/proof-of-residence"}
          ]
        },
        {
          "@id": "https://example.test/requirements/child-proof#national-card",
          "@type": "cccev:EvidenceTypeList",
          "dcterms:title": "National card",
          "cccev:specifiesEvidenceType": {"@id": "https://example.test/evidence-types/national-card"}
        },
        {
          "@id": "https://example.test/evidence-types/birth-certificate",
          "@type": "cccev:EvidenceType",
          "dcterms:title": "Birth certificate"
        },
        {
          "@id": "https://example.test/evidence-types/proof-of-residence",
          "@type": "cccev:EvidenceType",
          "dcterms:title": "Proof of residence"
        },
        {
          "@id": "https://example.test/evidence-types/national-card",
          "@type": "cccev:EvidenceType",
          "dcterms:title": "National card"
        },
        {
          "@id": "https://example.test/offerings/birth-certificate",
          "@type": "registry_manifest:EvidenceOffering",
          "registry_manifest:evidenceType": {"@id": "https://example.test/evidence-types/birth-certificate"},
          "registry_manifest:providedBy": {"@id": "https://example.test/providers/civil-registry"},
          "registry_manifest:evidenceService": {"@id": "https://example.test/data-services/birth-certificate"}
        },
        {
          "@id": "https://example.test/offerings/proof-of-residence",
          "@type": "registry_manifest:EvidenceOffering",
          "registry_manifest:evidenceType": {"@id": "https://example.test/evidence-types/proof-of-residence"},
          "registry_manifest:providedBy": {"@id": "https://example.test/providers/residence-registry"},
          "registry_manifest:evidenceService": {"@id": "https://example.test/data-services/proof-of-residence"}
        },
        {
          "@id": "https://example.test/providers/civil-registry",
          "@type": "registry_manifest:EvidenceProvider",
          "dcterms:title": "Civil Registry"
        },
        {
          "@id": "https://example.test/providers/residence-registry",
          "@type": "registry_manifest:EvidenceProvider",
          "dcterms:title": "Residence Registry"
        },
        {
          "@id": "https://example.test/data-services/birth-certificate",
          "@type": "dcat:DataService",
          "dcterms:title": "Birth certificate API",
          "dcat:endpointURL": "https://example.test/api/birth"
        },
        {
          "@id": "https://example.test/data-services/proof-of-residence",
          "@type": "dcat:DataService",
          "dcterms:title": "Residence API",
          "dcat:endpointURL": "https://example.test/api/residence"
        }
      ]
    }"#;
    let report = analyze_artifacts(AnalyzeInput {
        entry_url: "https://example.test/metadata/cpsv-ap".to_string(),
        analyzed_at: Some("2026-05-25T00:00:00Z".to_string()),
        artifacts: vec![fetched(
            "https://example.test/metadata/cpsv-ap",
            "application/ld+json",
            grouped,
        )],
        options: AnalyzeOptions::default(),
    })
    .expect("grouped evidence fixture analyzes");

    let graph = ServiceGraph::from_report(&report).expect("graph builds");
    let service = graph
        .public_service("https://example.test/services/family-benefit")
        .expect("service by IRI");
    let requirement = service.requirements().remove(0);
    let options = requirement.evidence_options();

    assert_eq!(options.len(), 2);
    assert_eq!(options[0].evidence_types().len(), 2);
    assert_eq!(options[1].evidence_types().len(), 1);
    assert!(options[0].is_satisfiable());
    assert!(!options[1].is_satisfiable());
    assert_eq!(
        options[1].missing_evidence_types()[0].asset.uri.as_deref(),
        Some("https://example.test/evidence-types/national-card")
    );

    let projection = service.projection();
    assert_eq!(projection.evidence_requirements.len(), 1);
    assert_eq!(projection.evidence_requirements[0].option_groups.len(), 2);
    assert!(projection.evidence_requirements[0].option_groups[0].satisfiable);
    assert_eq!(
        projection.evidence_requirements[0].option_groups[1]
            .missing_evidence_type_ids
            .len(),
        1
    );
}

#[test]
fn service_graph_reports_gap_when_evidence_type_has_no_declared_offering() {
    let gaps = gaps_without_predicates(&["registry_manifest:evidenceType"]);

    assert_gap(
        &gaps,
        "registry_manifest:evidenceType",
        "Evidence type has no discovered evidence offering.",
    );
}

#[test]
fn service_graph_reports_gap_when_offering_has_no_declared_provider() {
    let gaps = gaps_without_predicates(&[
        "registry_manifest:evidenceProvider",
        "registry_manifest:providedBy",
        "registry_manifest:issuingAuthority",
    ]);

    assert_gap(
        &gaps,
        "registry_manifest:providedBy",
        "Evidence offering has no declared evidence provider.",
    );
}

#[test]
fn service_graph_reports_gap_when_offering_has_no_declared_access_service() {
    let gaps =
        gaps_without_predicates(&["registry_manifest:evidenceService", "dcat:accessService"]);

    assert_gap(
        &gaps,
        "registry_manifest:evidenceService",
        "Evidence offering has no declared access data service.",
    );
}

#[test]
fn service_graph_reports_gap_when_requirement_has_no_declared_evidence_type() {
    let gaps = gaps_without_predicates(&[
        "cccev:hasEvidenceTypeList",
        "registry_manifest:acceptedEvidenceType",
    ]);

    assert_gap(
        &gaps,
        "cccev:hasEvidenceTypeList",
        "Requirement has no declared evidence type list.",
    );
}

fn analyze_service_first_fixture() -> semantic_asset_discovery_core::DiscoveryReport {
    analyze_artifacts(AnalyzeInput {
        entry_url: "https://demo.example.gov/metadata/cpsv-ap".to_string(),
        analyzed_at: Some("2026-05-25T00:00:00Z".to_string()),
        artifacts: vec![FetchedArtifact {
            url: "https://demo.example.gov/metadata/cpsv-ap".to_string(),
            final_url: None,
            status: 200,
            media_type: Some("application/ld+json".to_string()),
            request_accept: None,
            redirect_chain: Vec::new(),
            headers: Vec::new(),
            body: fs::read(fixture_path()).expect("service-first fixture exists"),
            fetched_at: "2026-05-25T00:00:00Z".to_string(),
            depth: 0,
            discovered_from: None,
            discovered_by: None,
        }],
        options: AnalyzeOptions::default(),
    })
    .expect("fixture should analyze")
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

fn gaps_without_predicates(predicates: &[&str]) -> Vec<ServiceGraphGap> {
    let mut report = analyze_service_first_fixture();
    remove_relations_with_predicates(&mut report, predicates);
    let graph = ServiceGraph::from_report(&report).expect("graph builds without selected edges");
    graph
        .public_service(SERVICE_IRI)
        .expect("service by IRI")
        .gaps()
}

fn remove_relations_with_predicates(report: &mut DiscoveryReport, predicates: &[&str]) {
    report
        .relations
        .retain(|relation| !predicates.contains(&relation.predicate.as_str()));
}

fn assert_gap(gaps: &[ServiceGraphGap], predicate: &str, message: &str) {
    assert!(
        gaps.iter()
            .any(|gap| gap.predicate == predicate && gap.message == message),
        "missing gap predicate {predicate}; gaps: {gaps:#?}"
    );
}

fn fetched(url: &str, media_type: &str, body: &str) -> FetchedArtifact {
    FetchedArtifact {
        url: url.to_string(),
        final_url: None,
        status: 200,
        media_type: Some(media_type.to_string()),
        request_accept: None,
        redirect_chain: Vec::new(),
        headers: Vec::new(),
        body: body.as_bytes().to_vec(),
        fetched_at: "2026-05-25T00:00:00Z".to_string(),
        depth: 0,
        discovered_from: None,
        discovered_by: None,
    }
}
