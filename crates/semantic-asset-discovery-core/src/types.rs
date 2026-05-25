use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

pub const REPORT_SCHEMA_VERSION: &str = "semantic-asset-discovery.report.v2";
pub const LEGACY_REPORT_SCHEMA_VERSION_V1: &str = "semantic-asset-discovery.report.v1";
pub const DEFAULT_MAX_NEXT_FETCHES: u64 = 20;
pub const DEFAULT_WASM_BODY_BUDGET_BYTES: u64 = 16_777_216;
pub const SENSITIVE_HEADER_NAMES: &[&str] = &[
    "authorization",
    "cookie",
    "proxy-authenticate",
    "set-cookie",
    "www-authenticate",
    "x-api-key",
    "x-auth-token",
    "proxy-authorization",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyzeInput {
    pub entry_url: String,
    #[serde(default)]
    pub analyzed_at: Option<String>,
    pub artifacts: Vec<FetchedArtifact>,
    #[serde(default)]
    pub options: AnalyzeOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyzeOptions {
    #[serde(default = "default_max_next_fetches")]
    pub max_next_fetches: u64,
    #[serde(default = "default_include_inferred_links")]
    pub include_inferred_links: bool,
    #[serde(default)]
    pub accepted_schemes: Vec<String>,
    #[serde(default)]
    pub enabled_profiles: Vec<String>,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            max_next_fetches: DEFAULT_MAX_NEXT_FETCHES,
            include_inferred_links: true,
            accepted_schemes: vec!["http".to_string(), "https".to_string()],
            enabled_profiles: Vec::new(),
        }
    }
}

fn default_max_next_fetches() -> u64 {
    DEFAULT_MAX_NEXT_FETCHES
}

fn default_include_inferred_links() -> bool {
    true
}

impl AnalyzeOptions {
    pub fn normalized(&self) -> Self {
        let mut normalized = self.clone();
        if normalized.max_next_fetches == 0 {
            normalized.max_next_fetches = DEFAULT_MAX_NEXT_FETCHES;
        }
        if normalized.accepted_schemes.is_empty() {
            normalized.accepted_schemes = vec!["http".to_string(), "https".to_string()];
        }
        normalized.accepted_schemes = normalized
            .accepted_schemes
            .into_iter()
            .map(|scheme| scheme.trim().to_ascii_lowercase())
            .fold(Vec::new(), |mut schemes, scheme| {
                if !schemes.contains(&scheme) {
                    schemes.push(scheme);
                }
                schemes
            });
        normalized.enabled_profiles = normalized
            .enabled_profiles
            .into_iter()
            .map(|profile| profile.trim().to_ascii_lowercase())
            .fold(Vec::new(), |mut profiles, profile| {
                if !profiles.contains(&profile) {
                    profiles.push(profile);
                }
                profiles
            });
        normalized
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FetchedArtifact {
    pub url: String,
    pub final_url: Option<String>,
    pub status: u16,
    pub media_type: Option<String>,
    pub request_accept: Option<String>,
    #[serde(default)]
    pub redirect_chain: Vec<String>,
    #[serde(default)]
    pub headers: Vec<HeaderPair>,
    pub body: Vec<u8>,
    pub fetched_at: String,
    pub depth: u8,
    pub discovered_from: Option<String>,
    pub discovered_by: Option<DiscoveryEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaderPair {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Error, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AnalyzeError {
    #[error("invalid entry URL: {message}")]
    InvalidEntryUrl { message: String },
    #[error("invalid options: {message}")]
    InvalidOptions { message: String },
    #[error("invalid input encoding: {message}")]
    InvalidInputEncoding { message: String },
    #[error("schema deserialization failed: {message}")]
    SchemaDeserialization { message: String },
    #[error("internal invariant failed: {message}")]
    InternalInvariant { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaVersion(pub String);

impl Default for SchemaVersion {
    fn default() -> Self {
        Self(REPORT_SCHEMA_VERSION.to_string())
    }
}

impl Serialize for SchemaVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == REPORT_SCHEMA_VERSION || value == LEGACY_REPORT_SCHEMA_VERSION_V1 {
            Ok(Self(value))
        } else {
            Err(serde::de::Error::custom(format!(
                "unsupported report schema version: {value}"
            )))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveryReport {
    #[serde(default)]
    pub schema_version: SchemaVersion,
    pub run_id: String,
    pub entry_url: String,
    pub analyzed_at: String,
    pub summary: DiscoverySummary,
    pub artifacts: Vec<DiscoveredArtifact>,
    pub assets: Vec<SemanticAsset>,
    #[serde(default)]
    pub relations: Vec<SemanticRelation>,
    #[serde(default)]
    pub relation_claims: Vec<RelationClaim>,
    pub links: Vec<DiscoveredLink>,
    pub standards: Vec<StandardClaim>,
    pub profiles: Vec<ProfileClaim>,
    pub findings: Vec<DiscoveryFinding>,
    pub next_fetches: Vec<FetchCandidate>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DiscoverySummary {
    pub artifact_count: u64,
    pub asset_count: u64,
    pub standard_count: u64,
    pub profile_count: u64,
    pub failed_artifact_count: u64,
    pub unsupported_artifact_count: u64,
    pub parse_error_count: u64,
    pub next_fetch_count: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    MetadataIndex,
    SemanticModelPackage,
    LinkMlSchema,
    DcatCatalog,
    DcatProfileCatalog,
    ProfProfile,
    ProfResource,
    Shacl,
    Skos,
    JsonLdContext,
    OwlOntology,
    JsonSchema,
    OpenApi,
    OgcRecords,
    OgcFeatures,
    OgcLanding,
    DidDocument,
    VerifiableCredential,
    HtmlLandingPage,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStatus {
    Fetched,
    Failed,
    Unsupported,
    Skipped,
    AuthRequired,
    TooLarge,
    ParseError,
    DisallowedByRobots,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredArtifact {
    pub id: String,
    pub url: String,
    pub final_url: Option<String>,
    pub kind: ArtifactKind,
    pub status: ArtifactStatus,
    pub media_type: Option<String>,
    pub http_status: Option<u16>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub discovered_from: Option<String>,
    pub discovered_by: Option<DiscoveryEvidence>,
    pub byte_length: Option<u64>,
    pub hash: Option<String>,
    pub error: Option<String>,
    pub analyzed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticAssetKind {
    PublicService,
    Channel,
    Requirement,
    InformationRequirement,
    InformationConcept,
    EvidenceType,
    EvidenceTypeList,
    FormDefinition,
    FormSection,
    FormField,
    PublicRegistryService,
    EvidenceOffering,
    EvidenceProvider,
    PublicOrganisation,
    SemanticModelPackage,
    Catalog,
    Dataset,
    DataService,
    Distribution,
    Profile,
    Vocabulary,
    VocabularyTerm,
    Class,
    Property,
    ShapeGraph,
    ConceptScheme,
    Alignment,
    Crosswalk,
    ApiDescription,
    RecordCollection,
    FeatureCollection,
    Policy,
    QualityMeasurement,
    LifecycleStatus,
    PrivacyBasis,
    TrustArtifact,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticAsset {
    pub id: String,
    pub kind: SemanticAssetKind,
    pub artifact_id: String,
    pub uri: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub publisher: Option<String>,
    pub endpoint_url: Option<String>,
    pub conforms_to: Vec<String>,
    pub source_hints: Vec<SourceHint>,
    pub raw_refs: Vec<RawReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RelationEndpoint {
    Asset {
        asset_id: String,
        uri: Option<String>,
    },
    External {
        uri: String,
    },
    BlankNode {
        artifact_id: String,
        node_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticRelation {
    pub id: String,
    pub subject: RelationEndpoint,
    pub predicate: String,
    pub object: RelationEndpoint,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationClaim {
    pub id: String,
    pub relation_id: String,
    pub asserted_by_artifact_id: String,
    pub evidence: DiscoveryEvidence,
    #[serde(default)]
    pub qualifiers: Vec<RelationQualifier>,
    #[serde(default)]
    pub contradicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationQualifier {
    pub predicate: String,
    pub value: String,
    pub evidence: Option<DiscoveryEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinkConfidence {
    Declared,
    Inferred,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredLink {
    pub id: String,
    pub from_artifact_id: Option<String>,
    pub from_url: String,
    pub to_url: String,
    pub rel: Option<String>,
    pub predicate: Option<String>,
    pub role: Option<String>,
    pub confidence: LinkConfidence,
    pub discovered_by: DiscoveryEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FetchCandidate {
    pub id: String,
    pub url: String,
    pub depth: u8,
    pub priority: u8,
    pub reason: String,
    pub discovered_from: String,
    pub discovered_by: DiscoveryEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StandardClaim {
    pub id: String,
    pub iri: String,
    pub label: Option<String>,
    pub version: Option<String>,
    pub claimed_by_artifact_id: String,
    pub evidence: DiscoveryEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileClaim {
    pub id: String,
    pub iri: String,
    pub label: Option<String>,
    pub version: Option<String>,
    pub base_standard_iri: Option<String>,
    pub claimed_by_artifact_id: String,
    pub evidence: DiscoveryEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Warning,
    Error,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveryFinding {
    pub id: String,
    pub severity: FindingSeverity,
    pub code: String,
    pub message: String,
    pub artifact_id: Option<String>,
    pub asset_id: Option<String>,
    pub standard_iri: Option<String>,
    pub evidence: Option<DiscoveryEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum DiscoveryEvidence {
    HttpHeader {
        artifact_id: Option<String>,
        header_name: String,
        rel: Option<String>,
        value: Option<String>,
    },
    JsonLdPredicate {
        artifact_id: Option<String>,
        predicate: String,
        pointer: Option<String>,
        value: Option<String>,
    },
    JsonPointer {
        artifact_id: Option<String>,
        pointer: String,
        value: Option<String>,
    },
    HtmlLink {
        artifact_id: Option<String>,
        rel: String,
        href: String,
        pointer: Option<String>,
    },
    UrlPattern {
        artifact_id: Option<String>,
        pattern: String,
        value: String,
    },
    ContentSniff {
        artifact_id: Option<String>,
        detector: String,
        marker: String,
    },
    HostPolicy {
        artifact_id: Option<String>,
        policy: String,
        value: Option<String>,
    },
    SchemaProperty {
        artifact_id: String,
        schema_pointer: String,
        property_path: String,
        property_name: String,
        value: Option<String>,
    },
    ShaclProperty {
        artifact_id: String,
        shape: String,
        path: String,
        predicate: String,
        value: Option<String>,
    },
    OpenApiOperation {
        artifact_id: String,
        path: String,
        method: String,
        operation_id: Option<String>,
        summary: Option<String>,
    },
    OgcCollection {
        artifact_id: String,
        collection_id: String,
        title: Option<String>,
    },
}

impl DiscoveryEvidence {
    pub fn location(&self) -> Option<String> {
        match self {
            DiscoveryEvidence::HttpHeader { value, .. } => value.clone(),
            DiscoveryEvidence::JsonLdPredicate { pointer, value, .. } => {
                pointer.clone().or_else(|| value.clone())
            }
            DiscoveryEvidence::JsonPointer { pointer, .. } => Some(pointer.clone()),
            DiscoveryEvidence::HtmlLink { pointer, href, .. } => {
                pointer.clone().or_else(|| Some(href.clone()))
            }
            DiscoveryEvidence::UrlPattern { value, .. } => Some(value.clone()),
            DiscoveryEvidence::ContentSniff { marker, .. } => Some(marker.clone()),
            DiscoveryEvidence::HostPolicy { value, policy, .. } => {
                value.clone().or_else(|| Some(policy.clone()))
            }
            DiscoveryEvidence::SchemaProperty { schema_pointer, .. } => {
                Some(schema_pointer.clone())
            }
            DiscoveryEvidence::ShaclProperty { path, .. } => Some(path.clone()),
            DiscoveryEvidence::OpenApiOperation { path, .. } => Some(path.clone()),
            DiscoveryEvidence::OgcCollection { collection_id, .. } => Some(collection_id.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceHint {
    pub label: String,
    pub predicate: Option<String>,
    pub path: Option<String>,
    pub artifact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawReference {
    pub artifact_id: String,
    pub pointer: Option<String>,
    pub subject_iri: Option<String>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum WasmAnalyzeResult {
    Ok { report: DiscoveryReport },
    Err { error: WasmAnalyzeError },
}

impl Serialize for WasmAnalyzeResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        match self {
            WasmAnalyzeResult::Ok { report } => {
                let mut state = serializer.serialize_struct("WasmAnalyzeResult", 2)?;
                state.serialize_field("ok", &true)?;
                state.serialize_field("report", report)?;
                state.end()
            }
            WasmAnalyzeResult::Err { error } => {
                let mut state = serializer.serialize_struct("WasmAnalyzeResult", 2)?;
                state.serialize_field("ok", &false)?;
                state.serialize_field("error", error)?;
                state.end()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WasmAnalyzeError {
    pub code: String,
    pub message: String,
}
