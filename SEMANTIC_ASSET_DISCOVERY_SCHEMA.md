# semantic-asset-discovery Schema

This document captures the v1 public schema for reports, inputs, errors, and
the WASM result envelope.

The Rust types are illustrative. The source of truth after implementation is
the checked Rust schema plus generated TypeScript or JSON Schema artifacts.

## Constants

```rust
pub const REPORT_SCHEMA_VERSION: &str = "semantic-asset-discovery.report.v1";
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
```

## Analyze Input

```rust
pub struct AnalyzeInput {
    pub entry_url: String,
    pub artifacts: Vec<FetchedArtifact>,
    pub options: AnalyzeOptions,
}

pub struct AnalyzeOptions {
    pub max_next_fetches: u64,
    pub include_inferred_links: bool,
    pub accepted_schemes: Vec<String>,
    pub enabled_profiles: Vec<String>,
}

pub struct FetchedArtifact {
    pub url: String,
    pub final_url: Option<String>,
    pub status: u16,
    pub media_type: Option<String>,
    pub request_accept: Option<String>,
    pub redirect_chain: Vec<String>,
    pub headers: Vec<HeaderPair>,
    pub body: Vec<u8>,
    pub fetched_at: String,
    pub depth: u8,
    pub discovered_from: Option<String>,
    pub discovered_by: Option<DiscoveryEvidence>,
}

pub struct HeaderPair {
    pub name: String,
    pub value: String,
}
```

`fetched_at` is an RFC 3339 timestamp.

Hosts MUST strip sensitive headers before constructing `FetchedArtifact`.

Default options:

- `max_next_fetches`: `DEFAULT_MAX_NEXT_FETCHES`
- `include_inferred_links`: `true`
- empty `accepted_schemes`: `["http", "https"]`
- empty `enabled_profiles`: all built-in profile packs

To disable all profile packs, callers pass `["none"]` for `enabled_profiles`.

## Analyze Error

```rust
pub enum AnalyzeError {
    InvalidEntryUrl { message: String },
    InvalidOptions { message: String },
    InvalidInputEncoding { message: String },
    SchemaDeserialization { message: String },
    InternalInvariant { message: String },
}
```

Parser failures become report findings, not `AnalyzeError`.

`InternalInvariant` is for unrecoverable core bugs only. It is not valid for
malformed input documents, parser failures, unsupported artifact kinds, missing
optional fields, or unknown standards.

## WASM Envelope

```rust
pub struct WasmAnalyzeResult {
    pub ok: bool,
    pub report: Option<DiscoveryReport>,
    pub error: Option<WasmAnalyzeError>,
}

pub struct WasmAnalyzeError {
    pub code: String,
    pub message: String,
}
```

Serialized shape:

```json
{
  "ok": true,
  "report": {}
}
```

or:

```json
{
  "ok": false,
  "error": {
    "code": "analyze.invalid_input",
    "message": "Invalid analyze input"
  }
}
```

The serialized JSON shape above is normative. Implementations MAY use a Rust
enum internally, but serde output MUST use the explicit `ok` discriminator so
TypeScript, Python, Java, and CLI callers can distinguish success from failure
without Rust enum knowledge.

Large JSON/WASM calls use:

```json
{
  "ok": false,
  "error": {
    "code": "analyze.payload_too_large",
    "message": "Analyze input exceeds the WASM body budget"
  }
}
```

## Discovery Report

```rust
pub struct SchemaVersion(String);

pub struct DiscoveryReport {
    pub schema_version: SchemaVersion,
    pub run_id: String,
    pub entry_url: String,
    pub analyzed_at: String,
    pub summary: DiscoverySummary,
    pub artifacts: Vec<DiscoveredArtifact>,
    pub assets: Vec<SemanticAsset>,
    pub links: Vec<DiscoveredLink>,
    pub standards: Vec<StandardClaim>,
    pub profiles: Vec<ProfileClaim>,
    pub findings: Vec<DiscoveryFinding>,
    pub next_fetches: Vec<FetchCandidate>,
}

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
```

`analyzed_at` is an RFC 3339 timestamp.

`SchemaVersion` serializes as the report schema string and deserializes only
supported schema versions. `run_id` identifies the analysis run and may be
host-provided or non-deterministic. It is not used for report object identity.

## Facade Request And Run Envelope

The core analyzer contract is `AnalyzeInput JSON -> DiscoveryReport JSON`.

The online facade contract is `DiscoveryRequest JSON -> DiscoveryRunEnvelope
JSON`. The facade envelope carries host fetch state that the core analyzer MUST
NOT create.

```rust
#[serde(rename_all = "snake_case")]
pub enum DiscoveryPolicyName {
    PublicWeb,
    LocalDevelopment,
    #[serde(other)]
    Unknown,
}

pub struct DiscoveryRequest {
    pub entry_url: String,
    pub policy: DiscoveryPolicyName,
    pub max_depth: u32,
    pub max_fetches: u64,
    pub max_body_bytes: u64,
    pub max_total_bytes: u64,
    pub max_concurrent_fetches: u64,
    pub timeout_ms: u64,
    pub total_timeout_ms: u64,
    pub user_agent: Option<String>,
    pub accepted_schemes: Vec<String>,
    pub allowed_origins: Vec<String>,
}

pub struct DiscoveryRunEnvelope {
    pub report: DiscoveryReport,
    pub fetched: FetchSummary,
    pub rejected_fetches: Vec<RejectedFetch>,
}

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

pub struct RejectedFetch {
    pub id: String,
    pub url: String,
    pub reason_code: String,
    pub discovered_from: Option<String>,
    pub credential_sent: bool,
}
```

`DiscoveryRequest` MUST NOT contain secret values. Credentials are attached by
the host process and are governed by facade policy.

`RejectedFetch.url` MUST be redacted before serialization. It MUST NOT include
URL userinfo or secret query parameters.

## Artifacts

```rust
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
```

## Semantic Assets

```rust
#[serde(rename_all = "snake_case")]
pub enum SemanticAssetKind {
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
```

ODRL, DQV, ADMS, and DPV are represented through asset kinds, standards claims,
profile claims, source hints, or findings unless a publisher serves them as
standalone fetchable documents.

## Links And Fetch Candidates

```rust
#[serde(rename_all = "snake_case")]
pub enum LinkConfidence {
    Declared,
    Inferred,
    #[serde(other)]
    Unknown,
}

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

pub struct FetchCandidate {
    pub id: String,
    pub url: String,
    pub depth: u8,
    pub priority: u8,
    pub reason: String,
    pub discovered_from: String,
    pub discovered_by: DiscoveryEvidence,
}
```

Rejected core links appear as findings. Host-rejected links appear in the
facade `DiscoveryRunEnvelope.rejected_fetches` and MAY also appear as safe
findings when the run can continue.

## Standards And Profiles

```rust
pub struct StandardClaim {
    pub id: String,
    pub iri: String,
    pub label: Option<String>,
    pub version: Option<String>,
    pub claimed_by_artifact_id: String,
    pub evidence: DiscoveryEvidence,
}

pub struct ProfileClaim {
    pub id: String,
    pub iri: String,
    pub label: Option<String>,
    pub version: Option<String>,
    pub base_standard_iri: Option<String>,
    pub claimed_by_artifact_id: String,
    pub evidence: DiscoveryEvidence,
}
```

`prof:isProfileOf` is the preferred source for `base_standard_iri`.

## Findings

```rust
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Warning,
    Error,
    #[serde(other)]
    Unknown,
}

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
```

Code namespaces:

- `fetch.*`
- `parse.*`
- `classify.*`
- `link.*`
- `standard.*`
- `profile.*`
- `security.*`
- `limit.*`
- `analyze.*`

## Tagged Evidence

```rust
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
    SchemaProperty {
        artifact_id: Option<String>,
        schema_pointer: String,
        property_path: String,
        property_name: Option<String>,
        value: Option<String>,
    },
    ShaclProperty {
        artifact_id: Option<String>,
        shape: Option<String>,
        path: String,
        predicate: Option<String>,
        value: Option<String>,
    },
    OpenApiOperation {
        artifact_id: Option<String>,
        path: String,
        method: String,
        operation_id: Option<String>,
        summary: Option<String>,
    },
    OgcCollection {
        artifact_id: Option<String>,
        collection_id: String,
        title: Option<String>,
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
}
```

Sensitive values MUST be redacted before they appear in evidence.

## Source Hints

```rust
pub struct SourceHint {
    pub label: String,
    pub predicate: Option<String>,
    pub path: Option<String>,
    pub artifact_id: String,
}

pub struct RawReference {
    pub artifact_id: String,
    pub pointer: Option<String>,
    pub subject_iri: Option<String>,
}
```
