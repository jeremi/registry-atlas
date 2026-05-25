use crate::profiles::{built_in_profile_packs, ProfilePack};
use crate::types::*;
#[cfg(not(target_arch = "wasm32"))]
use chrono::Utc;
use regex::Regex;
use rio_api::model::{Literal, NamedNode, Subject, Term};
use rio_api::parser::TriplesParser;
use rio_turtle::TurtleParser;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use url::Url;

const REDACTED: &str = "[redacted]";

pub fn analyze_artifacts(input: AnalyzeInput) -> Result<DiscoveryReport, AnalyzeError> {
    Url::parse(&input.entry_url).map_err(|error| AnalyzeError::InvalidEntryUrl {
        message: error.to_string(),
    })?;

    let options = input.options.normalized();
    validate_options(&options)?;
    let profile_packs = enabled_profile_packs(&options)?;
    let analyzed_at = input.analyzed_at.clone().unwrap_or_else(current_timestamp);
    let mut builder = ReportBuilder::new(
        input.entry_url.clone(),
        analyzed_at.clone(),
        options,
        profile_packs,
    );

    for artifact in input.artifacts {
        builder.process_artifact(artifact);
    }

    Ok(builder.finish())
}

#[cfg(not(target_arch = "wasm32"))]
fn current_timestamp() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(target_arch = "wasm32")]
fn current_timestamp() -> String {
    "1970-01-01T00:00:00Z".to_string()
}

fn validate_options(options: &AnalyzeOptions) -> Result<(), AnalyzeError> {
    for scheme in &options.accepted_schemes {
        if scheme.trim().is_empty() || scheme.contains(':') {
            return Err(AnalyzeError::InvalidOptions {
                message: format!("accepted scheme must be a bare scheme name: {scheme}"),
            });
        }
    }
    Ok(())
}

fn enabled_profile_packs(options: &AnalyzeOptions) -> Result<Vec<ProfilePack>, AnalyzeError> {
    let all = built_in_profile_packs().map_err(|error| AnalyzeError::InternalInvariant {
        message: format!("built-in profile pack failed to parse: {error}"),
    })?;
    if options
        .enabled_profiles
        .iter()
        .any(|profile| profile == "none")
    {
        return Ok(Vec::new());
    }
    if options.enabled_profiles.is_empty() {
        return Ok(all);
    }

    let requested: BTreeSet<_> = options.enabled_profiles.iter().cloned().collect();
    let known: BTreeSet<_> = all.iter().map(|pack| pack.id.clone()).collect();
    let unknown: Vec<_> = requested.difference(&known).cloned().collect();
    if !unknown.is_empty() {
        return Err(AnalyzeError::InvalidOptions {
            message: format!("unknown enabled profile pack(s): {}", unknown.join(", ")),
        });
    }

    Ok(all
        .into_iter()
        .filter(|pack| requested.contains(&pack.id))
        .collect())
}

#[derive(Debug, Clone)]
struct ParsedTriple {
    subject: String,
    predicate: String,
    object: String,
}

struct ReportBuilder {
    entry_url: String,
    analyzed_at: String,
    options: AnalyzeOptions,
    profile_packs: Vec<ProfilePack>,
    artifacts: Vec<DiscoveredArtifact>,
    assets: Vec<SemanticAsset>,
    relations: Vec<SemanticRelation>,
    relation_claims: Vec<RelationClaim>,
    links: Vec<DiscoveredLink>,
    standards: Vec<StandardClaim>,
    profiles: Vec<ProfileClaim>,
    findings: Vec<DiscoveryFinding>,
    next_fetches: Vec<FetchCandidate>,
    seen_assets: HashSet<String>,
    seen_relations: HashSet<String>,
    seen_relation_claims: HashSet<String>,
    seen_links: HashSet<String>,
    seen_standards: HashSet<String>,
    seen_profiles: HashSet<String>,
    seen_next_fetches: HashSet<String>,
    artifact_ids_by_url: HashMap<String, String>,
}

impl ReportBuilder {
    fn new(
        entry_url: String,
        analyzed_at: String,
        options: AnalyzeOptions,
        profile_packs: Vec<ProfilePack>,
    ) -> Self {
        Self {
            entry_url,
            analyzed_at,
            options,
            profile_packs,
            artifacts: Vec::new(),
            assets: Vec::new(),
            relations: Vec::new(),
            relation_claims: Vec::new(),
            links: Vec::new(),
            standards: Vec::new(),
            profiles: Vec::new(),
            findings: Vec::new(),
            next_fetches: Vec::new(),
            seen_assets: HashSet::new(),
            seen_relations: HashSet::new(),
            seen_relation_claims: HashSet::new(),
            seen_links: HashSet::new(),
            seen_standards: HashSet::new(),
            seen_profiles: HashSet::new(),
            seen_next_fetches: HashSet::new(),
            artifact_ids_by_url: HashMap::new(),
        }
    }

    fn finish(mut self) -> DiscoveryReport {
        let next_limit = self.options.max_next_fetches as usize;
        self.next_fetches.sort_by_key(|candidate| {
            (
                candidate.priority,
                candidate.url.clone(),
                candidate.id.clone(),
            )
        });
        let truncated = self.next_fetches.len() > next_limit;
        if self.next_fetches.len() > next_limit {
            self.next_fetches.truncate(next_limit);
        }

        let summary = DiscoverySummary {
            artifact_count: self.artifacts.len() as u64,
            asset_count: self.assets.len() as u64,
            standard_count: self.standards.len() as u64,
            profile_count: self.profiles.len() as u64,
            failed_artifact_count: self
                .artifacts
                .iter()
                .filter(|artifact| {
                    matches!(
                        artifact.status,
                        ArtifactStatus::Failed | ArtifactStatus::AuthRequired
                    )
                })
                .count() as u64,
            unsupported_artifact_count: self
                .artifacts
                .iter()
                .filter(|artifact| artifact.status == ArtifactStatus::Unsupported)
                .count() as u64,
            parse_error_count: self
                .findings
                .iter()
                .filter(|finding| finding.code.starts_with("parse."))
                .count() as u64,
            next_fetch_count: self.next_fetches.len() as u64,
            truncated,
        };

        DiscoveryReport {
            schema_version: SchemaVersion::default(),
            run_id: deterministic_id("run", [self.entry_url.as_str(), self.analyzed_at.as_str()]),
            entry_url: self.entry_url,
            analyzed_at: self.analyzed_at,
            summary,
            artifacts: self.artifacts,
            assets: self.assets,
            relations: self.relations,
            relation_claims: self.relation_claims,
            links: self.links,
            standards: self.standards,
            profiles: self.profiles,
            findings: self.findings,
            next_fetches: self.next_fetches,
        }
    }

    fn process_artifact(&mut self, mut fetched: FetchedArtifact) {
        let artifact_identity_url = normalized_artifact_url(&fetched);
        let artifact_id = deterministic_id("artifact", [artifact_identity_url.as_str()]);
        fetched.headers = redact_headers(fetched.headers);
        self.artifact_ids_by_url.insert(
            fetched
                .final_url
                .clone()
                .unwrap_or_else(|| fetched.url.clone()),
            artifact_id.clone(),
        );
        self.artifact_ids_by_url
            .insert(fetched.url.clone(), artifact_id.clone());

        let body_text = String::from_utf8_lossy(&fetched.body).to_string();
        let mut kind = classify_artifact(&fetched, &body_text);
        let mut status = http_status_to_artifact_status(fetched.status);
        let mut error = None;

        if matches!(status, ArtifactStatus::Fetched) && kind == ArtifactKind::Unknown {
            status = ArtifactStatus::Unsupported;
        }

        if matches!(status, ArtifactStatus::Fetched) {
            if let Err(message) = self.parse_artifact(&artifact_id, &fetched, &body_text, &mut kind)
            {
                status = ArtifactStatus::ParseError;
                error = Some(message.clone());
                self.add_finding(DiscoveryFinding {
                    id: deterministic_id(
                        "finding",
                        ["parse.failed", artifact_id.as_str(), "parser"],
                    ),
                    severity: FindingSeverity::Warning,
                    code: "parse.failed".to_string(),
                    message,
                    artifact_id: Some(artifact_id.clone()),
                    asset_id: None,
                    standard_iri: None,
                    evidence: Some(DiscoveryEvidence::ContentSniff {
                        artifact_id: Some(artifact_id.clone()),
                        detector: "parser".to_string(),
                        marker: format!("{kind:?}"),
                    }),
                });
            }
        }

        let title = self
            .assets
            .iter()
            .find(|asset| asset.artifact_id == artifact_id)
            .and_then(|asset| asset.title.clone());
        let description = self
            .assets
            .iter()
            .find(|asset| asset.artifact_id == artifact_id)
            .and_then(|asset| asset.description.clone());

        self.artifacts.push(DiscoveredArtifact {
            id: artifact_id,
            url: fetched.url,
            final_url: fetched.final_url,
            kind,
            status,
            media_type: fetched.media_type,
            http_status: Some(fetched.status),
            title,
            description,
            discovered_from: fetched.discovered_from,
            discovered_by: fetched.discovered_by,
            byte_length: Some(fetched.body.len() as u64),
            hash: Some(format!("sha256:{}", hex_sha256(&fetched.body))),
            error,
            analyzed_at: self.analyzed_at.clone(),
        });
    }

    fn parse_artifact(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        body_text: &str,
        kind: &mut ArtifactKind,
    ) -> Result<(), String> {
        self.extract_http_links(artifact_id, fetched);
        match kind {
            ArtifactKind::DcatCatalog
            | ArtifactKind::DcatProfileCatalog
            | ArtifactKind::ProfProfile
            | ArtifactKind::ProfResource
            | ArtifactKind::JsonLdContext
            | ArtifactKind::DidDocument
            | ArtifactKind::VerifiableCredential => {
                self.parse_json_ld_like(artifact_id, fetched, body_text, kind)
            }
            ArtifactKind::JsonSchema => self.parse_json_schema(artifact_id, fetched, body_text),
            ArtifactKind::OpenApi => self.parse_openapi(artifact_id, fetched, body_text),
            ArtifactKind::OgcLanding | ArtifactKind::OgcRecords | ArtifactKind::OgcFeatures => {
                self.parse_ogc(artifact_id, fetched, body_text, kind)
            }
            ArtifactKind::Shacl | ArtifactKind::Skos | ArtifactKind::OwlOntology => {
                if is_json_body(body_text, fetched.media_type.as_deref()) {
                    self.parse_json_ld_like(artifact_id, fetched, body_text, kind)
                } else {
                    self.parse_turtle(artifact_id, fetched, body_text, kind)
                }
            }
            ArtifactKind::SemanticModelPackage => {
                self.parse_semantic_package(artifact_id, fetched, body_text)
            }
            ArtifactKind::MetadataIndex => {
                self.parse_metadata_index(artifact_id, fetched, body_text)?;
                if is_json_ld_like_metadata(body_text) {
                    self.parse_json_ld_like(artifact_id, fetched, body_text, kind)?;
                }
                Ok(())
            }
            ArtifactKind::LinkMlSchema => self.parse_linkml(artifact_id, fetched, body_text),
            ArtifactKind::HtmlLandingPage => self.parse_html(artifact_id, fetched, body_text),
            ArtifactKind::Unknown => Ok(()),
        }
    }

    fn parse_json_ld_like(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        body_text: &str,
        kind: &mut ArtifactKind,
    ) -> Result<(), String> {
        let value: Value = serde_json::from_str(body_text).map_err(|error| error.to_string())?;
        let context_prefixes = json_ld_context_prefixes(&value);
        let nodes = json_nodes(&value);
        if *kind == ArtifactKind::JsonLdContext {
            self.add_asset(SemanticAsset {
                id: deterministic_id("asset", [artifact_id, "jsonld-context"]),
                kind: SemanticAssetKind::Vocabulary,
                artifact_id: artifact_id.to_string(),
                uri: Some(
                    fetched
                        .final_url
                        .clone()
                        .unwrap_or_else(|| fetched.url.clone()),
                ),
                title: get_json_string(&value, &["title", "dcterms:title", "name"]),
                description: get_json_string(&value, &["description", "dcterms:description"]),
                publisher: None,
                endpoint_url: None,
                conforms_to: Vec::new(),
                source_hints: Vec::new(),
                raw_refs: vec![RawReference {
                    artifact_id: artifact_id.to_string(),
                    pointer: Some("/".to_string()),
                    subject_iri: None,
                }],
            });
        }

        for node in nodes {
            let node_types = json_strings_for_keys(node, &["@type", "type"]);
            let uri = json_id(node);
            let title = get_json_string_map(
                node,
                &[
                    "dcterms:title",
                    "dct:title",
                    "title",
                    "rdfs:label",
                    "skos:prefLabel",
                    "name",
                ],
            );
            let description = get_json_string_map(
                node,
                &["dcterms:description", "dct:description", "description"],
            );
            let conforms_to = json_strings_for_keys(
                node,
                &["dcterms:conformsTo", "dct:conformsTo", "conformsTo"],
            );
            let conforms_to = conforms_to
                .into_iter()
                .map(|iri| expand_compact_iri(&iri, &context_prefixes).unwrap_or(iri))
                .collect::<Vec<_>>();

            if has_semantic_type(&node_types, "cpsv:PublicService", &context_prefixes) {
                let source_label =
                    if json_values_for_canonical_keys(node, &["cpsv:produces"], &context_prefixes)
                        .is_empty()
                    {
                        "cpsv:PublicService"
                    } else {
                        "cpsv:PublicRegistryService"
                    };
                let asset_id = json_asset_id(artifact_id, uri.as_deref(), source_label);
                self.add_asset(json_asset(
                    artifact_id,
                    if source_label == "cpsv:PublicRegistryService" {
                        SemanticAssetKind::PublicRegistryService
                    } else {
                        SemanticAssetKind::PublicService
                    },
                    uri.clone(),
                    title.clone(),
                    description.clone(),
                    get_json_string_map(
                        node,
                        &[
                            "cv:hasCompetentAuthority",
                            "http://data.europa.eu/m8g/hasCompetentAuthority",
                            "dcterms:publisher",
                            "dct:publisher",
                            "publisher",
                        ],
                    ),
                    None,
                    conforms_to.clone(),
                    source_label,
                ));
                self.add_standard_signal_findings(artifact_id, &asset_id, node, &context_prefixes);
            }
            for (canonical_type, kind, source_label) in [
                ("cv:Channel", SemanticAssetKind::Channel, "cv:Channel"),
                (
                    "cv:PublicOrganisation",
                    SemanticAssetKind::PublicOrganisation,
                    "cv:PublicOrganisation",
                ),
                (
                    "cccev:Requirement",
                    SemanticAssetKind::Requirement,
                    "cccev:Requirement",
                ),
                (
                    "cccev:InformationRequirement",
                    SemanticAssetKind::InformationRequirement,
                    "cccev:InformationRequirement",
                ),
                (
                    "cccev:InformationConcept",
                    SemanticAssetKind::InformationConcept,
                    "cccev:InformationConcept",
                ),
                (
                    "cccev:EvidenceType",
                    SemanticAssetKind::EvidenceType,
                    "cccev:EvidenceType",
                ),
                (
                    "cccev:EvidenceTypeList",
                    SemanticAssetKind::EvidenceTypeList,
                    "cccev:EvidenceTypeList",
                ),
                (
                    "registry_manifest:FormDefinition",
                    SemanticAssetKind::FormDefinition,
                    "registry_manifest:FormDefinition",
                ),
                (
                    "registry_manifest:FormSection",
                    SemanticAssetKind::FormSection,
                    "registry_manifest:FormSection",
                ),
                (
                    "registry_manifest:FormField",
                    SemanticAssetKind::FormField,
                    "registry_manifest:FormField",
                ),
                (
                    "registry_manifest:EvidenceOffering",
                    SemanticAssetKind::EvidenceOffering,
                    "registry_manifest:EvidenceOffering",
                ),
                (
                    "registry_manifest:EvidenceProvider",
                    SemanticAssetKind::EvidenceProvider,
                    "registry_manifest:EvidenceProvider",
                ),
            ] {
                if has_semantic_type(&node_types, canonical_type, &context_prefixes)
                    || (canonical_type == "cccev:Requirement"
                        && has_semantic_type(&node_types, "cv:Requirement", &context_prefixes))
                    || (canonical_type == "registry_manifest:FormDefinition"
                        && has_semantic_type(
                            &node_types,
                            "registry_manifest:Form",
                            &context_prefixes,
                        ))
                {
                    let asset_id = json_asset_id(artifact_id, uri.as_deref(), source_label);
                    self.add_asset(json_asset(
                        artifact_id,
                        kind,
                        uri.clone(),
                        title.clone(),
                        description.clone(),
                        get_json_string_map(
                            node,
                            &["dcterms:publisher", "dct:publisher", "publisher"],
                        ),
                        None,
                        conforms_to.clone(),
                        source_label,
                    ));
                    self.add_standard_signal_findings(
                        artifact_id,
                        &asset_id,
                        node,
                        &context_prefixes,
                    );
                }
            }

            if has_type(&node_types, "dcat:Catalog")
                || has_type(&node_types, "http://www.w3.org/ns/dcat#Catalog")
            {
                *kind = ArtifactKind::DcatCatalog;
                let asset_id = json_asset_id(artifact_id, uri.as_deref(), "dcat:Catalog");
                self.add_asset(json_asset(
                    artifact_id,
                    SemanticAssetKind::Catalog,
                    uri.clone(),
                    title.clone(),
                    description.clone(),
                    get_json_string_map(
                        node,
                        &[
                            "dcterms:publisher",
                            "dct:publisher",
                            "publisher",
                            "foaf:maker",
                        ],
                    ),
                    None,
                    conforms_to.clone(),
                    "dcat:Catalog",
                ));
                self.add_standard_signal_findings(artifact_id, &asset_id, node, &context_prefixes);
            }
            if has_type(&node_types, "dcat:Dataset")
                || has_type(&node_types, "http://www.w3.org/ns/dcat#Dataset")
            {
                let asset_id = json_asset_id(artifact_id, uri.as_deref(), "dcat:Dataset");
                self.add_asset(json_asset(
                    artifact_id,
                    SemanticAssetKind::Dataset,
                    uri.clone(),
                    title.clone(),
                    description.clone(),
                    get_json_string_map(node, &["dcterms:publisher", "dct:publisher", "publisher"]),
                    None,
                    conforms_to.clone(),
                    "dcat:Dataset",
                ));
                self.add_standard_signal_findings(artifact_id, &asset_id, node, &context_prefixes);
            }
            if has_type(&node_types, "dcat:DataService")
                || has_type(&node_types, "http://www.w3.org/ns/dcat#DataService")
            {
                let asset_id = json_asset_id(artifact_id, uri.as_deref(), "dcat:DataService");
                self.add_asset(json_asset(
                    artifact_id,
                    SemanticAssetKind::DataService,
                    uri.clone(),
                    title.clone(),
                    description.clone(),
                    get_json_string_map(node, &["dcterms:publisher", "dct:publisher", "publisher"]),
                    get_json_string_map(node, &["dcat:endpointURL", "endpointURL"]),
                    conforms_to.clone(),
                    "dcat:DataService",
                ));
                self.add_standard_signal_findings(artifact_id, &asset_id, node, &context_prefixes);
            }
            if has_type(&node_types, "dcat:Distribution") {
                let asset_id = json_asset_id(artifact_id, uri.as_deref(), "dcat:Distribution");
                self.add_asset(json_asset(
                    artifact_id,
                    SemanticAssetKind::Distribution,
                    uri.clone(),
                    title.clone(),
                    description.clone(),
                    None,
                    get_json_string_map(node, &["dcat:accessURL", "dcat:downloadURL"]),
                    conforms_to.clone(),
                    "dcat:Distribution",
                ));
                self.add_standard_signal_findings(artifact_id, &asset_id, node, &context_prefixes);
            }
            if has_type(&node_types, "prof:Profile")
                || has_type(&node_types, "http://www.w3.org/ns/dx/prof/Profile")
            {
                *kind = ArtifactKind::ProfProfile;
                self.add_asset(json_asset(
                    artifact_id,
                    SemanticAssetKind::Profile,
                    uri.clone(),
                    title.clone(),
                    description.clone(),
                    None,
                    None,
                    conforms_to.clone(),
                    "prof:Profile",
                ));
            }
            if has_type(&node_types, "skos:ConceptScheme") {
                self.add_asset(json_asset(
                    artifact_id,
                    SemanticAssetKind::ConceptScheme,
                    uri.clone(),
                    title.clone(),
                    description.clone(),
                    None,
                    None,
                    conforms_to.clone(),
                    "skos:ConceptScheme",
                ));
            }
            if is_odrl_policy_node(&node_types, node) {
                let asset_id = json_asset_id(artifact_id, uri.as_deref(), "odrl:Policy");
                let mut asset = json_asset(
                    artifact_id,
                    SemanticAssetKind::Policy,
                    uri.clone(),
                    title.clone().or_else(|| Some("Access policy".to_string())),
                    description.clone(),
                    get_json_string_map(node, &["odrl:assigner", "assigner"]),
                    None,
                    json_strings_for_keys(node, &["odrl:profile", "profile"])
                        .into_iter()
                        .map(|iri| expand_compact_iri(&iri, &context_prefixes).unwrap_or(iri))
                        .collect(),
                    "odrl:Policy",
                );
                asset
                    .source_hints
                    .extend(odrl_policy_source_hints(artifact_id, node));
                self.add_asset(asset);
                self.add_standard_signal_findings(artifact_id, &asset_id, node, &context_prefixes);
            }

            for iri in conforms_to {
                self.add_standard_or_profile_from_iri(
                    artifact_id,
                    &iri,
                    "dcterms:conformsTo",
                    None,
                );
            }

            for base in json_strings_for_keys(node, &["prof:isProfileOf"]) {
                if let Some(profile_iri) = uri.clone() {
                    let base = expand_compact_iri(&base, &context_prefixes).unwrap_or(base);
                    self.add_profile_claim(
                        artifact_id,
                        &profile_iri,
                        title.clone(),
                        None,
                        Some(base),
                        DiscoveryEvidence::JsonLdPredicate {
                            artifact_id: Some(artifact_id.to_string()),
                            predicate: "prof:isProfileOf".to_string(),
                            pointer: None,
                            value: Some(profile_iri.clone()),
                        },
                    );
                }
            }
        }

        self.add_json_ld_semantic_relations(artifact_id, &value, &context_prefixes);

        for (predicate, url) in json_ld_links(&value) {
            let predicate = canonical_compact_iri(&predicate, &context_prefixes);
            let url = expand_compact_iri(&url, &context_prefixes).unwrap_or(url);
            self.add_link_and_candidate(
                artifact_id,
                &fetched.url,
                &url,
                None,
                Some(predicate.clone()),
                predicate_role(&predicate),
                DiscoveryEvidence::JsonLdPredicate {
                    artifact_id: Some(artifact_id.to_string()),
                    predicate,
                    pointer: None,
                    value: Some(url.clone()),
                },
            );
        }
        self.add_json_ld_shacl_property_findings(artifact_id, &value, &context_prefixes);

        Ok(())
    }

    fn parse_metadata_index(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        body_text: &str,
    ) -> Result<(), String> {
        let value: Value = serde_json::from_str(body_text).map_err(|error| error.to_string())?;
        if let Some(links) = value.get("links").and_then(Value::as_array) {
            for (index, link) in links.iter().enumerate() {
                if let Some(href) = get_json_string(link, &["href"]) {
                    self.add_link_and_candidate(
                        artifact_id,
                        &fetched.url,
                        &href,
                        get_json_string(link, &["rel"]),
                        None,
                        get_json_string(link, &["type"]),
                        DiscoveryEvidence::JsonPointer {
                            artifact_id: Some(artifact_id.to_string()),
                            pointer: format!("/links/{index}/href"),
                            value: Some(href.clone()),
                        },
                    );
                }
            }
        }
        if value.get("datasets").and_then(Value::as_array).is_some() {
            self.add_relay_catalog_assets(artifact_id, fetched, &value);
        }
        Ok(())
    }

    fn add_relay_catalog_assets(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        value: &Value,
    ) {
        let base_url = get_json_string(value, &["base_url"]);
        let catalog_uri = get_json_string(value, &["id"])
            .or_else(|| fetched.final_url.clone())
            .or_else(|| Some(fetched.url.clone()));
        let publisher = get_json_string(value, &["publisher"]);
        let conforms_to = json_strings_for_value_keys(value, &["conforms_to", "conformsTo"]);

        self.add_asset(json_asset(
            artifact_id,
            SemanticAssetKind::Catalog,
            catalog_uri,
            get_json_string(value, &["title"]),
            get_json_string(value, &["description"]),
            publisher.clone(),
            None,
            conforms_to,
            "metadata index catalog",
        ));

        let Some(datasets) = value.get("datasets").and_then(Value::as_array) else {
            return;
        };
        for dataset in datasets {
            let Some(dataset_object) = dataset.as_object() else {
                continue;
            };
            let dataset_id = get_json_string_map(dataset_object, &["dataset_id", "id"]);
            let dataset_uri =
                get_json_string_map(dataset_object, &["@id", "uri", "url"]).or_else(|| {
                    base_url
                        .as_deref()
                        .zip(dataset_id.as_deref())
                        .map(|(base, id)| format!("{}/datasets/{id}", base.trim_end_matches('/')))
                });
            self.add_asset(json_asset(
                artifact_id,
                SemanticAssetKind::Dataset,
                dataset_uri,
                get_json_string_map(dataset_object, &["title", "name"]),
                get_json_string_map(dataset_object, &["description"]),
                get_json_string_map(dataset_object, &["publisher"]).or_else(|| publisher.clone()),
                None,
                json_strings_for_keys(dataset_object, &["conforms_to", "conformsTo"]),
                "metadata index dataset",
            ));
        }
    }

    fn add_json_ld_shacl_property_findings(
        &mut self,
        artifact_id: &str,
        value: &Value,
        prefixes: &HashMap<String, String>,
    ) {
        for property in json_ld_shacl_properties(value, prefixes) {
            self.add_evidence_finding(
                artifact_id,
                "semantic.shacl_property",
                "Embedded JSON-LD SHACL property path evidence",
                DiscoveryEvidence::ShaclProperty {
                    artifact_id: artifact_id.to_string(),
                    shape: property.shape,
                    path: property.path.clone(),
                    predicate: "sh:path".to_string(),
                    value: Some(property.value.unwrap_or(property.path)),
                },
            );
        }
    }

    fn parse_json_schema(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        body_text: &str,
    ) -> Result<(), String> {
        let value: Value = serde_json::from_str(body_text).map_err(|error| error.to_string())?;
        let uri = get_json_string(&value, &["$id"])
            .or_else(|| fetched.final_url.clone())
            .or_else(|| Some(fetched.url.clone()));
        self.add_standard_or_profile_from_iri(
            artifact_id,
            &get_json_string(&value, &["$schema"])
                .unwrap_or_else(|| "https://json-schema.org/draft/2020-12/schema".to_string()),
            "$schema",
            None,
        );
        self.add_asset(SemanticAsset {
            id: deterministic_id(
                "asset",
                [artifact_id, "json-schema", uri.as_deref().unwrap_or("")],
            ),
            kind: SemanticAssetKind::Class,
            artifact_id: artifact_id.to_string(),
            uri,
            title: get_json_string(&value, &["title"]),
            description: get_json_string(&value, &["description"]),
            publisher: None,
            endpoint_url: None,
            conforms_to: get_json_string(&value, &["$schema"]).into_iter().collect(),
            source_hints: vec![SourceHint {
                label: "JSON Schema".to_string(),
                predicate: Some("$schema".to_string()),
                path: Some("/$schema".to_string()),
                artifact_id: artifact_id.to_string(),
            }],
            raw_refs: vec![RawReference {
                artifact_id: artifact_id.to_string(),
                pointer: Some("/".to_string()),
                subject_iri: None,
            }],
        });

        for (key, role) in [("$id", "schema-id"), ("$schema", "schema-meta")] {
            if let Some(url) = get_json_string(&value, &[key]) {
                self.add_link_and_candidate(
                    artifact_id,
                    &fetched.url,
                    &url,
                    None,
                    Some(key.to_string()),
                    Some(role.to_string()),
                    DiscoveryEvidence::JsonPointer {
                        artifact_id: Some(artifact_id.to_string()),
                        pointer: format!("/{key}"),
                        value: Some(url.clone()),
                    },
                );
            }
        }

        for (pointer, url) in json_schema_refs(&value, "") {
            self.add_link_and_candidate(
                artifact_id,
                &fetched.url,
                &url,
                None,
                Some("$ref".to_string()),
                Some("schema-reference".to_string()),
                DiscoveryEvidence::JsonPointer {
                    artifact_id: Some(artifact_id.to_string()),
                    pointer,
                    value: Some(url.clone()),
                },
            );
        }
        for property in json_schema_properties(&value) {
            self.add_evidence_finding(
                artifact_id,
                "semantic.schema_property",
                "JSON Schema property evidence",
                DiscoveryEvidence::SchemaProperty {
                    artifact_id: artifact_id.to_string(),
                    schema_pointer: property.schema_pointer,
                    property_path: property.property_path,
                    property_name: property.property_name,
                    value: property.value,
                },
            );
        }
        Ok(())
    }

    fn parse_openapi(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        body_text: &str,
    ) -> Result<(), String> {
        let value: Value = serde_json::from_str(body_text).map_err(|error| error.to_string())?;
        self.add_standard_or_profile_from_iri(
            artifact_id,
            "https://spec.openapis.org/oas/v3.1.0",
            "openapi",
            get_json_string(&value, &["openapi"]),
        );
        self.add_asset(SemanticAsset {
            id: deterministic_id("asset", [artifact_id, "openapi"]),
            kind: SemanticAssetKind::ApiDescription,
            artifact_id: artifact_id.to_string(),
            uri: fetched
                .final_url
                .clone()
                .or_else(|| Some(fetched.url.clone())),
            title: value
                .get("info")
                .and_then(|info| get_json_string(info, &["title"]))
                .or_else(|| Some("OpenAPI description".to_string())),
            description: value
                .get("info")
                .and_then(|info| get_json_string(info, &["description"])),
            publisher: None,
            endpoint_url: first_server_url(&value),
            conforms_to: vec!["https://spec.openapis.org/oas/v3.1.0".to_string()],
            source_hints: vec![SourceHint {
                label: "OpenAPI".to_string(),
                predicate: Some("openapi".to_string()),
                path: Some("/openapi".to_string()),
                artifact_id: artifact_id.to_string(),
            }],
            raw_refs: vec![RawReference {
                artifact_id: artifact_id.to_string(),
                pointer: Some("/".to_string()),
                subject_iri: None,
            }],
        });

        if let Some(url) = value
            .get("externalDocs")
            .and_then(|external| get_json_string(external, &["url"]))
        {
            self.add_link_and_candidate(
                artifact_id,
                &fetched.url,
                &url,
                Some("describedby".to_string()),
                None,
                Some("external-docs".to_string()),
                DiscoveryEvidence::JsonPointer {
                    artifact_id: Some(artifact_id.to_string()),
                    pointer: "/externalDocs/url".to_string(),
                    value: Some(url.clone()),
                },
            );
        }
        if let Some(servers) = value.get("servers").and_then(Value::as_array) {
            for (index, server) in servers.iter().enumerate() {
                if let Some(url) = get_json_string(server, &["url"]) {
                    self.add_link_and_candidate(
                        artifact_id,
                        &fetched.url,
                        &url,
                        None,
                        Some("servers".to_string()),
                        Some("api-server".to_string()),
                        DiscoveryEvidence::JsonPointer {
                            artifact_id: Some(artifact_id.to_string()),
                            pointer: format!("/servers/{index}/url"),
                            value: Some(url.clone()),
                        },
                    );
                }
            }
        }
        if let Some(paths) = value.get("paths").and_then(Value::as_object) {
            for (path, path_item) in paths {
                let Some(operations) = path_item.as_object() else {
                    continue;
                };
                for (method, operation) in operations {
                    if !is_openapi_method(method) {
                        continue;
                    }
                    self.add_evidence_finding(
                        artifact_id,
                        "semantic.openapi_operation",
                        "OpenAPI operation evidence",
                        DiscoveryEvidence::OpenApiOperation {
                            artifact_id: artifact_id.to_string(),
                            path: path.clone(),
                            method: method.to_ascii_lowercase(),
                            operation_id: get_json_string(operation, &["operationId"]),
                            summary: get_json_string(operation, &["summary"]),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn parse_ogc(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        body_text: &str,
        kind: &mut ArtifactKind,
    ) -> Result<(), String> {
        let value: Value = serde_json::from_str(body_text).map_err(|error| error.to_string())?;
        let conforms_to = json_strings_for_value_keys(&value, &["conformsTo"]);
        let primary = primary_ogc_kind(&conforms_to);
        if primary != ArtifactKind::Unknown {
            *kind = primary;
        }
        for iri in &conforms_to {
            self.add_standard_or_profile_from_iri(artifact_id, iri, "conformsTo", None);
        }

        self.add_asset(SemanticAsset {
            id: deterministic_id("asset", [artifact_id, "ogc"]),
            kind: if *kind == ArtifactKind::OgcFeatures {
                SemanticAssetKind::FeatureCollection
            } else {
                SemanticAssetKind::RecordCollection
            },
            artifact_id: artifact_id.to_string(),
            uri: fetched
                .final_url
                .clone()
                .or_else(|| Some(fetched.url.clone())),
            title: get_json_string(&value, &["title"])
                .or_else(|| Some("OGC API landing page".to_string())),
            description: get_json_string(&value, &["description"]),
            publisher: None,
            endpoint_url: None,
            conforms_to,
            source_hints: vec![SourceHint {
                label: "OGC API".to_string(),
                predicate: Some("conformsTo".to_string()),
                path: Some("/conformsTo".to_string()),
                artifact_id: artifact_id.to_string(),
            }],
            raw_refs: vec![RawReference {
                artifact_id: artifact_id.to_string(),
                pointer: Some("/".to_string()),
                subject_iri: None,
            }],
        });

        if let Some(links) = value.get("links").and_then(Value::as_array) {
            for (index, link) in links.iter().enumerate() {
                if let Some(href) = get_json_string(link, &["href"]) {
                    self.add_link_and_candidate(
                        artifact_id,
                        &fetched.url,
                        &href,
                        get_json_string(link, &["rel"]),
                        None,
                        get_json_string(link, &["type"]),
                        DiscoveryEvidence::JsonPointer {
                            artifact_id: Some(artifact_id.to_string()),
                            pointer: format!("/links/{index}/href"),
                            value: Some(href.clone()),
                        },
                    );
                }
            }
        }
        if let Some(collections) = value.get("collections").and_then(Value::as_array) {
            for collection in collections {
                let Some(collection_id) = get_json_string(collection, &["id"]) else {
                    continue;
                };
                self.add_evidence_finding(
                    artifact_id,
                    "semantic.ogc_collection",
                    "OGC collection evidence",
                    DiscoveryEvidence::OgcCollection {
                        artifact_id: artifact_id.to_string(),
                        collection_id,
                        title: get_json_string(collection, &["title"]),
                    },
                );
            }
        }
        Ok(())
    }

    fn parse_turtle(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        body_text: &str,
        kind: &mut ArtifactKind,
    ) -> Result<(), String> {
        let triples = parse_turtle_triples(body_text)?;
        let by_subject = triples_by_subject(&triples);
        for (subject, subject_triples) in &by_subject {
            let types: Vec<_> = subject_triples
                .iter()
                .filter(|triple| is_type_predicate(&triple.predicate))
                .map(|triple| triple.object.clone())
                .collect();
            let title = object_for_predicates(
                subject_triples,
                &[
                    "http://www.w3.org/2000/01/rdf-schema#label",
                    "http://purl.org/dc/terms/title",
                    "http://www.w3.org/2004/02/skos/core#prefLabel",
                ],
            );
            let description = object_for_predicates(
                subject_triples,
                &[
                    "http://purl.org/dc/terms/description",
                    "http://www.w3.org/2004/02/skos/core#definition",
                ],
            );

            if type_contains(&types, "NodeShape")
                || has_predicate(subject_triples, "http://www.w3.org/ns/shacl#targetClass")
            {
                *kind = ArtifactKind::Shacl;
                self.add_asset(turtle_asset(
                    artifact_id,
                    SemanticAssetKind::ShapeGraph,
                    subject.clone(),
                    title.clone(),
                    description.clone(),
                    "sh:NodeShape",
                ));
            }
            if type_contains(&types, "ConceptScheme") {
                *kind = ArtifactKind::Skos;
                self.add_asset(turtle_asset(
                    artifact_id,
                    SemanticAssetKind::ConceptScheme,
                    subject.clone(),
                    title.clone(),
                    description.clone(),
                    "skos:ConceptScheme",
                ));
            }
            if type_contains(&types, "Class") || type_contains(&types, "Ontology") {
                if type_contains(&types, "Ontology") {
                    *kind = ArtifactKind::OwlOntology;
                }
                self.add_asset(turtle_asset(
                    artifact_id,
                    SemanticAssetKind::Class,
                    subject.clone(),
                    title.clone(),
                    description.clone(),
                    "owl/rdfs class",
                ));
            }
            if subject_triples
                .iter()
                .any(|triple| is_alignment_predicate(&triple.predicate))
            {
                self.add_asset(turtle_asset(
                    artifact_id,
                    SemanticAssetKind::Alignment,
                    subject.clone(),
                    title.clone(),
                    description.clone(),
                    "alignment mapping",
                ));
            }
        }

        for triple in &triples {
            if triple.predicate == "http://www.w3.org/ns/shacl#path" {
                self.add_evidence_finding(
                    artifact_id,
                    "semantic.shacl_property",
                    "SHACL property path evidence",
                    DiscoveryEvidence::ShaclProperty {
                        artifact_id: artifact_id.to_string(),
                        shape: shacl_property_shape(&triple.subject, &by_subject)
                            .unwrap_or_else(|| triple.subject.clone()),
                        path: triple.object.clone(),
                        predicate: "sh:path".to_string(),
                        value: Some(triple.object.clone()),
                    },
                );
            }
            if is_link_predicate(&triple.predicate) {
                self.add_link_and_candidate(
                    artifact_id,
                    &fetched.url,
                    &triple.object,
                    None,
                    Some(compact_predicate(&triple.predicate)),
                    predicate_role(&compact_predicate(&triple.predicate)),
                    DiscoveryEvidence::JsonLdPredicate {
                        artifact_id: Some(artifact_id.to_string()),
                        predicate: compact_predicate(&triple.predicate),
                        pointer: None,
                        value: Some(triple.object.clone()),
                    },
                );
            }

            if triple.predicate == "http://purl.org/dc/terms/conformsTo" {
                self.add_standard_or_profile_from_iri(
                    artifact_id,
                    &triple.object,
                    "dcterms:conformsTo",
                    None,
                );
            }
            if triple.predicate == "http://www.w3.org/ns/dx/prof/isProfileOf" {
                self.add_profile_claim(
                    artifact_id,
                    &triple.subject,
                    None,
                    None,
                    Some(triple.object.clone()),
                    DiscoveryEvidence::JsonLdPredicate {
                        artifact_id: Some(artifact_id.to_string()),
                        predicate: "prof:isProfileOf".to_string(),
                        pointer: None,
                        value: Some(triple.subject.clone()),
                    },
                );
            }
        }
        Ok(())
    }

    fn parse_semantic_package(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        body_text: &str,
    ) -> Result<(), String> {
        let value: toml::Value = toml::from_str(body_text).map_err(|error| error.to_string())?;
        let name = value
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .or_else(|| value.get("name").and_then(toml::Value::as_str))
            .unwrap_or("Semantic asset package");
        let version = value
            .get("package")
            .and_then(|package| package.get("version"))
            .and_then(toml::Value::as_str)
            .or_else(|| value.get("version").and_then(toml::Value::as_str));
        let package_id = value
            .get("package")
            .and_then(|package| package.get("id"))
            .and_then(toml::Value::as_str)
            .or_else(|| value.get("id").and_then(toml::Value::as_str));
        self.add_asset(SemanticAsset {
            id: deterministic_id("asset", [artifact_id, "semantic-package", name]),
            kind: SemanticAssetKind::SemanticModelPackage,
            artifact_id: artifact_id.to_string(),
            uri: package_id
                .map(str::to_string)
                .or_else(|| fetched.final_url.clone())
                .or_else(|| Some(fetched.url.clone())),
            title: value
                .get("package")
                .and_then(|package| package.get("title"))
                .and_then(toml::Value::as_str)
                .or_else(|| value.get("title").and_then(toml::Value::as_str))
                .or(Some(name))
                .map(str::to_string),
            description: value
                .get("package")
                .and_then(|package| package.get("description"))
                .and_then(toml::Value::as_str)
                .or_else(|| value.get("description").and_then(toml::Value::as_str))
                .map(str::to_string),
            publisher: value
                .get("package")
                .and_then(|package| package.get("publisher"))
                .and_then(toml::Value::as_str)
                .or_else(|| value.get("publisher").and_then(toml::Value::as_str))
                .map(str::to_string),
            endpoint_url: fetched
                .final_url
                .clone()
                .or_else(|| Some(fetched.url.clone())),
            conforms_to: version
                .map(|value| vec![format!("semantic-asset-package:{value}")])
                .unwrap_or_default(),
            source_hints: vec![SourceHint {
                label: "Semantic asset package manifest".to_string(),
                predicate: Some("package".to_string()),
                path: Some("/package".to_string()),
                artifact_id: artifact_id.to_string(),
            }],
            raw_refs: vec![RawReference {
                artifact_id: artifact_id.to_string(),
                pointer: Some("/".to_string()),
                subject_iri: None,
            }],
        });

        if let Some(artifacts) = value.get("artifacts").and_then(toml::Value::as_array) {
            for (index, artifact) in artifacts.iter().enumerate() {
                let Some(role) = artifact.get("role").and_then(toml::Value::as_str) else {
                    continue;
                };
                let asset_kind = match role {
                    "alignment" => Some(SemanticAssetKind::Alignment),
                    "crosswalk" => Some(SemanticAssetKind::Crosswalk),
                    _ => None,
                };
                let Some(kind) = asset_kind else {
                    continue;
                };
                let path = artifact
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .unwrap_or(role);
                self.add_asset(SemanticAsset {
                    id: deterministic_id("asset", [artifact_id, role, path]),
                    kind,
                    artifact_id: artifact_id.to_string(),
                    uri: resolve_url(&fetched.url, path).ok(),
                    title: Some(format!("{name} {role}")),
                    description: None,
                    publisher: None,
                    endpoint_url: None,
                    conforms_to: artifact
                        .get("conforms_to")
                        .and_then(toml::Value::as_str)
                        .map(|iri| vec![iri.to_string()])
                        .unwrap_or_default(),
                    source_hints: vec![SourceHint {
                        label: format!("Semantic package {role}"),
                        predicate: Some("artifacts.role".to_string()),
                        path: Some(format!("/artifacts/{index}/role")),
                        artifact_id: artifact_id.to_string(),
                    }],
                    raw_refs: vec![RawReference {
                        artifact_id: artifact_id.to_string(),
                        pointer: Some(format!("/artifacts/{index}")),
                        subject_iri: None,
                    }],
                });
            }
        }

        for url in toml_urls(&value) {
            self.add_link_and_candidate(
                artifact_id,
                &fetched.url,
                &url,
                Some("describedby".to_string()),
                None,
                Some("semantic-package-artifact".to_string()),
                DiscoveryEvidence::JsonPointer {
                    artifact_id: Some(artifact_id.to_string()),
                    pointer: "/".to_string(),
                    value: Some(url.clone()),
                },
            );
        }
        Ok(())
    }

    fn parse_linkml(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        body_text: &str,
    ) -> Result<(), String> {
        let value: serde_yaml::Value =
            serde_yaml::from_str(body_text).map_err(|error| error.to_string())?;
        let name = yaml_string(&value, "name").unwrap_or_else(|| "LinkML schema".to_string());
        let version = yaml_string(&value, "version");
        let prefixes = yaml_prefixes(&value);
        self.add_asset(SemanticAsset {
            id: deterministic_id("asset", [artifact_id, "linkml", &name]),
            kind: SemanticAssetKind::SemanticModelPackage,
            artifact_id: artifact_id.to_string(),
            uri: yaml_string(&value, "id")
                .or_else(|| fetched.final_url.clone())
                .or_else(|| Some(fetched.url.clone())),
            title: Some(name.clone()),
            description: yaml_string(&value, "description"),
            publisher: None,
            endpoint_url: None,
            conforms_to: version
                .map(|version| vec![format!("linkml:{version}")])
                .unwrap_or_else(|| vec!["https://w3id.org/linkml".to_string()]),
            source_hints: vec![SourceHint {
                label: "LinkML schema".to_string(),
                predicate: Some("name".to_string()),
                path: Some("/name".to_string()),
                artifact_id: artifact_id.to_string(),
            }],
            raw_refs: vec![RawReference {
                artifact_id: artifact_id.to_string(),
                pointer: Some("/".to_string()),
                subject_iri: None,
            }],
        });

        self.add_named_yaml_assets(artifact_id, &value, "classes", SemanticAssetKind::Class);
        self.add_named_yaml_assets(artifact_id, &value, "slots", SemanticAssetKind::Property);
        self.add_named_yaml_assets(artifact_id, &value, "enums", SemanticAssetKind::Vocabulary);

        if let Some(imports) = value
            .get("imports")
            .and_then(serde_yaml::Value::as_sequence)
        {
            for import in imports {
                if let Some(import_value) = import.as_str() {
                    let url = expand_compact_iri(import_value, &prefixes)
                        .unwrap_or_else(|| import_value.to_string());
                    self.add_link_and_candidate(
                        artifact_id,
                        &fetched.url,
                        &url,
                        Some("import".to_string()),
                        None,
                        Some("linkml-import".to_string()),
                        DiscoveryEvidence::JsonPointer {
                            artifact_id: Some(artifact_id.to_string()),
                            pointer: "/imports".to_string(),
                            value: Some(import_value.to_string()),
                        },
                    );
                }
            }
        }

        for url in yaml_urls(&value) {
            self.add_link_and_candidate(
                artifact_id,
                &fetched.url,
                &url,
                Some("describedby".to_string()),
                None,
                Some("linkml-reference".to_string()),
                DiscoveryEvidence::JsonPointer {
                    artifact_id: Some(artifact_id.to_string()),
                    pointer: "/".to_string(),
                    value: Some(url.clone()),
                },
            );
        }

        Ok(())
    }

    fn parse_html(
        &mut self,
        artifact_id: &str,
        fetched: &FetchedArtifact,
        body_text: &str,
    ) -> Result<(), String> {
        let link_re = Regex::new(r#"(?is)<link\s+[^>]*>"#).map_err(|error| error.to_string())?;
        let attr_re = Regex::new(r#"(?i)(rel|href|type)=["']([^"']+)["']"#)
            .map_err(|error| error.to_string())?;
        for (index, link_match) in link_re.find_iter(body_text).enumerate() {
            let mut rel = None;
            let mut href = None;
            let mut role = None;
            for cap in attr_re.captures_iter(link_match.as_str()) {
                match &cap[1].to_ascii_lowercase()[..] {
                    "rel" => rel = Some(cap[2].to_string()),
                    "href" => href = Some(cap[2].to_string()),
                    "type" => role = Some(cap[2].to_string()),
                    _ => {}
                }
            }
            if let (Some(rel_value), Some(href_value)) = (rel, href) {
                for rel_part in rel_value.split_whitespace() {
                    if matches!(
                        rel_part,
                        "alternate" | "describedby" | "canonical" | "profile"
                    ) {
                        self.add_link_and_candidate(
                            artifact_id,
                            &fetched.url,
                            &href_value,
                            Some(rel_part.to_string()),
                            None,
                            role.clone(),
                            DiscoveryEvidence::HtmlLink {
                                artifact_id: Some(artifact_id.to_string()),
                                rel: rel_part.to_string(),
                                href: href_value.clone(),
                                pointer: Some(format!("/html/link/{index}")),
                            },
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn extract_http_links(&mut self, artifact_id: &str, fetched: &FetchedArtifact) {
        for header in &fetched.headers {
            if header.name.eq_ignore_ascii_case("link") {
                for parsed in parse_link_header(&header.value) {
                    self.add_link_and_candidate(
                        artifact_id,
                        &fetched.url,
                        &parsed.url,
                        parsed.rel.clone(),
                        None,
                        parsed.kind.clone(),
                        DiscoveryEvidence::HttpHeader {
                            artifact_id: Some(artifact_id.to_string()),
                            header_name: "link".to_string(),
                            rel: parsed.rel,
                            value: Some(parsed.url.clone()),
                        },
                    );
                }
            }
        }
    }

    fn add_named_yaml_assets(
        &mut self,
        artifact_id: &str,
        value: &serde_yaml::Value,
        key: &str,
        kind: SemanticAssetKind,
    ) {
        if let Some(map) = value.get(key).and_then(serde_yaml::Value::as_mapping) {
            for (name_value, body) in map {
                if let Some(name) = name_value.as_str() {
                    self.add_asset(SemanticAsset {
                        id: deterministic_id("asset", [artifact_id, key, name]),
                        kind: kind.clone(),
                        artifact_id: artifact_id.to_string(),
                        uri: None,
                        title: Some(name.to_string()),
                        description: yaml_string(body, "description"),
                        publisher: None,
                        endpoint_url: None,
                        conforms_to: Vec::new(),
                        source_hints: vec![SourceHint {
                            label: format!("LinkML {key}"),
                            predicate: Some(key.to_string()),
                            path: Some(format!("/{key}/{name}")),
                            artifact_id: artifact_id.to_string(),
                        }],
                        raw_refs: vec![RawReference {
                            artifact_id: artifact_id.to_string(),
                            pointer: Some(format!("/{key}/{name}")),
                            subject_iri: None,
                        }],
                    });
                }
            }
        }
    }

    fn add_standard_or_profile_from_iri(
        &mut self,
        artifact_id: &str,
        iri: &str,
        predicate: &str,
        version: Option<String>,
    ) {
        let pack = self.profile_pack_for_iri(iri);
        let label = pack
            .map(|pack| pack.label.clone())
            .or_else(|| standard_label(iri));
        let evidence = DiscoveryEvidence::JsonLdPredicate {
            artifact_id: Some(artifact_id.to_string()),
            predicate: predicate.to_string(),
            pointer: None,
            value: Some(iri.to_string()),
        };
        if pack
            .map(is_profile_claim_pack)
            .unwrap_or_else(|| looks_like_profile(iri))
        {
            self.add_profile_claim(artifact_id, iri, label, version, None, evidence);
        } else {
            self.add_standard_claim(artifact_id, iri, label, version, evidence);
        }
    }

    fn profile_pack_for_iri(&self, iri: &str) -> Option<&ProfilePack> {
        let normalized = iri.trim_end_matches('/').to_ascii_lowercase();
        self.profile_packs.iter().find(|pack| {
            let pack_iri = pack.standard_iri.trim_end_matches('/').to_ascii_lowercase();
            normalized == pack_iri
                || normalized.starts_with(&format!("{pack_iri}/"))
                || pack_iri.starts_with(&format!("{normalized}/"))
        })
    }

    fn add_standard_claim(
        &mut self,
        artifact_id: &str,
        iri: &str,
        label: Option<String>,
        version: Option<String>,
        evidence: DiscoveryEvidence,
    ) {
        let id = deterministic_id("claim", [artifact_id, iri]);
        if self.seen_standards.insert(id.clone()) {
            self.standards.push(StandardClaim {
                id,
                iri: iri.to_string(),
                label,
                version,
                claimed_by_artifact_id: artifact_id.to_string(),
                evidence,
            });
        }
    }

    fn add_profile_claim(
        &mut self,
        artifact_id: &str,
        iri: &str,
        label: Option<String>,
        version: Option<String>,
        base_standard_iri: Option<String>,
        evidence: DiscoveryEvidence,
    ) {
        let id = deterministic_id("claim", [artifact_id, iri]);
        if self.seen_profiles.insert(id.clone()) {
            self.profiles.push(ProfileClaim {
                id,
                iri: iri.to_string(),
                label,
                version,
                base_standard_iri,
                claimed_by_artifact_id: artifact_id.to_string(),
                evidence,
            });
        }
    }

    fn add_asset(&mut self, asset: SemanticAsset) {
        if self.seen_assets.insert(asset.id.clone()) {
            self.assets.push(asset);
        }
    }

    fn add_json_ld_semantic_relations(
        &mut self,
        artifact_id: &str,
        value: &Value,
        prefixes: &HashMap<String, String>,
    ) {
        for node in json_nodes(value) {
            let Some(subject) = self.relation_endpoint_for_node(artifact_id, node, prefixes) else {
                continue;
            };
            for (key, raw_value) in node {
                if key.starts_with('@') {
                    continue;
                }
                let predicate = canonical_compact_iri(key, prefixes);
                if !is_required_relation_predicate(&predicate) {
                    continue;
                }
                for object in self.relation_endpoints_for_value(artifact_id, raw_value, prefixes) {
                    self.add_semantic_relation_claim(
                        artifact_id,
                        subject.clone(),
                        predicate.clone(),
                        object,
                        DiscoveryEvidence::JsonLdPredicate {
                            artifact_id: Some(artifact_id.to_string()),
                            predicate: predicate.clone(),
                            pointer: None,
                            value: Some(json_value_for_evidence(raw_value)),
                        },
                    );
                }
            }
        }
    }

    fn relation_endpoint_for_node(
        &self,
        artifact_id: &str,
        node: &Map<String, Value>,
        prefixes: &HashMap<String, String>,
    ) -> Option<RelationEndpoint> {
        if let Some(uri) = json_id(node).map(|value| expanded_iri(&value, prefixes)) {
            return Some(self.relation_endpoint_for_uri(&uri));
        }
        serde_json::to_string(node)
            .ok()
            .map(|key| RelationEndpoint::BlankNode {
                artifact_id: artifact_id.to_string(),
                node_id: deterministic_id("blank", [artifact_id, key.as_str()]),
            })
    }

    fn relation_endpoints_for_value(
        &self,
        artifact_id: &str,
        value: &Value,
        prefixes: &HashMap<String, String>,
    ) -> Vec<RelationEndpoint> {
        match value {
            Value::Array(values) => values
                .iter()
                .flat_map(|value| self.relation_endpoints_for_value(artifact_id, value, prefixes))
                .collect(),
            Value::Object(object) => {
                if let Some(uri) = json_id(object).map(|value| expanded_iri(&value, prefixes)) {
                    vec![self.relation_endpoint_for_uri(&uri)]
                } else {
                    serde_json::to_string(object)
                        .ok()
                        .map(|key| {
                            vec![RelationEndpoint::BlankNode {
                                artifact_id: artifact_id.to_string(),
                                node_id: deterministic_id("blank", [artifact_id, key.as_str()]),
                            }]
                        })
                        .unwrap_or_default()
                }
            }
            Value::String(value) => {
                let uri = expanded_iri(value, prefixes);
                if is_relation_resource_identifier(&uri) {
                    vec![self.relation_endpoint_for_uri(&uri)]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    fn relation_endpoint_for_uri(&self, uri: &str) -> RelationEndpoint {
        let uri = if Url::parse(uri)
            .ok()
            .as_ref()
            .is_some_and(contains_url_secret_material)
        {
            redact_url(uri)
        } else {
            uri.to_string()
        };
        if let Some(asset) = self
            .assets
            .iter()
            .find(|asset| asset.uri.as_deref() == Some(uri.as_str()))
        {
            RelationEndpoint::Asset {
                asset_id: asset.id.clone(),
                uri: Some(uri),
            }
        } else {
            RelationEndpoint::External { uri }
        }
    }

    fn add_semantic_relation_claim(
        &mut self,
        artifact_id: &str,
        subject: RelationEndpoint,
        predicate: String,
        object: RelationEndpoint,
        evidence: DiscoveryEvidence,
    ) {
        let relation_key = [
            relation_endpoint_key(&subject),
            predicate.clone(),
            relation_endpoint_key(&object),
        ]
        .join("|");
        let relation_id = deterministic_id("relation", [relation_key.as_str()]);
        if self.seen_relations.insert(relation_id.clone()) {
            self.relations.push(SemanticRelation {
                id: relation_id.clone(),
                subject,
                predicate,
                object,
                label: None,
            });
        }

        let evidence_key = serde_json::to_string(&evidence).unwrap_or_default();
        let claim_id = deterministic_id(
            "relation-claim",
            [relation_id.as_str(), artifact_id, evidence_key.as_str()],
        );
        if self.seen_relation_claims.insert(claim_id.clone()) {
            self.relation_claims.push(RelationClaim {
                id: claim_id,
                relation_id,
                asserted_by_artifact_id: artifact_id.to_string(),
                evidence,
                qualifiers: Vec::new(),
                contradicts: Vec::new(),
            });
        }
    }

    fn add_finding(&mut self, finding: DiscoveryFinding) {
        if !self
            .findings
            .iter()
            .any(|existing| existing.id == finding.id)
        {
            self.findings.push(finding);
        }
    }

    fn add_evidence_finding(
        &mut self,
        artifact_id: &str,
        code: &str,
        message: &str,
        evidence: DiscoveryEvidence,
    ) {
        let evidence_key = serde_json::to_string(&evidence).unwrap_or_else(|_| message.to_string());
        self.add_finding(DiscoveryFinding {
            id: deterministic_id("finding", [code, artifact_id, &evidence_key]),
            severity: FindingSeverity::Info,
            code: code.to_string(),
            message: message.to_string(),
            artifact_id: Some(artifact_id.to_string()),
            asset_id: None,
            standard_iri: None,
            evidence: Some(evidence),
        });
    }

    fn add_standard_signal_findings(
        &mut self,
        artifact_id: &str,
        asset_id: &str,
        node: &Map<String, Value>,
        context_prefixes: &HashMap<String, String>,
    ) {
        // This layer only preserves standard predicate evidence from
        // DCAT-AP/BRegDCAT-AP/CPSV/ELI-shaped metadata. It deliberately
        // does not decide whether a dataset is authoritative; that is an
        // Atlas interpretation performed downstream with evidence attached.
        for signal in standard_signal_predicates() {
            for raw_value in json_strings_for_keys(node, signal.keys) {
                let value = expand_compact_iri(&raw_value, context_prefixes).unwrap_or(raw_value);
                let evidence = DiscoveryEvidence::JsonLdPredicate {
                    artifact_id: Some(artifact_id.to_string()),
                    predicate: signal.predicate.to_string(),
                    pointer: None,
                    value: Some(value),
                };
                self.add_finding(DiscoveryFinding {
                    id: deterministic_id(
                        "finding",
                        [
                            "semantic.standard_signal",
                            artifact_id,
                            asset_id,
                            signal.predicate,
                            &serde_json::to_string(&evidence)
                                .unwrap_or_else(|_| signal.predicate.to_string()),
                        ],
                    ),
                    severity: FindingSeverity::Info,
                    code: "semantic.standard_signal".to_string(),
                    message: format!("Standard semantic signal {}", signal.predicate),
                    artifact_id: Some(artifact_id.to_string()),
                    asset_id: Some(asset_id.to_string()),
                    standard_iri: None,
                    evidence: Some(evidence),
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_link_and_candidate(
        &mut self,
        artifact_id: &str,
        from_url: &str,
        raw_to_url: &str,
        rel: Option<String>,
        predicate: Option<String>,
        role: Option<String>,
        evidence: DiscoveryEvidence,
    ) {
        let to_url = match resolve_url(from_url, raw_to_url) {
            Ok(url) => url,
            Err(message) => {
                self.add_link_rejection_finding(artifact_id, raw_to_url, message, evidence);
                return;
            }
        };
        let Ok(parsed_url) = Url::parse(&to_url) else {
            self.add_link_rejection_finding(
                artifact_id,
                raw_to_url,
                "Resolved link is not a syntactically valid URL.".to_string(),
                evidence,
            );
            return;
        };
        let scheme = parsed_url.scheme().to_ascii_lowercase();
        if contains_url_secret_material(&parsed_url) {
            self.add_link_rejection_finding(
                artifact_id,
                &redact_url(&to_url),
                "Rejected link target containing URL credentials or sensitive query parameters."
                    .to_string(),
                redact_evidence(evidence),
            );
            return;
        }
        if !self
            .options
            .accepted_schemes
            .iter()
            .any(|accepted| accepted == &scheme)
        {
            self.add_link_rejection_finding(
                artifact_id,
                &to_url,
                format!("Rejected link with unsupported URL scheme `{scheme}`."),
                evidence,
            );
            return;
        }

        let link_id = deterministic_id(
            "link",
            [
                from_url,
                &to_url,
                rel.as_deref().or(predicate.as_deref()).unwrap_or(""),
            ],
        );
        if self.seen_links.insert(link_id.clone()) {
            self.links.push(DiscoveredLink {
                id: link_id,
                from_artifact_id: Some(artifact_id.to_string()),
                from_url: from_url.to_string(),
                to_url: to_url.clone(),
                rel: rel.clone(),
                predicate: predicate.clone(),
                role: role.clone(),
                confidence: LinkConfidence::Declared,
                discovered_by: evidence.clone(),
            });
        }

        if !is_fetchable_metadata_link(rel.as_deref(), predicate.as_deref(), role.as_deref()) {
            return;
        }

        let candidate_id = deterministic_id("fetch", [artifact_id, &to_url]);
        if self.seen_next_fetches.insert(candidate_id.clone()) {
            self.next_fetches.push(FetchCandidate {
                id: candidate_id,
                url: to_url,
                depth: 1,
                priority: candidate_priority(rel.as_deref(), predicate.as_deref()),
                reason: role
                    .clone()
                    .filter(|value| value.contains("linkml"))
                    .or(rel)
                    .or(predicate)
                    .unwrap_or_else(|| "linked artifact".to_string()),
                discovered_from: from_url.to_string(),
                discovered_by: evidence,
            });
        }
    }

    fn add_link_rejection_finding(
        &mut self,
        artifact_id: &str,
        raw_url: &str,
        message: String,
        evidence: DiscoveryEvidence,
    ) {
        self.add_finding(DiscoveryFinding {
            id: deterministic_id(
                "finding",
                ["link.rejected_by_core", artifact_id, &redact_url(raw_url)],
            ),
            severity: FindingSeverity::Info,
            code: "link.rejected_by_core".to_string(),
            message,
            artifact_id: Some(artifact_id.to_string()),
            asset_id: None,
            standard_iri: None,
            evidence: Some(redact_evidence(evidence)),
        });
    }
}

fn classify_artifact(fetched: &FetchedArtifact, body_text: &str) -> ArtifactKind {
    let url = fetched
        .final_url
        .as_deref()
        .unwrap_or(&fetched.url)
        .to_ascii_lowercase();
    let media_type = fetched
        .media_type
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let trimmed = body_text.trim_start();

    if url.ends_with("semantic-asset-package.v1.toml")
        || trimmed.contains("[package]") && body_text.contains("artifacts")
    {
        return ArtifactKind::SemanticModelPackage;
    }
    if url.ends_with(".linkml.yaml") || url.ends_with(".linkml.yml") || is_linkml_yaml(trimmed) {
        return ArtifactKind::LinkMlSchema;
    }
    if media_type.contains("html")
        || trimmed.starts_with("<!doctype html")
        || trimmed.starts_with("<html")
    {
        return ArtifactKind::HtmlLandingPage;
    }
    if media_type.contains("turtle") || url.ends_with(".ttl") {
        return classify_turtle_body(body_text);
    }
    if media_type.contains("schema+json") || url.ends_with(".schema.json") {
        return ArtifactKind::JsonSchema;
    }
    if media_type.contains("openapi") || url.ends_with("openapi.json") {
        return ArtifactKind::OpenApi;
    }
    if media_type.contains("json") || trimmed.starts_with('{') || trimmed.starts_with('[') {
        return classify_json_body(body_text);
    }
    ArtifactKind::Unknown
}

fn classify_json_body(body_text: &str) -> ArtifactKind {
    let Ok(value) = serde_json::from_str::<Value>(body_text) else {
        return ArtifactKind::Unknown;
    };
    if value.get("openapi").is_some() {
        return ArtifactKind::OpenApi;
    }
    if value.get("$schema").is_some() || value.get("$id").is_some() {
        return ArtifactKind::JsonSchema;
    }
    if value.get("conformsTo").is_some() && value.get("links").is_some() {
        return primary_ogc_kind(&json_strings_for_value_keys(&value, &["conformsTo"]));
    }
    if value.get("links").and_then(Value::as_array).is_some() {
        return ArtifactKind::MetadataIndex;
    }
    if value.get("datasets").and_then(Value::as_array).is_some() {
        return ArtifactKind::MetadataIndex;
    }
    if is_jsonld_context_doc(&value) {
        return ArtifactKind::JsonLdContext;
    }
    let text = body_text.to_ascii_lowercase();
    if contains_shacl_signal(&text) {
        return ArtifactKind::Shacl;
    }
    if contains_odrl_policy_signal(&text) {
        return ArtifactKind::MetadataIndex;
    }
    if text.contains("dcat:catalog")
        || text.contains("dcat:dataset")
        || text.contains("dcat:dataservice")
        || text.contains("dcat#catalog")
        || text.contains("cpsv:publicservice")
        || text.contains("cv:channel")
        || text.contains("cv:requirement")
        || text.contains("cccev:requirement")
        || text.contains("cccev:evidencetype")
        || text.contains("data.europa.eu/m8g")
    {
        return ArtifactKind::DcatCatalog;
    }
    if text.contains("prof:profile") || text.contains("prof/profile") {
        return ArtifactKind::ProfProfile;
    }
    if text.contains("did:") && text.contains("verificationmethod") {
        return ArtifactKind::DidDocument;
    }
    ArtifactKind::Unknown
}

fn is_json_body(body_text: &str, media_type: Option<&str>) -> bool {
    media_type
        .map(|value| value.to_ascii_lowercase().contains("json"))
        .unwrap_or(false)
        || body_text.trim_start().starts_with('{')
        || body_text.trim_start().starts_with('[')
}

fn is_json_ld_like_metadata(body_text: &str) -> bool {
    let text = body_text.to_ascii_lowercase();
    text.contains("\"@context\"")
        || text.contains("\"@graph\"")
        || text.contains("\"@type\"")
        || contains_odrl_policy_signal(&text)
        || contains_shacl_signal(&text)
}

fn contains_shacl_signal(text: &str) -> bool {
    text.contains("sh:nodeshape")
        || text.contains("sh:propertyshape")
        || text.contains("sh:targetclass")
        || text.contains("sh:path")
        || text.contains("www.w3.org/ns/shacl")
}

fn contains_odrl_policy_signal(text: &str) -> bool {
    text.contains("odrl:offer")
        || text.contains("odrl:set")
        || text.contains("odrl:agreement")
        || text.contains("odrl:policy")
        || text.contains("odrl:permission")
        || text.contains("odrl:prohibition")
        || text.contains("odrl:obligation")
        || text.contains("www.w3.org/ns/odrl")
}

fn classify_turtle_body(body_text: &str) -> ArtifactKind {
    let text = body_text.to_ascii_lowercase();
    if text.contains("sh:nodeshape")
        || text.contains("sh:propertyshape")
        || text.contains("sh:targetclass")
    {
        ArtifactKind::Shacl
    } else if text.contains("skos:conceptscheme") {
        ArtifactKind::Skos
    } else if text.contains("owl:ontology") || text.contains("owl:class") {
        ArtifactKind::OwlOntology
    } else {
        ArtifactKind::Unknown
    }
}

fn http_status_to_artifact_status(status: u16) -> ArtifactStatus {
    match status {
        200..=299 => ArtifactStatus::Fetched,
        401 | 403 => ArtifactStatus::AuthRequired,
        404..=599 => ArtifactStatus::Failed,
        _ => ArtifactStatus::Failed,
    }
}

fn redact_headers(headers: Vec<HeaderPair>) -> Vec<HeaderPair> {
    headers
        .into_iter()
        .map(|header| {
            if is_sensitive_header(&header.name) {
                HeaderPair {
                    name: header.name,
                    value: REDACTED.to_string(),
                }
            } else {
                header
            }
        })
        .collect()
}

fn is_sensitive_header(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    SENSITIVE_HEADER_NAMES
        .iter()
        .any(|sensitive| *sensitive == normalized)
}

fn normalized_artifact_url(fetched: &FetchedArtifact) -> String {
    normalize_url(fetched.final_url.as_deref().unwrap_or(fetched.url.as_str())).unwrap_or_else(
        || {
            fetched
                .final_url
                .clone()
                .unwrap_or_else(|| fetched.url.clone())
        },
    )
}

fn normalize_url(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw).ok()?;
    url.set_fragment(None);
    Some(url.to_string())
}

fn deterministic_id<'a>(prefix: &str, parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hash = Sha256::new();
    hash.update(prefix.as_bytes());
    for part in parts {
        hash.update(b"\0");
        hash.update(part.as_bytes());
    }
    format!("{prefix}:{}", &hex::encode(hash.finalize())[..16])
}

fn hex_sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn json_nodes(value: &Value) -> Vec<&Map<String, Value>> {
    let mut nodes = Vec::new();
    collect_json_nodes(value, &mut nodes);
    nodes
}

fn collect_json_nodes<'a>(value: &'a Value, nodes: &mut Vec<&'a Map<String, Value>>) {
    match value {
        Value::Object(map) => {
            if map.contains_key("@id") || map.contains_key("@type") || map.contains_key("type") {
                nodes.push(map);
            }
            if let Some(graph) = map.get("@graph").and_then(Value::as_array) {
                for item in graph {
                    collect_json_nodes(item, nodes);
                }
            }
            for (key, nested) in map {
                if key == "@context" {
                    continue;
                }
                if matches!(nested, Value::Array(_) | Value::Object(_)) {
                    collect_json_nodes(nested, nodes);
                }
            }
        }
        Value::Array(values) => {
            for item in values {
                collect_json_nodes(item, nodes);
            }
        }
        _ => {}
    }
}

fn json_id(node: &Map<String, Value>) -> Option<String> {
    node.get("@id")
        .or_else(|| node.get("id"))
        .and_then(json_value_string)
}

fn get_json_string(value: &Value, keys: &[&str]) -> Option<String> {
    let object = value.as_object()?;
    get_json_string_map(object, keys)
}

fn get_json_string_map(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(json_value_string))
}

fn json_value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Object(object) => object
            .get("@id")
            .or_else(|| object.get("id"))
            .or_else(|| object.get("@value"))
            .or_else(|| object.get("value"))
            .or_else(|| object.get("foaf:name"))
            .or_else(|| object.get("name"))
            .or_else(|| object.get("dcterms:title"))
            .or_else(|| object.get("dct:title"))
            .and_then(json_value_string),
        Value::Array(values) => values.iter().find_map(json_value_string),
        _ => None,
    }
}

fn json_strings_for_keys(node: &Map<String, Value>, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .filter_map(|key| node.get(*key))
        .flat_map(json_strings)
        .collect()
}

fn json_strings_for_value_keys(value: &Value, keys: &[&str]) -> Vec<String> {
    value
        .as_object()
        .map(|node| json_strings_for_keys(node, keys))
        .unwrap_or_default()
}

fn json_strings(value: &Value) -> Vec<String> {
    match value {
        Value::String(value) => vec![value.clone()],
        Value::Array(values) => values.iter().flat_map(json_strings).collect(),
        Value::Object(object) => object
            .get("@id")
            .or_else(|| object.get("id"))
            .or_else(|| object.get("@value"))
            .or_else(|| object.get("value"))
            .or_else(|| object.get("foaf:name"))
            .or_else(|| object.get("name"))
            .or_else(|| object.get("dcterms:title"))
            .or_else(|| object.get("dct:title"))
            .map(json_strings)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn json_ld_context_prefixes(value: &Value) -> HashMap<String, String> {
    let mut prefixes = built_in_prefixes();
    collect_json_ld_context_prefixes(value, &mut prefixes);
    prefixes
}

fn collect_json_ld_context_prefixes(value: &Value, prefixes: &mut HashMap<String, String>) {
    match value {
        Value::Object(object) => {
            if let Some(context) = object.get("@context") {
                collect_context_value(context, prefixes);
            }
            for (key, nested) in object {
                if key != "@context" {
                    collect_json_ld_context_prefixes(nested, prefixes);
                }
            }
        }
        Value::Array(values) => {
            for item in values {
                collect_json_ld_context_prefixes(item, prefixes);
            }
        }
        _ => {}
    }
}

fn collect_context_value(value: &Value, prefixes: &mut HashMap<String, String>) {
    match value {
        Value::Object(object) => {
            for (prefix, definition) in object {
                let iri = match definition {
                    Value::String(iri) => Some(iri.as_str()),
                    Value::Object(definition) => definition.get("@id").and_then(Value::as_str),
                    _ => None,
                };
                if let Some(iri) = iri.filter(|iri| iri.ends_with('/') || iri.ends_with('#')) {
                    prefixes.insert(prefix.to_string(), iri.to_string());
                }
            }
        }
        Value::Array(values) => {
            for item in values {
                collect_context_value(item, prefixes);
            }
        }
        _ => {}
    }
}

fn built_in_prefixes() -> HashMap<String, String> {
    [
        ("adms", "http://www.w3.org/ns/adms#"),
        ("cccev", "http://data.europa.eu/m8g/"),
        ("cpsv", "http://purl.org/vocab/cpsv#"),
        ("cv", "http://data.europa.eu/m8g/"),
        ("dcat", "http://www.w3.org/ns/dcat#"),
        ("dcatap", "http://data.europa.eu/r5r/"),
        ("dct", "http://purl.org/dc/terms/"),
        ("dcterms", "http://purl.org/dc/terms/"),
        ("dpv", "https://w3id.org/dpv#"),
        ("dqv", "http://www.w3.org/ns/dqv#"),
        ("foaf", "http://xmlns.com/foaf/0.1/"),
        ("eli", "http://data.europa.eu/eli/ontology#"),
        ("odrl", "http://www.w3.org/ns/odrl/2/"),
        ("owl", "http://www.w3.org/2002/07/owl#"),
        ("prof", "http://www.w3.org/ns/dx/prof/"),
        ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
        ("registry_manifest", "https://registry-manifest.dev/ns/v1#"),
        ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
        ("sh", "http://www.w3.org/ns/shacl#"),
        ("skos", "http://www.w3.org/2004/02/skos/core#"),
        ("vcard", "http://www.w3.org/2006/vcard/ns#"),
        ("xsd", "http://www.w3.org/2001/XMLSchema#"),
    ]
    .into_iter()
    .map(|(prefix, iri)| (prefix.to_string(), iri.to_string()))
    .collect()
}

fn expand_compact_iri(value: &str, prefixes: &HashMap<String, String>) -> Option<String> {
    if value.starts_with("http://") || value.starts_with("https://") {
        return None;
    }
    let (prefix, suffix) = value.split_once(':')?;
    if suffix.starts_with("//") {
        return None;
    }
    prefixes.get(prefix).map(|base| format!("{base}{suffix}"))
}

fn expanded_iri(value: &str, prefixes: &HashMap<String, String>) -> String {
    expand_compact_iri(value, prefixes).unwrap_or_else(|| value.to_string())
}

fn is_relation_resource_identifier(value: &str) -> bool {
    value.starts_with('#')
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("urn:")
        || value.starts_with("did:")
}

fn canonical_compact_iri(value: &str, prefixes: &HashMap<String, String>) -> String {
    let expanded = expanded_iri(value, prefixes);
    compact_expanded_iri(&expanded).unwrap_or(expanded)
}

fn compact_expanded_iri(value: &str) -> Option<String> {
    let (prefix, suffix) = if let Some(suffix) = value.strip_prefix("http://data.europa.eu/m8g/") {
        let prefix = match suffix {
            "hasChannel"
            | "hasCompetentAuthority"
            | "holdsRequirement"
            | "Channel"
            | "PublicOrganisation" => "cv",
            _ => "cccev",
        };
        (prefix, suffix)
    } else if let Some(suffix) = value.strip_prefix("http://purl.org/vocab/cpsv#") {
        ("cpsv", suffix)
    } else if let Some(suffix) = value.strip_prefix("http://www.w3.org/ns/dcat#") {
        ("dcat", suffix)
    } else if let Some(suffix) = value.strip_prefix("http://purl.org/dc/terms/") {
        ("dcterms", suffix)
    } else if let Some(suffix) = value.strip_prefix("http://data.europa.eu/r5r/") {
        ("dcatap", suffix)
    } else if let Some(suffix) = value.strip_prefix("http://www.w3.org/2004/02/skos/core#") {
        ("skos", suffix)
    } else if let Some(suffix) = value.strip_prefix("http://www.w3.org/2000/01/rdf-schema#") {
        ("rdfs", suffix)
    } else if let Some(suffix) = value.strip_prefix("https://registry-manifest.dev/ns/v1#") {
        ("registry_manifest", suffix)
    } else if let Some(suffix) = value.strip_prefix("http://www.w3.org/ns/dx/prof/") {
        ("prof", suffix)
    } else if let Some(suffix) = value.strip_prefix("http://www.w3.org/ns/shacl#") {
        ("sh", suffix)
    } else {
        return None;
    };
    Some(format!("{prefix}:{suffix}"))
}

fn has_semantic_type(
    types: &[String],
    canonical_type: &str,
    prefixes: &HashMap<String, String>,
) -> bool {
    types
        .iter()
        .any(|value| canonical_compact_iri(value, prefixes) == canonical_type)
}

fn has_type(types: &[String], needle: &str) -> bool {
    types.iter().any(|value| {
        value == needle
            || value.ends_with(&format!("#{needle}"))
            || value.ends_with(&format!("/{needle}"))
    })
}

#[allow(clippy::too_many_arguments)]
fn json_asset(
    artifact_id: &str,
    kind: SemanticAssetKind,
    uri: Option<String>,
    title: Option<String>,
    description: Option<String>,
    publisher: Option<String>,
    endpoint_url: Option<String>,
    conforms_to: Vec<String>,
    source_label: &str,
) -> SemanticAsset {
    SemanticAsset {
        id: json_asset_id(artifact_id, uri.as_deref(), source_label),
        kind,
        artifact_id: artifact_id.to_string(),
        uri,
        title,
        description,
        publisher,
        endpoint_url,
        conforms_to,
        source_hints: vec![SourceHint {
            label: source_label.to_string(),
            predicate: Some("@type".to_string()),
            path: Some("/@type".to_string()),
            artifact_id: artifact_id.to_string(),
        }],
        raw_refs: vec![RawReference {
            artifact_id: artifact_id.to_string(),
            pointer: None,
            subject_iri: None,
        }],
    }
}

fn json_asset_id(artifact_id: &str, uri: Option<&str>, source_label: &str) -> String {
    deterministic_id(
        "asset",
        [artifact_id, uri.unwrap_or(source_label), source_label],
    )
}

fn odrl_policy_source_hints(artifact_id: &str, node: &Map<String, Value>) -> Vec<SourceHint> {
    let mut predicates = BTreeSet::new();
    collect_odrl_policy_predicates(&Value::Object(node.clone()), &mut predicates);
    predicates
        .into_iter()
        .map(|predicate| SourceHint {
            label: predicate.clone(),
            predicate: Some(predicate),
            path: None,
            artifact_id: artifact_id.to_string(),
        })
        .collect()
}

fn collect_odrl_policy_predicates(value: &Value, predicates: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if let Some(predicate) = canonical_odrl_policy_predicate(key) {
                    predicates.insert(predicate.to_string());
                }
                collect_odrl_policy_predicates(nested, predicates);
            }
        }
        Value::Array(values) => {
            for nested in values {
                collect_odrl_policy_predicates(nested, predicates);
            }
        }
        _ => {}
    }
}

fn canonical_odrl_policy_predicate(key: &str) -> Option<&'static str> {
    match key {
        "odrl:uid" | "http://www.w3.org/ns/odrl/2/uid" => Some("odrl:uid"),
        "odrl:assigner" | "http://www.w3.org/ns/odrl/2/assigner" => Some("odrl:assigner"),
        "odrl:assignee" | "http://www.w3.org/ns/odrl/2/assignee" => Some("odrl:assignee"),
        "odrl:profile" | "http://www.w3.org/ns/odrl/2/profile" => Some("odrl:profile"),
        "odrl:permission" | "http://www.w3.org/ns/odrl/2/permission" => Some("odrl:permission"),
        "odrl:prohibition" | "http://www.w3.org/ns/odrl/2/prohibition" => Some("odrl:prohibition"),
        "odrl:obligation" | "http://www.w3.org/ns/odrl/2/obligation" => Some("odrl:obligation"),
        "odrl:target" | "http://www.w3.org/ns/odrl/2/target" => Some("odrl:target"),
        "odrl:action" | "http://www.w3.org/ns/odrl/2/action" => Some("odrl:action"),
        "odrl:constraint" | "http://www.w3.org/ns/odrl/2/constraint" => Some("odrl:constraint"),
        "odrl:duty" | "http://www.w3.org/ns/odrl/2/duty" => Some("odrl:duty"),
        "odrl:leftOperand" | "http://www.w3.org/ns/odrl/2/leftOperand" => Some("odrl:leftOperand"),
        "odrl:operator" | "http://www.w3.org/ns/odrl/2/operator" => Some("odrl:operator"),
        "odrl:rightOperand" | "http://www.w3.org/ns/odrl/2/rightOperand" => {
            Some("odrl:rightOperand")
        }
        "odrl:unit" | "http://www.w3.org/ns/odrl/2/unit" => Some("odrl:unit"),
        _ => None,
    }
}

fn is_odrl_policy_node(types: &[String], node: &Map<String, Value>) -> bool {
    types.iter().any(|value| {
        matches!(
            value.as_str(),
            "odrl:Offer" | "odrl:Set" | "odrl:Agreement" | "odrl:Policy"
        ) || value.ends_with("#Offer")
            || value.ends_with("#Set")
            || value.ends_with("#Agreement")
            || value.ends_with("#Policy")
            || value.ends_with("/Offer")
            || value.ends_with("/Set")
            || value.ends_with("/Agreement")
            || value.ends_with("/Policy")
    }) || node.contains_key("odrl:permission")
        || node.contains_key("odrl:prohibition")
        || node.contains_key("odrl:obligation")
}

struct StandardSignalPredicate {
    predicate: &'static str,
    keys: &'static [&'static str],
}

fn standard_signal_predicates() -> &'static [StandardSignalPredicate] {
    &[
        StandardSignalPredicate {
            predicate: "dcterms:publisher",
            keys: &["dcterms:publisher", "dct:publisher", "publisher"],
        },
        StandardSignalPredicate {
            predicate: "dcterms:creator",
            keys: &["dcterms:creator", "dct:creator", "creator"],
        },
        StandardSignalPredicate {
            predicate: "dcterms:source",
            keys: &["dcterms:source", "dct:source", "source"],
        },
        StandardSignalPredicate {
            predicate: "dcterms:provenance",
            keys: &["dcterms:provenance", "dct:provenance", "provenance"],
        },
        StandardSignalPredicate {
            predicate: "dcatap:applicableLegislation",
            keys: &["dcatap:applicableLegislation", "applicableLegislation"],
        },
        StandardSignalPredicate {
            predicate: "dcterms:modified",
            keys: &["dcterms:modified", "dct:modified", "modified"],
        },
        StandardSignalPredicate {
            predicate: "dcterms:issued",
            keys: &["dcterms:issued", "dct:issued", "issued"],
        },
        StandardSignalPredicate {
            predicate: "dcterms:accrualPeriodicity",
            keys: &[
                "dcterms:accrualPeriodicity",
                "dct:accrualPeriodicity",
                "accrualPeriodicity",
            ],
        },
        StandardSignalPredicate {
            predicate: "adms:status",
            keys: &["adms:status", "status"],
        },
        StandardSignalPredicate {
            predicate: "dcatap:availability",
            keys: &["dcatap:availability", "availability"],
        },
        StandardSignalPredicate {
            predicate: "dcterms:accessRights",
            keys: &["dcterms:accessRights", "dct:accessRights", "accessRights"],
        },
        StandardSignalPredicate {
            predicate: "dcterms:rights",
            keys: &["dcterms:rights", "dct:rights", "rights"],
        },
        StandardSignalPredicate {
            predicate: "dcterms:license",
            keys: &["dcterms:license", "dct:license", "license"],
        },
        StandardSignalPredicate {
            predicate: "odrl:hasPolicy",
            keys: &["odrl:hasPolicy", "hasPolicy"],
        },
        StandardSignalPredicate {
            predicate: "odrl:profile",
            keys: &["odrl:profile", "profile"],
        },
        StandardSignalPredicate {
            predicate: "cpsv:produces",
            keys: &["cpsv:produces", "produces"],
        },
        StandardSignalPredicate {
            predicate: "dcat:servesDataset",
            keys: &["dcat:servesDataset", "servesDataset"],
        },
        StandardSignalPredicate {
            predicate: "dcat:accessService",
            keys: &["dcat:accessService", "accessService"],
        },
    ]
}

fn json_ld_links(value: &Value) -> Vec<(String, String)> {
    let mut links = Vec::new();
    collect_json_ld_links(value, &mut links);
    links
}

struct JsonLdShaclPropertyEvidence {
    shape: String,
    path: String,
    value: Option<String>,
}

fn json_ld_shacl_properties(
    value: &Value,
    prefixes: &HashMap<String, String>,
) -> Vec<JsonLdShaclPropertyEvidence> {
    let mut properties = Vec::new();
    collect_json_ld_shacl_properties(value, prefixes, None, &mut properties);
    properties
}

fn collect_json_ld_shacl_properties(
    value: &Value,
    prefixes: &HashMap<String, String>,
    current_shape: Option<String>,
    properties: &mut Vec<JsonLdShaclPropertyEvidence>,
) {
    match value {
        Value::Object(object) => {
            let shape = json_id(object).or(current_shape);
            for property_value in json_values_for_keys(
                object,
                &["sh:property", "http://www.w3.org/ns/shacl#property"],
            ) {
                for property_object in json_object_items(property_value) {
                    if let Some(path) = get_json_string_map(
                        property_object,
                        &["sh:path", "http://www.w3.org/ns/shacl#path"],
                    ) {
                        let path = expand_compact_iri(&path, prefixes).unwrap_or(path);
                        properties.push(JsonLdShaclPropertyEvidence {
                            shape: shape
                                .clone()
                                .unwrap_or_else(|| deterministic_id("shape", [path.as_str()])),
                            value: get_json_string_map(
                                property_object,
                                &[
                                    "sh:name",
                                    "rdfs:label",
                                    "dcterms:title",
                                    "http://www.w3.org/ns/shacl#name",
                                    "http://www.w3.org/2000/01/rdf-schema#label",
                                    "http://purl.org/dc/terms/title",
                                ],
                            ),
                            path,
                        });
                    }
                    collect_json_ld_shacl_properties(
                        &Value::Object(property_object.clone()),
                        prefixes,
                        shape.clone(),
                        properties,
                    );
                }
            }
            for (key, nested) in object {
                if key == "@context" {
                    continue;
                }
                if matches!(nested, Value::Array(_) | Value::Object(_)) {
                    collect_json_ld_shacl_properties(nested, prefixes, shape.clone(), properties);
                }
            }
        }
        Value::Array(values) => {
            for item in values {
                collect_json_ld_shacl_properties(item, prefixes, current_shape.clone(), properties);
            }
        }
        _ => {}
    }
}

fn json_values_for_keys<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Vec<&'a Value> {
    keys.iter().filter_map(|key| object.get(*key)).collect()
}

fn json_values_for_canonical_keys<'a>(
    object: &'a Map<String, Value>,
    canonical_keys: &[&str],
    prefixes: &HashMap<String, String>,
) -> Vec<&'a Value> {
    object
        .iter()
        .filter_map(|(key, value)| {
            canonical_keys
                .iter()
                .any(|canonical| canonical_compact_iri(key, prefixes) == *canonical)
                .then_some(value)
        })
        .collect()
}

fn json_object_items(value: &Value) -> Vec<&Map<String, Value>> {
    match value {
        Value::Object(object) => vec![object],
        Value::Array(values) => values.iter().filter_map(Value::as_object).collect(),
        _ => Vec::new(),
    }
}

fn collect_json_ld_links(value: &Value, links: &mut Vec<(String, String)>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if is_required_link_predicate(key) {
                    for url in json_strings(value) {
                        links.push((key.clone(), url));
                    }
                }
                collect_json_ld_links(value, links);
            }
        }
        Value::Array(values) => {
            for item in values {
                collect_json_ld_links(item, links);
            }
        }
        _ => {}
    }
}

fn is_required_link_predicate(predicate: &str) -> bool {
    let predicate = canonical_compact_iri(predicate, &built_in_prefixes());
    matches!(
        predicate.as_str(),
        "dcat:catalog"
            | "dcat:dataset"
            | "dcat:service"
            | "dcat:distribution"
            | "dcat:landingPage"
            | "dcat:endpointDescription"
            | "dcat:endpointURL"
            | "dcat:accessURL"
            | "dcat:downloadURL"
            | "dcat:accessService"
            | "dcat:servesDataset"
            | "dcat:hasPart"
            | "dcterms:hasPart"
            | "dct:hasPart"
            | "dcterms:conformsTo"
            | "dct:conformsTo"
            | "dcatap:applicableLegislation"
            | "cpsv:produces"
            | "prof:hasResource"
            | "prof:hasArtifact"
            | "prof:isProfileOf"
            | "odrl:hasPolicy"
            | "sh:shapesGraph"
    )
}

fn is_required_relation_predicate(predicate: &str) -> bool {
    matches!(
        predicate,
        "cv:hasChannel"
            | "cv:hasCompetentAuthority"
            | "cv:holdsRequirement"
            | "cpsv:hasInput"
            | "cpsv:produces"
            | "dcterms:type"
            | "cccev:hasRequirement"
            | "cccev:hasConcept"
            | "cccev:hasEvidenceTypeList"
            | "cccev:specifiesEvidenceType"
            | "cccev:isDerivedFrom"
            | "dcat:dataset"
            | "dcat:distribution"
            | "dcat:service"
            | "dcat:accessService"
            | "dcat:servesDataset"
            | "dcat:endpointURL"
            | "dcat:endpointDescription"
            | "dcat:landingPage"
            | "dcat:accessURL"
            | "dcat:downloadURL"
            | "dcterms:conformsTo"
            | "dcterms:hasPart"
            | "dcatap:applicableLegislation"
    ) || predicate.starts_with("registry_manifest:")
}

fn relation_endpoint_key(endpoint: &RelationEndpoint) -> String {
    match endpoint {
        RelationEndpoint::Asset { asset_id, uri } => {
            format!("asset:{asset_id}:{}", uri.as_deref().unwrap_or(""))
        }
        RelationEndpoint::External { uri } => format!("external:{uri}"),
        RelationEndpoint::BlankNode {
            artifact_id,
            node_id,
        } => format!("blank:{artifact_id}:{node_id}"),
    }
}

fn json_value_for_evidence(value: &Value) -> String {
    if let Some(value) = json_value_string(value) {
        return redact_url(&value);
    }
    match serde_json::to_string(value) {
        Ok(serialized) if serialized.len() <= 512 => serialized,
        Ok(serialized) => {
            let boundary = serialized
                .char_indices()
                .map(|(index, _)| index)
                .take_while(|index| *index <= 512)
                .last()
                .unwrap_or(0);
            format!("{}...", &serialized[..boundary])
        }
        Err(_) => String::new(),
    }
}

fn predicate_role(predicate: &str) -> Option<String> {
    match predicate {
        "dcat:dataset" => Some("dataset".to_string()),
        "dcat:service" => Some("service".to_string()),
        "dcat:distribution" => Some("distribution".to_string()),
        "dcat:accessService" => Some("access-service".to_string()),
        "dcat:servesDataset" => Some("serves-dataset".to_string()),
        "dcat:endpointDescription" => Some("endpoint-description".to_string()),
        "dcatap:applicableLegislation" => Some("applicable-legislation".to_string()),
        "cpsv:produces" => Some("produces".to_string()),
        "odrl:hasPolicy" => Some("policy".to_string()),
        "sh:shapesGraph" => Some("shacl".to_string()),
        "prof:hasArtifact" | "prof:hasResource" => Some("profile-resource".to_string()),
        _ => None,
    }
}

fn is_fetchable_metadata_link(
    rel: Option<&str>,
    predicate: Option<&str>,
    role: Option<&str>,
) -> bool {
    if rel.is_some_and(|rel| matches!(rel, "alternate" | "describedby" | "profile" | "import")) {
        return true;
    }
    if role.is_some_and(|role| role.contains("linkml")) {
        return true;
    }
    matches!(
        predicate,
        Some(
            "dcat:catalog"
                | "dcat:hasPart"
                | "dcterms:hasPart"
                | "dct:hasPart"
                | "dcat:endpointDescription"
                | "prof:hasArtifact"
                | "prof:hasResource"
                | "prof:isProfileOf"
                | "sh:shapesGraph"
        )
    )
}

fn resolve_url(base: &str, raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Rejected empty link target.".to_string());
    }
    if let Ok(url) = Url::parse(raw) {
        return Ok(url.to_string());
    }
    let base = Url::parse(base).map_err(|error| {
        format!("Cannot resolve relative link against invalid base URL: {error}")
    })?;
    base.join(raw)
        .map(|url| url.to_string())
        .map_err(|error| format!("Rejected malformed link target `{raw}`: {error}"))
}

fn contains_url_secret_material(url: &Url) -> bool {
    !url.username().is_empty()
        || url.password().is_some()
        || url
            .query_pairs()
            .any(|(key, _)| is_sensitive_query_name(key.as_ref()))
}

fn is_sensitive_query_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    [
        "token",
        "access_token",
        "id_token",
        "refresh_token",
        "api_key",
        "apikey",
        "key",
        "secret",
        "client_secret",
        "password",
        "signature",
        "sig",
    ]
    .iter()
    .any(|sensitive| normalized == *sensitive || normalized.contains(sensitive))
}

fn redact_url(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return value.to_string();
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    if url.query().is_some() {
        let pairs: Vec<_> = url
            .query_pairs()
            .map(|(key, value)| {
                if is_sensitive_query_name(key.as_ref()) {
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

fn redact_evidence(evidence: DiscoveryEvidence) -> DiscoveryEvidence {
    match evidence {
        DiscoveryEvidence::HttpHeader {
            artifact_id,
            header_name,
            rel,
            value,
        } => DiscoveryEvidence::HttpHeader {
            artifact_id,
            header_name,
            rel,
            value: value.map(|value| redact_url(&value)),
        },
        DiscoveryEvidence::JsonLdPredicate {
            artifact_id,
            predicate,
            pointer,
            value,
        } => DiscoveryEvidence::JsonLdPredicate {
            artifact_id,
            predicate,
            pointer,
            value: value.map(|value| redact_url(&value)),
        },
        DiscoveryEvidence::JsonPointer {
            artifact_id,
            pointer,
            value,
        } => DiscoveryEvidence::JsonPointer {
            artifact_id,
            pointer,
            value: value.map(|value| redact_url(&value)),
        },
        DiscoveryEvidence::HtmlLink {
            artifact_id,
            rel,
            href,
            pointer,
        } => DiscoveryEvidence::HtmlLink {
            artifact_id,
            rel,
            href: redact_url(&href),
            pointer,
        },
        DiscoveryEvidence::UrlPattern {
            artifact_id,
            pattern,
            value,
        } => DiscoveryEvidence::UrlPattern {
            artifact_id,
            pattern,
            value: redact_url(&value),
        },
        DiscoveryEvidence::ContentSniff {
            artifact_id,
            detector,
            marker,
        } => DiscoveryEvidence::ContentSniff {
            artifact_id,
            detector,
            marker,
        },
        DiscoveryEvidence::HostPolicy {
            artifact_id,
            policy,
            value,
        } => DiscoveryEvidence::HostPolicy {
            artifact_id,
            policy,
            value: value.map(|value| redact_url(&value)),
        },
        DiscoveryEvidence::SchemaProperty {
            artifact_id,
            schema_pointer,
            property_path,
            property_name,
            value,
        } => DiscoveryEvidence::SchemaProperty {
            artifact_id,
            schema_pointer,
            property_path,
            property_name,
            value,
        },
        DiscoveryEvidence::ShaclProperty {
            artifact_id,
            shape,
            path,
            predicate,
            value,
        } => DiscoveryEvidence::ShaclProperty {
            artifact_id,
            shape,
            path,
            predicate,
            value,
        },
        DiscoveryEvidence::OpenApiOperation {
            artifact_id,
            path,
            method,
            operation_id,
            summary,
        } => DiscoveryEvidence::OpenApiOperation {
            artifact_id,
            path,
            method,
            operation_id,
            summary,
        },
        DiscoveryEvidence::OgcCollection {
            artifact_id,
            collection_id,
            title,
        } => DiscoveryEvidence::OgcCollection {
            artifact_id,
            collection_id,
            title,
        },
    }
}

fn candidate_priority(rel: Option<&str>, predicate: Option<&str>) -> u8 {
    match (rel, predicate) {
        (_, Some("dcat:dataset" | "dcat:service" | "dcat:catalog")) => 10,
        (Some("import"), _) => 10,
        (_, Some("dcterms:conformsTo" | "dct:conformsTo")) => 20,
        (_, Some("sh:shapesGraph")) => 20,
        (Some("describedby" | "profile"), _) => 20,
        (Some("alternate"), _) => 30,
        _ => 50,
    }
}

fn standard_label(iri: &str) -> Option<String> {
    let normalized = iri.to_ascii_lowercase();
    if normalized.contains("dcat-ap") {
        Some("DCAT-AP".to_string())
    } else if normalized.contains("breg") || normalized.contains("base-registry") {
        Some("BRegDCAT-AP".to_string())
    } else if normalized.contains("opengis.net/spec/ogcapi-records") {
        Some("OGC API Records".to_string())
    } else if normalized.contains("opengis.net/spec/ogcapi-features") {
        Some("OGC API Features".to_string())
    } else if normalized.contains("json-schema.org") {
        Some("JSON Schema".to_string())
    } else if normalized.contains("openapis.org") {
        Some("OpenAPI".to_string())
    } else {
        None
    }
}

fn looks_like_profile(iri: &str) -> bool {
    let normalized = iri.to_ascii_lowercase();
    normalized.contains("profile") || normalized.contains("dcat-ap") || normalized.contains("breg")
}

fn is_profile_claim_pack(pack: &ProfilePack) -> bool {
    matches!(pack.id.as_str(), "dcat-ap" | "breg-dcat-ap" | "prof")
}

fn first_server_url(value: &Value) -> Option<String> {
    value
        .get("servers")
        .and_then(Value::as_array)
        .and_then(|servers| servers.first())
        .and_then(|server| get_json_string(server, &["url"]))
}

fn json_schema_refs(value: &Value, pointer: &str) -> Vec<(String, String)> {
    let mut refs = Vec::new();
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let child_pointer =
                    format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1"));
                if key == "$ref" {
                    if let Some(url) = json_value_string(value) {
                        refs.push((child_pointer, url));
                    }
                } else {
                    refs.extend(json_schema_refs(value, &child_pointer));
                }
            }
        }
        Value::Array(values) => {
            for (index, item) in values.iter().enumerate() {
                refs.extend(json_schema_refs(item, &format!("{pointer}/{index}")));
            }
        }
        _ => {}
    }
    refs
}

#[derive(Debug, Clone)]
struct JsonSchemaPropertyEvidence {
    schema_pointer: String,
    property_path: String,
    property_name: String,
    value: Option<String>,
}

fn json_schema_properties(value: &Value) -> Vec<JsonSchemaPropertyEvidence> {
    let mut properties = Vec::new();
    collect_json_schema_properties(value, "", "", &mut properties);
    properties
}

fn collect_json_schema_properties(
    value: &Value,
    pointer: &str,
    property_path: &str,
    properties: &mut Vec<JsonSchemaPropertyEvidence>,
) {
    let Some(object) = value.as_object() else {
        return;
    };

    if let Some(property_map) = object.get("properties").and_then(Value::as_object) {
        for (property_name, property_schema) in property_map {
            let escaped_name = json_pointer_segment(property_name);
            let property_pointer = format!("{pointer}/properties/{escaped_name}");
            let nested_path = if property_path.is_empty() {
                property_name.clone()
            } else {
                format!("{property_path}.{property_name}")
            };
            properties.push(JsonSchemaPropertyEvidence {
                schema_pointer: property_pointer.clone(),
                property_path: nested_path.clone(),
                property_name: property_name.clone(),
                value: serde_json::to_string(property_schema).ok(),
            });
            collect_json_schema_properties(
                property_schema,
                &property_pointer,
                &nested_path,
                properties,
            );
        }
    }

    for keyword in ["items", "additionalProperties", "contains", "propertyNames"] {
        if let Some(child) = object.get(keyword) {
            collect_json_schema_properties(
                child,
                &format!("{pointer}/{}", json_pointer_segment(keyword)),
                property_path,
                properties,
            );
        }
    }
    for keyword in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(items) = object.get(keyword).and_then(Value::as_array) {
            for (index, item) in items.iter().enumerate() {
                collect_json_schema_properties(
                    item,
                    &format!("{pointer}/{}/{index}", json_pointer_segment(keyword)),
                    property_path,
                    properties,
                );
            }
        }
    }
}

fn json_pointer_segment(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn is_openapi_method(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "get" | "put" | "post" | "delete" | "options" | "head" | "patch" | "trace"
    )
}

fn primary_ogc_kind(conforms_to: &[String]) -> ArtifactKind {
    let lowered: Vec<_> = conforms_to
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect();
    if lowered.iter().any(|value| value.contains("ogcapi-records")) {
        ArtifactKind::OgcRecords
    } else if lowered
        .iter()
        .any(|value| value.contains("ogcapi-features"))
    {
        ArtifactKind::OgcFeatures
    } else if lowered
        .iter()
        .any(|value| value.contains("opengis.net/spec/ogcapi"))
    {
        ArtifactKind::OgcLanding
    } else {
        ArtifactKind::Unknown
    }
}

fn parse_turtle_triples(body_text: &str) -> Result<Vec<ParsedTriple>, String> {
    let mut triples = Vec::new();
    TurtleParser::new(body_text.as_bytes(), None)
        .parse_all(&mut |triple| {
            triples.push(ParsedTriple {
                subject: subject_to_string(triple.subject),
                predicate: triple.predicate.iri.to_string(),
                object: term_to_string(triple.object),
            });
            Ok(()) as Result<(), rio_turtle::TurtleError>
        })
        .map_err(|error| error.to_string())?;
    Ok(triples)
}

fn subject_to_string(subject: Subject<'_>) -> String {
    match subject {
        Subject::NamedNode(NamedNode { iri }) => iri.to_string(),
        Subject::BlankNode(node) => format!("_:{}", node.id),
        Subject::Triple(_) => "_:triple".to_string(),
    }
}

fn term_to_string(term: Term<'_>) -> String {
    match term {
        Term::NamedNode(NamedNode { iri }) => iri.to_string(),
        Term::BlankNode(node) => format!("_:{}", node.id),
        Term::Literal(Literal::Simple { value }) => value.to_string(),
        Term::Literal(Literal::LanguageTaggedString { value, .. }) => value.to_string(),
        Term::Literal(Literal::Typed { value, .. }) => value.to_string(),
        Term::Triple(_) => "_:triple".to_string(),
    }
}

fn triples_by_subject(triples: &[ParsedTriple]) -> BTreeMap<String, Vec<&ParsedTriple>> {
    let mut by_subject: BTreeMap<String, Vec<&ParsedTriple>> = BTreeMap::new();
    for triple in triples {
        by_subject
            .entry(triple.subject.clone())
            .or_default()
            .push(triple);
    }
    by_subject
}

fn shacl_property_shape(
    property_shape: &str,
    by_subject: &BTreeMap<String, Vec<&ParsedTriple>>,
) -> Option<String> {
    by_subject.iter().find_map(|(shape, triples)| {
        triples
            .iter()
            .any(|triple| {
                triple.predicate == "http://www.w3.org/ns/shacl#property"
                    && triple.object == property_shape
            })
            .then(|| shape.clone())
    })
}

fn is_type_predicate(predicate: &str) -> bool {
    predicate == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
}

fn type_contains(types: &[String], suffix: &str) -> bool {
    types.iter().any(|value| value.ends_with(suffix))
}

fn has_predicate(triples: &[&ParsedTriple], predicate: &str) -> bool {
    triples.iter().any(|triple| triple.predicate == predicate)
}

fn object_for_predicates(triples: &[&ParsedTriple], predicates: &[&str]) -> Option<String> {
    predicates.iter().find_map(|predicate| {
        triples
            .iter()
            .find(|triple| triple.predicate == *predicate)
            .map(|triple| triple.object.clone())
    })
}

fn turtle_asset(
    artifact_id: &str,
    kind: SemanticAssetKind,
    uri: String,
    title: Option<String>,
    description: Option<String>,
    label: &str,
) -> SemanticAsset {
    SemanticAsset {
        id: deterministic_id("asset", [artifact_id, &uri, label]),
        kind,
        artifact_id: artifact_id.to_string(),
        uri: Some(uri.clone()),
        title,
        description,
        publisher: None,
        endpoint_url: None,
        conforms_to: Vec::new(),
        source_hints: vec![SourceHint {
            label: label.to_string(),
            predicate: Some("rdf:type".to_string()),
            path: None,
            artifact_id: artifact_id.to_string(),
        }],
        raw_refs: vec![RawReference {
            artifact_id: artifact_id.to_string(),
            pointer: None,
            subject_iri: Some(uri),
        }],
    }
}

fn is_link_predicate(predicate: &str) -> bool {
    matches!(
        predicate,
        "http://www.w3.org/ns/dcat#catalog"
            | "http://www.w3.org/ns/dcat#dataset"
            | "http://www.w3.org/ns/dcat#service"
            | "http://www.w3.org/ns/dcat#hasPart"
            | "http://purl.org/dc/terms/hasPart"
            | "http://purl.org/dc/terms/conformsTo"
            | "http://www.w3.org/ns/dx/prof/hasResource"
            | "http://www.w3.org/ns/dx/prof/hasArtifact"
            | "http://www.w3.org/ns/dx/prof/isProfileOf"
            | "http://www.w3.org/ns/shacl#shapesGraph"
    )
}

fn is_alignment_predicate(predicate: &str) -> bool {
    matches!(
        predicate,
        "http://www.w3.org/2004/02/skos/core#exactMatch"
            | "http://www.w3.org/2004/02/skos/core#closeMatch"
            | "http://www.w3.org/2004/02/skos/core#broadMatch"
            | "http://www.w3.org/2004/02/skos/core#narrowMatch"
            | "http://www.w3.org/2002/07/owl#equivalentClass"
            | "http://www.w3.org/2002/07/owl#equivalentProperty"
    )
}

fn compact_predicate(predicate: &str) -> String {
    compact_expanded_iri(predicate)
        .or_else(|| {
            predicate
                .strip_prefix("http://www.w3.org/ns/dx/prof/")
                .map(|value| format!("prof:{value}"))
        })
        .or_else(|| {
            predicate
                .strip_prefix("http://www.w3.org/ns/shacl#")
                .map(|value| format!("sh:{value}"))
        })
        .unwrap_or_else(|| predicate.to_string())
}

fn is_jsonld_context_doc(value: &Value) -> bool {
    value
        .as_object()
        .map(|object| {
            object.contains_key("@context")
                && !object.contains_key("@type")
                && !object.contains_key("@graph")
        })
        .unwrap_or(false)
}

fn is_linkml_yaml(text: &str) -> bool {
    text.contains("\nclasses:") && text.contains("\nslots:")
        || text.starts_with("id: ") && text.contains("\nprefixes:")
}

fn toml_urls(value: &toml::Value) -> BTreeSet<String> {
    let mut urls = BTreeSet::new();
    collect_toml_urls(value, &mut urls);
    urls
}

fn collect_toml_urls(value: &toml::Value, urls: &mut BTreeSet<String>) {
    match value {
        toml::Value::String(value)
            if value.starts_with("http://")
                || value.starts_with("https://")
                || value.ends_with(".json")
                || value.ends_with(".jsonld")
                || value.ends_with(".yaml")
                || value.ends_with(".yml")
                || value.ends_with(".ttl")
                || value.ends_with(".toml") =>
        {
            urls.insert(value.clone());
        }
        toml::Value::Array(values) => {
            for value in values {
                collect_toml_urls(value, urls);
            }
        }
        toml::Value::Table(table) => {
            for value in table.values() {
                collect_toml_urls(value, urls);
            }
        }
        _ => {}
    }
}

fn yaml_string(value: &serde_yaml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_string)
}

fn yaml_urls(value: &serde_yaml::Value) -> BTreeSet<String> {
    let mut urls = BTreeSet::new();
    collect_yaml_urls(value, &mut urls);
    urls
}

fn yaml_prefixes(value: &serde_yaml::Value) -> HashMap<String, String> {
    let mut prefixes = built_in_prefixes();
    let Some(prefix_map) = value
        .get("prefixes")
        .and_then(serde_yaml::Value::as_mapping)
    else {
        return prefixes;
    };

    for (name, definition) in prefix_map {
        let Some(name) = name.as_str() else {
            continue;
        };
        let iri = definition
            .get("prefix_reference")
            .or_else(|| definition.get("reference"))
            .and_then(serde_yaml::Value::as_str)
            .or_else(|| definition.as_str());
        if let Some(iri) = iri {
            prefixes.insert(name.to_string(), iri.to_string());
        }
    }
    prefixes
}

fn collect_yaml_urls(value: &serde_yaml::Value, urls: &mut BTreeSet<String>) {
    match value {
        serde_yaml::Value::String(value)
            if value.starts_with("http://")
                || value.starts_with("https://")
                || value.ends_with(".json")
                || value.ends_with(".jsonld")
                || value.ends_with(".yaml")
                || value.ends_with(".yml")
                || value.ends_with(".ttl")
                || value.ends_with(".toml") =>
        {
            urls.insert(value.clone());
        }
        serde_yaml::Value::Sequence(values) => {
            for value in values {
                collect_yaml_urls(value, urls);
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for value in map.values() {
                collect_yaml_urls(value, urls);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone)]
struct ParsedLinkHeader {
    url: String,
    rel: Option<String>,
    kind: Option<String>,
}

fn parse_link_header(value: &str) -> Vec<ParsedLinkHeader> {
    let mut parsed = Vec::new();
    for part in value.split(',') {
        let trimmed = part.trim();
        let Some(end) = trimmed.find('>') else {
            continue;
        };
        if !trimmed.starts_with('<') {
            continue;
        }
        let url = trimmed[1..end].to_string();
        let mut rel = None;
        let mut kind = None;
        for parameter in trimmed[end + 1..].split(';').map(str::trim) {
            if let Some(value) = parameter.strip_prefix("rel=") {
                rel = Some(value.trim_matches('"').to_string());
            }
            if let Some(value) = parameter.strip_prefix("type=") {
                kind = Some(value.trim_matches('"').to_string());
            }
        }
        parsed.push(ParsedLinkHeader { url, rel, kind });
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

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
            fetched_at: "2026-05-19T00:00:00Z".to_string(),
            depth: 0,
            discovered_from: None,
            discovered_by: None,
        }
    }

    #[test]
    fn parser_failures_become_findings() {
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/schema.json".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/schema.json",
                "application/schema+json",
                "{",
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should return partial report");

        assert_eq!(report.summary.parse_error_count, 1);
        assert_eq!(report.artifacts[0].status, ArtifactStatus::ParseError);
    }

    #[test]
    fn rejects_unsupported_link_schemes_as_findings() {
        let body = r#"{"@context":{},"@id":"https://example.test/catalog","@type":"dcat:Catalog","dcat:dataset":"ftp://example.test/dataset"}"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/catalog".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/catalog",
                "application/ld+json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        assert!(report.next_fetches.is_empty());
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "link.rejected_by_core"));
    }

    #[test]
    fn redacts_sensitive_headers_defensively() {
        let mut artifact = fetched(
            "https://example.test/catalog",
            "application/ld+json",
            r#"{"@context":{},"@id":"https://example.test/catalog","@type":"dcat:Catalog"}"#,
        );
        artifact.headers = vec![HeaderPair {
            name: "Authorization".to_string(),
            value: "Bearer secret".to_string(),
        }];
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: artifact.url.clone(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![artifact],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");
        let serialized = serde_json::to_string(&report).expect("report serializes");
        assert!(!serialized.contains("secret"));
    }

    #[test]
    fn detects_ogc_records_before_features() {
        let body = r#"{
          "title":"Mixed OGC landing",
          "conformsTo":[
            "http://www.opengis.net/spec/ogcapi-features-1/1.0/conf/core",
            "http://www.opengis.net/spec/ogcapi-records-1/1.0/conf/core"
          ],
          "links":[]
        }"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/ogc".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/ogc",
                "application/json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");
        assert_eq!(report.artifacts[0].kind, ArtifactKind::OgcRecords);
    }

    #[test]
    fn artifact_ids_are_stable_for_same_normalized_url() {
        let report_a = analyze_artifacts(AnalyzeInput {
            entry_url: "https://EXAMPLE.test/catalog".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://EXAMPLE.test/catalog#fragment",
                "application/ld+json",
                r#"{"@context":{},"@id":"https://example.test/catalog","@type":"dcat:Catalog","dcterms:title":"A"}"#,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");
        let report_b = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/catalog".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/catalog",
                "application/ld+json",
                r#"{"@context":{},"@id":"https://example.test/catalog","@type":"dcat:Catalog","dcterms:title":"B"}"#,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        assert_eq!(report_a.artifacts[0].id, report_b.artifacts[0].id);
        assert!(report_a.artifacts[0].id.starts_with("artifact:"));
    }

    #[test]
    fn malformed_links_are_auditable_findings() {
        let body = r#"{"@context":{},"@id":"https://example.test/catalog","@type":"dcat:Catalog","dcat:dataset":"http://[::1"}"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/catalog".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/catalog",
                "application/ld+json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        let finding = report
            .findings
            .iter()
            .find(|finding| finding.code == "link.rejected_by_core")
            .expect("malformed link finding exists");
        assert!(finding.id.starts_with("finding:"));
        assert!(finding.message.contains("Rejected malformed link target"));
    }

    #[test]
    fn secret_link_targets_are_rejected_and_redacted() {
        let body = r#"{"@context":{},"@id":"https://example.test/catalog","@type":"dcat:Catalog","dcat:dataset":"https://user:pass@example.test/dataset.jsonld?api_key=secret&ok=true"}"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/catalog".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/catalog",
                "application/ld+json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        assert!(report.links.is_empty());
        assert!(report.next_fetches.is_empty());
        let serialized = serde_json::to_string(&report).expect("report serializes");
        assert!(!serialized.contains("user:pass"));
        assert!(!serialized.contains("secret"));
        assert!(serialized.contains("api_key=REDACTED"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "link.rejected_by_core"));
    }

    #[test]
    fn json_ld_context_prefixes_expand_link_targets() {
        let body = r#"{
          "@context": {
            "dcat": "http://www.w3.org/ns/dcat#",
            "ex": "https://example.test/assets/"
          },
          "@id":"https://example.test/catalog",
          "@type":"dcat:Catalog",
          "dcat:dataset":"ex:dataset-1"
        }"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/catalog".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/catalog",
                "application/ld+json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        assert!(report
            .links
            .iter()
            .any(|link| link.to_url == "https://example.test/assets/dataset-1"));
        assert!(
            report.next_fetches.is_empty(),
            "dcat:dataset identifies a dataset; it is not necessarily a metadata document to fetch"
        );
        assert!(report
            .findings
            .iter()
            .all(|finding| finding.code != "link.rejected_by_core"));
    }

    #[test]
    fn metadata_index_links_are_fetch_candidates() {
        let body = r#"{
          "links": [
            {"rel": "self", "href": "/metadata"},
            {"rel": "alternate", "href": "/metadata/dcat/bregdcat-ap", "type": "application/ld+json"},
            {"rel": "describedby", "href": "/metadata/policies", "type": "application/ld+json"}
          ]
        }"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/metadata".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/metadata",
                "application/json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        assert_eq!(report.artifacts[0].kind, ArtifactKind::MetadataIndex);
        assert!(report.next_fetches.iter().any(|candidate| {
            candidate.url == "https://example.test/metadata/dcat/bregdcat-ap"
        }));
        assert!(report
            .next_fetches
            .iter()
            .any(|candidate| candidate.url == "https://example.test/metadata/policies"));
    }

    #[test]
    fn catalogue_json_index_is_a_metadata_index_with_dataset_assets() {
        let body = r#"{
          "id": "https://example.test/metadata/catalog",
          "base_url": "https://example.test",
          "title": "Example Catalog",
          "publisher": {"name": "Ministry of Data"},
          "datasets": [
            {"dataset_id": "farmer_registry", "title": "Farmer Registry"},
            {"dataset_id": "education_registry", "title": "Education Registry"}
          ]
        }"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/metadata/catalog".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/metadata/catalog",
                "application/json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        assert_eq!(report.artifacts[0].kind, ArtifactKind::MetadataIndex);
        assert!(report
            .assets
            .iter()
            .any(|asset| asset.kind == SemanticAssetKind::Catalog));
        assert!(report.assets.iter().any(|asset| {
            asset.kind == SemanticAssetKind::Dataset
                && asset.uri.as_deref() == Some("https://example.test/datasets/farmer_registry")
                && asset.title.as_deref() == Some("Farmer Registry")
        }));
    }

    #[test]
    fn json_ld_shacl_is_parsed_as_shape_graph() {
        let body = r#"{
          "@context": {
            "sh": "http://www.w3.org/ns/shacl#",
            "ex": "https://example.test/"
          },
          "@graph": [{
            "@id": "ex:PersonShape",
            "@type": "sh:NodeShape",
            "sh:property": [{
              "@type": "sh:PropertyShape",
              "sh:path": "ex:farmerStatus"
            }]
          }]
        }"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/metadata/shacl".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/metadata/shacl",
                "application/ld+json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        assert_eq!(report.artifacts[0].kind, ArtifactKind::Shacl);
        assert!(report.findings.iter().any(|finding| {
            finding.code == "semantic.shacl_property"
                && matches!(
                    finding.evidence.as_ref(),
                    Some(DiscoveryEvidence::ShaclProperty { path, .. })
                        if path == "https://example.test/farmerStatus"
                )
        }));
    }

    #[test]
    fn conforms_to_is_preserved_but_not_fetched() {
        let body = r#"{
          "@context": {
            "dcat": "http://www.w3.org/ns/dcat#",
            "dcterms": "http://purl.org/dc/terms/"
          },
          "@id": "https://example.test/catalog",
          "@type": "dcat:Catalog",
          "dcterms:conformsTo": {"@id": "https://standards.example/profile"}
        }"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/catalog".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/catalog",
                "application/ld+json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        assert!(report
            .profiles
            .iter()
            .any(|profile| profile.iri == "https://standards.example/profile"));
        assert!(report
            .links
            .iter()
            .any(|link| link.to_url == "https://standards.example/profile"));
        assert!(
            report.next_fetches.is_empty(),
            "conformance IRIs identify standards or profiles; they are not necessarily fetchable metadata documents"
        );
    }

    #[test]
    fn odrl_policy_nodes_are_policy_assets_without_data_plane_fetches() {
        let body = r#"{
          "@context": {
            "dcat": "http://www.w3.org/ns/dcat#",
            "dcterms": "http://purl.org/dc/terms/",
            "odrl": "http://www.w3.org/ns/odrl/2/"
          },
          "@id": "https://example.test/metadata/dcat",
          "@type": "dcat:Catalog",
          "dcat:dataset": [{
            "@id": "https://example.test/datasets/farmers",
            "@type": "dcat:Dataset",
            "dcterms:title": "Farmers",
            "odrl:hasPolicy": {
              "@id": "https://example.test/datasets/farmers#offer",
              "@type": "odrl:Offer",
              "odrl:assigner": {"@id": "did:web:authority.example"},
              "odrl:permission": [{
                "odrl:target": {"@id": "https://example.test/datasets/farmers"},
                "odrl:action": {"@id": "odrl:use"}
              }]
            },
            "dcat:distribution": [{
              "@id": "https://example.test/datasets/farmers/farmer",
              "@type": "dcat:Distribution",
              "dcat:accessURL": "https://example.test/datasets/farmers/farmer"
            }]
          }]
        }"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/metadata/dcat".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/metadata/dcat",
                "application/ld+json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        let policy_asset = report
            .assets
            .iter()
            .find(|asset| {
                asset.kind == SemanticAssetKind::Policy
                    && asset.uri.as_deref() == Some("https://example.test/datasets/farmers#offer")
            })
            .expect("policy asset should be preserved");
        let policy_predicates = policy_asset
            .source_hints
            .iter()
            .filter_map(|hint| hint.predicate.as_deref())
            .collect::<BTreeSet<_>>();
        assert!(policy_predicates.contains("odrl:permission"));
        assert!(policy_predicates.contains("odrl:target"));
        assert!(policy_predicates.contains("odrl:action"));
        assert!(report.findings.iter().any(|finding| {
            finding.code == "semantic.standard_signal"
                && matches!(
                    finding.evidence.as_ref(),
                    Some(DiscoveryEvidence::JsonLdPredicate { predicate, .. })
                        if predicate == "odrl:hasPolicy"
                )
        }));
        assert!(
            report.next_fetches.iter().all(|candidate| {
                candidate.url != "https://example.test/datasets/farmers/farmer"
            }),
            "dcat:accessURL is an access method, not a metadata artifact"
        );
    }

    #[test]
    fn dcat_standard_signals_are_preserved_without_interpreting_authority() {
        let body = r#"{
          "@context": {
            "dcat": "http://www.w3.org/ns/dcat#",
            "dcterms": "http://purl.org/dc/terms/",
            "dcatap": "http://data.europa.eu/r5r/",
            "adms": "http://www.w3.org/ns/adms#",
            "cpsv": "http://purl.org/vocab/cpsv#",
            "foaf": "http://xmlns.com/foaf/0.1/"
          },
          "@graph": [
            {
              "@id": "https://example.test/datasets/farmers",
              "@type": "dcat:Dataset",
              "dcterms:title": "Farmers",
              "dcterms:publisher": {"foaf:name": "Agriculture Authority"},
              "dcatap:applicableLegislation": {"@id": "https://example.test/law/farmers"},
              "dcterms:accrualPeriodicity": {"@id": "http://publications.europa.eu/resource/authority/frequency/MONTHLY"},
              "adms:status": {"@id": "http://purl.org/adms/status/Completed"}
            },
            {
              "@id": "https://example.test/services/farmer-registry",
              "@type": "cpsv:PublicService",
              "cpsv:produces": {"@id": "https://example.test/datasets/farmers"}
            }
          ]
        }"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/catalog".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/catalog",
                "application/ld+json",
                body,
            )],
            options: AnalyzeOptions::default(),
        })
        .expect("analysis should succeed");

        let signal_predicates = report
            .findings
            .iter()
            .filter(|finding| finding.code == "semantic.standard_signal")
            .filter_map(|finding| match finding.evidence.as_ref()? {
                DiscoveryEvidence::JsonLdPredicate { predicate, .. } => Some(predicate.as_str()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();

        assert!(signal_predicates.contains("dcterms:publisher"));
        assert!(signal_predicates.contains("dcatap:applicableLegislation"));
        assert!(signal_predicates.contains("dcterms:accrualPeriodicity"));
        assert!(signal_predicates.contains("adms:status"));
        assert!(report
            .links
            .iter()
            .any(|link| link.predicate.as_deref() == Some("cpsv:produces")));
    }

    #[test]
    fn profile_labels_come_from_enabled_profile_packs() {
        let body = r#"{
          "@context":{"dcterms":"http://purl.org/dc/terms/"},
          "@id":"https://example.test/catalog",
          "@type":"dcat:Catalog",
          "dcterms:conformsTo":{"@id":"https://semiceu.github.io/BRegDCAT-AP/releases/2.1.0/"}
        }"#;
        let report = analyze_artifacts(AnalyzeInput {
            entry_url: "https://example.test/catalog".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![fetched(
                "https://example.test/catalog",
                "application/ld+json",
                body,
            )],
            options: AnalyzeOptions {
                enabled_profiles: vec!["breg-dcat-ap".to_string()],
                ..AnalyzeOptions::default()
            },
        })
        .expect("analysis should succeed");

        assert!(report.profiles.iter().any(|profile| {
            profile.iri == "https://semiceu.github.io/BRegDCAT-AP/releases/2.1.0/"
                && profile.label.as_deref() == Some("BRegDCAT-AP")
        }));
    }

    #[test]
    fn long_structured_relation_evidence_truncates_on_utf8_boundary() {
        let value = serde_json::json!({
            "nested": "é".repeat(400)
        });
        let evidence = json_value_for_evidence(&value);

        assert!(evidence.ends_with("..."));
        assert!(evidence.is_char_boundary(evidence.len()));
    }
}
