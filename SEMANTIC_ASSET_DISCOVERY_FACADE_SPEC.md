# semantic-asset-discovery Facade And Navigation Specification

## Status

This document specifies the ergonomic host library named
`semantic-asset-discovery`.

Normative words use RFC 2119 meaning:

- **MUST** means required.
- **MUST NOT** means forbidden.
- **SHOULD** means recommended unless there is a documented reason.
- **MAY** means optional.

This document complements:

- [`SEMANTIC_ASSET_DISCOVERY_SPEC.md`](SEMANTIC_ASSET_DISCOVERY_SPEC.md), which
  specifies the network-free core analyzer.
- [`SEMANTIC_ASSET_DISCOVERY_SCHEMA.md`](SEMANTIC_ASSET_DISCOVERY_SCHEMA.md),
  which specifies the canonical report schema.

## Purpose

`semantic-asset-discovery` is the pleasant Rust host API over
`semantic-asset-discovery-core`.

It exists because the core API is intentionally explicit:

```rust
analyze_artifacts(AnalyzeInput { artifacts, options, .. })
```

That boundary is correct for WebAssembly, tests, and controlled hosts, but it is
too manual for application developers.

The facade MUST make the common case simple:

```rust
let run = DiscoveryClient::new()
    .discover("https://publisher.example/catalog")
    .await?;
```

The facade MUST preserve the core invariant:

```text
core analyzes already-fetched artifacts
facade fetches, protects, walks, and returns ergonomic views
```

## Design Goals

The facade MUST be:

- small enough to understand in one sitting;
- safe by default for public-web discovery;
- host-neutral and not specific to Dataspace Atlas or Registry Relay;
- able to expose discovered assets, links, access methods, policy signals,
  evidence, and conditions as primitives for higher-level system discovery;
- useful for SEMIC-style, PublicSchema-style, national profile, and sectoral
  metadata environments;
- friendly to future Java, Python, Node, and CLI wrappers through a stable JSON
  report contract;
- explicit about evidence, links, conditions, rejected fetches, and security
  boundaries.

The facade SHOULD feel closer to `reqwest` than to a semantic-web framework:

```rust
let client = DiscoveryClient::builder()
    .policy(DiscoveryPolicy::public_web())
    .max_depth(2)
    .max_fetches(50)
    .build()?;

let run = client.discover(url).await?;
```

## Non-Goals

The facade MUST NOT:

- become a central registry;
- persist reports;
- schedule recurring harvests;
- approve publishers;
- validate trust chains;
- run SHACL validation as a hidden side effect;
- infer governance approval from standards evidence;
- infer operational capabilities that are not present in the canonical report;
- perform cross-report system identity resolution;
- rank systems for user questions;
- hide policy-rejected URLs without an auditable reason.

Atlas, or another host application, MAY build registry storage, review,
scheduling, and approval workflows on top.

## Crate Shape

The workspace SHOULD contain these crates:

```text
semantic-asset-discovery-core
  Pure analyzer. No networking, no async runtime, no app state.

semantic-asset-discovery
  Ergonomic Rust facade. Fetch loop, policy, sanitization, and navigation views.

semantic-asset-discovery-cli
  Thin command-line wrapper over the facade for online harvest and over core for
  offline bundle analysis.

semantic-asset-discovery-wasm
  Browser wrapper over core. Browser fetch policy remains host-owned.
```

`semantic-asset-discovery-core` MUST NOT depend on the facade crate.

`semantic-asset-discovery-cli` SHOULD depend on the facade for `harvest` and on
the core for explicit `analyze` and `analyze-bundle` commands.

`semantic-asset-discovery-wasm` MUST keep using the core JSON envelope because
browser hosts must own credential handling, proxy policy, and private-network
protection.

## Public API

### Simple Discovery

The common path MUST be one constructor plus one async method:

```rust
use semantic_asset_discovery::DiscoveryClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let run = DiscoveryClient::new()
        .discover("https://publisher.example/catalog")
        .await?;

    for dataset in run.registry().datasets() {
        println!("{}", dataset.title().unwrap_or("Untitled"));
    }

    Ok(())
}
```

`DiscoveryClient::new()` MUST use safe public-web defaults.

### Builder

The builder SHOULD expose the small set of options that materially affect
discovery:

```rust
let client = DiscoveryClient::builder()
    .policy(DiscoveryPolicy::public_web())
    .credentials(Credentials::bearer(token).same_origin_only())
    .max_depth(2)
    .max_fetches(50)
    .max_body_bytes(8_000_000)
    .max_total_bytes(64_000_000)
    .max_concurrent_fetches(8)
    .timeout(Duration::from_secs(10))
    .total_timeout(Duration::from_secs(120))
    .user_agent("my-registry/0.1")
    .build()?;
```

The builder MUST avoid exposing parser internals as top-level configuration.

Required builder options:

| Option | Default | Meaning |
|---|---:|---|
| `policy` | `DiscoveryPolicy::public_web()` | URL, scheme, DNS, and network allow/deny rules. |
| `max_depth` | `2` | Maximum discovery depth from the entry artifact. |
| `max_fetches` | `50` | Maximum fetched artifacts in one run. |
| `max_body_bytes` | `8_388_608` | Maximum response body accepted per artifact. |
| `max_total_bytes` | `67_108_864` | Maximum decompressed response bytes accepted across one run. |
| `max_concurrent_fetches` | `8` | Maximum concurrent in-flight fetches. |
| `timeout` | `10s` | Per-request timeout. |
| `total_timeout` | `120s` | Maximum wall-clock time for one online discovery run. |
| `user_agent` | crate/version string | User-Agent sent by the default fetcher. |
| `accepted_schemes` | `http`, `https` | URL schemes eligible for fetching. |
| `credentials` | none | Optional credential policy for protected metadata endpoints. |

`build()` MAY be fallible only for concrete configuration errors:

- invalid header names;
- invalid user-agent value;
- invalid allowed-origin URL;
- zero timeout, fetch budget, concurrency, or body byte limit;
- incompatible policy and credential scope.

If the implementation can reject those errors in setter methods, `build()` MAY
be infallible. The public API MUST document which style it uses.

### Offline Bundles

The facade MUST also support local and pre-fetched analysis without networking:

```rust
let run = DiscoveryBundle::new("https://publisher.example/catalog")
    .add_file("catalog.jsonld")?
    .add_file("shapes.ttl")?
    .analyze()?;
```

This path MUST call the same core analyzer and return the same `DiscoveryRun`
view type.

### Custom Fetchers

The facade SHOULD expose a small fetcher trait so host applications can provide
their own HTTP stack, cache, authentication, proxy, or test fixture store.

```rust
#[async_trait::async_trait]
pub trait DiscoveryFetcher: Send + Sync {
    async fn fetch(&self, request: FetchRequest) -> Result<FetchResponse, FetchError>;
}
```

The default fetcher MAY use `reqwest`.

The facade uses `async_trait` for v1 because callers need
`Arc<dyn DiscoveryFetcher>`. Native `async fn` in traits MAY replace this when
object-safe async trait objects are practical for the supported Rust version.

The facade MUST enforce policy rules before delegating to a custom fetcher. A
custom fetcher MUST NOT receive a request that the policy would reject.

The facade MUST also validate the custom fetcher response before constructing a
core `FetchedArtifact`. At minimum, it MUST enforce:

- final URL policy;
- status and redirect metadata shape;
- decompressed body byte limit;
- response header allowlist;
- credential and userinfo redaction;
- media-type and body-sniff consistency.

## Authentication

The facade MUST support protected metadata endpoints without making the core
aware of credentials.

This is required for publishers such as Registry Relay, where metadata and API
descriptions can be protected by:

- `Authorization: Bearer <api-key>`;
- `X-Api-Key: <api-key>`;
- `Authorization: Bearer <OIDC JWT>`.

The common case SHOULD be concise:

```rust
let run = DiscoveryClient::builder()
    .credentials(Credentials::bearer(token).same_origin_only())
    .build()?
    .discover("https://relay.example.org/metadata")
    .await?;
```

The core invariant still applies:

```text
facade may send credentials while fetching
core never receives credential policy or secret values
```

### Credential Types

The facade SHOULD provide these built-in credential helpers:

```rust
Credentials::bearer(token)
Credentials::api_key_header("X-Api-Key", token)
Credentials::none()
```

`Credentials::bearer(token)` and `Credentials::api_key_header(name, token)`
MUST default to entry-origin-only forwarding. `same_origin_only()` is an
explicit readability helper that returns the same credential value with
RFC 6454 origin scoping. It exists so examples communicate the policy clearly.

The facade MAY provide a custom credential provider trait:

```rust
#[async_trait::async_trait]
pub trait CredentialsProvider: Send + Sync {
    async fn headers_for(&self, request: &FetchRequest) -> Result<Vec<HeaderPair>, CredentialError>;
}
```

The custom provider exists for hosts that need token refresh, OIDC client
credentials, device-code login, mTLS-bound tokens, or service-specific signing.
Those flows SHOULD NOT be built into v1.

### Credential Forwarding Rules

For this document, "same origin" means the RFC 6454 origin tuple: scheme, host,
and port after URL normalization. Path, query, and fragment are not part of the
origin.

Default credential forwarding MUST be conservative:

1. Send credentials only to the entry URL origin.
2. Send credentials to same-origin follow-up fetches.
3. Do not send credentials to cross-origin links discovered in metadata.
4. Drop credentials on cross-origin redirects.
5. Do not send credentials to vocabulary, standards, profile, schema, or
   documentation hosts such as `w3.org`, `schema.org`, SEMIC, or other external
   reference URLs unless the caller explicitly allows that origin.
6. Reject URLs with embedded credentials such as
   `https://user:pass@example.org/catalog`.
7. Never write credential values into `DiscoveryReport`, `DiscoveryRun`,
   `RejectedFetch`, findings, evidence, debug output, or errors.
8. Redact userinfo from every URL that appears in `DiscoveryReport`,
   `DiscoveryRun`, `RejectedFetch`, findings, evidence, errors, and logs.

The facade MUST enforce these rules before calling a custom
`CredentialsProvider`. The provider MAY decide whether credentials exist for an
allowed request, but it MUST NOT expand the destination set beyond the facade's
policy.

Callers MAY opt into a broader allowlist:

```rust
let run = DiscoveryClient::builder()
    .credentials(
        Credentials::bearer(token)
            .allowed_origins(["https://relay.example.org", "https://metadata.example.org"])
    )
    .build()?
    .discover("https://relay.example.org/metadata")
    .await?;
```

The facade MUST NOT support an "send credentials to every discovered URL" mode.
That behavior is too easy to misuse during semantic discovery, where metadata
often links to third-party standards and profile documents.

### Auth-Required Results

When a metadata artifact cannot be fetched because credentials are missing or
rejected, the facade SHOULD keep the run usable when possible.

It MUST record the failure through host-visible state:

```rust
RejectedFetch {
    url,
    reason_code: "auth.required" | "auth.rejected" | "auth.scope_denied",
    discovered_from,
    credential_sent: false | true,
}
```

`credential_sent` MUST indicate whether any credential was attached, but MUST
NOT reveal the credential type or value when that would leak sensitive
information.

Hosts that expose discovery results across tenant boundaries MAY suppress
`credential_sent` or replace it with an access-controlled diagnostic field. The
facade default is useful for local debugging, but a multi-tenant service MUST
treat it as potentially sensitive operational metadata.

By default, `credential_sent` MUST be `true` only for requests sent to the entry
origin or to an origin explicitly listed in `allowed_origins`. Rejections for
third-party vocabulary, standards, profile, or documentation hosts MUST report
`credential_sent: false`.

If the protected artifact is the entry URL and no safe partial report exists,
`discover(url)` MAY return `DiscoveryError::FetchFailed` or
`DiscoveryError::FetchRejected`.

If the protected artifact is a follow-up URL, the run SHOULD continue and expose
the auth-required gap through:

- `DiscoveryRun::rejected_fetches()`;
- the condition named `HasNoBlockingFetchFailures` surfaced through
  `ConditionView`;
- `DiscoveryReport.findings` when the rejection can be safely represented
  without host secrets.

### Failure Surfaces

The facade MUST use the following decision table for fetch and authorization
failures:

| Case | Return value | `RejectedFetch` | Report finding | Artifact status |
|---|---|---:|---:|---|
| Entry URL rejected by policy | `Err(DiscoveryError::FetchRejected)` | yes | no report | no report |
| Entry URL requires auth and none is available | `Err(DiscoveryError::FetchRejected)` | yes | no report | no report |
| Entry URL fetch fails before any artifact exists | `Err(DiscoveryError::FetchFailed)` | yes | no report | no report |
| Follow-up URL rejected by policy | `Ok(DiscoveryRun)` | yes | optional safe finding | not applicable |
| Follow-up URL requires auth or scope is denied | `Ok(DiscoveryRun)` | yes | optional safe finding | `AuthRequired` only if represented as an artifact |
| Follow-up URL body is too large | `Ok(DiscoveryRun)` | yes | optional safe finding | `TooLarge` only if represented as an artifact |
| Core parser cannot understand a fetched artifact | `Ok(DiscoveryRun)` | no | yes | `ParseError` or `Unsupported` |

Auth and policy failures MUST NOT be reported through four independent surfaces
with contradictory meanings. The table above is normative for v1.

### Registry Relay Compatibility

The facade MUST support Registry Relay's metadata discovery use case:

```rust
let run = DiscoveryClient::builder()
    .credentials(Credentials::bearer(metadata_key).same_origin_only())
    .build()?
    .discover("https://relay.example.org/metadata")
    .await?;
```

For local development, callers MAY use a non-public-web policy that allows
loopback:

```rust
let run = DiscoveryClient::builder()
    .policy(DiscoveryPolicy::local_development())
    .credentials(Credentials::bearer(metadata_key).same_origin_only())
    .build()?
    .discover("http://127.0.0.1:4242/metadata")
    .await?;
```

`DiscoveryPolicy::local_development()` MUST be opt-in and MUST NOT be the
default.

## Policy

`DiscoveryPolicy::public_web()` MUST:

- allow only `http` and `https`;
- block loopback, private, link-local, multicast, and unspecified IP ranges;
- pin resolved IP addresses for each request so the address validated by policy
  is the address used for the TCP connection;
- re-check scheme, origin rules, and resolved addresses after each redirect;
- reject redirects after a configurable limit, default `10`;
- reject `https` to `http` downgrades unless explicitly allowed by policy;
- re-validate every `Location` value before fetching it;
- drop response headers that are not on the core allowlist before constructing
  `FetchedArtifact`;
- cap decompressed body size;
- preserve host rejections as `RejectedFetch` records.

The core response header allowlist MUST be minimal. It SHOULD include only:

- `content-type`;
- `content-length`;
- `etag`;
- `last-modified`;
- `link`;
- `location` for redirect evidence after URL redaction.

All other response headers MUST be dropped before the core sees the artifact.
This includes, but is not limited to:

- `authorization`;
- `cookie`;
- `set-cookie`;
- `www-authenticate`;
- `x-api-key`;
- `proxy-authorization`.

The core MAY redact defensively, but the facade MUST treat host-side stripping as
the primary security control.

`max_body_bytes` applies to the decompressed body bytes passed to the core. The
default fetcher MUST also apply a reasonable compressed transfer limit so a
small compressed payload cannot expand without bound.

The default fetcher MUST NOT enable an HTTP cookie store. Cookies MUST NOT be
accepted, persisted, or replayed as silent credentials during discovery unless a
host provides an explicit custom fetcher and accepts that policy responsibility.

`DiscoveryPolicy::local_development()` MUST require an explicit
`local-development-policy` Cargo feature or emit a runtime warning when used. It
MUST never be selected by `DiscoveryClient::new()`.

## Return Contract

The facade MUST return `DiscoveryRun`.

```rust
pub struct DiscoveryRun {
    report: DiscoveryReport,
    fetched: FetchSummary,
    rejected_fetches: Vec<RejectedFetch>,
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

`DiscoveryReport` remains the canonical durable core contract. It is what hosts
store for the analyzer result, validate in core fixtures, and pass to future
Java, Python, Node, or CLI wrappers.

Online discovery also has a small host envelope because rejected fetches and
fetch summaries are produced by the facade, not by the core analyzer:

```rust
pub struct DiscoveryRunEnvelope {
    pub report: DiscoveryReport,
    pub fetched: FetchSummary,
    pub rejected_fetches: Vec<RejectedFetch>,
}
```

Cross-language facade wrappers MUST treat this envelope as the portable online
discovery result.

`DiscoveryRun` is an ergonomic wrapper. It MUST NOT contain hidden semantic
truth that is absent from `DiscoveryReport`, except host-runtime fetch summary
data and rejected fetch details.

For v1, `DiscoveryRun` MUST NOT expose normative `SystemsView` or
`CapabilitiesView`. Those concepts are useful, but they require domain
classification rules that do not exist in the canonical report yet. They belong
in `system-capability-discovery` until the report schema explicitly carries
derived systems and capabilities.

Required methods:

```rust
impl DiscoveryRun {
    pub fn report(&self) -> &DiscoveryReport;
    pub fn into_report(self) -> DiscoveryReport;
    pub fn fetched(&self) -> &FetchSummary;
    pub fn rejected_fetches(&self) -> &[RejectedFetch];

    pub fn registry<'r>(&'r self) -> RegistryView<'r>;
    pub fn substrate<'r>(&'r self) -> SubstrateView<'r>;
    pub fn graph<'r>(&'r self) -> GraphView<'r>;
    pub fn evidence<'r>(&'r self) -> EvidenceView<'r>;
    pub fn conditions<'r>(&'r self) -> ConditionView<'r>;
}
```

View items MUST borrow from `DiscoveryRun` or its `DiscoveryReport`, not from a
temporary view value. This example MUST compile:

```rust
let first_dataset = run.registry().datasets().next();
```

## Navigation Model

The facade MUST avoid making standards the primary navigation hierarchy.

Callers should ask practical questions:

- what can be registered;
- how it is described;
- how it can be accessed;
- what policy or authorization signals affect access;
- what standards and profiles support the conclusion;
- what evidence proves it;
- what is missing or blocked.

Standards and profiles are evidence and capability markers, not the first object
most callers should traverse.

### Systems And Capabilities Boundary

The base facade MUST expose the evidence needed to derive systems and
capabilities, but it MUST NOT publish those as first-class v1 views.

The reason is practical: the canonical `DiscoveryReport` carries artifacts,
assets, links, standards, profiles, findings, and fetch candidates. It does not
carry `DiscoveredSystem`, `DiscoveredCapability`, `Confidence`, or
domain-specific capability kinds. If the Rust facade invented those projections
without schema-level rules, Java, Python, Node, and Atlas implementations would
drift.

For v1, callers derive system and capability candidates from:

- `RegistryView` assets;
- `GraphView` relationships;
- access methods and endpoint URLs;
- standards and profile claims;
- findings and conditions;
- evidence references.

The separate `system-capability-discovery` layer owns:

- operational capability kinds;
- confidence and ranking;
- cross-report system identity;
- domain synonym and vocabulary mapping;
- user question matching such as "registered as a farmer" or "has
  disabilities".

### Registry View

`RegistryView` presents assets that a central registry can reasonably display or
review.

```rust
let registry = run.registry();

for dataset in registry.datasets() {
    println!("{}", dataset.title().unwrap_or("Untitled"));

    for access in dataset.access_methods() {
        println!("access: {}", access.url());
    }
}
```

Required selectors:

```rust
impl<'r> RegistryView<'r> {
    pub fn catalogues(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
    pub fn datasets(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
    pub fn services(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
    pub fn distributions(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
    pub fn profiles(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
    pub fn semantic_models(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
    pub fn registerable_assets(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
}
```

`RegistryAsset` MUST provide:

```rust
impl<'r> RegistryAsset<'r> {
    pub fn id(&self) -> &str;
    pub fn kind(&self) -> SemanticAssetKind;
    pub fn uri(&self) -> Option<&str>;
    pub fn title(&self) -> Option<&str>;
    pub fn description(&self) -> Option<&str>;
    pub fn publisher(&self) -> Option<&str>;
    pub fn source_url(&self) -> Option<&str>;

    pub fn access_methods(&self) -> AccessMethodsView<'r>;
    pub fn semantics(&self) -> SemanticsFacet<'r>;
    pub fn policy(&self) -> PolicyFacet<'r>;
    pub fn trust(&self) -> TrustFacet<'r>;
    pub fn claims(&self) -> ClaimsView<'r>;
    pub fn evidence(&self) -> impl Iterator<Item = EvidenceItem<'r>> + 'r;
    pub fn conditions(&self) -> impl Iterator<Item = Condition<'r>> + 'r;
}
```

Selector mapping MUST be deterministic:

| Selector | `SemanticAssetKind` values |
|---|---|
| `catalogues()` | `Catalog` |
| `datasets()` | `Dataset`, `RecordCollection`, `FeatureCollection` |
| `services()` | `DataService`, `ApiDescription` |
| `distributions()` | `Distribution` |
| `profiles()` | `Profile` |
| `semantic_models()` | `SemanticModelPackage`, `ShapeGraph`, `ConceptScheme`, `Vocabulary`, `VocabularyTerm`, `Class`, `Property`, `Alignment`, `Crosswalk` |
| `registerable_assets()` | All selector results above, plus `Policy`, `QualityMeasurement`, `LifecycleStatus`, `PrivacyBasis`, and `TrustArtifact` when they have a stable URI or title |

### Substrate View

`SubstrateView` maps discovery results to the production standards stack used by
DPI and dataspace environments.

```rust
let substrate = run.substrate();

if substrate.catalogue().has_dcat() {
    println!("machine-readable catalogue found");
}

for api in substrate.exchange().openapi_specs() {
    println!("OpenAPI: {}", api.source_url());
}
```

Required lenses:

```rust
impl<'r> SubstrateView<'r> {
    pub fn catalogue(&self) -> CatalogueLayer<'r>;
    pub fn semantics(&self) -> SemanticsLayer<'r>;
    pub fn trust(&self) -> TrustLayer<'r>;
    pub fn policy(&self) -> PolicyLayer<'r>;
    pub fn runtime_auth(&self) -> RuntimeAuthLayer<'r>;
    pub fn exchange(&self) -> ExchangeLayer<'r>;
    pub fn profiles(&self) -> ProfileLayer<'r>;
}
```

Layer mapping:

| Layer | Standards and profiles surfaced |
|---|---|
| `catalogue` | DCAT, DCAT-AP, BRegDCAT-AP, GeoDCAT-AP, national DCAT profiles. |
| `semantics` | RDF, JSON-LD, SHACL, JSON Schema, LinkML, OWL, SKOS, alignments, crosswalks. |
| `trust` | DID, Verifiable Credentials, OpenID4VC references where discoverable. |
| `policy` | ODRL, rights statements, access-rights metadata, policy artifacts. |
| `runtime_auth` | OAuth2, OIDC, scopes, authorization-server metadata where discoverable. |
| `exchange` | OpenAPI, AsyncAPI, CloudEvents, OGC API, FHIR, DSP. |
| `profiles` | SEMIC, OSLO, ePING, DCAT-BR, PublicSchema, and other declared community or country profiles. |

The facade MUST NOT pretend unsupported substrate layers were searched. If a
layer has no parser support in core, its lens MUST expose a parser-support
condition such as `Unknown` or `Warning`, not only an empty iterator. DSP,
OAuth/OIDC metadata, ODRL policy artifacts, DID documents, and OpenID4VC
references are valid lenses, but v1 MUST mark them according to actual parser
coverage.

### Graph View

`GraphView` provides explicit nodes and edges for applications that need richer
navigation.

```rust
let graph = run.graph();
let node = graph.asset(asset_id)?;

for linked in graph.outgoing(node.id()) {
    println!("{} -> {}", linked.rel(), linked.target_id_or_url());
}
```

The graph MUST expose:

- artifacts;
- semantic assets;
- discovered links;
- standards claims;
- profile claims;
- evidence references.

The graph MUST NOT require RDF knowledge for basic traversal.

### Evidence View

Every user-visible conclusion SHOULD be explainable.

```rust
for evidence in run.evidence().for_asset(dataset.id()) {
    println!("{} from {}", evidence.term(), evidence.source_url());
}
```

`EvidenceView` MUST let callers find:

- evidence for an asset;
- evidence for a condition;
- evidence for a standards claim;
- evidence for a profile claim;
- source artifact and source URL for each item;
- predicate, JSON pointer, HTML relation, HTTP header, URL pattern, or content
  sniff marker when available.

Evidence MUST be read-only. It MUST point back to data in `DiscoveryReport`.

### Condition View

The facade MUST use conditions rather than one vague readiness boolean.

```rust
let conditions = run.conditions();

if conditions.can_register_catalogue().is_true() {
    println!("good enough for registry review");
}

for condition in conditions.all() {
    println!("{}: {:?}", condition.name(), condition.status());
}
```

Required condition statuses:

```rust
pub enum ConditionStatus {
    True,
    False,
    Unknown,
    Warning,
}
```

Required registry-level conditions:

| Condition | Meaning |
|---|---|
| `HasMachineReadableEntry` | Entry URL produced at least one recognized artifact. |
| `HasRegisterableAsset` | At least one catalogue, dataset, service, profile, or semantic model can be shown in a registry. |
| `HasStableIdentity` | Registerable assets have stable URI or deterministic IDs. |
| `HasHumanLabel` | Registerable assets have title or label metadata. |
| `HasAccessMethod` | At least one service, distribution, endpoint, or API description exists. |
| `HasSemanticConstraints` | SHACL, JSON Schema, LinkML, OWL, or equivalent semantic structure exists. |
| `HasDeclaredProfile` | A community, country, or application profile claim exists. |
| `HasPolicySignal` | Access-rights, ODRL, rights statement, or equivalent policy signal exists. |
| `HasTrustSignal` | DID, VC, issuer, verifier, or trust metadata exists. |
| `HasNoBlockingFetchFailures` | The harvest did not fail on required first-order metadata. |

`HasStableIdentity` MUST be conservative in v1. It is `True` only when the
registerable asset has an absolute `http`, `https`, `urn`, or `did` URI, or a
deterministic report ID derived from stable URL plus content evidence. Blank
nodes, generated array positions, and labels alone are not stable identity.

Condition derivation MUST follow this table:

| Condition | `True` rule | `Warning` or `False` rule |
|---|---|---|
| `HasMachineReadableEntry` | At least one fetched artifact connected to the entry URL has status `Fetched` and a kind other than `Unknown`, `Unsupported`, or `HtmlLandingPage`. | `False` when no recognized entry-connected artifact exists. |
| `HasRegisterableAsset` | `RegistryView::registerable_assets()` has at least one asset other than a standalone `Distribution`. | `False` when no such asset exists. Distributions alone are useful access evidence but do not satisfy this condition. |
| `HasStableIdentity` | Every registerable asset that satisfies `HasRegisterableAsset` has a stable URI or deterministic report ID as defined above. | `Warning` when at least one registerable asset lacks stable identity. `False` when no registerable asset exists. |
| `HasHumanLabel` | Every registerable asset that satisfies `HasRegisterableAsset` has `title`, `label`, or equivalent human-readable source hint. | `Warning` when at least one registerable asset lacks a human label. `False` when no registerable asset exists. |
| `HasAccessMethod` | At least one `DataService`, `ApiDescription`, `Distribution`, endpoint URL, OpenAPI, OGC API, FHIR, DSP, or equivalent access artifact is linked to a registerable asset or catalogue. | `Unknown` when parser support for the declared access standard is missing. `False` when no access evidence exists. |
| `HasSemanticConstraints` | At least one SHACL, JSON Schema, LinkML, OWL, FHIR profile, or equivalent semantic constraint asset or artifact exists. | `Unknown` when declared semantics require unsupported parser support. `False` when no semantic-constraint evidence exists. |
| `HasDeclaredProfile` | At least one profile claim exists in `DiscoveryReport.profiles` or an asset has a profile-like `conforms_to` value. | `False` when no declared profile evidence exists. |
| `HasPolicySignal` | At least one access-rights, rights statement, ODRL, policy asset, or policy source hint exists. | `Unknown` when a policy artifact is declared but unsupported. `False` when no policy evidence exists. |
| `HasTrustSignal` | At least one DID, VC, issuer, verifier, trust artifact, or trust source hint exists. | `Unknown` when a trust artifact is declared but unsupported. `False` when no trust evidence exists. |
| `HasNoBlockingFetchFailures` | No `RejectedFetch` is required to understand the entry catalogue or first-order registerable assets. | `Warning` for rejected optional follow-up links. `False` for rejected entry URL or rejected first-order catalogue, profile, schema, API, or distribution links. |

Conditions MUST include:

```rust
pub struct Condition<'a> {
    pub name: &'a str,
    pub status: ConditionStatus,
    pub reason: &'a str,
    pub message: &'a str,
    pub evidence: Vec<EvidenceRef<'a>>,
}
```

Conditions MUST be deterministic for the same `DiscoveryRunEnvelope`. Conditions
that depend only on core analyzer output MUST be deterministic for the same
`DiscoveryReport`. Conditions that depend on host fetch behavior, such as
`HasNoBlockingFetchFailures`, MUST read only `FetchSummary` and
`RejectedFetch` values from the envelope.

## Facets

An asset MAY expose multiple facets. This is the main API move that keeps the
returned data elegant.

Example:

```text
Dataset
  catalogue facet: DCAT / DCAT-AP metadata
  semantics facet: SHACL / JSON Schema / LinkML
  access facet: OpenAPI / FHIR / OGC API / DSP
  policy facet: ODRL / access rights
  trust facet: VC / DID / issuer references
  profile facet: SEMIC / OSLO / ePING / PublicSchema / country profile
```

The facade SHOULD let callers start from the asset and move outward:

```rust
let dataset = run.registry().datasets().next().unwrap();

dataset.catalogue_metadata();
dataset.semantic_constraints();
dataset.access_methods();
dataset.policy();
dataset.trust();
dataset.claims().profiles();
dataset.evidence();
dataset.conditions();
```

This avoids fragmenting the API into disconnected standard-specific modules.

Minimum facet shapes:

```rust
impl<'r> AccessMethodsView<'r> {
    pub fn all(&self) -> impl Iterator<Item = AccessMethod<'r>> + 'r;
    pub fn api_descriptions(&self) -> impl Iterator<Item = AccessMethod<'r>> + 'r;
    pub fn distributions(&self) -> impl Iterator<Item = AccessMethod<'r>> + 'r;
    pub fn human_processes(&self) -> impl Iterator<Item = AccessMethod<'r>> + 'r;
}

impl<'r> SemanticsFacet<'r> {
    pub fn constraints(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
    pub fn classes(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
    pub fn properties(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
    pub fn vocabularies(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
}

impl<'r> PolicyFacet<'r> {
    pub fn access_rights(&self) -> impl Iterator<Item = PolicySignal<'r>> + 'r;
    pub fn rights_statements(&self) -> impl Iterator<Item = PolicySignal<'r>> + 'r;
    pub fn policy_artifacts(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
}

impl<'r> TrustFacet<'r> {
    pub fn issuers(&self) -> impl Iterator<Item = TrustSignal<'r>> + 'r;
    pub fn verifiers(&self) -> impl Iterator<Item = TrustSignal<'r>> + 'r;
    pub fn trust_artifacts(&self) -> impl Iterator<Item = RegistryAsset<'r>> + 'r;
}

impl<'r> ClaimsView<'r> {
    pub fn standards(&self) -> impl Iterator<Item = StandardClaimView<'r>> + 'r;
    pub fn profiles(&self) -> impl Iterator<Item = ProfileClaimView<'r>> + 'r;
    pub fn conforms_to(&self) -> impl Iterator<Item = &'r str> + 'r;
}
```

Facet methods MUST be projections over `DiscoveryReport` and
`DiscoveryRunEnvelope`; they MUST NOT infer policy approval, runtime
authorization, trust validity, or domain capabilities.

## Cross-Language Contract

The core cross-language contract is:

```text
AnalyzeInput JSON -> DiscoveryReport JSON
```

The online facade cross-language contract is:

```text
DiscoveryRequest JSON -> DiscoveryRunEnvelope JSON
```

`DiscoveryRequest` MUST be a serializable host request:

```rust
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
```

`DiscoveryPolicyName` MUST include at least `PublicWeb` and
`LocalDevelopment`. `LocalDevelopment` MUST fail unless the receiving host has
explicitly enabled local development policy.

`DiscoveryRequest` MUST NOT contain secret credential values. Hosts attach
credentials through process-local configuration, environment-specific secret
stores, or explicit in-memory builders.

Future wrappers SHOULD preserve this layering:

```text
Python facade
  uses native FFI, WASM, or CLI subprocess
  returns Python DiscoveryRun helpers over DiscoveryRunEnvelope JSON

Java facade
  uses native FFI, WASM, or CLI subprocess
  returns Java DiscoveryRun helpers over DiscoveryRunEnvelope JSON

Node facade
  may use WASM or native binding
  returns JS DiscoveryRun helpers over DiscoveryRunEnvelope JSON
```

The Rust facade MUST NOT rely on Rust-only enum behavior that cannot be
represented in the canonical JSON report or online run envelope.

## Error Model

Facade errors MUST be separate from core `AnalyzeError`.

Required categories:

```rust
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum DiscoveryError {
    #[error("invalid URL: {url}")]
    InvalidUrl { url: String, #[source] source: url::ParseError },

    #[error("invalid discovery policy: {message}")]
    InvalidPolicy { message: String },

    #[error("fetch failed for {url}: {message}")]
    FetchFailed {
        url: String,
        message: String,
        rejected: Option<Box<RejectedFetch>>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("fetch rejected for {url}: {reason_code}")]
    FetchRejected {
        url: String,
        reason_code: String,
        rejected: Box<RejectedFetch>,
    },

    #[error("body too large for {url}: {actual_bytes} > {limit_bytes}")]
    BodyTooLarge {
        url: String,
        actual_bytes: u64,
        limit_bytes: u64,
    },

    #[error("too many redirects for {url}: {limit}")]
    TooManyRedirects {
        url: String,
        limit: u32,
    },

    #[error("core analysis failed")]
    CoreAnalyze {
        #[source]
        source: semantic_asset_discovery_core::AnalyzeError,
    },

    #[error("internal discovery invariant failed: {message}")]
    Internal { message: String },
}
```

`DiscoveryError::Internal` MUST be reserved for implementation bugs or
panic-equivalent invariant failures. Input-shape problems, parser discomfort,
unknown media types, unsupported standards, auth failures, policy rejections, and
network failures MUST use the more specific categories above or become report
findings when a safe partial run exists.

Input-shape and parse failures discovered by the core SHOULD remain findings in
`DiscoveryReport` when a safe partial report can be returned.

Network and host-policy failures MUST follow the failure-surfaces table above.
They MAY appear both as `DiscoveryError` and `RejectedFetch` only when the table
requires an error return and the rejected URL is still safe to expose.

## Storage And Registry Use

A central registry SHOULD store the full `DiscoveryReport`, not only derived
views.

Recommended registry flow:

```text
DiscoveryClient::discover(url)
  -> DiscoveryRun
  -> store DiscoveryReport JSON and DiscoveryRunEnvelope host metadata
  -> index RegistryView assets
  -> index SubstrateView layer signals
  -> render ConditionView for review
  -> keep EvidenceView links for audit and debugging
  -> pass stored reports to system-capability-discovery for system and
     capability matching when needed
```

The registry MUST treat conditions as discovery state, not governance approval.

## Minimal Implementation Slice

The first implementation of this facade is complete only when all items below
are true:

1. `semantic-asset-discovery` crate exists and depends on
   `semantic-asset-discovery-core`.
2. `DiscoveryClient::new().discover(url).await` performs bounded public-web
   discovery with safe defaults.
3. `DiscoveryClient::builder()` supports policy, depth, fetch count, per-body
   byte limit, total byte limit, concurrency, per-request timeout, total
   timeout, and user-agent configuration.
4. Public-web policy blocks private, loopback, link-local, multicast, and
   unsupported schemes before fetching and after redirects.
5. Sensitive headers are stripped before constructing core `FetchedArtifact`
   values.
6. Host-rejected candidates are returned as `RejectedFetch` records with a
   stable reason code.
7. `DiscoveryBundle::new(entry_url).add_file(...).analyze()` supports offline
   analysis.
8. `DiscoveryRun::report()` returns the canonical report produced by core.
9. `RegistryView`, `SubstrateView`, `GraphView`, `EvidenceView`, and
   `ConditionView` exist and are covered by tests.
10. The CLI `harvest` command uses the facade instead of its existing parallel
   fetch loop. Existing CLI harvest tests MUST be updated rather than bypassed.
11. Existing core, WASM, CLI, and Atlas tests still pass.
12. A fixture based on a DCAT-AP catalogue with linked SHACL and OpenAPI
    artifacts demonstrates end-to-end navigation through registry, substrate,
    graph, evidence, rejected fetches, and conditions.

## Test Requirements

The facade MUST have tests for:

- simple one-call discovery against a local HTTP fixture;
- redirect handling and redirect policy rejection;
- private-network rejection;
- body byte limit rejection;
- total byte, total timeout, and concurrency budget enforcement;
- disabled cookie-store behavior in the default fetcher;
- sensitive-header stripping;
- deterministic conditions for the same report or run envelope, depending on
  the condition inputs;
- registry view selectors;
- substrate layer selectors;
- evidence traversal back to source artifact URLs;
- offline bundle analysis;
- CLI harvest using the facade.

Security tests MUST run without live internet access.

## Acceptance Commands

At minimum, release verification MUST include:

```bash
cargo test -p semantic-asset-discovery-core
cargo test -p semantic-asset-discovery
cargo test -p semantic-asset-discovery-cli
cargo test -p semantic-asset-discovery-wasm
pnpm check
pnpm check:semantic
```

Dependency checks MUST confirm:

```bash
cargo tree -p semantic-asset-discovery-core | rg 'reqwest|hyper|ureq|isahc' && exit 1 || true
cargo tree -p semantic-asset-discovery | rg 'reqwest'
```

The first command MUST pass without networking dependencies in core. The second
command MAY show the facade networking dependency.

## Design References

The API intentionally borrows these patterns:

- `reqwest`: builder-based client with safe defaults and explicit advanced
  configuration.
- Crawlee: bounded crawl queue, handled and pending requests, and depth/fetch
  budgets.
- Apache Tika: one detection and extraction interface over many artifact
  formats.
- JSON:API: resources, relationships, links, and included objects are distinct
  concepts.
- Kubernetes: conditions communicate current state better than one readiness
  boolean.
- OpenTelemetry: IDs, links, status, and evidence should be traversable.
- RO-Crate: a discovery result should be self-contained, portable, and
  understandable as a metadata object.

## Non-Blocking Open Decisions

These decisions MUST NOT block v1 implementation.

1. Whether the first facade should expose a blocking API behind a feature flag.
   Default remains async.
2. Whether `DiscoveryRun` helper views should be generated in future wrappers
   from a shared schema, or hand-written per language.
