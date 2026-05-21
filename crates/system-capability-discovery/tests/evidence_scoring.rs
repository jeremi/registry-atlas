use semantic_asset_discovery_core::{
    DiscoveryEvidence, DiscoveryFinding, DiscoveryReport, DiscoverySummary, FindingSeverity,
    RawReference, SchemaVersion, SemanticAsset, SemanticAssetKind, SourceHint,
};
use system_capability_discovery::{
    AccessKind, CandidateRouteRole, CapabilityGap, CapabilityIndex, CapabilityQuery,
    EvidenceLocation, InformationNeed, MatchConfidence, Term,
};

fn report(asset: SemanticAsset) -> DiscoveryReport {
    report_with_findings(asset, Vec::new())
}

fn report_with_findings(asset: SemanticAsset, findings: Vec<DiscoveryFinding>) -> DiscoveryReport {
    DiscoveryReport {
        schema_version: SchemaVersion::default(),
        run_id: "report:score".to_string(),
        entry_url: "https://example.test/catalog".to_string(),
        analyzed_at: "2026-05-20T00:00:00Z".to_string(),
        summary: DiscoverySummary::default(),
        artifacts: Vec::new(),
        assets: vec![asset],
        links: Vec::new(),
        standards: Vec::new(),
        profiles: Vec::new(),
        findings,
        next_fetches: Vec::new(),
    }
}

fn api_asset() -> SemanticAsset {
    SemanticAsset {
        id: "farmer_api".to_string(),
        kind: SemanticAssetKind::ApiDescription,
        artifact_id: "artifact:api".to_string(),
        uri: Some("https://example.test/openapi.json".to_string()),
        title: Some("Farmer Status API".to_string()),
        description: None,
        publisher: Some("Agriculture Gateway".to_string()),
        endpoint_url: Some("https://example.test/api".to_string()),
        conforms_to: Vec::new(),
        source_hints: vec![SourceHint {
            label: "farmerStatus".to_string(),
            predicate: Some("https://example.test/ns/farmerStatus".to_string()),
            path: Some("farmerStatus".to_string()),
            artifact_id: "artifact:api".to_string(),
        }],
        raw_refs: vec![RawReference {
            artifact_id: "artifact:api".to_string(),
            pointer: Some("/components/schemas/Farmer/properties/farmerStatus".to_string()),
            subject_iri: None,
        }],
    }
}

#[test]
fn structured_match_with_access_scores_high() {
    let index = CapabilityIndex::from_reports(vec![report(api_asset())]).unwrap();
    let result = index
        .search(CapabilityQuery::new("q").need(
            InformationNeed::new("farmer_status").requires_any([Term::field("farmerStatus")]),
        ))
        .unwrap();
    let first = &result.needs[0].matches[0];

    assert_eq!(first.score.direct_structured_matches, 1);
    assert_eq!(first.score.access_evidence_matches, 1);
    assert_eq!(first.confidence, MatchConfidence::High);
    assert_eq!(first.access.kind, AccessKind::ApiDescriptionAvailable);
    assert!(first.evidence.iter().any(|evidence| matches!(
        evidence.location,
        Some(EvidenceLocation::JsonPointer { .. })
    )));
}

#[test]
fn exact_label_case_folds_but_does_not_tokenize() {
    let index = CapabilityIndex::from_reports(vec![report(api_asset())]).unwrap();
    let exact = index
        .search(
            CapabilityQuery::new("q").need(
                InformationNeed::new("farmer_status")
                    .requires_any([Term::label(" FARMER STATUS API ")]),
            ),
        )
        .unwrap();
    assert_eq!(exact.needs[0].matches.len(), 1);

    let token = index
        .search(
            CapabilityQuery::new("q")
                .need(InformationNeed::new("farmer_status").requires_any([Term::label("Farmer")])),
        )
        .unwrap();
    assert!(token.needs[0].matches.is_empty());
}

#[test]
fn standard_signals_resolve_gaps_and_upgrade_candidate_source_conservatively() {
    let asset = api_asset();
    let findings = vec![
        standard_signal(
            "legal",
            &asset.id,
            "dcatap:applicableLegislation",
            "https://example.test/law/farmers",
        ),
        standard_signal("freshness", &asset.id, "dcterms:modified", "2026-05-20"),
        standard_signal(
            "source",
            &asset.id,
            "cpsv:produces",
            "https://example.test/openapi.json",
        ),
    ];
    let index = CapabilityIndex::from_reports(vec![report_with_findings(asset, findings)]).unwrap();
    let result = index
        .search(CapabilityQuery::new("q").need(
            InformationNeed::new("farmer_status").requires_any([Term::field("farmerStatus")]),
        ))
        .unwrap();
    let first = &result.needs[0].matches[0];

    assert_eq!(first.route.role, CandidateRouteRole::CandidateSource);
    assert!(!first.gaps.contains(&CapabilityGap::AuthorityUnknown));
    assert!(!first.gaps.contains(&CapabilityGap::LegalBasisUnknown));
    assert!(!first.gaps.contains(&CapabilityGap::FreshnessUnknown));
    assert!(!first.gaps.contains(&CapabilityGap::SourceOfTruthUnknown));
    assert!(first
        .gaps
        .contains(&CapabilityGap::RequiredIdentifierUnknown));
}

fn standard_signal(id: &str, asset_id: &str, predicate: &str, value: &str) -> DiscoveryFinding {
    DiscoveryFinding {
        id: format!("finding:{id}"),
        severity: FindingSeverity::Info,
        code: "semantic.standard_signal".to_string(),
        message: format!("Standard semantic signal {predicate}"),
        artifact_id: Some("artifact:api".to_string()),
        asset_id: Some(asset_id.to_string()),
        standard_iri: None,
        evidence: Some(DiscoveryEvidence::JsonLdPredicate {
            artifact_id: Some("artifact:api".to_string()),
            predicate: predicate.to_string(),
            pointer: None,
            value: Some(value.to_string()),
        }),
    }
}
