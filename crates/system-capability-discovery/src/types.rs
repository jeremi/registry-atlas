use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub use semantic_asset_discovery_core::DiscoveryReport;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    #[error("capability index requires at least one source")]
    EmptySources,
    #[error("capability query `{query_id}` requires at least one information need")]
    EmptyQuery { query_id: String },
    #[error("reviewed mapping `{mapping_set_id}/{mapping_id}` is not indexed")]
    UnsupportedReviewedMapping {
        mapping_set_id: String,
        mapping_id: String,
    },
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilitySource {
    pub id: String,
    pub report: DiscoveryReport,
    #[serde(default)]
    pub envelope: Option<DiscoveryRunEnvelope>,
    #[serde(default)]
    pub mappings: Vec<ReviewedMappingSet>,
    #[serde(default)]
    pub review: Vec<ReviewedCapabilityAssertion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewedMappingSet {
    pub id: String,
    pub version: String,
    pub authority: String,
    #[serde(default)]
    pub mappings: Vec<ReviewedMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewedMapping {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub iris: Vec<String>,
    #[serde(default)]
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewedCapabilityAssertion {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityQuery {
    pub id: String,
    #[serde(default)]
    pub purpose: Option<Term>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub prefixes: BTreeMap<String, String>,
    #[serde(default)]
    pub needs: Vec<InformationNeed>,
}

impl CapabilityQuery {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            purpose: None,
            country: None,
            prefixes: BTreeMap::new(),
            needs: Vec::new(),
        }
    }

    pub fn purpose(mut self, purpose: Term) -> Self {
        self.purpose = Some(purpose);
        self
    }

    pub fn country(mut self, country: impl Into<String>) -> Self {
        self.country = Some(country.into());
        self
    }

    pub fn prefix(mut self, prefix: impl Into<String>, iri: impl Into<String>) -> Self {
        self.prefixes.insert(prefix.into(), iri.into());
        self
    }

    pub fn need(mut self, need: InformationNeed) -> Self {
        self.needs.push(need);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InformationNeed {
    pub id: String,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub about_any: Vec<Term>,
    #[serde(default)]
    pub requires_any: Vec<Term>,
    #[serde(default)]
    pub requires_all: Vec<Term>,
}

impl InformationNeed {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            question: None,
            about_any: Vec::new(),
            requires_any: Vec::new(),
            requires_all: Vec::new(),
        }
    }

    pub fn question(mut self, question: impl Into<String>) -> Self {
        self.question = Some(question.into());
        self
    }

    pub fn about_any<I>(mut self, terms: I) -> Self
    where
        I: IntoIterator<Item = Term>,
    {
        self.about_any.extend(terms);
        self
    }

    pub fn requires_any<I>(mut self, terms: I) -> Self
    where
        I: IntoIterator<Item = Term>,
    {
        self.requires_any.extend(terms);
        self
    }

    pub fn requires_all<I>(mut self, terms: I) -> Self
    where
        I: IntoIterator<Item = Term>,
    {
        self.requires_all.extend(terms);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Term {
    Iri(String),
    Label(String),
    Field(String),
    ReviewedMapping {
        mapping_set_id: String,
        mapping_id: String,
    },
}

impl Term {
    pub fn iri(value: impl Into<String>) -> Self {
        Self::Iri(value.into())
    }

    pub fn label(value: impl Into<String>) -> Self {
        Self::Label(value.into())
    }

    pub fn field(value: impl Into<String>) -> Self {
        Self::Field(value.into())
    }

    pub fn reviewed_mapping(
        mapping_set_id: impl Into<String>,
        mapping_id: impl Into<String>,
    ) -> Self {
        Self::ReviewedMapping {
            mapping_set_id: mapping_set_id.into(),
            mapping_id: mapping_id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilitySearchResult {
    pub query_id: String,
    pub inputs_summary: InputsSummary,
    pub needs: Vec<NeedSearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputsSummary {
    pub report_ids: Vec<String>,
    pub envelope_ids: Vec<String>,
    pub reviewed_mapping_sets: Vec<String>,
    pub review_assertions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NeedSearchResult {
    pub need_id: String,
    pub matches: Vec<CapabilityMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityMatch {
    pub route: CandidateAnswerRoute,
    pub score: EvidenceScore,
    pub confidence: MatchConfidence,
    pub access: AccessSummary,
    pub signals: Vec<CapabilitySignal>,
    pub evidence: Vec<EvidenceRef>,
    pub explanation: Option<String>,
    pub gaps: Vec<CapabilityGap>,
    pub review_flags: Vec<ReviewFlag>,
    pub review_state: ReviewState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnswerRoute {
    pub id: String,
    pub source_id: String,
    pub role: CandidateRouteRole,
    pub boundary: SystemBoundary,
    pub components: Vec<RouteComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CandidateRouteRole {
    CandidateRoute,
    CandidateSource,
    CandidateConsumerOrDuplicate,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteComponent {
    pub id: String,
    pub label: String,
    pub kind: RouteComponentKind,
    pub url: Option<String>,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RouteComponentKind {
    Publisher,
    Catalogue,
    Dataset,
    Entity,
    Schema,
    Collection,
    Service,
    Distribution,
    Metadata,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SystemBoundary {
    Explicit {
        label: String,
        uri: Option<String>,
        evidence: Vec<EvidenceRef>,
    },
    GatewayOrIntermediary {
        label: String,
        domain_hint: Option<String>,
        evidence: Vec<EvidenceRef>,
    },
    Ambiguous {
        candidates: Vec<RouteComponent>,
        reason: String,
    },
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessSummary {
    pub kind: AccessKind,
    pub endpoint_url: Option<String>,
    pub distribution_url: Option<String>,
    pub source_url: Option<String>,
    pub protocol_hint: Option<String>,
    pub interaction_hint: Option<String>,
    pub credential_sent_in_discovery: Option<bool>,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AccessKind {
    MetadataOnly,
    ApiDescriptionAvailable,
    DatasetDistribution,
    HumanProcess,
    RejectedOrGated,
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvidenceScore {
    pub direct_structured_matches: u32,
    pub direct_metadata_matches: u32,
    pub reviewed_mapping_matches: u32,
    pub access_evidence_matches: u32,
    pub gap_count: u32,
    pub review_flag_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MatchConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvidenceId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceRef {
    pub id: EvidenceId,
    pub source: EvidenceSource,
    pub location: Option<EvidenceLocation>,
    pub claim: EvidenceClaim,
    #[serde(default)]
    pub derived_from: Vec<EvidenceId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvidenceSource {
    DiscoveryArtifact {
        report_id: String,
        artifact_id: String,
    },
    SemanticAsset {
        report_id: String,
        asset_id: String,
    },
    DiscoveryLink {
        report_id: String,
        link_id: String,
    },
    StandardClaim {
        report_id: String,
        claim_id: String,
    },
    ProfileClaim {
        report_id: String,
        claim_id: String,
    },
    Finding {
        report_id: String,
        finding_id: String,
    },
    RejectedFetch {
        source_id: String,
        rejected_fetch_id: String,
    },
    ReviewedMapping {
        mapping_set_id: String,
        mapping_id: String,
    },
    ReviewAssertion {
        assertion_id: String,
    },
    AiSuggestion {
        suggestion_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvidenceLocation {
    JsonPointer {
        pointer: String,
    },
    RdfTriple {
        subject: String,
        predicate: String,
        object: Option<String>,
    },
    OpenApiOperation {
        path: String,
        method: String,
        operation_id: Option<String>,
        summary: Option<String>,
    },
    SchemaProperty {
        schema_pointer: String,
        property_path: String,
        property_name: Option<String>,
    },
    ShaclProperty {
        shape: Option<String>,
        path: String,
    },
    OgcCollection {
        collection_id: String,
        title: Option<String>,
    },
    HttpHeader {
        name: String,
    },
    HtmlLink {
        rel: String,
        href: String,
    },
    Url {
        url: String,
    },
    RejectedFetch {
        url: String,
        method: Option<String>,
        status: Option<u16>,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceClaim {
    pub capability_need_id: Option<String>,
    pub matched_term: Option<Term>,
    pub basis: MatchBasis,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MatchBasis {
    RequiredInformation,
    SubjectContext,
    PurposeContext,
    ReviewedMapping,
    AccessEvidence,
    Gap,
    ReviewFlag,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilitySignal {
    pub kind: CapabilitySignalKind,
    pub label: String,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySignalKind {
    RequiredInformation,
    Subject,
    Purpose,
    Access,
    Profile,
    Policy,
    Trust,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum CapabilityGap {
    NoCallableAccessMethod,
    OperationDetailsUnavailable,
    RequiredIdentifierUnknown,
    AuthSchemeUnknown,
    PurposePolicyUnknown,
    LegalBasisUnknown,
    DataSharingAgreementUnknown,
    PublisherUnknown,
    AuthorityUnknown,
    SourceOfTruthUnknown,
    DomainSystemUnknown,
    FreshnessUnknown,
    ValidationEvidenceMissing,
    TrustEvidenceMissing,
    IncompleteProfileEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReviewFlag {
    SensitiveData,
    BoundaryAmbiguous,
    PolicyConflict,
    PolicyReviewRequired,
    ReviewedMappingUsed,
    AiAssisted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Unreviewed,
    #[serde(alias = "not_reviewed")]
    NotReviewed,
    Accepted,
    Rejected,
    NeedsMoreEvidence,
    Reviewed,
}
