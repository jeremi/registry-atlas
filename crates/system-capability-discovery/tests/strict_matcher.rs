use semantic_asset_discovery_core::{
    ArtifactKind, ArtifactStatus, DiscoveredArtifact, DiscoveryEvidence, DiscoveryFinding,
    DiscoveryReport, DiscoverySummary, FindingSeverity, SchemaVersion,
};
use system_capability_discovery::{
    CapabilityIndex, CapabilityQuery, CapabilitySource, EvidenceLocation, InformationNeed,
    MatchBasis, ReviewFlag, ReviewedMapping, ReviewedMappingSet, Term,
};

const ANALYZED_AT: &str = "2026-05-20T00:00:00Z";

#[test]
fn strict_field_match_returns_machine_evidence() {
    let index = CapabilityIndex::from_reports(vec![sample_report(vec![schema_property(
        "farmer",
        "farmerStatus",
    )])])
    .expect("index builds");
    let query = CapabilityQuery::new("program")
        .need(InformationNeed::new("farmer_status").requires_any([Term::field("farmerStatus")]));

    let result = index.search(query).expect("search succeeds");
    let matches = &result.needs[0].matches;

    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].evidence[0].claim.basis,
        MatchBasis::RequiredInformation
    );
    assert!(matches[0].evidence.iter().any(|evidence| matches!(
        evidence.location,
        Some(EvidenceLocation::SchemaProperty { .. })
    )));
}

#[test]
fn question_text_and_subject_context_do_not_create_matches() {
    let index = CapabilityIndex::from_reports(vec![sample_report(vec![schema_property(
        "farmer",
        "farmerStatus",
    )])])
    .expect("index builds");
    let query = CapabilityQuery::new("program").need(
        InformationNeed::new("farmer_status")
            .question("Where can I check school attendance?")
            .about_any([Term::label("Person")]),
    );

    let result = index.search(query).expect("search succeeds");

    assert!(result.needs[0].matches.is_empty());
}

#[test]
fn question_text_cannot_contaminate_results() {
    let index = CapabilityIndex::from_reports(vec![sample_report(vec![
        schema_property("farmer", "farmerStatus"),
        schema_property("school", "attendance_rate"),
    ])])
    .expect("index builds");
    let query = CapabilityQuery::new("program").need(
        InformationNeed::new("farmer_status")
            .question("This question mentions attendance_rate but asks for farmer status.")
            .requires_any([Term::field("farmerStatus")]),
    );

    let result = index.search(query).expect("search succeeds");
    let serialized = serde_json::to_string(&result.needs[0].matches).expect("serializes");

    assert!(serialized.contains("farmerStatus"));
    assert!(!serialized.contains("attendance_rate"));
}

#[test]
fn reviewed_mapping_is_explicit_and_flagged() {
    let report = sample_report(vec![schema_property("farmer", "farmerStatus")]);
    let source = CapabilitySource {
        id: "source-1".to_string(),
        report,
        envelope: None,
        mappings: vec![ReviewedMappingSet {
            id: "rw-agriculture".to_string(),
            version: "2026-05".to_string(),
            authority: "review-board".to_string(),
            mappings: vec![ReviewedMapping {
                id: "registered-farmer".to_string(),
                label: Some("registered farmer".to_string()),
                labels: vec!["registered farmer".to_string()],
                iris: Vec::new(),
                fields: vec!["farmerStatus".to_string()],
            }],
        }],
        review: Vec::new(),
    };
    let index = CapabilityIndex::from_sources(vec![source]).expect("index builds");
    let query =
        CapabilityQuery::new("program").need(InformationNeed::new("farmer_status").requires_any([
            Term::reviewed_mapping("rw-agriculture", "registered-farmer"),
        ]));

    let result = index.search(query).expect("search succeeds");

    assert!(!result.needs[0].matches.is_empty());
    assert!(result.needs[0]
        .matches
        .iter()
        .any(|item| item.review_flags.contains(&ReviewFlag::ReviewedMappingUsed)));
    assert!(result
        .inputs_summary
        .reviewed_mapping_sets
        .contains(&"rw-agriculture@2026-05".to_string()));
}

#[test]
fn removing_exact_evidence_invalidates_match() {
    let index = CapabilityIndex::from_reports(vec![sample_report(vec![schema_property(
        "farmer",
        "farmerStatus",
    )])])
    .expect("index builds");
    let empty_index =
        CapabilityIndex::from_reports(vec![sample_report(Vec::new())]).expect("index builds");
    let query = CapabilityQuery::new("program")
        .need(InformationNeed::new("farmer_status").requires_any([Term::field("farmerStatus")]));

    assert_eq!(
        index.search(query.clone()).expect("search succeeds").needs[0]
            .matches
            .len(),
        1
    );
    assert!(empty_index.search(query).expect("search succeeds").needs[0]
        .matches
        .is_empty());
}

fn sample_report(findings: Vec<DiscoveryFinding>) -> DiscoveryReport {
    DiscoveryReport {
        schema_version: SchemaVersion::default(),
        run_id: "report-1".to_string(),
        entry_url: "https://example.gov/catalog".to_string(),
        analyzed_at: ANALYZED_AT.to_string(),
        summary: DiscoverySummary::default(),
        artifacts: vec![DiscoveredArtifact {
            id: "artifact-1".to_string(),
            url: "https://example.gov/schema.json".to_string(),
            final_url: None,
            kind: ArtifactKind::JsonSchema,
            status: ArtifactStatus::Fetched,
            media_type: Some("application/schema+json".to_string()),
            http_status: Some(200),
            title: Some("Farmer schema".to_string()),
            description: None,
            discovered_from: None,
            discovered_by: None,
            byte_length: Some(128),
            hash: None,
            error: None,
            analyzed_at: ANALYZED_AT.to_string(),
        }],
        assets: Vec::new(),
        relations: Vec::new(),
        relation_claims: Vec::new(),
        links: Vec::new(),
        standards: Vec::new(),
        profiles: Vec::new(),
        findings,
        next_fetches: Vec::new(),
    }
}

fn schema_property(id: &str, field: &str) -> DiscoveryFinding {
    DiscoveryFinding {
        id: format!("finding-{id}-{field}"),
        severity: FindingSeverity::Info,
        code: "semantic.schema_property".to_string(),
        message: "JSON Schema property evidence".to_string(),
        artifact_id: Some("artifact-1".to_string()),
        asset_id: None,
        standard_iri: None,
        evidence: Some(DiscoveryEvidence::SchemaProperty {
            artifact_id: "artifact-1".to_string(),
            schema_pointer: format!("/properties/{field}"),
            property_path: field.to_string(),
            property_name: field.to_string(),
            value: Some(field.to_string()),
        }),
    }
}
