# System Capability Discovery Specification

## Status

This is a draft specification for a higher-level library or Atlas module built
on top of `semantic-asset-discovery`.

Normative words use RFC 2119 meaning:

- **MUST** means required.
- **MUST NOT** means forbidden.
- **SHOULD** means recommended unless there is a documented reason.
- **MAY** means optional.

This document is intentionally separate from
[`SEMANTIC_ASSET_DISCOVERY_FACADE_SPEC.md`](SEMANTIC_ASSET_DISCOVERY_FACADE_SPEC.md).
The base discovery library finds semantic evidence. This layer turns that
evidence into candidate routes for answering operational questions.
The boundary between discovered facts, Atlas hypotheses, and reviewed claims is
documented in [`STANDARDS_ASSUMPTIONS.md`](STANDARDS_ASSUMPTIONS.md).

## Purpose

System capability discovery helps users answer questions such as:

```text
Where can I find if someone is registered as a farmer?
Where can I find if someone has disabilities?
How could an authorized system access that answer?
What policy, authorization, consent, or review steps apply?
How confident are we, what is missing, and what evidence supports the match?
Can every important claim be traced to machine-checkable metadata evidence?
```

It does not answer person-level questions itself. It discovers where an answer
may live and how a properly authorized system could request it.

The primary output is a set of **candidate answer routes**, not final truth
claims about infrastructure. This wording is deliberate: government metadata is
often incomplete, bundled behind gateways, synchronized into hubs, or published
by intermediaries. A candidate route can be useful even when system ownership,
authority, freshness, or access semantics still need review.

Evidence in this layer is not primarily explanatory prose. It is a reproducible
trace to discovered metadata, host fetch state, reviewed mappings, or review
assertions. Human explanations MAY be generated from that trace, but MUST NOT
be the source of truth for a match.

## Layering

The intended layering is:

```text
semantic-asset-discovery-core
  Parses already-fetched metadata artifacts and emits DiscoveryReport.

semantic-asset-discovery
  Fetches, protects, walks links, and exposes registry, substrate, graph,
  evidence, rejected fetches, and conditions views over one DiscoveryReport and
  its host run envelope.

system-capability-discovery
  Indexes one or many discovery sources and answers operational discovery
  questions by deriving candidate answer routes from assets, labels, profiles,
  links, findings, host run envelopes, and optional reviewed mappings.

Registry Atlas
  Stores reports, lets people review matches, publishes registry views, and
  controls governance workflow.
```

This layer MAY live inside Registry Atlas first. It SHOULD become a separate
library only when another application needs to reuse it.

## Non-Goals

This layer MUST NOT:

- query live person-level data;
- bypass authorization;
- decide that a user is allowed to access a system;
- make governance approval decisions;
- perform fuzzy matching, implicit synonym expansion, approximate semantic
  ranking, or hidden query rewriting in the core matching layer;
- require embeddings, language models, ontology reasoners, databases, or a
  large search stack before useful matching works;
- hide uncertainty, missing evidence, or ambiguous system boundaries;
- turn `semantic-asset-discovery` into a domain-specific government registry.

## V0.1 Scope

V0.1 is a conservative evidence-to-route index.

V0.1 does not include query assist, AI, embeddings, natural-language search, or
fuzzy matching. Those belong to a later optional layer. The v0.1 implementation
MUST be useful when every query is already expressed as accepted `Term` values
or reviewed mappings.

It can answer:

```text
This metadata contains an information object that exactly matches the requested
farmer status term, IRI, or reviewed mapping.
This metadata contains an information object that exactly matches the requested
disability status term, IRI, or reviewed mapping.
This route has explicit metadata, API, distribution, or gated follow-up evidence.
This route has missing policy, authority, identifier, or access detail.
This route is ambiguous and should be reviewed.
```

It MUST NOT claim:

```text
This is definitely the authoritative farmer registry.
This user can access the route.
This operation verifies farmer status with these exact input parameters.
This legal basis or data-sharing agreement is sufficient.
```

Those stronger claims require either first-class parser support in
`semantic-asset-discovery`, reviewed mappings, or human governance state.

## Input Model

The input is one or many `CapabilitySource` values:

```rust
pub struct CapabilitySource {
    pub id: String,
    pub report: DiscoveryReport,
    pub envelope: Option<DiscoveryRunEnvelope>,
    pub mappings: Vec<ReviewedMappingSet>,
    pub review: Vec<ReviewedCapabilityAssertion>,
}
```

`DiscoveryReport` remains the portable evidence contract. `DiscoveryRunEnvelope`
is optional host state from online discovery, such as rejected fetches and fetch
limits. Reviewed mappings and review assertions MUST be explicit and versioned.
They MUST NOT be silently merged into discovered evidence.

The first implementation MAY expose convenience constructors:

```rust
CapabilityIndex::from_reports(reports)?;
CapabilityIndex::from_sources(sources)?;
```

`from_reports` MUST behave as if every report had no envelope, no reviewed
mappings, and no review assertions.

## Reviewed Mappings

Reviewed mappings MAY include:

- domain synonyms;
- country profile mappings;
- sector vocabulary mappings;
- preferred standards profiles;
- organization aliases;
- manually reviewed capability labels;
- access-policy classifications;
- sensitive concept lists.

Reviewed mappings MUST carry:

```rust
pub struct ReviewedMappingSet {
    pub id: String,
    pub version: String,
    pub authority: String,
    pub mappings: Vec<ReviewedMapping>,
}
```

Every match that uses reviewed mappings MUST expose the mapping set id and
version in its signals or evidence references.

## Query API And Modes

The core API MUST support strict need-based queries. This is the default and
only normative v0.1 matching mode.

The caller does not provide a free-form predicate string and expect the library
to interpret it. The caller provides one or more **information needs**. Each
need contains accepted terms, IRIs, or reviewed mappings that the strict matcher
is allowed to verify against discovered metadata.

Strict mode means:

- every query term belongs to a named information need;
- every query term is explicit input accepted by the caller, a reviewed mapping,
  or a reviewed assertion;
- matching is exact after documented canonicalization only;
- the engine does not invent synonyms, infer related concepts, expand
  acronyms, use embeddings, or perform approximate similarity ranking;
- every matched query term is backed by machine-verifiable evidence.

Documented canonicalization MAY include trimming whitespace, Unicode
normalization, case folding for labels, and compact IRI expansion from a known
prefix map. It MUST NOT include stemming, fuzzy edit distance, semantic
similarity, or unreviewed synonym expansion.

Structured query:

```rust
let query = CapabilityQuery::new("social_protection_eligibility")
    .purpose(Term::label("eligibility verification"))
    .country("RWA")
    .need(
        InformationNeed::new("farmer_status")
            .question("Is the person registered as a farmer?")
            .about_any([
                Term::iri("https://schema.org/Person"),
                Term::label("Person"),
            ])
            .requires_any([
                Term::iri("https://example.gov/ns/farmerStatus"),
                Term::label("registered farmer status"),
                Term::label("farmerStatus"),
            ]),
    );

let matches = index.search(query)?;
```

Reviewed mappings are not fuzzy matching. They are explicit, versioned inputs.
For example, a reviewed mapping may say that `smallholder`,
`agricultural producer`, and `registered farmer` are equivalent for a named
country profile and version. A result that uses that mapping MUST include
reviewed mapping evidence.

```rust
pub struct CapabilityQuery {
    pub id: String,
    pub purpose: Option<Term>,
    pub country: Option<String>,
    pub needs: Vec<InformationNeed>,
}

pub struct InformationNeed {
    pub id: String,
    pub question: Option<String>,
    pub about_any: Vec<Term>,
    pub requires_any: Vec<Term>,
    pub requires_all: Vec<Term>,
}

pub enum Term {
    Iri(String),
    Label(String),
    Field(String),
    ReviewedMapping { mapping_set_id: String, mapping_id: String },
}
```

Examples use `Term::iri(...)` and `Term::label(...)` as convenience
constructors for `Term::Iri` and `Term::Label`.

`question` is user-facing context. It MUST NOT be searched by the strict matcher.
Only `about_any`, `requires_any`, `requires_all`, `purpose`, and reviewed mappings may
participate in strict matching.

`Term::Field` matches exact extracted field or property names only. It may match
`DiscoveryEvidence::SchemaProperty.property_name`,
`DiscoveryEvidence::SchemaProperty.property_path`,
`DiscoveryEvidence::ShaclProperty.path`, or equivalent property evidence emitted
by `semantic-asset-discovery`. It MUST NOT match arbitrary description text.

### Need Matching Semantics

`requires_any` and `requires_all` are the core of a need. V0.1 MUST NOT create
a match for a need unless at least one required term has strict evidence or
reviewed mapping support. If `requires_any` is present, at least one of those
terms MUST match. Every `requires_all` term MUST match the same candidate
asset or route. `about_any` narrows or contextualizes the need, but MUST NOT
create a match by itself.

This prevents a generic term such as `Person`, `Household`, or `Organization`
from matching every system that mentions people or organizations.

The matcher SHOULD treat need terms as follows:

| Field | Role |
|---|---|
| `requires_any` | Required information terms. At least one must match for the need to produce a `CapabilityMatch`. |
| `requires_all` | Required conjunctive terms. Every term must match the same candidate asset or route. Use this when a field name is too generic without a label, IRI, or reviewed mapping. |
| `about_any` | Subject or population context. It can improve ordering and evidence explanation, but cannot match alone. |
| `purpose` | Program or use context. It can match policy or purpose evidence, but cannot match alone. |
| `question` | Human wording only. It never participates in strict matching. |

If a user wants to search for systems about people without a required fact, that
is registry search, not system capability discovery.

Natural-language text search is out of the core matching contract for v0.1.
Atlas or another host MAY provide a query-assist layer that turns text into a
draft need-based query:

```rust
let draft = query_assist.prepare(
    "where can I verify whether a person is registered as a farmer?"
)?;

let query = draft.accept_terms([
    "term:person",
    "term:registered-farmer-status",
])?;

let matches = index.search(query)?;
```

`query_assist.prepare` is not part of the strict matcher. It MAY use AI,
embeddings, dictionaries, or other heuristics to propose a need-based query,
but its output MUST be marked as a draft. Draft terms MUST NOT be used by the
strict matcher unless they are accepted by the caller for the current query or
come from a reviewed mapping set. If AI was involved, the resulting matches
MUST carry `ReviewFlag::AiAssisted` and evidence for the AI suggestion.

### Assisted Query Workflow

The intended workflow is:

1. A user states an operational need in their own words.
2. Query assist proposes candidate terms, IRIs, profiles, and reviewed mappings.
3. The user accepts terms for the current query, or a reviewer promotes them
   into a `ReviewedMappingSet`.
4. The strict matcher runs only against accepted query terms and reviewed
   mappings.
5. The result includes both the strict evidence trace and, when relevant, the
   query-assist provenance.

This lets AI help users find the right vocabulary without letting AI decide
that a system is relevant.

```rust
pub struct QueryDraft {
    pub id: String,
    pub proposed_needs: Vec<QueryDraftNeed>,
}

pub struct QueryDraftNeed {
    pub id: String,
    pub question: String,
    pub terms: Vec<QueryDraftTerm>,
}

pub struct QueryDraftTerm {
    pub id: String,
    pub term: Term,
    pub role: QueryTermRole,
    pub status: QueryTermStatus,
    pub source: QueryTermSource,
    pub evidence: Vec<EvidenceRef>,
}

pub enum QueryTermStatus {
    Proposed,
    AcceptedForThisQuery,
    ReviewedMapping,
    Rejected,
}

pub enum QueryTermRole {
    About,
    RequiredInformation,
    Purpose,
}

pub enum QueryTermSource {
    UserInput,
    QueryAssist,
    ReviewedMapping,
    ReviewAssertion,
}
```

Only `AcceptedForThisQuery` and `ReviewedMapping` terms may enter strict
matching. A term with status `Proposed` is visible to the user but has no effect
on canonical results.

## V0.1 Output Model

The main output is a `CapabilitySearchResult` grouped by information need:

```rust
pub struct CapabilitySearchResult {
    pub query_id: String,
    pub inputs_summary: InputsSummary,
    pub needs: Vec<NeedSearchResult>,
}

pub struct InputsSummary {
    pub report_ids: Vec<String>,
    pub envelope_ids: Vec<String>,
    pub reviewed_mapping_sets: Vec<String>,
    pub review_assertions: Vec<String>,
}

pub struct NeedSearchResult {
    pub need_id: String,
    pub matches: Vec<CapabilityMatch>,
}
```

Each need's matches MUST be deterministically ordered:

```rust
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
```

After `EvidenceScore` ties, v0.1 MUST sort matches by stable values only:
`confidence`, source id, need id, route component ids, and evidence ids. It MUST
NOT use insertion order, map iteration order, wall-clock time, random ids, or UI
locale collation.

`EvidenceScore` MUST be derived from countable evidence classes, not embeddings
or opaque model scores:

```rust
pub struct EvidenceScore {
    pub direct_structured_matches: u32,
    pub direct_metadata_matches: u32,
    pub reviewed_mapping_matches: u32,
    pub access_evidence_matches: u32,
    pub gap_count: u32,
}
```

Implementations MAY expose a display score in the UI, but the API contract MUST
preserve the underlying evidence counts.

`MatchConfidence` MUST be derived mechanically from `EvidenceScore`:

| Confidence | Required evidence |
|---|---|
| `High` | `direct_structured_matches >= 1` and `access_evidence_matches >= 1`. |
| `Medium` | `direct_structured_matches >= 1`, or `reviewed_mapping_matches >= 1` with `direct_metadata_matches >= 1`. |
| `Low` | `direct_metadata_matches >= 1` or `reviewed_mapping_matches >= 1`, with no direct structured match. |

If none of these rows applies, no `CapabilityMatch` is produced.

This deliberately small output surface is easier to bind into Java, Python,
TypeScript, and future Atlas storage. Richer policy, trust, identifier, and
operation contracts MAY be layered on top after the base discovery schema
preserves enough structured evidence.

`confidence` describes deterministic evidence match strength. It MUST NOT be
computed from embeddings, fuzzy similarity, or AI probability. It also MUST NOT
hide governance, sensitivity, or authorization concerns. Those concerns belong
in `review_flags` and `gaps`.

The output MUST distinguish:

- discovered facts from `DiscoveryReport`;
- host fetch state from `DiscoveryRunEnvelope`;
- derived matches from this layer;
- reviewed mappings supplied by humans or profile packs;
- accepted information-need terms;
- human review assertions;
- optional AI suggestions.

The optional `explanation` field is for user experience only. It MUST be
derivable from `signals`, `evidence`, `gaps`, and `review_flags`.

## Candidate Answer Route

The route is the central concept.

```rust
pub struct CandidateAnswerRoute {
    pub id: String,
    pub label: String,
    pub components: Vec<RouteComponent>,
    pub boundary: SystemBoundary,
}

pub struct RouteComponent {
    pub role: RouteComponentRole,
    pub label: Option<String>,
    pub uri: Option<String>,
    pub source: RouteComponentSource,
    pub evidence: Vec<EvidenceRef>,
}
```

A candidate answer route says:

- what operational question it may help answer;
- where the relevant metadata says the answer may live;
- which broad access pattern is evidenced;
- what evidence supports the route;
- what gaps or review flags remain.

It MUST NOT claim that the answer is present for a specific person.

### Route Components

V0.1 route components are optional, evidence-backed references. They are not a
complete asset graph.

```rust
pub enum RouteComponentRole {
    PublisherOrGateway,
    Catalogue,
    Dataset,
    InformationObject,
    AccessMethod,
    StandardOrProfile,
}

pub enum RouteComponentSource {
    DiscoveryArtifact,
    SemanticAsset,
    DiscoveryLink,
    StandardClaim,
    ProfileClaim,
    Finding,
    RejectedFetch,
    ReviewedMapping,
    ReviewAssertion,
    AiSuggestion,
}
```

The implementation MUST tolerate missing components. Current
`semantic-asset-discovery` reports do not provide complete asset-to-asset parent
edges, so v0.1 MUST NOT pretend it can always connect class, dataset, service,
and system as a full hierarchy.

## System Boundary And Authority

This layer MUST distinguish:

- publisher or gateway;
- catalogue;
- dataset;
- entity, schema class, shape, collection, or information object;
- access method;
- domain system candidate;
- operational capability candidate.

It MUST NOT blindly promote a dataset title into the system name. This matters
for bundled demos, shared gateways, synchronized sector hubs, caches, and
eligibility engines.

```rust
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
```

When container semantics and entity semantics conflict, the match MUST include
`ReviewFlag::BoundaryAmbiguous`. It MAY still have high strict evidence
confidence if the matched entity or API evidence is direct and strong.

At minimum, v0.1 MUST mark `BoundaryAmbiguous` when one harvested gateway or
catalogue contains candidate matches for at least two different information
needs across different datasets or entity namespaces, and no discovered or
reviewed evidence identifies a single domain system boundary for the match.

V0.1 MUST treat metadata as bundled when one catalogue, gateway, or publisher
artifact contains candidate matches for multiple unrelated domain namespaces,
datasets, or entity groups, and the same container evidence is reused as the
system boundary for each match. Bundled metadata does not make a match invalid,
but it MUST add `ReviewFlag::BoundaryAmbiguous` unless reviewed boundary
evidence names the specific domain system.

### Authority Signals

Government infrastructure often contains copies, extracts, federated access
points, caches, eligibility engines, and derived datasets. V0.1 MUST expose
authority uncertainty as signals and gaps rather than guessing.

The implementation SHOULD emit signals when evidence explicitly contains:

- source of truth or authoritative registry;
- data controller, data processor, steward, or publisher;
- copy, synchronized extract, cache, mirror, or derived data;
- freshness, update cadence, or last modified time;
- lineage or provenance.

If those signals are missing for a sensitive or operationally important match,
the match SHOULD include the relevant `CapabilityGap`.

## Access Summary

Every match MUST say what "access" means, while staying within what the current
evidence can support.

```rust
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
```

Allowed access kinds:

| Kind | Meaning |
|---|---|
| `MetadataOnly` | The route is discoverable, but no callable access path is found. |
| `ApiDescriptionAvailable` | An API or service description exists, but exact operation semantics are unknown. |
| `DatasetDistribution` | The route exposes a file, feed, collection, or queryable distribution. |
| `HumanProcess` | Metadata points to contact, onboarding, bilateral agreement, or manual access. |
| `RejectedOrGated` | Follow-up access exists but discovery could not fetch it because of auth, policy, size, or method constraints. |
| `Unknown` | Access evidence is insufficient. |

`protocol_hint` MAY contain values such as `http`, `openapi`, `ogc-api`,
`sparql`, `soap`, `sftp`, `message_queue`, `x-road`, `e-delivery`, or
`manual`. `interaction_hint` MAY contain values such as `request_response`,
`batch`, `async`, `publish_subscribe`, `file_drop`, `portal`, `polling`, or
`bilateral_onboarding`.

These are protocol and interaction classifications, not authorization grants.
The layer MUST NOT imply that a caller is authorized just because an access
method exists.

V0.1 has no separate `RouteCapabilityKind`. The route's broad operational
surface is expressed by `AccessSummary.kind`, evidence, and gaps. Richer
capability kinds such as `Lookup`, `Verification`, `EligibilitySignal`, and
`CredentialIssuance` are later work and require first-class operation,
parameter, security, credential, trust, reviewed mapping, or human review
evidence.

## Future Access Contract

Detailed access contracts are out of v0.1 unless supplied by reviewed mappings
or future parser support.

Later versions MAY add:

```rust
pub struct AccessContract {
    pub endpoint: Option<String>,
    pub method: Option<String>,
    pub operation_id: Option<String>,
    pub required_identifiers: Vec<RequiredIdentifier>,
    pub auth: Vec<AuthSignal>,
    pub policy: Vec<PolicyCondition>,
    pub payload_format: Option<String>,
    pub response_semantics: Option<String>,
    pub evidence: Vec<EvidenceRef>,
}
```

That richer contract needs first-class evidence for OpenAPI operations,
parameters, security schemes, OGC queryables, WSDL/SOAP operations, SHACL
properties, identifier namespaces, or reviewed mappings. V0.1 MUST expose gaps
instead of fabricating these details.

## Policy And Governance Conditions

V0.1 policy handling is intentionally conservative.

Policy signals MAY include:

- restricted, public, non-public, or unknown access rights;
- purpose requirement;
- legal basis requirement;
- consent requirement;
- data-sharing agreement requirement;
- requester role or institutional mandate;
- jurisdiction;
- assurance level;
- audit obligation;
- attribute-level disclosure limits.

If the evidence is absent or prose-only, the implementation SHOULD emit a gap
instead of classifying policy intent. AI MUST NOT be the sole basis for legal
or authorization classification.

## Required Identifiers

V0.1 does not require exact identifier extraction. It MAY report identifier
candidates only when labels, schema fields, operation names, or reviewed
mappings exactly match documented identifier terms.

Future `RequiredIdentifier` values SHOULD include:

- label;
- namespace or scheme;
- issuing authority;
- assurance or verification level;
- input, output, or join-key role;
- acceptable alternatives;
- whether the identifier is personal, pseudonymous, or organizational.

When the query, reviewed mapping, access class, or human review says an
identifier is required but exact identifier evidence is missing, the match
SHOULD include `CapabilityGap::RequiredIdentifierUnknown`.

## Typed Standard Projections

The implementation SHOULD normalize matching signals by standard surface rather
than treating all evidence as free text.

V0.1 projections MAY be shallow:

| Surface | Useful deterministic signals |
|---|---|
| DCAT | `Catalog`, `Dataset`, `DataService`, `Distribution`, `accessService`, `endpointURL`, `accessRights`, publisher, contact. |
| OpenAPI | API description asset, server URL, title, path, method, operation id, and operation summary when extracted. |
| OGC API | landing page, conformance classes, collections, collection titles, queryable metadata when available. |
| SHACL | shape graph asset, target class, property labels when present in extracted source metadata. |
| JSON Schema | class/property-like labels when present in extracted source metadata. |
| PROF | profile claims and base standards. |
| SKOS | concept schemes and vocabulary terms. |
| Host envelope | rejected fetch reason, gated URL, credential-sent flag, byte and depth limits. |

Positive validation and conformance evidence SHOULD be represented as signals
when available. A discovered claim such as "DCAT-AP is declared in metadata" and a checked
conformance result MUST remain distinguishable.

## Matching Signals

The first version MUST order matches using deterministic, explainable signals.

| Signal | Examples |
|---|---|
| Subject match | `Person`, `Household`, `Farmer`, `Beneficiary`, `Patient`. |
| Required information match | `registered farmer`, `disability status`, `eligible`, `receives benefit`. |
| Information object match | dataset title, schema class, SHACL target class, OpenAPI path, method, operation id, or operation summary. |
| Access match | API description, OGC collection, DCAT distribution, rejected gated route. |
| Profile match | SEMIC, OSLO, ePING, DCAT-BR, PublicSchema, country profile. |
| Policy match | restricted, purpose required, consent required, public, non-public. |
| Trust match | DID, VC, issuer, credential schema, verifier endpoint, when explicitly discovered. |
| Evidence strength | direct schema/API evidence outranks weak title-only evidence. |
| Fetch state | auth-gated or rejected follow-up links are useful access signals but not success. |

Evidence strength SHOULD follow this order:

1. Direct schema property, shape property, class, collection, or explicit service
   evidence with an exact IRI, field, operation, collection, or reviewed mapping
   match.
2. Dataset, distribution, service, or profile evidence with an exact label,
   IRI, or reviewed mapping match.
3. Catalogue-level or publisher-level evidence with an exact label, IRI, or
   reviewed mapping match.
4. Reviewed mapping or organization alias with no direct discovered match.

Title, description, and URL fields MAY be searched in strict mode only as exact
canonical label or literal matches. They MUST NOT be tokenized into fuzzy
evidence. AI suggestions without caller acceptance or reviewed mapping support
MUST NOT produce a `CapabilityMatch`.

The first version MUST NOT require machine learning.

## Signals

`CapabilitySignal` is the generic extension point for concepts that are not yet
stable enough to deserve dedicated structs:

```rust
pub struct CapabilitySignal {
    pub kind: CapabilitySignalKind,
    pub label: String,
    pub value: Option<String>,
    pub confidence: SignalConfidence,
    pub evidence: Vec<EvidenceRef>,
}
```

Signal kinds SHOULD include `Subject`, `RequiredInformation`, `Access`,
`Policy`, `Trust`, `Authority`, `Freshness`, `Profile`, `Validation`,
`Identifier`, `Lineage`, `ReviewedMapping`, and `AiSuggestion`.

## Confidence And Review Flags

Confidence MUST be explainable and deterministic in v0.1.

```rust
pub enum MatchConfidence {
    High,
    Medium,
    Low,
}
```

Suggested interpretation:

| Level | Meaning |
|---|---|
| `High` | Subject or required information term matched in direct structured schema, collection, service, or strong metadata evidence, and a broad access summary exists. |
| `Medium` | Good metadata, profile, or reviewed mapping exists, but schema, boundary, or access evidence is incomplete. |
| `Low` | Exact but weak evidence exists, such as catalogue-level or publisher-level evidence without direct schema, service, or access evidence. |

Gaps and review flags are deliberately separate:

- `CapabilityGap` means machine evidence is missing, incomplete, or
  insufficient for an operational claim.
- `ReviewFlag` means a human, governance, or product workflow needs attention.

The same concern can produce both only when both statements are true. For
example, missing legal-basis evidence is a `CapabilityGap::LegalBasisUnknown`;
sensitive personal data also adds `ReviewFlag::PolicyReviewRequired`.

Normative gap/flag mapping:

| Concern | Gap | Review flag |
|---|---|---|
| Missing required identifier evidence | `RequiredIdentifierUnknown` | None unless a reviewer must choose an identifier policy. |
| Missing authority or steward evidence | `AuthorityUnknown` | `BoundaryAmbiguous` only when the missing authority affects the system boundary. |
| Missing source-of-truth evidence | `SourceOfTruthUnknown` | `BoundaryAmbiguous` only when gateway, cache, or bundled metadata may be mistaken for the source. |
| Missing freshness evidence | `FreshnessUnknown` | None unless a reviewer sets a freshness threshold. |
| Missing legal basis or purpose policy evidence | `LegalBasisUnknown` or `PurposePolicyUnknown` | `PolicyReviewRequired` for sensitive or operationally consequential routes. |
| Missing data-sharing agreement evidence | `DataSharingAgreementUnknown` | `PolicyReviewRequired` when access depends on a bilateral or institutional agreement. |
| Missing trust or validation evidence | `TrustEvidenceMissing` or `ValidationEvidenceMissing` | None unless a reviewer must approve the trust model before use. |
| Sensitive concepts | Optional policy or authority gaps if evidence is missing | `SensitiveData` always. |

Review flags are:

```rust
pub enum ReviewFlag {
    SensitiveData,
    BoundaryAmbiguous,
    PolicyConflict,
    PolicyReviewRequired,
    ReviewedMappingUsed,
    AiAssisted,
}
```

Sensitive concepts such as disability, health, income, eligibility, household
composition, identity, migration status, and child data MUST add
`ReviewFlag::SensitiveData`. Reviewed policy may add legal handling details,
but MUST NOT suppress the safety flag.

AI MUST NOT change `confidence` in v0.1. It MAY add suggestions or synonyms for
review through `CapabilitySignal` and an `EvidenceRef` whose source is
`EvidenceSourceRef::AiSuggestion`.

## Evidence

Every match MUST be traceable back to `DiscoveryReport` evidence and optional
host, reviewed, or AI-assist evidence.

Evidence MUST be machine-verifiable where the upstream artifact allows it.
This means a reviewer or another implementation can locate the same artifact,
field, triple, JSON Pointer, HTTP header, rejected fetch, reviewed mapping, or
review assertion and reproduce the signal.

V0.1 depends on `semantic-asset-discovery` emitting property-level evidence for
schemas, SHACL, OpenAPI operations, and OGC collections. If that evidence is
missing upstream, it MUST be added there first. This layer MUST NOT reparse
source artifacts to compensate for missing upstream extraction.

Human-readable explanations are allowed, but they are derived views. A match is
not valid because a sentence says "the title mentions farmers." It is valid
because an evidence item points to a concrete title field, schema property,
SHACL triple, OpenAPI operation, OGC collection, profile claim, access-rights
term, rejected fetch, or reviewed mapping.

At minimum, every evidence item MUST show:

- the report or source it came from;
- the artifact, asset, link, standard claim, profile claim, finding, rejected
  fetch, reviewed mapping, or review assertion id;
- a machine-addressable location when possible, such as JSON Pointer, RDF
  subject/predicate/object, OpenAPI path and operation, schema property path,
  SHACL property path, HTTP header name, HTML link relation, or OGC collection
  id;
- the claim it supports;
- the match basis, such as exact IRI, exact field name, normalized label,
  reviewed mapping, rejected fetch reason, or AI suggestion;
- whether it is discovered, derived, reviewed, host-supplied, or AI-assisted.

```rust
pub struct EvidenceRef {
    pub id: EvidenceId,
    pub kind: EvidenceKind,
    pub source: EvidenceSourceRef,
    pub location: Option<EvidenceLocation>,
    pub claim: EvidenceClaim,
    pub match_basis: MatchBasis,
    pub derived_from: Vec<EvidenceId>,
    pub human_summary: Option<String>,
}

pub struct EvidenceId(String);

pub enum EvidenceKind {
    Discovered,
    HostFetchState,
    DerivedSignal,
    ReviewedMapping,
    ReviewAssertion,
    AiSuggestion,
}

pub enum EvidenceSourceRef {
    DiscoveryArtifact { report_id: String, artifact_id: String },
    SemanticAsset { report_id: String, asset_id: String },
    DiscoveryLink { report_id: String, link_id: String },
    StandardClaim { report_id: String, claim_id: String },
    ProfileClaim { report_id: String, claim_id: String },
    Finding { report_id: String, finding_id: String },
    RejectedFetch { source_id: String, rejected_fetch_id: String },
    ReviewedMapping { mapping_set_id: String, mapping_id: String },
    ReviewAssertion { assertion_id: String },
    AiSuggestion { suggestion_id: String },
}

pub enum EvidenceLocation {
    JsonPointer { pointer: String },
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
    HttpHeader { name: String },
    HtmlLink { rel: String, href: String },
    Url { url: String },
    RejectedFetch {
        url: String,
        method: Option<String>,
        status: Option<u16>,
        reason: String,
    },
}

pub struct EvidenceClaim {
    pub capability_need_id: Option<String>,
    pub signal_kind: CapabilitySignalKind,
    pub matched_term: Option<Term>,
    pub value: Option<String>,
}

pub enum MatchBasis {
    ExactIri,
    ExactField,
    ExactLiteral,
    ExactOperation,
    ExactCollection,
    NormalizedLabel,
    ProfileConformance,
    RejectedFetchReason,
    ReviewedMapping,
    ReviewAssertion,
    AiSuggestion,
}
```

`EvidenceLocation` is the downstream location projection used by
`system-capability-discovery`. It is not a duplicate source of truth. When an
upstream `semantic-asset-discovery` evidence variant exists, projection MUST be
one-to-one:

| Upstream `DiscoveryEvidence` | Downstream `EvidenceLocation` |
|---|---|
| `SchemaProperty { schema_pointer, property_path, property_name, .. }` | `SchemaProperty { schema_pointer, property_path, property_name }` |
| `ShaclProperty { shape, path, .. }` | `ShaclProperty { shape, path }` |
| `OpenApiOperation { path, method, operation_id, summary, .. }` | `OpenApiOperation { path, method, operation_id, summary }` |
| `OgcCollection { collection_id, title, .. }` | `OgcCollection { collection_id, title }` |

If the upstream evidence does not carry a structured property, operation, or
collection variant, `system-capability-discovery` MUST NOT invent that
structured location by reparsing the raw artifact.

`EvidenceId` SHOULD be stable within a deterministic run and SHOULD use the
prefix form `evidence:<kind>:<short-hash-or-source-id>`.

`derived_from` is required when the evidence item is a derived signal,
AI suggestion, or human-facing aggregate. It MUST reference one or more
lower-level evidence ids unless the item is a root reviewed mapping or review
assertion.

AI suggestions MUST NOT be treated as discovered evidence. They are draft inputs
that may become accepted query terms, reviewed mappings, or review tasks. An
AI-assisted match MUST include both the `AiSuggestion` evidence item and the
discovered or reviewed evidence that grounds it.

Example machine-verifiable evidence:

```json
{
  "id": "evidence:schema-property:farm-001",
  "kind": "Discovered",
  "source": {
    "SemanticAsset": {
      "report_id": "report:agriculture",
      "asset_id": "asset:farmer-registration-shape"
    }
  },
  "location": {
    "ShaclProperty": {
      "shape": "https://example.gov/ns/FarmerRegistrationShape",
      "path": "https://example.gov/ns/farmerStatus"
    }
  },
  "claim": {
    "capability_need_id": "farmer_status",
    "signal_kind": "RequiredInformation",
    "matched_term": {
      "Iri": "https://example.gov/ns/farmerStatus"
    },
    "value": "farmer registration status"
  },
  "match_basis": "ExactIri",
  "derived_from": [],
  "human_summary": "SHACL property identifies farmer registration status."
}
```

Example generated explanation:

```text
Matched "registered as farmer" because:
- schema class or entity label exactly matches an accepted farmer term, IRI, or
  reviewed mapping;
- API, distribution, or gated follow-up evidence provides an access route;
- access rights evidence is restricted;
- required identifiers are unknown;
- the route is hosted behind a broad gateway, so the system boundary needs review.
```

The generated explanation is useful for the UI, but the evidence object is what
must survive API serialization, tests, and cross-language wrappers.

## Capability Gaps

Matches SHOULD expose gaps directly rather than lowering confidence silently.

```rust
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
```

Gaps are productively actionable: Atlas can use them to tell publishers which
metadata would make the route more useful.

## Deterministic Feasibility

This layer is possible without AI when published metadata contains enough of:

- meaningful dataset, class, property, collection, path, method, operation id,
  or operation summary values that exactly match query terms, IRIs, or reviewed
  mappings;
- stable identifiers or URLs;
- schema, SHACL, OpenAPI, OGC, DCAT, or profile evidence;
- accessRights, contact, auth, purpose, stewardship, or policy metadata;
- host fetch state that shows which follow-up routes are gated or blocked;
- reviewed mappings for local terminology and aliases.

Without AI, the layer can:

- find candidate routes by exact matching against explicit query terms,
  identifiers, labels, and reviewed mappings;
- order direct schema/API/collection matches above exact title-only matches;
- separate dataset, gateway, and domain-system boundaries when metadata says so;
- mark ambiguity, authority gaps, freshness gaps, and missing policy as review
  flags;
- produce useful "where to look next" answers over multiple reports.

Without AI, the layer cannot reliably:

- infer a hidden domain system when metadata only exposes a generic gateway;
- know that two unrelated labels are equivalent without reviewed mappings,
  profiles, or caller-accepted query expansion;
- understand vague descriptions that lack structured schema or operation terms;
- resolve legal access conditions from prose without a reviewed classification;
- decide source-of-truth status without explicit metadata or review;
- decide whether a person is authorized to access a route.

## Query Assist And AI Boundary

AI, embeddings, dictionaries, or heuristic NLP belong in a separate query-assist
layer, not in the strict matcher.

This section describes later optional work. It is not part of the v0.1
definition of done.

The query-assist layer MAY produce:

- draft structured queries from natural language;
- proposed synonyms, acronyms, and domain concepts;
- proposed mappings between local labels and known concepts;
- review tasks for maintaining `ReviewedMappingSet` files;
- human-readable summaries and missing metadata recommendations.

The query-assist layer MUST NOT produce canonical `CapabilityMatch` values by
itself. It can only produce draft query inputs, suggestions, review tasks, or
non-normative explanations.

AI MAY be used as an optional assistant for:

- expanding user needs into candidate subject terms, required information
  terms, IRIs, and reviewed mapping proposals;
- suggesting mappings between local labels and known concepts;
- summarizing long descriptions for human review;
- proposing missing metadata recommendations;
- clustering near-duplicate candidate routes for reviewer ergonomics;
- helping reviewers draft mapping candidates.

AI MUST NOT:

- create a `CapabilityMatch` without deterministic evidence or reviewed mapping
  support;
- change `confidence` in v0.1;
- be the only evidence for access summary, authority status,
  source-of-truth status, legal basis, or policy classification;
- process secrets or person-level data;
- bypass the evidence model or review workflow.

When AI is used to help prepare a query, the result MUST preserve which terms
were AI-suggested, user-accepted, or reviewed. If an accepted AI suggestion
contributes to a match, the match MUST include an `AiSuggestion` evidence
reference, `ReviewFlag::AiAssisted`, and the deterministic discovered or
reviewed evidence that grounds the final match.

## Review Workflow

Atlas SHOULD treat matches as candidates for review.

```rust
pub enum ReviewState {
    Unreviewed,
    Accepted,
    Rejected,
    NeedsMoreEvidence,
}
```

Human review MAY attach notes, preferred labels, country-specific mappings,
approved access guidance, rejected mappings, local policy classifications,
authority assertions, and source-of-truth assertions. Those review artifacts
MUST be separate from the original `DiscoveryReport`.

Human review MAY promote an ambiguous route to an accepted system boundary, but
the promoted boundary MUST be stored as review evidence or reviewed mapping, not
as a mutation of the original discovery report.

## Diverse Use Cases

### Social Protection Program Eligibility

Program designer need:

```text
I am creating a social protection program.
I need to know whether a person is a farmer.
I need to know whether their landholding is less than 2 hectares.
I need to know whether their children are attending school.
```

Expected behavior:

- return separate candidate answer routes for farmer status, landholding size,
  and school attendance;
- treat "less than 2 hectares" as the program's eligibility threshold, not as
  something the discovery layer must prove. The route only needs evidence that
  landholding or parcel area data may exist and may be accessible;
- group matches by need so the caller can see whether one system, several
  systems, or a gateway may satisfy the program requirements;
- identify access patterns such as API description, dataset distribution,
  gated route, contact workflow, or unknown access;
- mark sensitive child and household information with
  `ReviewFlag::SensitiveData`;
- expose gaps for required identifiers, legal basis, data-sharing agreement,
  source-of-truth status, freshness, and policy review;
- avoid claiming that eligibility can be decided until the actual access
  contracts, identifiers, policy, and data quality are reviewed.

Good evidence examples:

- SHACL property or JSON Schema field for farmer status;
- cadastral dataset field for parcel area, landholding size, tenure, parcel
  owner, or cultivator;
- education API operation or schema property for school attendance;
- DCAT distribution, OGC collection, OpenAPI operation, or rejected gated link
  showing an access route;
- reviewed mapping that links local terms such as "smallholder" or "producer"
  to farmer status.

Bad evidence examples:

- a page title saying "agriculture" with no entity, schema, dataset, service,
  or reviewed mapping;
- AI-only synonym expansion from "land" to "cadastral registry" without caller
  acceptance or reviewed mapping support;
- a gateway name that sounds related to "social protection" but has no underlying dataset,
  API, or service evidence.

### Disaster Response Household Targeting

Program designer need:

```text
After flooding, identify candidate systems that may help find affected
households, evacuation status, address or settlement location, and assistance
delivery channels.
```

Expected behavior:

- find candidate routes in disaster management, civil registration,
  geospatial, social registry, and payment or assistance systems;
- distinguish current operational data from static reference datasets when
  freshness evidence exists;
- mark household, location, and assistance data as sensitive where appropriate;
- expose `CapabilityGap::FreshnessUnknown` when update cadence or last modified
  evidence is missing;
- flag ambiguous boundaries when a humanitarian coordination gateway exposes
  many datasets but does not identify the authoritative source.

Good evidence examples:

- OGC collection for flood impact zones or evacuation centers;
- DCAT dataset with `dcterms:modified`, publisher, and contact;
- schema class or property for household location, assistance status, or
  displacement status;
- access-rights metadata showing restricted humanitarian access.

### Business Permit One-Stop Service

Service designer need:

```text
To issue a business permit, find where to verify company registration, tax
standing, premises address, sector license status, and inspection results.
```

Expected behavior:

- find company registry, tax authority, licensing, land or address, and
  inspection candidate routes;
- distinguish verification-like metadata from simple dataset downloads;
- mark access and authority gaps rather than assuming that a published dataset
  is legally usable for permit decisions;
- expose reviewed mappings when local terms such as "TIN", "trade license",
  or "business identification number" are mapped to known identifier concepts.

Good evidence examples:

- OpenAPI operation for company lookup or tax status verification;
- JSON Schema field for registration number or tax identifier;
- DCAT `DataService` or distribution linked to a company registry dataset;
- policy evidence requiring a government requester role or bilateral agreement.

### Health Referral And Benefits Coordination

Program designer need:

```text
Find candidate systems that can confirm whether a person is enrolled in health
insurance, has a valid referral, or is eligible for transport support.
```

Expected behavior:

- identify health insurance, referral, provider registry, and social assistance
  candidate routes;
- mark health, disability, income, and household data as sensitive;
- surface legal basis, consent, requester role, and data-sharing agreement gaps;
- avoid operation-level claims unless OpenAPI, SOAP/WSDL, FHIR capability
  statements, reviewed mappings, or human review provide that detail.

Good evidence examples:

- service or schema evidence for insurance enrollment;
- profile or standards evidence for FHIR, OpenAPI, or DCAT health profile;
- rejected authenticated endpoint that indicates a gated access route;
- reviewed mapping that connects local benefit names to transport support.

## Examples

### Clear Agriculture Registry

Input question:

```text
Where can I find if someone is registered as a farmer?
```

Possible match:

```text
Route: Farmer registration evidence route
Boundary: Explicit Agriculture Registry, if metadata or review says so
Publisher or gateway: Ministry of Agriculture API Gateway
Need: farmer_status
About accepted terms: Person
Required accepted terms: registered farmer status, farmerStatus, or reviewed mapping
Access: ApiDescriptionAvailable or DatasetDistribution in v0.1
Protocol hint: openapi, ogc-api, or http when discovered
Required identifiers: unknown in v0.1 unless reviewed mapping says otherwise
Policy: restricted or unknown
Authority: source of truth only if explicitly declared or reviewed
Confidence: High when direct farmer schema/service evidence exists
Gaps: RequiredIdentifierUnknown, LegalBasisUnknown, AuthorityUnknown when evidence is absent
Review flags: SensitiveData, PolicyReviewRequired
Evidence:
- `ShaclProperty` or `SchemaProperty` for farmer status
- `JsonPointer` or `RdfTriple` for the DCAT dataset or distribution title
- `OpenApiOperation`, `OgcCollection`, `Url`, or `RejectedFetch` for access
Review: Unreviewed
```

### Ambiguous Gateway Example

Input question:

```text
Where can I find if someone is registered as a farmer?
```

Possible match over a broad gateway or demo bundle:

```text
Route: Farmer status route candidate
Boundary: Ambiguous
Publisher or gateway: Registry Relay demo gateway
Container: disability_registry demo bundle
Need: farmer_status
About accepted terms: Person, if accepted for the query or supplied by reviewed mapping
Required accepted terms: farmer registration or membership status, if accepted for the query or supplied by reviewed mapping
Access: MetadataOnly or RejectedOrGated, depending on host fetch state
Required identifiers: Unknown
Policy: restricted if discovered
Authority: Unknown
Confidence: Medium
Gaps: RequiredIdentifierUnknown, AuthorityUnknown
Review flags: BoundaryAmbiguous, SensitiveData, PolicyReviewRequired
Evidence:
- `SchemaProperty` or `JsonPointer` for exact farmer terms or reviewed mappings
- `JsonPointer` or `RdfTriple` for the DCAT dataset container
- `RejectedFetch` with host rejection reason for protected row/API links
Review: NeedsMoreEvidence
```

The layer MUST NOT label this as "Agriculture Registry" unless metadata,
reviewed mapping, or human review establishes that boundary.

### Disability Status

Input question:

```text
Where can I find if someone has disabilities?
```

Possible match:

```text
Route: Disability status route candidate
Boundary: Social protection, health, or disability registry only if declared or reviewed
Need: disability_status
About accepted terms: Person
Required accepted terms: disability status, disability category, support need, or reviewed mapping
Access: MetadataOnly, RejectedOrGated, ApiDescriptionAvailable, or DatasetDistribution
Policy: sensitive personal data, restricted or unknown
Authority: source of truth only if declared or reviewed
Confidence: High when direct schema/service evidence exists, otherwise Medium or Low
Gaps: LegalBasisUnknown, AuthorityUnknown when evidence is absent
Review flags: SensitiveData, PolicyReviewRequired
Evidence:
- `SchemaProperty` or `ShaclProperty` for disability status or category
- `JsonPointer` or `RdfTriple` for dataset description and access rights
- `Url`, `OpenApiOperation`, or `RejectedFetch` for access method evidence
Review: NeedsMoreEvidence until policy/access are reviewed
```

## V0.1 Definition Of Done

The first useful implementation is complete only when:

1. It indexes multiple `DiscoveryReport` files.
2. It can optionally accept `DiscoveryRunEnvelope` files and use
   `rejected_fetches` as gated-access signals.
3. It derives candidate answer routes from `DiscoveryReport` artifacts, assets,
   links, standards, profiles, findings, evidence, and optional host rejected
   fetches.
4. It supports strict need-based search with `CapabilityQuery` and
   `InformationNeed`.
5. It supports strict matching only over accepted information-need terms, exact
   IRIs, exact documented canonical labels, and reviewed mappings.
6. `semantic-asset-discovery` emits property-level evidence needed by this
   layer: `SchemaProperty`, `ShaclProperty`, `OpenApiOperation`, and
   `OgcCollection` where those structures exist in source artifacts.
7. It returns `CapabilitySearchResult` grouped by information need, with
   deterministically ordered `CapabilityMatch` values for each need.
8. A need does not produce matches from `question`, `about_any`, or `purpose`
   alone. At least one `requires_any` term must match strict evidence or a
   reviewed mapping.
9. Scores are deterministic evidence counts, not opaque floats, model scores,
   embedding scores, or fuzzy similarity scores.
10. Every match, signal, route component, access summary, and review flag that
   depends on discovered or reviewed facts is backed by at least one
   machine-verifiable `EvidenceRef`.
11. Evidence references include source ids, machine-addressable locations when
   available, supported claims, match basis, and derivation links for derived
   signals.
12. Human explanations are generated from evidence, signals, gaps, and flags,
   and tests prove that removing the evidence invalidates the match.
13. It uses only broad v0.1 access kinds unless reviewed mappings provide
   stronger semantics.
14. It marks sensitive matches with `ReviewFlag::SensitiveData`.
15. It marks ambiguous gateway, bundled metadata, unknown authority, and unknown
   source-of-truth situations with explicit flags or gaps.
16. It includes tests for clear farmer registration evidence, ambiguous farmer
    evidence in a broad gateway, disability status evidence, the social
    protection multi-need use case, and at least one non-social-protection use
    case.
17. It includes tests that assert exact evidence locations for at least JSON
    Pointer, schema property, rejected fetch, and reviewed mapping evidence.
18. It includes tests proving that question text does not affect strict results
    unless terms are accepted into `about_any`, `requires_any`, `purpose`, or
    reviewed mappings.
19. It includes a contamination test: populated `requires_any` matches dataset X
    while the question text mentions unrelated dataset Y, and Y is not returned.
20. It includes tests proving that proposed query-assist terms do not affect
    strict results until accepted for the query or promoted to reviewed
    mappings.
21. It can run fully offline against fixtures.
22. It has no mandatory AI, embeddings, fuzzy matching, network, database, or
    external search dependency.
23. It does not implement query assist as part of the v0.1 acceptance path.
    Query-assist tests, if present, MUST be separate from strict matcher tests
    and MUST prove that proposed terms do not affect strict results.

V0.1 is not done if it claims operation-level parameters, required identifiers,
legal basis sufficiency, source-of-truth status, or user authorization without
explicit discovered evidence or reviewed mappings. It is also not done if a
match can be produced from human explanation text without a corresponding
machine-verifiable evidence trace, if question text changes strict results
without accepted terms, or if unreviewed fuzzy matching can change the
canonical result set.

## Registry Relay Demo Test Design

The Registry Relay demo is a useful integration fixture because it publishes
standard-facing metadata, protected runtime routes, gated fetches, and several
domain datasets through one gateway.

The tests SHOULD use Registry Relay only to create discovery fixtures. The
system capability discovery tests themselves MUST run offline against saved
`DiscoveryReport` and optional `DiscoveryRunEnvelope` fixtures.

The baseline tests SHOULD use normal Registry Relay metadata and registry
surfaces, not SP DCI-specific API routes. SP DCI-specific routes may be added as
later protocol-specific tests, but they are not the v0.1 baseline.

### Fixture Generation

Run Registry Relay with the all-standards demo:

```sh
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-relay
just demo-keys
just demo-run demo/config/all_standards.yaml
```

In another shell, harvest the standard-facing catalog:

```sh
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-atlas
mkdir -p fixtures/system-capability
set -a
. ../registry-relay/demo/.env.local
set +a
cargo run -p semantic-asset-discovery-cli -- harvest \
  --allow-private-network \
  --bearer-token-env CATALOG_VIEWER_RAW \
  http://127.0.0.1:4242/metadata \
  > fixtures/system-capability/registry-relay-all-standards.envelope.json

jq '.report' fixtures/system-capability/registry-relay-all-standards.envelope.json \
  > fixtures/system-capability/registry-relay-all-standards.report.json
```

The harvest command emits a `DiscoveryRunEnvelope`. `validate-report` validates
the nested `.report`, not the envelope. The generated fixture MUST NOT contain
raw bearer tokens. It MAY contain redacted credential-sent state and rejected
fetches.

If Registry Relay demo files are changed to make strict evidence available,
those changes SHOULD be reviewed and committed in the Registry Relay repository
separately. The Atlas fixture metadata SHOULD record the Registry Relay commit
SHA used to generate the fixture.

### Required Demo Cases

The v0.1 system capability implementation is releasable only when these
Registry Relay fixture cases pass:

1. **Farmer status route**
   - Query need: `farmer_status`.
   - `requires_any`: `Term::Label("Farmer")`, a field/property term that exists
     in the farmer entity metadata, or a reviewed mapping to the normal
     `farmer_registry` fixture.
   - Expected: at least one match in `farmer_registry` metadata.
   - Expected evidence: entity title, `rdfs:label`, JSON Pointer, SHACL
     property, or JSON Schema property evidence for the farmer entity or field.
   - Do not rely on `sh:targetClass` being the literal word `Farmer`; Registry
     Relay's current metadata may use a profile-specific IRI while the entity
     title or label carries the human label.
   - Expected access: `DatasetDistribution` when the normal registry dataset
     route is declared in metadata. `MetadataOnly`, `ApiDescriptionAvailable`,
     or `RejectedOrGated` are acceptable only when the harvested fixture lacks
     that route evidence.
   - Must not claim: exact lookup operation parameters, source-of-truth status,
     or user authorization.

2. **Disability status route**
   - Query need: `disability_status`.
   - `requires_all`: `Term::Label("Disabled Person")` and
     `Term::Field("disability_status")`.
   - Expected: the demo query matches disability registry metadata when
     present. Other datasets that happen to carry a `disability_status` field
     are not returned by this strict demo query unless the accepted label, IRI,
     or reviewed mapping also matches the same candidate route.
   - Expected evidence: exact field or schema property evidence for
     `disability_status`.
   - Expected flags: `SensitiveData`, plus policy, authority, or source-of-truth
     gaps when not explicitly declared.
   - Must not collapse multiple matches into one authoritative registry without
     reviewed boundary evidence.

3. **School attendance route**
   - Query need: `school_attendance`.
   - `about_any`: `Term::Label("Student")`.
   - `requires_any`: `Term::Field("attendance_rate")`,
     `Term::Label("Attendance Summary")`, or a reviewed mapping.
   - Expected: match in `education_registry` for `attendance_summary`.
   - Expected evidence: exact field/entity/schema evidence for
     `attendance_rate` or `Attendance Summary`.
   - Expected flags: `SensitiveData` because this is child or education data.
   - Must not claim: the child is attending school, only that a candidate route
     to attendance information exists.

4. **Question text ignored**
   - Query need question: "Where can I know if a child goes to school?"
   - `requires_any`: empty.
   - Expected: no matches.
   - Purpose: prove that question text is never searched by the strict matcher.

5. **Subject-only query ignored**
   - `about_any`: `Term::Label("Person")`.
   - `requires_any`: empty.
   - Expected: no matches.
   - Purpose: prove that generic subject context cannot match every person-like
     dataset.

6. **Unaccepted query-assist proposal ignored**
   - Draft query contains proposed term `Term::Label("Farmer")`.
   - The term status remains `Proposed`.
   - Expected: no strict match.
   - Then mark the same term `AcceptedForThisQuery`.
   - Expected: farmer route can match if exact evidence exists.

7. **Evidence removal invalidates match**
   - Use an in-memory mutation of the deserialized `DiscoveryReport` to remove
     or mutate the exact evidence location for `attendance_rate`.
   - Expected: the school attendance match disappears or loses the
     `RequiredInformation` signal.
   - Purpose: prove that explanations cannot create matches without evidence.

### Demo Acceptance Commands

The implementation SHOULD add the focused Rust test command:

```sh
cargo test -p system-capability-discovery --test registry_relay_demo
```

The release gate MUST include the focused system capability tests plus the
existing Atlas release check:

```sh
cargo test -p system-capability-discovery
cargo test -p system-capability-discovery --test registry_relay_demo
pnpm check:release
```

## Relationship To Atlas

Registry Atlas SHOULD use this layer to power:

- system and capability search;
- candidate answer route review queues;
- "how could I access this?" panels;
- machine-verifiable evidence traces and generated explanations;
- missing metadata recommendations for publishers;
- reviewer-approved mappings and local policy packs.

Atlas SHOULD store separately:

- the original `DiscoveryReport`;
- optional `DiscoveryRunEnvelope` host state;
- derived `CapabilityMatch` results;
- reviewed mapping sets;
- human review state;
- optional AI suggestions.

## Later Work

Later versions may become an operational access contract engine when the
evidence exists. That likely requires:

- operation, parameter, security-scheme, and response extraction from OpenAPI;
- queryable and schema extraction from OGC API collections;
- richer SHACL and JSON Schema constraint extraction beyond property paths;
- protocol-specific adapters for X-Road, eDelivery, SOAP/WSDL, SPARQL, SFTP,
  message queues, and secure file drops;
- positive validation results and profile conformance evidence;
- authority, stewardship, lineage, freshness, and source-of-truth metadata;
- reviewed legal and policy condition packs;
- optional query-assist modules that propose structured queries, mappings, and
  review tasks without changing strict matcher semantics.

## Open Decisions

1. How country profile mappings should be packaged and versioned.
2. Whether optional query-assist belongs only in Atlas, or can be exposed as a
   separate crate that never participates in strict matching by default.

## Implementation Plan

This implementation SHOULD run in a dedicated worktree. Work is split into
parallel waves with disjoint ownership so workers can make progress without
overwriting each other. Each wave has a code-review gate before the next wave
can be treated as complete.

Parallelization rules:

- worker write scopes SHOULD be disjoint;
- workers SHOULD be told they are not alone in the codebase and must not revert
  or overwrite changes made by others;
- Wave 1 starts first and publishes the type/API skeleton early;
- Wave 1.5 may start once the `wave1-skeleton` checkpoint exists and
  `semantic-asset-discovery` integration points are identified;
- Wave 2 may start against the Wave 1 skeleton and Wave 1.5 evidence contract
  once both compile;
- Wave 3 may start fixture and test design in parallel with Wave 1.5 and Wave 2,
  but cannot mark tests complete until Wave 2 evidence extraction exists;
- Wave 4 may start UI/API sketches in parallel, but cannot merge integration
  until Wave 1, Wave 1.5, and Wave 2 pass;
- every worker result receives review before integration, and completed workers
  are closed before new review or follow-up workers are opened.

### Worker Briefing

Use this briefing when dispatching implementation workers:

```text
You are not alone in the codebase. Work only in your assigned write scope.
Do not revert, overwrite, reformat, or clean up changes made by others. If your
task requires a change outside your scope, report the need instead of editing
it. Keep the implementation strict: no AI, no embeddings, no fuzzy matching, no
natural-language search, and no reparsing in system-capability-discovery to
make up for missing semantic-asset-discovery evidence.
```

Every worker wave SHOULD receive this block verbatim before work starts. Wave 1
also receives the explicit no-fuzzy/no-AI review checklist below.

The initial implementation will be a separate Rust crate named
`system-capability-discovery` inside the Atlas workspace. The crate MUST remain
offline and deterministic. It MAY be wrapped by Atlas UI and server code after
the core tests pass.

### Wave 0: Worktree And Fixtures

Owner: main agent.

Tasks:

- create or use a dedicated implementation worktree;
- identify the final crate/module location;
- add fixture directories for system capability tests;
- add a checked-in fixture canonicalization command or script for
  `fixtures/system-capability/**`;
- generate a Registry Relay `all_standards` discovery fixture from a running
  local Registry Relay instance;
- verify that generated fixtures contain no raw bearer tokens;
- save the exact commands used to regenerate fixtures.

Registry Relay live validation commands:

```sh
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-relay
just demo-keys
just demo-run demo/config/all_standards.yaml
```

```sh
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-atlas
mkdir -p fixtures/system-capability
set -a
. ../registry-relay/demo/.env.local
set +a
cargo run -p semantic-asset-discovery-cli -- harvest \
  --allow-private-network \
  --bearer-token-env CATALOG_VIEWER_RAW \
  http://127.0.0.1:4242/metadata \
  > fixtures/system-capability/registry-relay-all-standards.envelope.json

jq '.report' fixtures/system-capability/registry-relay-all-standards.envelope.json \
  > fixtures/system-capability/registry-relay-all-standards.report.json
```

Done means:

- the Registry Relay server was actually started locally;
- the harvest command completed successfully;
- the fixture validates with:

  ```sh
  cargo run -p semantic-asset-discovery-cli -- validate-report \
    fixtures/system-capability/registry-relay-all-standards.report.json
  ```

- a secret scan over the fixture does not find any raw demo token value;
- regenerated fixtures are canonicalized with the checked-in fixture
  canonicalization command before comparison by stripping volatile fetch
  timestamps, sorting object keys, and preserving stable content-derived ids;
- if the demo metadata lacks evidence needed by the spec, the Registry Relay
  demo or metadata manifest is updated in the appropriate repository and
  re-harvested.

Review gate:

- one reviewer checks that fixture generation is reproducible and secrets are
  not persisted.

### Wave 1: Core Types And Strict Matcher

Owner: worker A.

Write scope:

- `crates/system-capability-discovery/src/lib.rs`;
- `crates/system-capability-discovery/src/types.rs`;
- `crates/system-capability-discovery/src/query.rs`;
- `crates/system-capability-discovery/src/matcher.rs`;
- `crates/system-capability-discovery/tests/strict_matcher.rs`.

Tasks:

- implement `CapabilityQuery`, `InformationNeed`, `Term`,
  `CapabilitySearchResult`, `NeedSearchResult`, `CapabilityMatch`,
  `EvidenceScore`, `EvidenceRef`, and supporting enums;
- implement strict matching over accepted `requires_any` and `requires_all`
  terms only;
- implement documented canonicalization only: trim, Unicode normalization, case
  folding for labels, and compact IRI expansion when a prefix map is supplied;
- ensure `question`, `about_any`, and `purpose` cannot create matches without a
  matching `requires_any` or `requires_all` term;
- ensure there is no AI, embedding, fuzzy, stemming, edit-distance, or
  natural-language search dependency.

Done means:

- all public v0.1 types serialize and deserialize in JSON;
- a need with empty `requires_any` and empty `requires_all` returns no matches;
- changing only `question` text does not change results;
- generic `about_any = Person` with empty required terms returns no matches;
- unsupported or empty queries return typed errors, not silent empty states;
- tests prove strict matching is exact and deterministic;
- a `wave1-skeleton` checkpoint is recorded in git before Wave 2 changes are
  integrated.

Review gate:

- one reviewer checks API ergonomics and strictness against this spec;
- one reviewer checks that no fuzzy or AI dependency entered the crate.

### Wave 1.5: Semantic Asset Evidence Extraction

Owner: worker B.

Write scope:

- `crates/semantic-asset-discovery-core/**`;
- `crates/semantic-asset-discovery/**` only where facade serialization or CLI
  output must expose the new evidence;
- semantic discovery tests and fixtures.

Tasks:

- implement upstream `DiscoveryEvidence::SchemaProperty`,
  `DiscoveryEvidence::ShaclProperty`, `DiscoveryEvidence::OpenApiOperation`,
  and `DiscoveryEvidence::OgcCollection`;
- ensure JSON Schema properties, SHACL property paths, OpenAPI operations, and
  OGC collections are emitted by `semantic-asset-discovery`;
- inspect Registry Relay normal metadata/API surfaces and identify whether they
  expose enough evidence for farmer, disability, and attendance fixtures;
- own any required Registry Relay demo metadata or normal registry API output
  changes if the demo lacks standard-facing evidence needed for strict
  matching;
- avoid using SP DCI-specific API routes as the baseline.

Done means:

- semantic discovery fixtures include property-level evidence for JSON Schema
  and SHACL;
- OpenAPI and OGC fixtures include operation and collection evidence when those
  artifacts are present;
- the Registry Relay harvested envelope includes evidence sufficient for the
  farmer, disability, and school attendance demo cases;
- no system capability code reparses raw JSON-LD, SHACL, JSON Schema, OpenAPI,
  or OGC documents to compensate for missing upstream evidence.

Review gate:

- one reviewer checks new evidence records against source artifact locations,
  and checks that Registry Relay demo changes, if any, are standard-facing
  metadata/API improvements rather than DCI-specific shortcuts.

### Wave 2: Evidence Extraction And Scoring

Owner: worker C.

Write scope:

- `crates/system-capability-discovery/src/evidence.rs`;
- `crates/system-capability-discovery/src/score.rs`;
- `crates/system-capability-discovery/src/explain.rs`;
- `crates/system-capability-discovery/tests/evidence_scoring.rs`.

Tasks:

- extract machine-verifiable evidence from `DiscoveryReport` artifacts, assets,
  links, claims, findings, and optional `DiscoveryRunEnvelope` rejected fetches;
- produce evidence locations for JSON Pointer, schema property, URL, rejected
  fetch, and reviewed mapping evidence;
- compute `EvidenceScore` from countable evidence classes;
- order matches deterministically from `EvidenceScore` and a documented
  tie-breaker;
- generate explanations only from evidence, signals, gaps, and flags.

Done means:

- every `CapabilityMatch` has at least one evidence item for the matched
  `requires_any` term;
- every signal that affects confidence or score references evidence;
- removing the exact evidence location from a fixture invalidates the match or
  removes the signal;
- no match can be created from explanation text;
- scores are count structs, not floats or opaque model values.

Review gate:

- one reviewer checks evidence traceability by following fixture evidence ids
  and locations back to source report content, and checks score determinism by
  running tests twice and comparing output snapshots.

### Wave 3: Registry Relay Demo Coverage

Owner: worker D.

Write scope:

- `crates/system-capability-discovery/tests/registry_relay_demo.rs`;
- `fixtures/system-capability/**`.

Tasks:

- add offline tests using the saved Registry Relay discovery fixture;
- cover farmer status, disability status, school attendance, question text
  ignored, subject-only ignored, unaccepted query-assist proposal ignored, and
  evidence removal invalidates match;
- if required evidence is missing or too ambiguous for strict matching, report
  the gap back to Wave 1.5 instead of editing Registry Relay files;
- regenerate the fixture after Wave 1.5 Registry Relay changes land;
- record the Registry Relay commit SHA in the Atlas fixture metadata so fixture
  drift can be detected.

Done means:

- `farmer_status` matches farmer registry evidence without claiming operation
  parameters, authority, or authorization;
- `disability_status` returns the disability registry candidate source for the
  strict demo query and does not leak unrelated student or benefits records just
  because they contain the same field name;
- `school_attendance` matches `education_registry` attendance evidence and
  marks sensitive data;
- negative strictness tests pass;
- the fixture was generated from a running Registry Relay instance, not hand
  invented.

Review gate:

- one reviewer checks that the tests reflect real Registry Relay demo metadata
  and that fixture regeneration uses the pinned Registry Relay commit.

### Wave 4: Atlas Integration

Owner: worker E.

Write scope:

- Atlas server and UI integration only;
- no core matcher changes except reviewed bug fixes.

Tasks:

- expose a small Atlas API or local module entry point for loading reports and
  running strict `CapabilityQuery` values;
- add a focused UI surface for information needs, candidate answer routes,
  evidence traces, gaps, and review flags;
- make question text visibly separate from accepted terms;
- do not add AI query assist in v0.1.

Done means:

- users can run strict searches against loaded discovery reports;
- UI shows grouped results by need;
- each match can reveal evidence source, location, claim, and match basis;
- UI does not offer natural-language search as v0.1 capability discovery;
- Atlas tests cover at least one rendered match and one no-match strictness
  case.

Review gate:

- one reviewer checks UI hierarchy, wording, and that Atlas did not bypass
  strict matcher semantics.

### Wave 5: Release Validation

Owner: main agent, with a final reviewer.

Tasks:

- run focused Rust or Atlas system capability tests;
- run Registry Relay live fixture regeneration once more;
- canonicalize regenerated fixtures with the checked-in fixture canonicalization
  command before comparing or committing;
- run offline Registry Relay fixture tests;
- run Atlas release checks;
- run code review over the complete diff;
- update release notes or README only if the feature is user-facing in this
  version.

Required commands:

```sh
cargo test -p system-capability-discovery
cargo test -p system-capability-discovery --test registry_relay_demo
pnpm check:release
```

The live Registry Relay validation MUST also be run before release:

```sh
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-relay
just demo-run demo/config/all_standards.yaml
```

Then regenerate, canonicalize, and validate the Atlas fixture from the running
instance.

Done means:

- every item in **V0.1 Definition Of Done** is satisfied;
- every Registry Relay demo case in this document has a passing test;
- live Registry Relay fixture generation was performed during the release pass;
- regenerated fixtures were canonicalized with the same checked-in command used
  in Wave 0;
- no raw tokens are committed in fixtures, logs, screenshots, or test output;
- all focused tests and `pnpm check:release` pass;
- code review findings are either fixed or documented as non-blocking with a
  specific reason;
- no "partial", "todo", placeholder, fuzzy matching, AI assist, or
  natural-language search path remains in v0.1 core behavior.
