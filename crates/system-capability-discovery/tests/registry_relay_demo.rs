use std::path::PathBuf;
use std::process::Command;
use system_capability_discovery::{
    CapabilityIndex, CapabilityQuery, DiscoveryRunEnvelope, InformationNeed, ReviewFlag, Term,
};

const ENVELOPE: &str =
    include_str!("../../../fixtures/system-capability/registry-relay-all-standards.envelope.json");

#[test]
fn registry_relay_demo_answers_core_social_protection_needs_offline() {
    let envelope = demo_envelope();
    let report = envelope.report.clone();
    let index =
        CapabilityIndex::from_sources(vec![system_capability_discovery::CapabilitySource {
            id: "registry-relay-all-standards".to_string(),
            report,
            envelope: Some(envelope),
            mappings: Vec::new(),
            review: Vec::new(),
        }])
        .expect("index builds");

    let result = index
        .search(
            CapabilityQuery::new("social_protection_program")
                .need(
                    InformationNeed::new("farmer_status")
                        .question("Is the person registered as a farmer?")
                        .requires_any([Term::label("Farmer")]),
                )
                .need(
                    InformationNeed::new("disability_status")
                        .question("Does the person have a disability status?")
                        .requires_all([
                            Term::label("Disabled Person"),
                            Term::field("disability_status"),
                        ]),
                )
                .need(
                    InformationNeed::new("school_attendance")
                        .question("Are the person's children going to school?")
                        .requires_any([Term::field("attendance_rate")]),
                ),
        )
        .expect("search succeeds");

    let farmer = result
        .needs
        .iter()
        .find(|need| need.need_id == "farmer_status")
        .unwrap();
    assert!(!farmer.matches.is_empty());
    assert!(serde_json::to_string(&farmer.matches)
        .unwrap()
        .contains("/datasets/farmer_registry/farmer"));
    assert!(farmer
        .matches
        .iter()
        .all(|item| item.access.endpoint_url.as_deref()
            != Some("http://127.0.0.1:4242/datasets/benefits_casework/case")));

    let disability = result
        .needs
        .iter()
        .find(|need| need.need_id == "disability_status")
        .unwrap();
    assert_eq!(disability.matches.len(), 1);
    assert_eq!(
        disability.matches[0].access.endpoint_url.as_deref(),
        Some("http://127.0.0.1:4242/datasets/disability_registry/disabled_person")
    );
    assert!(disability
        .matches
        .iter()
        .any(|item| item.review_flags.contains(&ReviewFlag::SensitiveData)));
    assert!(disability
        .matches
        .iter()
        .all(|item| item.access.endpoint_url.as_deref()
            != Some("http://127.0.0.1:4242/datasets/benefits_casework/case")));
    assert!(disability
        .matches
        .iter()
        .all(|item| item.access.endpoint_url.as_deref()
            != Some("http://127.0.0.1:4242/datasets/education_registry/student")));

    let attendance = result
        .needs
        .iter()
        .find(|need| need.need_id == "school_attendance")
        .unwrap();
    assert!(!attendance.matches.is_empty());
    assert!(serde_json::to_string(&attendance.matches)
        .unwrap()
        .contains("education_registry"));
    assert!(attendance
        .matches
        .iter()
        .any(|item| item.review_flags.contains(&ReviewFlag::SensitiveData)));
    assert!(attendance.matches.iter().any(|item| {
        item.access.endpoint_url.as_deref()
            == Some("http://127.0.0.1:4242/datasets/education_registry/attendance_summary")
    }));
}

#[test]
fn registry_relay_question_text_does_not_contaminate_strict_results() {
    let report = demo_envelope().report;
    let index = CapabilityIndex::from_reports(vec![report]).expect("index builds");
    let result = index
        .search(
            CapabilityQuery::new("strict").need(
                InformationNeed::new("farmer_status")
                    .question("This mentions attendance_rate but only accepts farmer status.")
                    .requires_any([Term::label("Farmer")]),
            ),
        )
        .expect("search succeeds");
    let serialized = serde_json::to_string(&result.needs[0].matches).unwrap();

    assert!(serialized.contains("farmer_registry"));
    assert!(!serialized.contains("attendance_rate"));
}

#[test]
fn registry_relay_evidence_removal_invalidates_attendance_match() {
    let mut report = demo_envelope().report;
    let index = CapabilityIndex::from_reports(vec![report.clone()]).expect("index builds");
    report.findings.retain(|finding| {
        let Some(semantic_asset_discovery_core::DiscoveryEvidence::SchemaProperty {
            property_name,
            ..
        }) = finding.evidence.as_ref()
        else {
            return true;
        };
        property_name != "attendance_rate"
    });
    let mutated = CapabilityIndex::from_reports(vec![report]).expect("mutated index builds");
    let query = CapabilityQuery::new("strict").need(
        InformationNeed::new("school_attendance").requires_any([Term::field("attendance_rate")]),
    );

    assert!(
        !index.search(query.clone()).expect("search succeeds").needs[0]
            .matches
            .is_empty()
    );
    assert!(mutated.search(query).expect("search succeeds").needs[0]
        .matches
        .is_empty());
}

#[test]
fn query_cli_outputs_full_evidence_shaped_json() {
    let envelope_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/system-capability/registry-relay-all-standards.envelope.json");
    let output = Command::new(env!("CARGO_BIN_EXE_system-capability-query"))
        .arg("--envelope")
        .arg(envelope_path)
        .arg("--demo-social-protection")
        .output()
        .expect("query CLI runs");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("CLI emits JSON result");
    assert_eq!(value["query_id"], "social_protection_program");
    assert_eq!(
        value["needs"][0]["matches"][0]["route"]["role"],
        "candidate_route"
    );
    assert!(serde_json::to_string(&value)
        .unwrap()
        .contains("http://127.0.0.1:4242/datasets/farmer_registry/farmer"));
}

#[test]
fn query_cli_supports_conjunctive_strict_terms() {
    let envelope_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/system-capability/registry-relay-all-standards.envelope.json");
    let output = Command::new(env!("CARGO_BIN_EXE_system-capability-query"))
        .arg("--envelope")
        .arg(envelope_path)
        .arg("--need-all")
        .arg("disability_status")
        .arg("label")
        .arg("Disabled Person")
        .arg("--need-all")
        .arg("disability_status")
        .arg("field")
        .arg("disability_status")
        .output()
        .expect("query CLI runs");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("CLI emits JSON result");
    let matches = value["needs"][0]["matches"]
        .as_array()
        .expect("matches array");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0]["access"]["endpoint_url"],
        "http://127.0.0.1:4242/datasets/disability_registry/disabled_person"
    );
}

fn demo_envelope() -> DiscoveryRunEnvelope {
    serde_json::from_str(ENVELOPE).expect("demo envelope fixture is valid")
}
