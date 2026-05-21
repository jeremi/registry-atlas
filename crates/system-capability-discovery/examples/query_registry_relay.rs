use system_capability_discovery::{
    CapabilityIndex, CapabilityQuery, CapabilitySource, DiscoveryRunEnvelope, InformationNeed, Term,
};

const ENVELOPE: &str =
    include_str!("../../../fixtures/system-capability/registry-relay-all-standards.envelope.json");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let envelope: DiscoveryRunEnvelope = serde_json::from_str(ENVELOPE)?;
    let source = CapabilitySource {
        id: "registry-relay-all-standards".to_string(),
        report: envelope.report.clone(),
        envelope: Some(envelope),
        mappings: Vec::new(),
        review: Vec::new(),
    };
    let index = CapabilityIndex::from_sources(vec![source])?;
    let result = index.search(
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
    )?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
