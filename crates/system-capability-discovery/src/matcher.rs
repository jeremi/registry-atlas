use crate::types::*;
use semantic_asset_discovery_core::{
    DiscoveryEvidence, DiscoveryFinding, DiscoveryReport, SemanticAsset, SemanticAssetKind,
};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Clone)]
pub struct CapabilityIndex {
    sources: Vec<IndexedSource>,
}

impl CapabilityIndex {
    pub fn from_reports(reports: Vec<DiscoveryReport>) -> Result<Self, CapabilityError> {
        let sources = reports
            .into_iter()
            .map(|report| CapabilitySource {
                id: report.run_id.clone(),
                report,
                envelope: None,
                mappings: Vec::new(),
                review: Vec::new(),
            })
            .collect();
        Self::from_sources(sources)
    }

    pub fn from_sources(sources: Vec<CapabilitySource>) -> Result<Self, CapabilityError> {
        if sources.is_empty() {
            return Err(CapabilityError::EmptySources);
        }
        Ok(Self {
            sources: sources.into_iter().map(IndexedSource::new).collect(),
        })
    }

    pub fn search(
        &self,
        query: CapabilityQuery,
    ) -> Result<CapabilitySearchResult, CapabilityError> {
        if query.needs.is_empty() {
            return Err(CapabilityError::EmptyQuery {
                query_id: query.id.clone(),
            });
        }
        for need in &query.needs {
            for term in &need.requires_any {
                self.validate_term(term)?;
            }
            for term in &need.requires_all {
                self.validate_term(term)?;
            }
        }

        let mut needs = Vec::new();
        for need in &query.needs {
            let mut matches = Vec::new();
            for source in &self.sources {
                matches.extend(source.search_need(&query, need));
            }
            matches.sort_by_key(match_sort_key);
            dedupe_matches_by_route_identity(&mut matches);
            needs.push(NeedSearchResult {
                need_id: need.id.clone(),
                matches,
            });
        }

        Ok(CapabilitySearchResult {
            query_id: query.id,
            inputs_summary: self.inputs_summary(),
            needs,
        })
    }

    fn has_mapping(&self, mapping_set_id: &str, mapping_id: &str) -> bool {
        self.sources.iter().any(|source| {
            source.source.mappings.iter().any(|set| {
                set.id == mapping_set_id
                    && set.mappings.iter().any(|mapping| mapping.id == mapping_id)
            })
        })
    }

    fn validate_term(&self, term: &Term) -> Result<(), CapabilityError> {
        if let Term::ReviewedMapping {
            mapping_set_id,
            mapping_id,
        } = term
        {
            if !self.has_mapping(mapping_set_id, mapping_id) {
                return Err(CapabilityError::UnsupportedReviewedMapping {
                    mapping_set_id: mapping_set_id.clone(),
                    mapping_id: mapping_id.clone(),
                });
            }
        }
        Ok(())
    }

    fn inputs_summary(&self) -> InputsSummary {
        InputsSummary {
            report_ids: self
                .sources
                .iter()
                .map(|source| source.source.report.run_id.clone())
                .collect(),
            envelope_ids: self
                .sources
                .iter()
                .filter(|source| source.source.envelope.is_some())
                .map(|source| source.source.id.clone())
                .collect(),
            reviewed_mapping_sets: self
                .sources
                .iter()
                .flat_map(|source| {
                    source
                        .source
                        .mappings
                        .iter()
                        .map(|set| format!("{}@{}", set.id, set.version))
                })
                .collect(),
            review_assertions: self
                .sources
                .iter()
                .flat_map(|source| source.source.review.iter().map(|review| review.id.clone()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct IndexedSource {
    source: CapabilitySource,
    records: Vec<Record>,
    ambiguous_boundary: bool,
}

impl IndexedSource {
    fn new(source: CapabilitySource) -> Self {
        let prefix_map = default_prefix_map();
        let mut records = Vec::new();
        for artifact in &source.report.artifacts {
            let evidence = EvidenceRef {
                id: EvidenceId(format!(
                    "evidence:artifact:{}:{}",
                    source.report.run_id, artifact.id
                )),
                source: EvidenceSource::DiscoveryArtifact {
                    report_id: source.report.run_id.clone(),
                    artifact_id: artifact.id.clone(),
                },
                location: Some(EvidenceLocation::Url {
                    url: artifact
                        .final_url
                        .clone()
                        .unwrap_or_else(|| artifact.url.clone()),
                }),
                claim: EvidenceClaim {
                    capability_need_id: None,
                    matched_term: None,
                    basis: MatchBasis::RequiredInformation,
                    value: artifact
                        .title
                        .clone()
                        .or_else(|| Some(artifact.url.clone())),
                },
                derived_from: Vec::new(),
            };
            records.push(Record::new(
                artifact
                    .title
                    .iter()
                    .chain(artifact.description.iter())
                    .cloned()
                    .collect(),
                vec![artifact.url.clone()]
                    .into_iter()
                    .chain(artifact.final_url.clone())
                    .collect(),
                Vec::new(),
                evidence,
                Strength::Metadata,
                None,
                None,
                &prefix_map,
            ));
        }
        for asset in &source.report.assets {
            records.extend(records_for_asset(&source.report.run_id, asset, &prefix_map));
        }
        for link in &source.report.links {
            let evidence = EvidenceRef {
                id: EvidenceId(format!(
                    "evidence:link:{}:{}",
                    source.report.run_id, link.id
                )),
                source: EvidenceSource::DiscoveryLink {
                    report_id: source.report.run_id.clone(),
                    link_id: link.id.clone(),
                },
                location: evidence_location(&link.discovered_by).or_else(|| {
                    link.rel.as_ref().map(|rel| EvidenceLocation::HtmlLink {
                        rel: rel.clone(),
                        href: link.to_url.clone(),
                    })
                }),
                claim: EvidenceClaim {
                    capability_need_id: None,
                    matched_term: None,
                    basis: MatchBasis::AccessEvidence,
                    value: Some(link.to_url.clone()),
                },
                derived_from: Vec::new(),
            };
            records.push(Record::new(
                [link.rel.clone(), link.role.clone()]
                    .into_iter()
                    .flatten()
                    .collect(),
                [
                    Some(link.from_url.clone()),
                    Some(link.to_url.clone()),
                    link.predicate.clone(),
                ]
                .into_iter()
                .flatten()
                .collect(),
                Vec::new(),
                evidence,
                Strength::Access,
                None,
                Some(link.to_url.clone()),
                &prefix_map,
            ));
        }
        for standard in &source.report.standards {
            let evidence = EvidenceRef {
                id: EvidenceId(format!(
                    "evidence:standard:{}:{}",
                    source.report.run_id, standard.id
                )),
                source: EvidenceSource::StandardClaim {
                    report_id: source.report.run_id.clone(),
                    claim_id: standard.id.clone(),
                },
                location: evidence_location(&standard.evidence),
                claim: EvidenceClaim {
                    capability_need_id: None,
                    matched_term: None,
                    basis: MatchBasis::RequiredInformation,
                    value: standard
                        .label
                        .clone()
                        .or_else(|| Some(standard.iri.clone())),
                },
                derived_from: Vec::new(),
            };
            records.push(Record::new(
                standard.label.iter().cloned().collect(),
                vec![standard.iri.clone()],
                Vec::new(),
                evidence,
                Strength::Metadata,
                None,
                None,
                &prefix_map,
            ));
        }
        for profile in &source.report.profiles {
            let evidence = EvidenceRef {
                id: EvidenceId(format!(
                    "evidence:profile:{}:{}",
                    source.report.run_id, profile.id
                )),
                source: EvidenceSource::ProfileClaim {
                    report_id: source.report.run_id.clone(),
                    claim_id: profile.id.clone(),
                },
                location: evidence_location(&profile.evidence),
                claim: EvidenceClaim {
                    capability_need_id: None,
                    matched_term: None,
                    basis: MatchBasis::RequiredInformation,
                    value: profile.label.clone().or_else(|| Some(profile.iri.clone())),
                },
                derived_from: Vec::new(),
            };
            records.push(Record::new(
                profile.label.iter().cloned().collect(),
                [Some(profile.iri.clone()), profile.base_standard_iri.clone()]
                    .into_iter()
                    .flatten()
                    .collect(),
                Vec::new(),
                evidence,
                Strength::Metadata,
                None,
                None,
                &prefix_map,
            ));
        }
        for finding in &source.report.findings {
            let inferred_asset_id = finding.asset_id.clone().or_else(|| {
                finding.artifact_id.as_ref().and_then(|artifact_id| {
                    source
                        .report
                        .assets
                        .iter()
                        .find(|asset| asset.artifact_id == *artifact_id)
                        .map(|asset| asset.id.clone())
                })
            });
            records.push(record_for_finding(
                &source.report.run_id,
                finding,
                inferred_asset_id,
                &prefix_map,
            ));
        }
        if let Some(envelope) = &source.envelope {
            for rejected in &envelope.rejected_fetches {
                let evidence = rejected_evidence(&source.id, rejected);
                records.push(Record::new(
                    vec![rejected.reason_code.clone()],
                    vec![rejected.url.clone()],
                    Vec::new(),
                    evidence,
                    Strength::Access,
                    None,
                    Some(rejected.url.clone()),
                    &prefix_map,
                ));
            }
        }
        for set in &source.mappings {
            for mapping in &set.mappings {
                let evidence = EvidenceRef {
                    id: EvidenceId(format!(
                        "evidence:reviewed-mapping:{}:{}",
                        set.id, mapping.id
                    )),
                    source: EvidenceSource::ReviewedMapping {
                        mapping_set_id: set.id.clone(),
                        mapping_id: mapping.id.clone(),
                    },
                    location: None,
                    claim: EvidenceClaim {
                        capability_need_id: None,
                        matched_term: Some(Term::reviewed_mapping(&set.id, &mapping.id)),
                        basis: MatchBasis::ReviewedMapping,
                        value: mapping.label.clone(),
                    },
                    derived_from: Vec::new(),
                };
                records.push(Record::new(
                    mapping.labels.clone(),
                    mapping.iris.clone(),
                    mapping.fields.clone(),
                    evidence,
                    Strength::ReviewedMapping,
                    None,
                    None,
                    &prefix_map,
                ));
            }
        }
        let dataset_count = source
            .report
            .assets
            .iter()
            .filter(|asset| matches!(asset.kind, SemanticAssetKind::Dataset))
            .map(|asset| {
                asset
                    .uri
                    .clone()
                    .or_else(|| asset.title.clone())
                    .unwrap_or_else(|| asset.id.clone())
            })
            .collect::<BTreeSet<_>>()
            .len();
        Self {
            source,
            records,
            ambiguous_boundary: dataset_count >= 2,
        }
    }

    fn search_need(&self, query: &CapabilityQuery, need: &InformationNeed) -> Vec<CapabilityMatch> {
        if need.requires_any.is_empty() && need.requires_all.is_empty() {
            return Vec::new();
        }
        let any_terms = self.expand_required_terms(&need.requires_any);
        let any_hits = self.match_terms(&any_terms, &query.prefixes);
        if !need.requires_any.is_empty() && any_hits.is_empty() {
            return Vec::new();
        }
        let all_hit_groups = need
            .requires_all
            .iter()
            .map(|term| {
                let terms = self.expand_required_terms(std::slice::from_ref(term));
                self.match_terms(&terms, &query.prefixes)
            })
            .collect::<Vec<_>>();
        if all_hit_groups.iter().any(Vec::is_empty) {
            return Vec::new();
        }
        let about_hits = self.match_terms(&need.about_any, &query.prefixes);
        let purpose_hits = query
            .purpose
            .as_ref()
            .map(|term| self.match_terms(std::slice::from_ref(term), &query.prefixes))
            .unwrap_or_default();

        let mut asset_ids = any_hits
            .iter()
            .filter_map(|hit| hit.record.asset_id.clone())
            .collect::<BTreeSet<_>>();
        for group in &all_hit_groups {
            for hit in group {
                if let Some(asset_id) = &hit.record.asset_id {
                    asset_ids.insert(asset_id.clone());
                }
            }
        }
        if asset_ids.is_empty() {
            asset_ids.insert("metadata".to_string());
        }
        asset_ids
            .into_iter()
            .filter_map(|asset_id| {
                let hits = self.required_hits_for_asset(&asset_id, &any_hits, &all_hit_groups)?;
                Some((asset_id, hits))
            })
            .map(|(asset_id, hits)| {
                let asset = self
                    .source
                    .report
                    .assets
                    .iter()
                    .find(|asset| asset.id == asset_id);
                self.build_match(
                    need,
                    asset,
                    &asset_id,
                    hits,
                    about_hits.clone(),
                    purpose_hits.clone(),
                )
            })
            .collect()
    }

    fn required_hits_for_asset(
        &self,
        asset_id: &str,
        any_hits: &[Hit],
        all_hit_groups: &[Vec<Hit>],
    ) -> Option<Vec<Hit>> {
        let mut hits = Vec::new();
        let any_for_asset = hits_for_asset(any_hits, asset_id);
        if !any_hits.is_empty() && any_for_asset.is_empty() {
            return None;
        }
        hits.extend(any_for_asset);
        for group in all_hit_groups {
            let group_hits = hits_for_asset(group, asset_id);
            if group_hits.is_empty() {
                return None;
            }
            hits.extend(group_hits);
        }
        Some(hits)
    }

    fn build_match(
        &self,
        need: &InformationNeed,
        asset: Option<&SemanticAsset>,
        asset_id: &str,
        required_hits: Vec<Hit>,
        about_hits: Vec<Hit>,
        purpose_hits: Vec<Hit>,
    ) -> CapabilityMatch {
        let mut evidence = Vec::new();
        let mut signals = Vec::new();
        let mut score = EvidenceScore::default();

        for hit in &required_hits {
            let ev = hit.evidence_for(need, MatchBasis::RequiredInformation);
            match hit.record.strength {
                Strength::Structured => score.direct_structured_matches += 1,
                Strength::Metadata | Strength::Access => score.direct_metadata_matches += 1,
                Strength::ReviewedMapping => score.reviewed_mapping_matches += 1,
            }
            evidence.push(ev.clone());
            signals.push(CapabilitySignal {
                kind: CapabilitySignalKind::RequiredInformation,
                label: "required information match".to_string(),
                evidence: vec![ev],
            });
        }
        for hit in about_hits {
            let ev = hit.evidence_for(need, MatchBasis::SubjectContext);
            evidence.push(ev.clone());
            signals.push(CapabilitySignal {
                kind: CapabilitySignalKind::Subject,
                label: "subject context match".to_string(),
                evidence: vec![ev],
            });
        }
        for hit in purpose_hits {
            let ev = hit.evidence_for(need, MatchBasis::PurposeContext);
            evidence.push(ev.clone());
            signals.push(CapabilitySignal {
                kind: CapabilitySignalKind::Purpose,
                label: "purpose context match".to_string(),
                evidence: vec![ev],
            });
        }

        let access = self.access_summary(asset, &required_hits);
        if !access.evidence.is_empty()
            || !matches!(access.kind, AccessKind::MetadataOnly | AccessKind::Unknown)
        {
            score.access_evidence_matches = access.evidence.len().max(1) as u32;
        }
        evidence.extend(access.evidence.clone());
        if !access.evidence.is_empty() {
            signals.push(CapabilitySignal {
                kind: CapabilitySignalKind::Access,
                label: "access evidence".to_string(),
                evidence: access.evidence.clone(),
            });
        }

        let standard_signals = asset
            .map(|asset| self.standard_signals_for_asset(asset))
            .unwrap_or_default();
        let mut gaps = vec![
            CapabilityGap::RequiredIdentifierUnknown,
            CapabilityGap::LegalBasisUnknown,
            CapabilityGap::AuthorityUnknown,
            CapabilityGap::SourceOfTruthUnknown,
            CapabilityGap::FreshnessUnknown,
        ];
        if matches!(access.kind, AccessKind::MetadataOnly | AccessKind::Unknown) {
            gaps.push(CapabilityGap::NoCallableAccessMethod);
            gaps.push(CapabilityGap::OperationDetailsUnavailable);
        }
        if standard_signals.has_authority() {
            gaps.retain(|gap| gap != &CapabilityGap::AuthorityUnknown);
        }
        if standard_signals.has_legal_basis() {
            gaps.retain(|gap| gap != &CapabilityGap::LegalBasisUnknown);
        }
        if standard_signals.has_freshness() {
            gaps.retain(|gap| gap != &CapabilityGap::FreshnessUnknown);
        }
        gaps.sort();
        gaps.dedup();
        score.gap_count = gaps.len() as u32;

        let mut review_flags = BTreeSet::new();
        if self.ambiguous_boundary {
            review_flags.insert(ReviewFlag::BoundaryAmbiguous);
        }
        if required_hits
            .iter()
            .any(|hit| hit.record.strength == Strength::ReviewedMapping)
        {
            review_flags.insert(ReviewFlag::ReviewedMappingUsed);
        }
        if is_sensitive(need, &required_hits) {
            review_flags.insert(ReviewFlag::SensitiveData);
            review_flags.insert(ReviewFlag::PolicyReviewRequired);
        }
        score.review_flag_count = review_flags.len() as u32;

        let component = asset.map(|asset| RouteComponent {
            id: asset.id.clone(),
            label: asset
                .title
                .clone()
                .or_else(|| asset.uri.clone())
                .unwrap_or_else(|| asset.id.clone()),
            kind: route_component_kind(&asset.kind),
            url: asset.endpoint_url.clone().or_else(|| asset.uri.clone()),
            evidence: required_hits
                .iter()
                .map(|hit| hit.evidence_for(need, MatchBasis::RequiredInformation))
                .collect(),
        });
        let mut components = component.into_iter().collect::<Vec<_>>();
        if !access.evidence.is_empty() {
            components.push(RouteComponent {
                id: format!("{}:access", asset_id),
                label: format!("{:?}", access.kind),
                kind: RouteComponentKind::Service,
                url: access
                    .endpoint_url
                    .clone()
                    .or_else(|| access.distribution_url.clone())
                    .or_else(|| access.source_url.clone()),
                evidence: access.evidence.clone(),
            });
        }

        let role = role_from_standard_signals(&standard_signals);
        if role == CandidateRouteRole::CandidateSource {
            gaps.retain(|gap| gap != &CapabilityGap::SourceOfTruthUnknown);
            score.gap_count = gaps.len() as u32;
        }

        let boundary = if self.ambiguous_boundary {
            SystemBoundary::Ambiguous {
                candidates: components.clone(),
                reason: "multiple datasets are present without reviewed system boundary evidence"
                    .to_string(),
            }
        } else if let Some(asset) = asset {
            SystemBoundary::GatewayOrIntermediary {
                label: asset
                    .publisher
                    .clone()
                    .unwrap_or_else(|| self.source.id.clone()),
                domain_hint: asset.uri.clone(),
                evidence: components
                    .first()
                    .map(|component| component.evidence.clone())
                    .unwrap_or_default(),
            }
        } else {
            SystemBoundary::Unknown
        };

        let route_label = asset
            .and_then(|asset| asset.title.clone())
            .unwrap_or_else(|| "Capability route candidate".to_string());
        let confidence =
            confidence_from_score(&score).expect("required evidence produces confidence");
        CapabilityMatch {
            route: CandidateAnswerRoute {
                id: format!("{}:{}:{}", self.source.id, need.id, asset_id),
                source_id: self.source.id.clone(),
                role,
                boundary,
                components,
            },
            score,
            confidence,
            access,
            signals,
            evidence: dedupe_evidence(evidence),
            explanation: Some(format!("{route_label} matched strict accepted terms.")),
            gaps,
            review_flags: review_flags.into_iter().collect(),
            review_state: ReviewState::Unreviewed,
        }
    }

    fn expand_required_terms(&self, terms: &[Term]) -> Vec<Term> {
        let mut expanded = Vec::new();
        for term in terms {
            expanded.push(term.clone());
            if let Term::ReviewedMapping {
                mapping_set_id,
                mapping_id,
            } = term
            {
                if let Some(mapping) = self.source.mappings.iter().find_map(|set| {
                    if set.id == *mapping_set_id {
                        set.mappings
                            .iter()
                            .find(|mapping| mapping.id == *mapping_id)
                    } else {
                        None
                    }
                }) {
                    expanded.extend(mapping.labels.iter().cloned().map(Term::Label));
                    expanded.extend(mapping.iris.iter().cloned().map(Term::Iri));
                    expanded.extend(mapping.fields.iter().cloned().map(Term::Field));
                }
            }
        }
        expanded
    }

    fn match_terms(&self, terms: &[Term], query_prefixes: &BTreeMap<String, String>) -> Vec<Hit> {
        let mut prefix_map = default_prefix_map();
        prefix_map.extend(query_prefixes.clone());
        let mut hits = Vec::new();
        for term in terms {
            let canonical = CanonicalTerm::new(term, &prefix_map);
            for record in &self.records {
                if record.matches(&canonical) {
                    hits.push(Hit {
                        record: record.clone(),
                        term: term.clone(),
                    });
                }
            }
        }
        hits.sort_by(|a, b| {
            a.record
                .evidence
                .id
                .cmp(&b.record.evidence.id)
                .then(a.term.cmp(&b.term))
        });
        hits.dedup_by(|a, b| a.record.evidence.id == b.record.evidence.id && a.term == b.term);
        hits
    }

    fn access_summary(
        &self,
        asset: Option<&SemanticAsset>,
        required_hits: &[Hit],
    ) -> AccessSummary {
        for hit in required_hits {
            match hit.record.evidence.location.as_ref() {
                Some(EvidenceLocation::OpenApiOperation { .. }) => {
                    return AccessSummary {
                        kind: AccessKind::ApiDescriptionAvailable,
                        endpoint_url: None,
                        distribution_url: None,
                        source_url: None,
                        protocol_hint: Some("openapi".to_string()),
                        interaction_hint: Some("request_response".to_string()),
                        credential_sent_in_discovery: None,
                        evidence: vec![hit.record.evidence.clone()],
                    };
                }
                Some(EvidenceLocation::OgcCollection { .. }) => {
                    return AccessSummary {
                        kind: AccessKind::DatasetDistribution,
                        endpoint_url: None,
                        distribution_url: None,
                        source_url: None,
                        protocol_hint: Some("ogc-api".to_string()),
                        interaction_hint: Some("request_response".to_string()),
                        credential_sent_in_discovery: None,
                        evidence: vec![hit.record.evidence.clone()],
                    };
                }
                Some(EvidenceLocation::RejectedFetch { url, reason, .. }) => {
                    return AccessSummary {
                        kind: AccessKind::RejectedOrGated,
                        endpoint_url: Some(url.clone()),
                        distribution_url: None,
                        source_url: Some(url.clone()),
                        protocol_hint: Some("http".to_string()),
                        interaction_hint: None,
                        credential_sent_in_discovery: None,
                        evidence: vec![{
                            let mut evidence = hit.record.evidence.clone();
                            evidence.claim.value = Some(reason.clone());
                            evidence
                        }],
                    };
                }
                _ => {}
            }
        }
        if let Some(asset) = asset {
            if let Some(rejected) = self.rejected_for_asset(asset) {
                return self.rejected_access_summary(rejected);
            }
            if let Some(endpoint) = &asset.endpoint_url {
                return AccessSummary {
                    kind: if matches!(
                        asset.kind,
                        SemanticAssetKind::ApiDescription | SemanticAssetKind::DataService
                    ) {
                        AccessKind::ApiDescriptionAvailable
                    } else {
                        AccessKind::DatasetDistribution
                    },
                    endpoint_url: Some(endpoint.clone()),
                    distribution_url: None,
                    source_url: Some(endpoint.clone()),
                    protocol_hint: Some(protocol_hint(asset)),
                    interaction_hint: None,
                    credential_sent_in_discovery: None,
                    evidence: vec![EvidenceRef {
                        id: EvidenceId(format!(
                            "evidence:access:{}:{}",
                            self.source.report.run_id, asset.id
                        )),
                        source: EvidenceSource::SemanticAsset {
                            report_id: self.source.report.run_id.clone(),
                            asset_id: asset.id.clone(),
                        },
                        location: Some(EvidenceLocation::Url {
                            url: endpoint.clone(),
                        }),
                        claim: EvidenceClaim {
                            capability_need_id: None,
                            matched_term: None,
                            basis: MatchBasis::AccessEvidence,
                            value: Some(endpoint.clone()),
                        },
                        derived_from: Vec::new(),
                    }],
                };
            }
            if matches!(
                asset.kind,
                SemanticAssetKind::Distribution
                    | SemanticAssetKind::RecordCollection
                    | SemanticAssetKind::FeatureCollection
            ) {
                return AccessSummary {
                    kind: AccessKind::DatasetDistribution,
                    endpoint_url: asset.uri.clone(),
                    distribution_url: asset.uri.clone(),
                    source_url: asset.uri.clone(),
                    protocol_hint: Some(protocol_hint(asset)),
                    interaction_hint: Some("batch".to_string()),
                    credential_sent_in_discovery: None,
                    evidence: Vec::new(),
                };
            }
            if let Some(distribution) = self.related_distribution(asset) {
                if let Some(rejected) = self.rejected_for_asset(distribution) {
                    return self.rejected_access_summary(rejected);
                }
                if let Some(endpoint) = distribution
                    .endpoint_url
                    .clone()
                    .or_else(|| distribution.uri.clone())
                {
                    return AccessSummary {
                        kind: AccessKind::DatasetDistribution,
                        endpoint_url: Some(endpoint.clone()),
                        distribution_url: Some(endpoint.clone()),
                        source_url: Some(endpoint.clone()),
                        protocol_hint: Some(protocol_hint(distribution)),
                        interaction_hint: Some("batch".to_string()),
                        credential_sent_in_discovery: None,
                        evidence: vec![EvidenceRef {
                            id: EvidenceId(format!(
                                "evidence:access:{}:{}",
                                self.source.report.run_id, distribution.id
                            )),
                            source: EvidenceSource::SemanticAsset {
                                report_id: self.source.report.run_id.clone(),
                                asset_id: distribution.id.clone(),
                            },
                            location: Some(EvidenceLocation::Url {
                                url: endpoint.clone(),
                            }),
                            claim: EvidenceClaim {
                                capability_need_id: None,
                                matched_term: None,
                                basis: MatchBasis::AccessEvidence,
                                value: Some(endpoint),
                            },
                            derived_from: Vec::new(),
                        }],
                    };
                }
            }
        }
        AccessSummary {
            kind: AccessKind::MetadataOnly,
            endpoint_url: None,
            distribution_url: None,
            source_url: None,
            protocol_hint: None,
            interaction_hint: None,
            credential_sent_in_discovery: None,
            evidence: Vec::new(),
        }
    }

    fn rejected_for_asset(&self, asset: &SemanticAsset) -> Option<&RejectedFetch> {
        let envelope = self.source.envelope.as_ref()?;
        let access_urls = access_urls_for_asset(asset);
        envelope
            .rejected_fetches
            .iter()
            .find(|rejected| access_urls.contains(&rejected.url))
    }

    fn rejected_access_summary(&self, rejected: &RejectedFetch) -> AccessSummary {
        AccessSummary {
            kind: AccessKind::RejectedOrGated,
            endpoint_url: Some(rejected.url.clone()),
            distribution_url: None,
            source_url: Some(rejected.url.clone()),
            protocol_hint: Some("http".to_string()),
            interaction_hint: None,
            credential_sent_in_discovery: Some(rejected.credential_sent),
            evidence: vec![rejected_evidence(&self.source.id, rejected)],
        }
    }

    fn related_distribution(&self, asset: &SemanticAsset) -> Option<&SemanticAsset> {
        if matches!(
            asset.kind,
            SemanticAssetKind::Distribution
                | SemanticAssetKind::RecordCollection
                | SemanticAssetKind::FeatureCollection
        ) {
            return None;
        }
        let title = asset.title.as_deref().map(canonical_access_label);
        let route_key = asset_route_key(asset);
        self.source.report.assets.iter().find(|candidate| {
            let candidate_title = candidate.title.as_deref().map(canonical_access_label);
            let candidate_route_key = asset_route_key(candidate);
            matches!(
                candidate.kind,
                SemanticAssetKind::Distribution
                    | SemanticAssetKind::RecordCollection
                    | SemanticAssetKind::FeatureCollection
            ) && (title
                .as_ref()
                .zip(candidate_title.as_ref())
                .is_some_and(|(left, right)| left == right)
                || route_key
                    .as_ref()
                    .zip(candidate_route_key.as_ref())
                    .is_some_and(|(left, right)| left == right))
        })
    }

    fn related_dataset(&self, asset: &SemanticAsset) -> Option<&SemanticAsset> {
        if matches!(asset.kind, SemanticAssetKind::Dataset) {
            return self
                .source
                .report
                .assets
                .iter()
                .find(|candidate| candidate.id == asset.id);
        }
        let url = asset.endpoint_url.as_ref().or(asset.uri.as_ref())?;
        let dataset_url = dataset_url_prefix(url)?;
        self.source.report.assets.iter().find(|candidate| {
            matches!(candidate.kind, SemanticAssetKind::Dataset)
                && candidate.uri.as_deref() == Some(dataset_url.as_str())
        })
    }

    fn standard_signals_for_asset(&self, asset: &SemanticAsset) -> StandardSignals {
        let mut signals = StandardSignals::default();
        if asset.publisher.is_some() {
            signals.predicates.insert("dcterms:publisher".to_string());
        }
        let related_distribution = self.related_distribution(asset);
        let related_dataset = self.related_dataset(asset).or_else(|| {
            related_distribution.and_then(|distribution| self.related_dataset(distribution))
        });
        let asset_ids = [Some(asset), related_distribution, related_dataset]
            .into_iter()
            .flatten()
            .map(|asset| asset.id.as_str())
            .collect::<BTreeSet<_>>();

        for finding in &self.source.report.findings {
            if finding.code != "semantic.standard_signal" {
                continue;
            }
            let Some(finding_asset_id) = finding.asset_id.as_deref() else {
                continue;
            };
            if !asset_ids.contains(finding_asset_id) {
                continue;
            }
            if let Some(DiscoveryEvidence::JsonLdPredicate {
                predicate, value, ..
            }) = finding.evidence.as_ref()
            {
                signals.predicates.insert(predicate.clone());
                if let Some(value) = value {
                    signals.values.insert((predicate.clone(), value.clone()));
                }
            }
        }

        let related_urls = [Some(asset), related_distribution, related_dataset]
            .into_iter()
            .flatten()
            .flat_map(|asset| {
                access_urls_for_asset(asset)
                    .into_iter()
                    .chain(asset.uri.clone())
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        for link in &self.source.report.links {
            if link.predicate.as_deref() == Some("cpsv:produces")
                && url_set_contains_resolved_fragment(&related_urls, &link.to_url)
            {
                signals.predicates.insert("cpsv:produces".to_string());
            }
        }

        signals
    }
}

#[derive(Debug, Clone, Default)]
struct StandardSignals {
    predicates: BTreeSet<String>,
    values: BTreeSet<(String, String)>,
}

impl StandardSignals {
    fn has_any(&self, predicates: &[&str]) -> bool {
        predicates
            .iter()
            .any(|predicate| self.predicates.contains(*predicate))
    }

    fn has_authority(&self) -> bool {
        self.has_any(&["dcterms:publisher", "dcterms:creator"])
    }

    fn has_legal_basis(&self) -> bool {
        self.has_any(&["dcatap:applicableLegislation"])
    }

    fn has_freshness(&self) -> bool {
        self.has_any(&[
            "dcterms:modified",
            "dcterms:issued",
            "dcterms:accrualPeriodicity",
            "adms:status",
            "dcatap:availability",
        ])
    }

    fn has_base_registry_source_signal(&self) -> bool {
        self.has_any(&["cpsv:produces"])
    }
}

#[derive(Debug, Clone)]
struct Record {
    labels: HashSet<String>,
    iris: HashSet<String>,
    fields: HashSet<String>,
    evidence: EvidenceRef,
    strength: Strength,
    asset_id: Option<String>,
}

impl Record {
    #[allow(clippy::too_many_arguments)]
    fn new(
        labels: Vec<String>,
        iris: Vec<String>,
        fields: Vec<String>,
        evidence: EvidenceRef,
        strength: Strength,
        asset_id: Option<String>,
        _access_url: Option<String>,
        prefix_map: &BTreeMap<String, String>,
    ) -> Self {
        Self {
            labels: labels
                .into_iter()
                .map(|value| canonical_label(&value))
                .collect(),
            iris: iris
                .into_iter()
                .map(|value| canonical_iri(&value, prefix_map))
                .collect(),
            fields: fields
                .into_iter()
                .map(|value| canonical_field(&value))
                .collect(),
            evidence,
            strength,
            asset_id,
        }
    }

    fn matches(&self, term: &CanonicalTerm) -> bool {
        match term {
            CanonicalTerm::Iri(value) => self.iris.contains(value),
            CanonicalTerm::Label(value) => self.labels.contains(value),
            CanonicalTerm::Field(value) => self.fields.contains(value),
            CanonicalTerm::ReviewedMapping {
                mapping_set_id,
                mapping_id,
            } => matches!(
                &self.evidence.source,
                EvidenceSource::ReviewedMapping { mapping_set_id: set, mapping_id: mapping }
                if set == mapping_set_id && mapping == mapping_id
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Strength {
    Structured,
    Metadata,
    Access,
    ReviewedMapping,
}

#[derive(Debug, Clone)]
struct Hit {
    record: Record,
    term: Term,
}

impl Hit {
    fn evidence_for(&self, need: &InformationNeed, basis: MatchBasis) -> EvidenceRef {
        let mut evidence = self.record.evidence.clone();
        evidence.claim.capability_need_id = Some(need.id.clone());
        evidence.claim.matched_term = Some(self.term.clone());
        evidence.claim.basis = if self.record.strength == Strength::ReviewedMapping {
            MatchBasis::ReviewedMapping
        } else {
            basis
        };
        evidence
    }
}

fn hits_for_asset(hits: &[Hit], asset_id: &str) -> Vec<Hit> {
    if asset_id == "metadata" {
        return hits
            .iter()
            .filter(|hit| hit.record.asset_id.is_none())
            .cloned()
            .collect();
    }
    hits.iter()
        .filter(|hit| hit.record.asset_id.as_deref() == Some(asset_id))
        .cloned()
        .collect()
}

#[derive(Debug)]
enum CanonicalTerm {
    Iri(String),
    Label(String),
    Field(String),
    ReviewedMapping {
        mapping_set_id: String,
        mapping_id: String,
    },
}

impl CanonicalTerm {
    fn new(term: &Term, prefix_map: &BTreeMap<String, String>) -> Self {
        match term {
            Term::Iri(value) => Self::Iri(canonical_iri(value, prefix_map)),
            Term::Label(value) => Self::Label(canonical_label(value)),
            Term::Field(value) => Self::Field(canonical_field(value)),
            Term::ReviewedMapping {
                mapping_set_id,
                mapping_id,
            } => Self::ReviewedMapping {
                mapping_set_id: mapping_set_id.trim().to_string(),
                mapping_id: mapping_id.trim().to_string(),
            },
        }
    }
}

fn records_for_asset(
    report_id: &str,
    asset: &SemanticAsset,
    prefix_map: &BTreeMap<String, String>,
) -> Vec<Record> {
    let mut records = Vec::new();
    let base_evidence = EvidenceRef {
        id: EvidenceId(format!("evidence:asset:{}:{}", report_id, asset.id)),
        source: EvidenceSource::SemanticAsset {
            report_id: report_id.to_string(),
            asset_id: asset.id.clone(),
        },
        location: asset
            .raw_refs
            .first()
            .and_then(|raw| raw.pointer.clone())
            .map(|pointer| EvidenceLocation::JsonPointer { pointer })
            .or_else(|| asset.uri.clone().map(|url| EvidenceLocation::Url { url })),
        claim: EvidenceClaim {
            capability_need_id: None,
            matched_term: None,
            basis: MatchBasis::RequiredInformation,
            value: asset.title.clone().or_else(|| asset.uri.clone()),
        },
        derived_from: Vec::new(),
    };
    let mut labels = Vec::new();
    labels.extend(asset.title.iter().cloned());
    labels.extend(asset.description.iter().cloned());
    labels.extend(asset.publisher.iter().cloned());
    let mut iris = Vec::new();
    iris.extend(asset.uri.iter().cloned());
    iris.extend(asset.endpoint_url.iter().cloned());
    iris.extend(asset.conforms_to.iter().cloned());
    iris.extend(
        asset
            .raw_refs
            .iter()
            .filter_map(|raw| raw.subject_iri.clone()),
    );
    records.push(Record::new(
        labels,
        iris,
        Vec::new(),
        base_evidence,
        asset_strength(&asset.kind),
        Some(asset.id.clone()),
        asset.endpoint_url.clone(),
        prefix_map,
    ));

    for (idx, hint) in asset.source_hints.iter().enumerate() {
        let evidence = EvidenceRef {
            id: EvidenceId(format!(
                "evidence:asset-hint:{}:{}:{}",
                report_id, asset.id, idx
            )),
            source: EvidenceSource::SemanticAsset {
                report_id: report_id.to_string(),
                asset_id: asset.id.clone(),
            },
            location: hint
                .path
                .clone()
                .map(|pointer| EvidenceLocation::JsonPointer { pointer }),
            claim: EvidenceClaim {
                capability_need_id: None,
                matched_term: None,
                basis: MatchBasis::RequiredInformation,
                value: Some(hint.label.clone()),
            },
            derived_from: Vec::new(),
        };
        records.push(Record::new(
            vec![hint.label.clone()],
            hint.predicate.iter().cloned().collect(),
            [hint.path.clone(), hint.predicate.clone()]
                .into_iter()
                .flatten()
                .collect(),
            evidence,
            Strength::Structured,
            Some(asset.id.clone()),
            asset.endpoint_url.clone(),
            prefix_map,
        ));
    }
    records
}

fn record_for_finding(
    report_id: &str,
    finding: &DiscoveryFinding,
    asset_id: Option<String>,
    prefix_map: &BTreeMap<String, String>,
) -> Record {
    let evidence = EvidenceRef {
        id: EvidenceId(format!("evidence:finding:{}:{}", report_id, finding.id)),
        source: EvidenceSource::Finding {
            report_id: report_id.to_string(),
            finding_id: finding.id.clone(),
        },
        location: finding.evidence.as_ref().and_then(evidence_location),
        claim: EvidenceClaim {
            capability_need_id: None,
            matched_term: None,
            basis: MatchBasis::RequiredInformation,
            value: Some(finding.code.clone()),
        },
        derived_from: Vec::new(),
    };
    let mut labels = vec![finding.code.clone(), finding.message.clone()];
    let mut iris = finding.standard_iri.iter().cloned().collect::<Vec<_>>();
    let mut fields = Vec::new();
    let mut strength = Strength::Metadata;
    if let Some(discovery_evidence) = &finding.evidence {
        apply_finding_evidence(
            discovery_evidence,
            &mut labels,
            &mut iris,
            &mut fields,
            &mut strength,
        );
    }
    Record::new(
        labels, iris, fields, evidence, strength, asset_id, None, prefix_map,
    )
}

fn apply_finding_evidence(
    evidence: &DiscoveryEvidence,
    labels: &mut Vec<String>,
    iris: &mut Vec<String>,
    fields: &mut Vec<String>,
    strength: &mut Strength,
) {
    match evidence {
        DiscoveryEvidence::SchemaProperty {
            schema_pointer,
            property_path,
            property_name,
            value,
            ..
        } => {
            fields.extend([
                schema_pointer.clone(),
                property_path.clone(),
                property_name.clone(),
            ]);
            labels.extend(value.iter().cloned());
            *strength = Strength::Structured;
        }
        DiscoveryEvidence::ShaclProperty {
            shape,
            path,
            predicate,
            value,
            ..
        } => {
            fields.extend([path.clone(), predicate.clone()]);
            iris.extend([shape.clone(), path.clone()]);
            labels.extend(value.iter().cloned());
            *strength = Strength::Structured;
        }
        DiscoveryEvidence::OpenApiOperation {
            path,
            method,
            operation_id,
            summary,
            ..
        } => {
            fields.extend([path.clone(), method.clone()]);
            fields.extend(operation_id.iter().cloned());
            labels.extend(operation_id.iter().cloned());
            labels.extend(summary.iter().cloned());
            *strength = Strength::Access;
        }
        DiscoveryEvidence::OgcCollection {
            collection_id,
            title,
            ..
        } => {
            fields.push(collection_id.clone());
            labels.extend(title.iter().cloned());
            *strength = Strength::Access;
        }
        DiscoveryEvidence::JsonPointer { pointer, value, .. } => {
            fields.push(pointer.clone());
            labels.extend(value.iter().cloned());
        }
        DiscoveryEvidence::JsonLdPredicate {
            predicate, value, ..
        } => {
            fields.push(predicate.clone());
            labels.extend(value.iter().cloned());
            iris.extend(value.iter().cloned());
        }
        DiscoveryEvidence::HttpHeader {
            header_name,
            rel,
            value,
            ..
        } => {
            fields.push(header_name.clone());
            labels.extend(rel.iter().cloned());
            iris.extend(value.iter().cloned());
        }
        DiscoveryEvidence::HtmlLink { rel, href, .. } => {
            labels.push(rel.clone());
            iris.push(href.clone());
        }
        DiscoveryEvidence::UrlPattern { pattern, value, .. } => {
            labels.push(pattern.clone());
            iris.push(value.clone());
        }
        DiscoveryEvidence::ContentSniff {
            detector, marker, ..
        } => {
            labels.extend([detector.clone(), marker.clone()]);
        }
        DiscoveryEvidence::HostPolicy { policy, value, .. } => {
            labels.push(policy.clone());
            labels.extend(value.iter().cloned());
        }
    }
}

fn evidence_location(evidence: &DiscoveryEvidence) -> Option<EvidenceLocation> {
    match evidence {
        DiscoveryEvidence::HttpHeader { header_name, .. } => Some(EvidenceLocation::HttpHeader {
            name: header_name.clone(),
        }),
        DiscoveryEvidence::JsonLdPredicate {
            predicate,
            pointer,
            value,
            ..
        } => Some(EvidenceLocation::RdfTriple {
            subject: pointer.clone().unwrap_or_default(),
            predicate: predicate.clone(),
            object: value.clone(),
        }),
        DiscoveryEvidence::JsonPointer { pointer, .. } => Some(EvidenceLocation::JsonPointer {
            pointer: pointer.clone(),
        }),
        DiscoveryEvidence::HtmlLink { rel, href, .. } => Some(EvidenceLocation::HtmlLink {
            rel: rel.clone(),
            href: href.clone(),
        }),
        DiscoveryEvidence::UrlPattern { value, .. } => {
            Some(EvidenceLocation::Url { url: value.clone() })
        }
        DiscoveryEvidence::SchemaProperty {
            schema_pointer,
            property_path,
            property_name,
            ..
        } => Some(EvidenceLocation::SchemaProperty {
            schema_pointer: schema_pointer.clone(),
            property_path: property_path.clone(),
            property_name: Some(property_name.clone()),
        }),
        DiscoveryEvidence::ShaclProperty { shape, path, .. } => {
            Some(EvidenceLocation::ShaclProperty {
                shape: Some(shape.clone()),
                path: path.clone(),
            })
        }
        DiscoveryEvidence::OpenApiOperation {
            path,
            method,
            operation_id,
            summary,
            ..
        } => Some(EvidenceLocation::OpenApiOperation {
            path: path.clone(),
            method: method.clone(),
            operation_id: operation_id.clone(),
            summary: summary.clone(),
        }),
        DiscoveryEvidence::OgcCollection {
            collection_id,
            title,
            ..
        } => Some(EvidenceLocation::OgcCollection {
            collection_id: collection_id.clone(),
            title: title.clone(),
        }),
        DiscoveryEvidence::ContentSniff { .. } | DiscoveryEvidence::HostPolicy { .. } => None,
    }
}

fn rejected_evidence(source_id: &str, rejected: &RejectedFetch) -> EvidenceRef {
    EvidenceRef {
        id: EvidenceId(format!(
            "evidence:rejected-fetch:{}:{}",
            source_id, rejected.id
        )),
        source: EvidenceSource::RejectedFetch {
            source_id: source_id.to_string(),
            rejected_fetch_id: rejected.id.clone(),
        },
        location: Some(EvidenceLocation::RejectedFetch {
            url: rejected.url.clone(),
            method: None,
            status: None,
            reason: rejected.reason_code.clone(),
        }),
        claim: EvidenceClaim {
            capability_need_id: None,
            matched_term: None,
            basis: MatchBasis::AccessEvidence,
            value: Some(rejected.reason_code.clone()),
        },
        derived_from: Vec::new(),
    }
}

fn role_from_standard_signals(signals: &StandardSignals) -> CandidateRouteRole {
    // `candidate_source` is an Atlas interpretation, not a DCAT-AP,
    // BRegDCAT-AP, CPSV, or ELI class. We only derive it when standard
    // metadata provides the base-registry production link plus authority
    // and legal-basis evidence. Without those signals, the result remains
    // a candidate route and callers must inspect the gaps.
    if signals.has_base_registry_source_signal()
        && signals.has_authority()
        && signals.has_legal_basis()
    {
        CandidateRouteRole::CandidateSource
    } else {
        CandidateRouteRole::CandidateRoute
    }
}

fn access_urls_for_asset(asset: &SemanticAsset) -> BTreeSet<String> {
    let mut urls = BTreeSet::new();
    urls.extend(asset.endpoint_url.iter().cloned());
    if matches!(
        asset.kind,
        SemanticAssetKind::Distribution
            | SemanticAssetKind::RecordCollection
            | SemanticAssetKind::FeatureCollection
            | SemanticAssetKind::ApiDescription
            | SemanticAssetKind::DataService
    ) {
        urls.extend(asset.uri.iter().cloned());
    }
    urls
}

fn url_set_contains_resolved_fragment(urls: &BTreeSet<String>, candidate: &str) -> bool {
    urls.contains(candidate)
        || urls
            .iter()
            .any(|url| url.starts_with('#') && candidate.ends_with(url))
}

fn dataset_url_prefix(url: &str) -> Option<String> {
    let marker = "/datasets/";
    let (base, rest) = url.split_once(marker)?;
    let dataset = rest.split('/').next()?;
    if dataset.is_empty() {
        return None;
    }
    Some(format!("{base}{marker}{dataset}"))
}

fn asset_strength(kind: &SemanticAssetKind) -> Strength {
    match kind {
        SemanticAssetKind::Class
        | SemanticAssetKind::Property
        | SemanticAssetKind::ShapeGraph
        | SemanticAssetKind::ApiDescription
        | SemanticAssetKind::DataService
        | SemanticAssetKind::RecordCollection
        | SemanticAssetKind::FeatureCollection => Strength::Structured,
        _ => Strength::Metadata,
    }
}

fn route_component_kind(kind: &SemanticAssetKind) -> RouteComponentKind {
    match kind {
        SemanticAssetKind::Catalog => RouteComponentKind::Catalogue,
        SemanticAssetKind::Dataset => RouteComponentKind::Dataset,
        SemanticAssetKind::ApiDescription | SemanticAssetKind::DataService => {
            RouteComponentKind::Service
        }
        SemanticAssetKind::Distribution => RouteComponentKind::Distribution,
        SemanticAssetKind::Class | SemanticAssetKind::Property => RouteComponentKind::Entity,
        SemanticAssetKind::ShapeGraph => RouteComponentKind::Schema,
        SemanticAssetKind::RecordCollection | SemanticAssetKind::FeatureCollection => {
            RouteComponentKind::Collection
        }
        _ => RouteComponentKind::Metadata,
    }
}

fn confidence_from_score(score: &EvidenceScore) -> Option<MatchConfidence> {
    if score.direct_structured_matches >= 1 && score.access_evidence_matches >= 1 {
        Some(MatchConfidence::High)
    } else if score.direct_structured_matches >= 1
        || (score.reviewed_mapping_matches >= 1 && score.direct_metadata_matches >= 1)
    {
        Some(MatchConfidence::Medium)
    } else if score.direct_metadata_matches >= 1 || score.reviewed_mapping_matches >= 1 {
        Some(MatchConfidence::Low)
    } else {
        None
    }
}

fn match_sort_key(
    item: &CapabilityMatch,
) -> (
    Reverse<u32>,
    Reverse<u32>,
    Reverse<u32>,
    Reverse<u32>,
    u32,
    MatchConfidence,
    String,
    Vec<String>,
) {
    (
        Reverse(item.score.direct_structured_matches),
        Reverse(item.score.direct_metadata_matches),
        Reverse(item.score.reviewed_mapping_matches),
        Reverse(item.score.access_evidence_matches),
        item.score.gap_count,
        item.confidence.clone(),
        item.route.id.clone(),
        item.evidence
            .iter()
            .map(|evidence| evidence.id.0.clone())
            .collect(),
    )
}

fn dedupe_evidence(items: Vec<EvidenceRef>) -> Vec<EvidenceRef> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for item in items {
        if seen.insert(item.id.clone()) {
            deduped.push(item);
        }
    }
    deduped
}

fn dedupe_matches_by_route_identity(items: &mut Vec<CapabilityMatch>) {
    let mut seen = BTreeSet::new();
    items.retain(|item| seen.insert(route_identity(item)));
}

fn route_identity(item: &CapabilityMatch) -> String {
    item.access
        .endpoint_url
        .as_ref()
        .or(item.access.distribution_url.as_ref())
        .or(item.access.source_url.as_ref())
        .cloned()
        .unwrap_or_else(|| item.route.id.clone())
}

fn is_sensitive(need: &InformationNeed, hits: &[Hit]) -> bool {
    let sensitive = [
        "disability",
        "health",
        "income",
        "eligibility",
        "household",
        "identity",
        "migration",
        "child",
        "student",
        "attendance",
    ];
    let mut values = Vec::new();
    values.extend(need.requires_any.iter().filter_map(term_text));
    values.extend(need.requires_all.iter().filter_map(term_text));
    values.extend(need.about_any.iter().filter_map(term_text));
    values.extend(
        hits.iter()
            .filter_map(|hit| hit.record.evidence.claim.value.clone()),
    );
    values
        .iter()
        .map(|value| canonical_label(value))
        .any(|value| sensitive.iter().any(|needle| value.contains(needle)))
}

fn term_text(term: &Term) -> Option<String> {
    match term {
        Term::Iri(value) | Term::Label(value) | Term::Field(value) => Some(value.clone()),
        Term::ReviewedMapping { .. } => None,
    }
}

fn protocol_hint(asset: &SemanticAsset) -> String {
    if matches!(
        asset.kind,
        SemanticAssetKind::ApiDescription | SemanticAssetKind::DataService
    ) {
        "openapi".to_string()
    } else {
        "http".to_string()
    }
}

fn canonical_label(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn canonical_access_label(value: &str) -> String {
    let mut label = canonical_label(value);
    for suffix in [" api", " service", " distribution", " endpoint"] {
        if let Some(stripped) = label.strip_suffix(suffix) {
            label = stripped.to_string();
            break;
        }
    }
    label
}

fn asset_route_key(asset: &SemanticAsset) -> Option<String> {
    asset
        .endpoint_url
        .as_ref()
        .or(asset.uri.as_ref())
        .and_then(|url| route_key_from_url(url))
}

fn route_key_from_url(value: &str) -> Option<String> {
    // This is a conservative Atlas projection over common catalogue URL
    // shapes. `/datasets/{dataset}/{entity}` is a standard-ish access
    // pattern in Relay's DCAT output, while `/metadata/schema/{dataset}/{entity}/...`
    // is Registry Relay's schema-document convention. We use it only to
    // connect metadata evidence to declared access methods, never as proof
    // that the endpoint is callable for a given user.
    if let Some((_base, rest)) = value.split_once("/datasets/") {
        return first_two_path_segments(rest);
    }
    if let Some((_base, rest)) = value.split_once("/metadata/schema/") {
        return first_two_path_segments(rest);
    }
    None
}

fn first_two_path_segments(value: &str) -> Option<String> {
    let mut segments = value.split('/').filter(|segment| !segment.is_empty());
    let dataset = segments.next()?;
    let entity = segments.next()?;
    Some(format!("{dataset}/{entity}"))
}

fn canonical_field(value: &str) -> String {
    value.trim().to_string()
}

fn canonical_iri(value: &str, prefix_map: &BTreeMap<String, String>) -> String {
    let trimmed = value.trim();
    if let Some((prefix, suffix)) = trimmed.split_once(':') {
        if !trimmed.contains("://") {
            if let Some(base) = prefix_map.get(prefix) {
                return format!("{base}{suffix}");
            }
        }
    }
    trimmed.to_string()
}

fn default_prefix_map() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("schema".to_string(), "https://schema.org/".to_string()),
        (
            "dcterms".to_string(),
            "http://purl.org/dc/terms/".to_string(),
        ),
        ("dcat".to_string(), "http://www.w3.org/ns/dcat#".to_string()),
        (
            "rdf".to_string(),
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#".to_string(),
        ),
        (
            "rdfs".to_string(),
            "http://www.w3.org/2000/01/rdf-schema#".to_string(),
        ),
        ("sh".to_string(), "http://www.w3.org/ns/shacl#".to_string()),
        (
            "skos".to_string(),
            "http://www.w3.org/2004/02/skos/core#".to_string(),
        ),
    ])
}
