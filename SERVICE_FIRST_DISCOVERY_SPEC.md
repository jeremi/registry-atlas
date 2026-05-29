# Service-First Discovery Coordination Specification

## Status

Draft implementation contract for coordinated changes across Registry Atlas,
Registry Manifest, Registry Lab, Registry Relay, and Registry Notary.

Normative words use RFC 2119 meaning:

- **MUST** means required.
- **MUST NOT** means forbidden.
- **SHOULD** means recommended unless there is a documented reason.
- **MAY** means optional.

This spec is cross-repository. It defines the shared contract before one
project hardens an incompatible model.

## Decisions

These decisions unblock implementation:

1. Registry Manifest MUST expose a separate service catalogue render format
   named `cpsv-ap`.
2. Registry Lab MUST publish that render at `/metadata/cpsv-ap` and link to it
   from `/metadata`.
3. The first implementation MUST keep top-level procedure services in the same
   metadata manifest as datasets under `public_services`.
4. Atlas report v2 MUST use generic semantic assets plus typed semantic
   relations as the canonical wire form.
5. Atlas and route consumers MUST expose typed projections, such as
   `PublicServiceView` and `RequirementView`, as derived views over the report.
6. Registry Lab discovery scripts MUST use the Atlas Rust navigation API over
   discovery reports, not hand-coded JSON traversal.
7. Atlas UI changes are deferred until the semantic model, Rust navigation API,
   and Registry Lab script story are working.
8. The shared fixture contract MUST be implemented before Atlas v2, Registry
   Manifest renderers, or Registry Lab stories are treated as complete.

## Definitions

**Semantic asset** means a discovered node that has an identity, type, label,
description, artifact source, and evidence. It may represent a public service,
requirement, dataset, data service, form field, policy, shape, or vocabulary
term.

**Semantic relation** means a typed edge between semantic assets, external IRIs,
or blank nodes. It is not necessarily fetchable.

**Discovered link** means a URL link or fetch candidate discovered from an
artifact. It supports navigation and harvesting. It is not a substitute for a
semantic relation.

**Standards claim** means a machine-readable statement that an artifact claims
conformance to a standard or application profile.

**Review claim** means a human or governance assertion that cannot be proven
from metadata alone, such as source-of-truth status, sufficient legal basis, or
production approval.

**Evidence type** means a CCCEV evidence type. It is the type of acceptable
evidence, not the evidence instance and not the service that provides evidence.

**Evidence offering** means a local Registry Manifest record describing how a
provider can supply or evaluate evidence of a given type.

**Evidence provider** means the organization or service that can provide or
evaluate evidence. Registry Notary can be one provider implementation.

**Explainability artifact** means a generated demo artifact that records the
service, requirement, evidence type, provider, route, gaps, and source evidence
used in a discovery story.

**Service graph excerpt** means the subset of a discovery report required to
explain one service workflow.

## Current State

Registry Manifest currently has dataset-scoped `public_services` with
`deny_unknown_fields`. These records render as CPSV public service nodes in
BRegDCAT output, but the schema does not yet support channels, jurisdiction,
procedure requirements, form links, or service-level evidence bindings.

Atlas currently records some CPSV evidence, especially `cpsv:produces`, as link
or signal evidence. It does not emit `SemanticAsset` nodes for
`cpsv:PublicService` in production code today. A parser test fixture contains a
`cpsv:PublicService`, but that is not yet a first-class service model.

Registry Lab currently demonstrates Postgres, OIDC, OpenFn, registry metadata,
and evidence-service flows. Adding an Atlas-style service discovery story is
greenfield work, not an extension of an existing service-first story runner.

## Purpose

The demo and libraries MUST move from registry-first discovery to service-first
discovery.

Current discovery starts too low in the stack:

```text
Discover registry metadata
Discover datasets and evidence offerings
Call evidence services
Evaluate claims
```

The target discovery starts from the administrative service or procedure:

```text
Discover public service
Inspect channels
Inspect requirements and accepted evidence types
Resolve evidence providers, datasets, and data services
Prefill, verify, or continue the service workflow
```

Registry discovery remains required. The change is that registry and evidence
provider discovery MUST be explainable from the public-service context that
needs the data.

## Standards Model

The standards-aligned division of responsibility is:

| Layer | Standard or profile | Responsibility |
|---|---|---|
| Public service discovery | CPSV-AP | Discover public services, competent authorities, channels, inputs, outputs, requirements, and procedures. |
| Requirement and evidence semantics | CCCEV | Model requirements, information concepts, evidence types, evidence type lists, and evidence instances. |
| Registry and source discovery | BRegDCAT-AP / DCAT | Discover base registries, datasets, distributions, and data services. |
| Cross-border evidence provider discovery | OOTS Common Services, including Evidence Broker and Data Service Directory | Discover evidence type classifications, providers, access services, and once-only exchange details. |
| Form definition | Local application profile | Model fields, sections, widgets, validation, conditional logic, and UI binding. |
| Payload validation | JSON Schema / SHACL / XML Schema | Validate payload shape and semantic constraints. |
| API access | OpenAPI / DCAT DataService | Describe submission, lookup, verification, and retrieval APIs. |

There is no mature SEMIC Form-AP equivalent in scope. Forms are a local profile
that MUST link back to CPSV-AP and CCCEV concepts.

## Standards Corrections

Implementations MUST apply these corrections:

- `cv:hasInputType` MUST NOT be emitted as a CPSV-AP 3.2 predicate.
- Public services MAY use `cpsv:hasInput` for evidence instances.
- Evidence instances linked with `cpsv:hasInput` SHOULD be typed with
  `dcterms:type` pointing to a CCCEV evidence type.
- Accepted evidence types MUST be reachable through `cv:holdsRequirement` and
  CCCEV requirement nodes with evidence type lists.
- If a direct service-to-evidence-type shortcut is needed, it MUST use a local
  extension IRI such as `registry_manifest:acceptedEvidenceType`.
- `cv:Requirement` and `cccev:Requirement` resolve to the same IRI in the Core
  Vocabularies namespace. Parsers MUST canonicalize and deduplicate them.
- Implementations MUST use `cccev:hasConcept`, not
  `cccev:hasInformationConcept`.
- A public registry service that produces a dataset MUST preserve CPSV output
  semantics. The produced catalogued resource SHOULD be dual-typed as
  `cpsv:Output` and `dcat:Dataset` when the dataset itself is the published
  output.
- Prefix maps MUST include `cv`, `cccev`, `cpsv`, `dcat`, `dcterms`,
  `dcatap`, `adms`, `skos`, `rdfs`, and `registry_manifest`.
- The `registry_manifest` prefix MUST expand to
  `https://registry-manifest.dev/ns/v1#` until a later versioned namespace is
  published.

## Core Principle

The public service is the canonical discoverable thing for user-facing
procedures.

The form is not the public service. The form is an instrument attached to a
channel for a public service.

The registry is not the public service. The registry is an authoritative source
or evidence provider that may support one or more public services.

The discovery graph MUST support traversal in both directions:

```text
PublicService -> Requirement -> EvidenceType -> DataService
EvidenceType -> PublicService
InformationConcept -> FormField -> DataService
Dataset -> PublicRegistryService -> PublicService context
```

## Shared Fixture Contract

The first implementation step MUST define a shared fixture contract.

Registry Manifest MUST own the source YAML fixture:

```text
apps/registry-manifest/fixtures/cpsv-ap/health-linked-child-support.metadata.yaml
```

Registry Manifest MUST render the expected JSON-LD fixture:

```text
apps/registry-manifest/fixtures/cpsv-ap/health-linked-child-support.cpsv-ap.jsonld
```

Registry Atlas MUST consume that JSON-LD fixture, either by vendoring a copy or
by referencing a stable fixture package during tests:

```text
apps/registry-atlas/fixtures/service-first/health-linked-child-support.cpsv-ap.jsonld
```

Registry Lab MUST use the same logical service, requirement, evidence type, and
provider identifiers in its demo metadata.

The fixture contract MUST specify the mapping:

```text
Manifest YAML field -> rendered JSON-LD node/predicate -> Atlas asset/relation
```

Minimum contract rows:

| Manifest field | JSON-LD output | Atlas report v2 output |
|---|---|---|
| `public_services[].id` | `cpsv:PublicService @id` | `SemanticAssetKind::PublicService` |
| `public_services[].channels[]` | `cv:Channel` and `cv:hasChannel` | `Channel` asset plus `cv:hasChannel` relation |
| `public_services[].holds_requirements[]` | `cv:holdsRequirement` | relation from service to requirement |
| `authorities[]` | `cv:PublicOrganisation` | authority asset plus authority relations |
| `requirements[]` | `cv:Requirement` or `cccev:Requirement` | canonical `Requirement` asset |
| `evidence_types[]` | `cccev:EvidenceType` | `EvidenceType` asset |
| requirement evidence options | `cccev:hasEvidenceTypeList` and `cccev:specifiesEvidenceType` | evidence type list asset plus relations |
| `datasets[].public_services[]` | `cpsv:PublicService` and `cpsv:produces` | registry service asset plus produce relation |
| `evidence_offerings[]` | local offering node plus DCAT service hints | provider or service route relation |
| `data_services[]` | `dcat:DataService` | data service asset plus access relations |
| `forms[]` | local form profile nodes | form assets and relations |

No implementation slice is complete until its tests prove this contract.

## Target Demo Story

Registry Lab MUST support this narrated discovery story:

```text
1. A client discovers the service catalogue.
2. The client selects "Health-linked child support eligibility review".
3. Atlas shows the service authority, jurisdiction, and online channel.
4. Atlas shows the service requirements.
5. Atlas shows accepted evidence types for each requirement.
6. Atlas resolves evidence providers and registry data services.
7. The demo calls the relevant Notary endpoints.
8. The demo writes an explainability artifact with gaps and source evidence.
```

This story is stronger than discovering endpoints first because it explains the
administrative purpose before exposing the technical route.

## Atlas Report V2

`semantic-asset-discovery-core` MUST introduce a new report schema version:

```text
semantic-asset-discovery.report.v2
```

This is allowed to be a breaking change. The current libraries are not stable
external APIs. v1 fixtures MAY remain for regression comparison, but v2 is the
only schema required for service-first discovery.

The report MUST distinguish:

- fetched artifacts;
- discovered links and fetch candidates;
- semantic assets;
- semantic relations;
- relation claims;
- standards and profile claims;
- findings and parser warnings.

`DiscoveredLink` remains available for fetch/navigation links. It MUST NOT be
the canonical representation of semantic graph edges.

### Semantic Asset Kinds

`SemanticAssetKind` MUST add first-class variants for:

```text
PublicService
Channel
Requirement
InformationRequirement
InformationConcept
EvidenceType
EvidenceTypeList
FormDefinition
FormSection
FormField
PublicRegistryService
EvidenceOffering
EvidenceProvider
```

The existing variants MUST remain available unless a separate migration removes
them:

```text
SemanticModelPackage
Catalog
Dataset
DataService
Distribution
Profile
Vocabulary
VocabularyTerm
Class
Property
ShapeGraph
ConceptScheme
Alignment
Crosswalk
ApiDescription
RecordCollection
FeatureCollection
Policy
QualityMeasurement
LifecycleStatus
PrivacyBasis
TrustArtifact
Unknown
```

### Relation Endpoints

Semantic relations MUST use a closed endpoint model rather than independent
optional strings.

```rust
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

pub struct SemanticRelation {
    pub id: String,
    pub subject: RelationEndpoint,
    pub predicate: String,
    pub object: RelationEndpoint,
    pub label: Option<String>,
}
```

A relation MUST have exactly one subject endpoint and one object endpoint.
Relations MUST NOT require either side to be fetchable.

For `RelationEndpoint::Asset`, `asset_id` is the canonical in-report target.
`uri`, when present, is the expanded IRI observed at the relation use site. It
preserves round-trip and canonicalization evidence. Consumers SHOULD read the
asset for canonical labels and metadata.

### Relation Claims

Statements need provenance and sometimes qualifiers. Report v2 MUST include a
claim layer over relations:

```rust
pub struct RelationClaim {
    pub id: String,
    pub relation_id: String,
    pub asserted_by_artifact_id: String,
    pub evidence: DiscoveryEvidence,
    pub qualifiers: Vec<RelationQualifier>,
    pub contradicts: Vec<String>,
}

pub struct RelationQualifier {
    pub predicate: String,
    pub value: String,
    pub evidence: Option<DiscoveryEvidence>,
}
```

`SemanticRelation` is the normalized edge. `RelationClaim` records who asserted
it, where it was asserted, and what qualifications or contradictions were
observed.

Every `SemanticRelation` in a report MUST be referenced by at least one
`RelationClaim`. A relation without a claim is unprovenanced and MUST NOT be
emitted.

### Required Relation Predicates

The parser MUST preserve at least these predicates as semantic relations when
present:

```text
cv:hasChannel
cv:hasCompetentAuthority
cv:holdsRequirement
cpsv:hasInput
cpsv:produces
dcterms:type
cccev:hasRequirement
cccev:hasConcept
cccev:hasEvidenceTypeList
cccev:specifiesEvidenceType
cccev:isDerivedFrom
dcat:dataset
dcat:distribution
dcat:service
dcat:accessService
dcat:servesDataset
dcat:endpointURL
dcat:endpointDescription
dcat:landingPage
dcat:accessURL
dcat:downloadURL
dcterms:conformsTo
dcatap:applicableLegislation
```

The parser MUST accept compact IRIs and expanded IRIs. It MUST canonicalize
aliases that resolve to the same expanded IRI.

The parser MUST also preserve local extension predicates such as
`registry_manifest:acceptedEvidenceType` when present. Local extension
predicates are optional hints, not the canonical standards path for accepted
evidence.

Endpoint fields such as `dcat:endpointURL` and `dcat:landingPage` are semantic
relations in the canonical graph. Asset-level `endpoint_url` or similar fields
MAY remain as denormalized projections derived from relations. They MUST NOT be
treated as a second source of truth.

### Parser Requirements

The JSON-LD parser MUST recognize:

- `cpsv:PublicService`;
- `cv:Channel`;
- `cv:Requirement`;
- `cccev:Requirement`;
- `cccev:InformationRequirement`;
- `cccev:InformationConcept`;
- `cccev:EvidenceType`;
- `cccev:EvidenceTypeList`;
- `dcat:Catalog`;
- `dcat:Dataset`;
- `dcat:Distribution`;
- `dcat:DataService`.

The parser MUST preserve labels and descriptions from common keys:

```text
dcterms:title
dcterms:description
skos:prefLabel
rdfs:label
name
description
```

### Typed Projections

The canonical report is generic assets plus typed relations. Consumers MUST use
typed projections over the report for convenience:

```rust
pub struct PublicServiceView<'a> {
    pub asset: &'a SemanticAsset,
    pub channels: Vec<ChannelView<'a>>,
    pub requirements: Vec<RequirementView<'a>>,
    pub accepted_evidence_types: Vec<EvidenceTypeView<'a>>,
}
```

Typed projections MUST be derived from report evidence. They MUST NOT invent
empty vectors that imply completeness. Missing relations are gaps, not proof of
absence.

### Rust Navigation API

Atlas MUST expose a Rust navigation API over `DiscoveryReport` before the
Registry Lab service-first story is implemented. The API MUST hide raw graph
walking for common discovery questions while preserving evidence references.

The API shape MAY evolve, but it MUST support the following capabilities:

```rust
let graph = ServiceGraph::from_report(&report)?;

let service = graph
    .public_service("https://demo.example.gov/services/health-linked-child-support")?;

let requirements = service.requirements();
let evidence_types = service.accepted_evidence_types();
let providers = service.evidence_providers();
let forms = service.forms();

for route in graph.routes_for_service(service.id()) {
    assert!(route.evidence().iter().all(|evidence| evidence.location().is_some()));
}
```

The navigation API MUST provide:

- lookup by public service IRI;
- lookup by evidence type IRI;
- service-to-requirements traversal;
- requirement-to-evidence-type traversal;
- service-to-evidence-provider traversal;
- service-to-form traversal;
- access to relation claims and source evidence for every returned edge;
- explicit gaps when expected relations are missing.

The API MUST NOT silently synthesize a route from labels alone. It MUST use
semantic relations and relation claims from the discovery report.

## Service Route Discovery

Service route discovery is application reasoning over discovery reports. It
MUST remain separate from `semantic-asset-discovery-core`.

The current `system-capability-discovery` crate may be used as the starting
point, but before service-first discovery is treated as release-ready it MUST be
documented as a peer application-reasoning crate, not as canonical metadata
discovery. It MAY remain physically co-located in the Atlas repository while the
API is still moving, but its crate README and public API docs MUST state that it
derives hypotheses and route views from reports.

Route status MUST be type-level:

```rust
pub enum RouteStatus {
    Hypothesis,
    Declared {
        source_artifact_id: String,
    },
    Reviewed {
        reviewer: String,
        decision: ReviewDecision,
        reviewed_at: String,
    },
}
```

There MUST NOT be a public helper that promotes `Hypothesis` to `Declared` or
`Reviewed` without evidence or review data.

Route discovery MUST keep strict matching as the default. The default matcher
MUST NOT use hidden synonym expansion, embeddings, language models, or
approximate matching. A future fuzzy or AI-assisted matcher MAY exist only as an
explicit opt-in layer whose suggestions are not accepted without strict evidence
or review.

Route discovery MUST answer:

- Which public services require this evidence type?
- Which public service requirements can this evidence provider satisfy?
- Which datasets or data services support this public service?
- Which form fields map to an information concept, when form metadata exists?

Candidate source status MUST remain conservative. A match to a service,
requirement, field, or dataset is not proof that a provider is authoritative.
Authority, legal basis, access rights, freshness, and source-of-truth status
remain separate evidence or review claims.

## Registry Manifest Contract

Registry Manifest owns the portable metadata manifest and standards renderers.
It MUST separate:

1. Dataset-scoped public registry services that produce registry datasets.
2. Top-level public services or procedures that consume requirements and
   evidence.

### Dataset-Scoped Public Registry Services

Existing dataset-scoped `public_services` MUST remain available for
BRegDCAT-style publication:

```yaml
datasets:
  - id: civil_registry
    public_services:
      - id: https://demo.example.gov/services/civil-registry-service
        title: Civil registry service
        description: Service responsible for maintaining civil registration data.
```

These records MUST render as public registry services with `cpsv:produces`
pointing to a produced resource. When the produced resource is the dataset, the
dataset node SHOULD be dual-typed as `cpsv:Output` and `dcat:Dataset`.

These records MUST NOT be overloaded to represent citizen-facing application
procedures.

### Top-Level Public Services

The manifest MUST define the referenced public-service inputs in top-level
blocks before procedure services can compile:

```yaml
authorities:
  - id: civil_registration_authority
    iri: did:web:civil-registry.demo.example.gov
    name: Civil Registration Authority
    country: ZZ
  - id: social_protection_authority
    iri: did:web:social-protection.demo.example.gov
    name: Social Protection Authority
    country: ZZ
  - id: health_services_authority
    iri: did:web:health-registry.demo.example.gov
    name: Health Services Authority
    country: ZZ

requirements:
  - id: child_identity_requirement
    iri: https://demo.example.gov/requirements/child-identity
    title: Child identity and alive-status requirement
    evidence_type_options:
      - evidence_types:
          - civil_child_status_evidence
  - id: household_support_requirement
    iri: https://demo.example.gov/requirements/household-support
    title: Household support eligibility requirement
    evidence_type_options:
      - evidence_types:
          - household_support_evidence
  - id: health_linked_support_requirement
    iri: https://demo.example.gov/requirements/health-linked-support
    title: Health-linked support requirement
    evidence_type_options:
      - evidence_types:
          - health_service_availability_evidence

evidence_types:
  - id: civil_child_status_evidence
    iri: https://demo.example.gov/evidence-types/civil-child-status
    title: Civil child status evidence
    proves:
      - child_identity_requirement
  - id: household_support_evidence
    iri: https://demo.example.gov/evidence-types/household-support
    title: Household support evidence
    proves:
      - household_support_requirement
  - id: health_service_availability_evidence
    iri: https://demo.example.gov/evidence-types/health-service-availability
    title: Health service availability evidence
    proves:
      - health_linked_support_requirement

evidence_offerings:
  - id: civil_child_status_evidence_service
    evidence_type: civil_child_status_evidence
    provider: civil_registration_authority
    access_service: https://demo.example.gov/evidence-services/civil-child-status
  - id: household_support_evidence_service
    evidence_type: household_support_evidence
    provider: social_protection_authority
    access_service: https://demo.example.gov/evidence-services/household-support
  - id: health_service_availability_evidence_service
    evidence_type: health_service_availability_evidence
    provider: health_services_authority
    access_service: https://demo.example.gov/evidence-services/health-service-availability

data_services:
  - id: health_facility_lookup_api
    iri: https://demo.example.gov/data-services/health-facility-lookup
    title: Health facility lookup API
    endpoint_url: https://demo.example.gov/api/health/facilities
```

`MetadataManifest` MUST add top-level `public_services`:

```yaml
public_services:
  - id: https://demo.example.gov/services/health-linked-child-support
    title: Health-linked child support eligibility review
    description: Public service for reviewing child support eligibility using civil, social protection, and health evidence.
    competent_authority: social_protection_authority
    jurisdiction:
      country: ZZ
    channels:
      - id: https://demo.example.gov/services/health-linked-child-support/channels/online
        type: online
        title: Online review channel
        landing_page: https://demo.example.gov/services/health-linked-child-support
        form_definition: https://demo.example.gov/forms/health-linked-child-support/v1
        submission_endpoint: https://demo.example.gov/api/child-support/reviews
    holds_requirements:
      - child_identity_requirement
      - household_support_requirement
      - health_linked_support_requirement
    accepted_evidence_types:
      - civil_child_status_evidence
      - household_support_evidence
      - health_service_availability_evidence
    uses_evidence_offerings:
      - civil_child_status_evidence_service
      - household_support_evidence_service
      - health_service_availability_evidence_service
    uses_data_services:
      - health_facility_lookup_api
```

`accepted_evidence_types` is a manifest convenience field. The canonical
standards render MUST express accepted evidence through CCCEV
requirement/evidence type list relations. A renderer MAY also emit
`registry_manifest:acceptedEvidenceType` as a denormalized local hint, but that
hint MUST be derived from the CCCEV path and MUST NOT be the only expression of
accepted evidence. Renderers MUST NOT emit `cv:hasInputType`.

Use `uses_evidence_offerings` when the manifest models the evidence bundle
locally, including evidence type, provider, policy, and access. Use
`uses_data_services` when only the wire-level DCAT API or submission/retrieval
service is known.

The model MUST preserve distinct fields for:

- service identity;
- competent authority;
- jurisdiction;
- channels;
- requirements;
- accepted evidence types;
- evidence offerings;
- data services;
- optional form definition links;
- optional submission endpoint links.

### Cardinality And Identifiers

Top-level procedure services MUST have:

- one absolute IRI `id`;
- one non-empty `title`;
- one `competent_authority` reference to a top-level `authorities[]` record;
- at least one `channel`;
- at least one `holds_requirements` entry.

Dataset-scoped registry services MUST have:

- one absolute IRI or manifest-resolvable `id`;
- one non-empty `title`.

Channel ids MUST be absolute IRIs in the first implementation. Local aliases
MAY be added later if compilation resolves them to absolute IRIs before render.

Public service query inputs MUST use the canonical service IRI.

Top-level `authorities[].id` is a local manifest identifier used for references
and `dcterms:identifier`. `authorities[].iri`, when present, is the rendered
JSON-LD `@id`. If `iri` is absent, compilation MUST mint a deterministic IRI
from the catalogue base URL and authority id before rendering.

### Renderer

Registry Manifest MUST add a `cpsv-ap` render format.

The lab publication path for that format is:

```text
/metadata/cpsv-ap
```

The metadata index MUST link to it with `dcterms:hasPart`, the media type, and a
profile or conformance hint:

```text
predicate: dcterms:hasPart
href: /metadata/cpsv-ap
type: application/ld+json
conforms_to: CPSV-AP
```

The renderer MUST emit:

- `cpsv:PublicService`;
- `cv:Channel`;
- `cv:holdsRequirement`;
- CCCEV requirement, evidence type list, evidence type, and concept nodes;
- `cpsv:hasInput` only for evidence instances, not evidence type shortcuts;
- `dcat:DataService` nodes for submission and evidence endpoints when modeled;
- local extension predicates only with the `registry_manifest:` prefix.

The BRegDCAT renderer MUST continue to publish dataset and registry metadata.
It MAY include dataset-scoped registry service nodes. It MUST NOT collapse
procedure services into registry datasets.

### Validation

Validation MUST reject:

- dangling service requirement references;
- dangling service evidence type references;
- dangling service evidence offering references;
- dangling service data service references;
- dangling competent authority references;
- missing procedure service channel lists;
- duplicate channel ids within a service;
- non-IRI procedure service ids;
- non-IRI channel ids;
- invalid form links or submission endpoint links;
- unknown public service fields before the schema is updated;
- attempts to merge dataset-scoped registry services with top-level procedure
  services.

## Registry Lab Contract

Registry Lab MUST become the first end-to-end demonstration of service-first
discovery.

### Demo Public Service

The lab MUST add this citizen-facing public service:

```text
Health-linked child support eligibility review
```

The service MUST connect the existing lab requirements and evidence types:

```text
child_identity_requirement
household_support_requirement
health_linked_support_requirement

civil_child_status_evidence
household_support_evidence
health_service_availability_evidence
```

### Demo Registry Services

The lab SHOULD add dataset-scoped public registry services:

```text
Civil registry service -> civil_registry dataset
Social protection registry service -> social_protection_registry dataset
Health facility registry service -> health_registry dataset
```

These are not substitutes for the citizen-facing service. They explain registry
production and authority.

### Static Metadata Publisher

The static metadata publisher MUST publish:

```text
/metadata
/metadata/dcat/bregdcat-ap
/metadata/cpsv-ap
```

`/metadata` MUST link to `/metadata/cpsv-ap` so Atlas can discover the service
catalogue from the normal entry point.

### Live Story Runner

`scripts/demo-live-stories.py` MUST add a greenfield service-first story:

```text
Discover service catalogue
Select health-linked child support eligibility review
Show requirements
Show accepted evidence types
Resolve evidence providers
Call the relevant Notary endpoints
Write explainability artifacts
```

Generated artifacts MUST include:

- service discovery response;
- service graph excerpt;
- requirement-to-evidence map;
- evidence-provider map;
- route status and gaps;
- source evidence references.

## Registry Relay Contract

Registry Relay MUST remain focused on runtime registry and dataset publication.
It MUST NOT become an application workflow engine.

Relay MUST:

- continue serving dataset, DCAT, BRegDCAT, SHACL, JSON Schema, policy, and
  evidence offering metadata;
- preserve dataset-scoped public registry services;
- expose richer manifest fields only when a configured manifest includes them;
- avoid inventing application procedures from runtime table config.

If Relay serves top-level public services from a manifest, it MUST treat them as
standards-facing metadata, not runtime authorization or eligibility logic.

## Registry Notary Contract

Registry Notary does not own service discovery.

Notary discovery MUST remain evidence-provider discovery:

```text
Evidence type
Claim or evaluation endpoint
Supported formats
Policies and access hints
```

Atlas and Lab MAY link Notary service documents into the service-first graph
through evidence offerings. Notary MUST NOT decide which public service
requires its evidence.

## Local Form Profile

The form layer remains a local profile until a mature external standard exists.

The initial local model MUST support:

- form definitions;
- sections;
- fields;
- repeating sections or field groups;
- field cardinality;
- conditional visibility;
- validation references;
- fulfillment modes at field or information requirement level.

Example:

```yaml
forms:
  - id: https://demo.example.gov/forms/health-linked-child-support/v1
    title: Health-linked child support application form
    for_public_service: https://demo.example.gov/services/health-linked-child-support
    validates_with:
      json_schema: https://demo.example.gov/forms/health-linked-child-support/v1/schema
      shacl: https://demo.example.gov/forms/health-linked-child-support/v1/shacl
    sections:
      - id: applicant
        title: Applicant
        fields:
          - id: applicant_national_id
            name: nationalId
            label: National ID
            widget_type: text
            data_type: xsd:string
            required: true
            min_occurs: 1
            max_occurs: 1
            maps_to_information_concept: https://demo.example.gov/concepts/national-id
            supports_requirement: child_identity_requirement
            fulfillment:
              modes: [manual_input, registry_lookup, oots_evidence_exchange]
              preferred_mode: registry_lookup
      - id: children
        title: Children
        repeatable: true
        min_occurs: 1
        fields:
          - id: child_national_id
            name: childNationalId
            label: Child national ID
            widget_type: text
            data_type: xsd:string
            required: true
            supports_requirement: child_identity_requirement
      - id: health_linked
        title: Health service context
        visible_when:
          field: supportType
          equals: health_linked
```

Conditional visibility MUST start with one-level equality predicates. The first
implementation MUST NOT require JSONLogic or another expression language.

Form fields MUST be linked to information concepts or requirements where
possible. They MUST NOT be treated as evidence types or registry sources.

Fulfillment modes MUST attach to either:

- a form field; or
- an information requirement.

Allowed fulfillment mode values for the first implementation:

```text
manual_input
file_upload
registry_lookup
oots_evidence_exchange
self_declaration
known_from_context
```

## Discovery Queries

The coordinated system MUST support these query patterns.

### Find Services For Evidence Type

Input:

```text
EvidenceType IRI = https://demo.example.gov/evidence-types/civil-child-status
```

Return:

```text
Public services that require or accept that evidence type
Relevant requirements
Channels and form definitions
Evidence providers and unresolved gaps
```

### Find Evidence Providers For Service

Input:

```text
PublicService IRI = https://demo.example.gov/services/health-linked-child-support
```

Return:

```text
Requirements
Evidence types
Evidence offerings
Notary endpoints
Registry datasets and data services
```

### Find Data Sources For Information Concept

Input:

```text
InformationConcept IRI = https://demo.example.gov/concepts/registered-address
```

Return:

```text
Requirements using that concept
Fields mapped to that concept
Evidence types that can satisfy it
Datasets and data services that may provide it
Gaps requiring review
```

### Find Forms For Service

Input:

```text
PublicService IRI = https://demo.example.gov/services/health-linked-child-support
```

Return:

```text
Online channels
Landing pages
Form definitions
Submission endpoints
Validation profiles
```

## Implementation Sequence

Implementation MUST proceed in this order:

1. Define the shared YAML and JSON-LD fixture contract.
2. Add Atlas fixture tests that fail until report v2 assets and relations are
   implemented.
3. Add Registry Manifest fixture tests that fail until the `cpsv-ap` renderer
   emits the contract JSON-LD.
4. Implement Atlas report v2 generic assets, relation endpoints, relation
   claims, prefix canonicalization, and typed projections.
5. Implement the Atlas Rust navigation API over report v2.
6. Implement Registry Manifest top-level `public_services`, validation, and
   `cpsv-ap` renderer.
7. Update route discovery as a peer application-reasoning layer over report v2.
8. Add Registry Lab demo services, registry services, and `/metadata/cpsv-ap`
   publication.
9. Add the service-first live story and explainability artifacts using the
   Atlas Rust navigation API or a CLI built on it.
10. Run fixture, Rust, TypeScript, and lab smoke checks.
11. Later, update Atlas UI to expose service-first views.

Atlas and Registry Manifest may implement in parallel after step 1 because both
will be pinned to the same fixture contract.

The Atlas UI step is intentionally last. It MUST NOT block the semantic model,
Rust API, renderer, or Registry Lab script story.

## Acceptance Criteria

### Shared Contract

- The shared YAML fixture renders to the shared JSON-LD fixture.
- Atlas parses the shared JSON-LD fixture into the expected assets, relations,
  and relation claims.
- The fixture includes at least one procedure service, one registry service,
  three requirements, three evidence types, one channel, one form definition,
  and at least two evidence providers.

### Atlas Core

- A fixture containing CPSV, CCCEV, DCAT, and BRegDCAT JSON-LD produces
  first-class service, channel, requirement, evidence type, dataset, and data
  service assets.
- `cv:Requirement` and `cccev:Requirement` dedupe to one canonical requirement
  asset when they resolve to the same IRI.
- The same fixture produces typed semantic relations with relation claims.
- Blank-node relation endpoints are represented without losing artifact
  provenance.
- `DiscoveredLink` remains available for fetch/navigation links.
- Asset endpoint hints are projections over relation evidence. If a
  `dcat:endpointURL` relation is removed from a report, the corresponding
  asset-level endpoint hint MUST disappear, unless another relation claim still
  supports it. Endpoint hints SHOULD cite the underlying `relation_id`.

### Rust Navigation API

- `ServiceGraph::from_report` or an equivalent API builds from report v2 without
  fetching or reading Registry Manifest YAML.
- A caller can look up the health-linked child support service by IRI.
- The API returns that service's requirements, accepted evidence types,
  evidence providers, and forms from semantic relations.
- Every returned traversal edge exposes relation claim evidence.
- Missing expected edges are returned as gaps, not empty success values.

### Route Discovery

- Strict search can find public services from an evidence type IRI.
- Strict search can find evidence providers from a public service IRI.
- Every route has a typed `RouteStatus`.
- No test can promote a hypothesis to declared or reviewed without source
  evidence or review data.

### Registry Manifest

- Manifest validation rejects dangling service requirement references.
- Manifest validation rejects dangling service evidence type references.
- Manifest validation rejects dangling service evidence offering references.
- Manifest validation rejects procedure services without channels.
- Renderer output includes CPSV public service and channel nodes.
- Renderer output links service requirements and evidence types through CCCEV.
- Renderer output does not emit `cv:hasInputType`.
- Existing BRegDCAT renderer tests continue to pass.

### Registry Lab

- Static metadata includes at least one citizen-facing public service.
- Static metadata includes dataset-scoped public registry services where useful.
- `/metadata` links to `/metadata/cpsv-ap`.
- The service catalogue is reachable from the metadata entry point.
- The live story runner demonstrates service-to-evidence-provider discovery by
  calling the Atlas Rust navigation API or a thin CLI built on that API.
- The live story runner does not hand-code JSON-LD graph traversal for the core
  service-first route.
- Smoke artifacts include service graph explainability output.
- Smoke artifacts include one sample service-form payload validated against the
  referenced JSON Schema when a form declares `validates_with.json_schema`.

### Documentation

- Atlas docs describe service-first discovery and report v2.
- Registry Manifest docs distinguish registry services from procedure services.
- Registry Lab README or demo docs describe the new discovery story.
- Standards assumptions continue to distinguish discovered facts, Atlas
  hypotheses, declared metadata, and reviewed claims.

## Boundary Requirements

This work MUST NOT:

- generate application procedures from Registry Relay runtime table config;
- add public-service requirement ownership to Registry Notary;
- claim source-of-truth status from a field match alone;
- infer legal authority from a public service link alone;
- emit `cv:hasInputType` as if it were CPSV-AP;
- require OOTS for non-cross-border demo flows;
- query protected person-level data during metadata discovery;
- hide missing legal, authority, access, freshness, or identifier evidence.

Some boundary requirements are structurally testable, such as rejecting
`cv:hasInputType` output. Others are review-time governance boundaries, such as
not inferring legal authority or source-of-truth status from weak evidence.
Those governance boundaries MUST be covered by implementation review checklists
and route-status tests, not only by manifest validators.

## Migration

Report v2 is a breaking schema change. The first implementation MAY keep v1
fixtures and UI adapters for comparison, but service-first discovery MUST target
v2 only.

External consumers are not yet promised v1 compatibility. Once v2 lands, future
breaking changes SHOULD use an explicit deprecation note and fixture migration
plan.

## Design Position

The recommended ownership split is:

```text
Atlas core discovers and explains the graph.
Route discovery derives conservative service routes from reports.
Registry Manifest models and renders portable metadata.
Registry Lab demonstrates the story.
Registry Relay publishes registry and dataset metadata.
Registry Notary publishes evidence provider metadata.
```

This keeps each project honest about its role while giving the demo a complete
service-first discovery path.

## Delivery Plan

Implementation MUST proceed in reviewed waves. A wave is not done until its
definition of done is met, focused checks pass, and a code review has accepted
the diff.

### Parallel Work Model

The parent implementer remains responsible for orchestration, integration,
final verification, and resolving cross-repository contract conflicts.

Workers MAY run in parallel only when their ownership is disjoint:

- **Fixture worker** owns the shared YAML and JSON-LD contract fixtures.
- **Atlas core worker** owns report v2 assets, relations, relation claims,
  parser changes, and fixture parser tests.
- **Atlas navigation worker** owns the Rust navigation API and route projection
  tests over report v2.
- **Registry Manifest worker** owns manifest schema, validation, `cpsv-ap`
  rendering, and renderer golden tests.
- **Registry Lab worker** owns demo metadata, static publication paths, live
  story script changes, smoke artifacts, and lab docs.
- **Review worker** performs final diff review for standards correctness,
  evidence provenance, boundary violations, and test coverage.

Workers MUST NOT edit the same modules in parallel unless the parent explicitly
serializes their changes. Each worker MUST report files changed, tests run,
blockers, and residual risks.

### Wave 0: Contract Fixture

Scope:

- Add the shared service-first YAML fixture in Registry Manifest.
- Add the expected `cpsv-ap` JSON-LD fixture.
- Add an Atlas fixture copy or stable fixture reference.
- Document the YAML to JSON-LD to Atlas report mapping.

Definition of done:

- Fixture identifiers are stable IRIs or manifest-local ids with deterministic
  IRI expansion.
- The fixture includes one procedure service, one registry service, three
  requirements, three evidence types, one channel, one form definition, and at
  least two evidence providers.
- The fixture contains no `cv:hasInputType`.
- Reviewer confirms the fixture follows CPSV-AP, CCCEV, DCAT, and BRegDCAT
  semantics described in this spec.

### Wave 1: Atlas Report V2

Scope:

- Add report v2 schema types for semantic assets, relation endpoints, semantic
  relations, relation claims, and prefix canonicalization.
- Parse CPSV, CCCEV, DCAT, BRegDCAT, and local form/profile nodes from the
  shared JSON-LD fixture.
- Preserve `DiscoveredLink` only for navigation and fetch evidence.

Definition of done:

- Atlas parser tests pass against the shared JSON-LD fixture.
- Every emitted `SemanticRelation` has at least one `RelationClaim`.
- `cv:Requirement` and `cccev:Requirement` dedupe to one canonical requirement
  asset.
- Blank-node endpoints preserve artifact provenance.
- Endpoint hints are projections over relation evidence and cite or derive from
  relation ids.
- No report v2 test inspects Registry Manifest YAML or private lab config.

### Wave 2: Atlas Rust Navigation API

Scope:

- Add `ServiceGraph` or equivalent Rust navigation API over report v2.
- Add typed projections for public services, channels, requirements, evidence
  types, evidence providers, forms, routes, and gaps.
- Keep strict matching as the default.

Definition of done:

- A Rust test can load the shared report fixture and look up the health-linked
  child support service by IRI.
- The API returns requirements, accepted evidence types, providers, forms, and
  gaps from semantic relations and relation claims.
- Every returned traversal edge exposes source evidence.
- Missing expected relations return explicit gaps, not silent empty success.
- No route is created from labels alone.

### Wave 3: Registry Manifest Renderer

Scope:

- Add top-level `authorities`, `public_services`, `data_services`, and form
  profile fields needed by the fixture.
- Add validation rules for references, channel cardinality, ids, and forbidden
  predicates.
- Add the `cpsv-ap` renderer and publication artifact support.

Definition of done:

- Registry Manifest tests render the shared YAML fixture to the expected
  `cpsv-ap` JSON-LD.
- Validation rejects dangling service requirement, evidence type, evidence
  offering, data service, and authority references.
- Validation rejects procedure services without channels.
- Renderer output includes CPSV service and channel nodes and CCCEV evidence
  type list relations.
- Renderer output does not emit `cv:hasInputType`.
- Existing BRegDCAT, DCAT, SHACL, JSON Schema, and CLI tests still pass or have
  documented unrelated failures.

### Wave 4: Route Discovery Layer

Scope:

- Update the peer route discovery layer to use report v2 and the navigation API.
- Add typed `RouteStatus`.
- Preserve conservative hypothesis, declared, and reviewed boundaries.

Definition of done:

- Tests find public services from an evidence type IRI.
- Tests find evidence providers from a public service IRI.
- Every route has a typed `RouteStatus`.
- There is no public helper that promotes a hypothesis to declared or reviewed
  without source evidence or review data.
- Reviewer confirms route results are derived from discovery metadata and not
  private configuration.

### Wave 5: Registry Lab Story

Scope:

- Add lab demo services, registry services, forms, and `/metadata/cpsv-ap`
  publication.
- Update the service-first live story script.
- Make the script call the Atlas Rust navigation API or a thin CLI built on it.

Definition of done:

- `/metadata` links to `/metadata/cpsv-ap` using `dcterms:hasPart`.
- `/metadata/cpsv-ap` is reachable in the lab.
- The live story discovers the service, requirements, evidence types, evidence
  providers, and Notary endpoints without hand-coded JSON-LD graph traversal.
- Smoke artifacts include service discovery response, service graph excerpt,
  requirement-to-evidence map, provider map, route status, gaps, and source
  evidence references.
- If a form declares `validates_with.json_schema`, smoke artifacts include one
  sample payload validated against that schema.

### Wave 6: Documentation And Deferred UI

Scope:

- Update docs in Atlas, Registry Manifest, and Registry Lab.
- Keep Atlas UI changes deferred until the semantic model, Rust API, renderer,
  and lab story are complete.

Definition of done:

- Docs explain service-first discovery, report v2, the navigation API, and the
  lab story.
- Registry Manifest docs distinguish procedure services from registry services.
- Registry Lab docs show how to run the service-first story and inspect
  artifacts.
- Atlas UI work is tracked separately and is not required for this spec to be
  implemented.

### Review Cadence

Each wave MUST have:

- one implementation review focused on correctness and scope control;
- one standards review focused on CPSV-AP, CCCEV, DCAT, BRegDCAT, and local
  extension semantics;
- one verification review confirming tests, smoke checks, and generated
  artifacts match the wave definition of done.

The parent implementer MUST not start integrating the next wave until the
current wave's blocking review findings are resolved or explicitly deferred as
out of scope with a written reason.

### Overall Definition Of Done

The service-first discovery work is done only when all of the following are
true:

- The shared fixture contract exists and is used by both Registry Manifest and
  Atlas tests.
- Registry Manifest validates and renders `cpsv-ap` JSON-LD for the fixture.
- Atlas report v2 parses the rendered JSON-LD into assets, relations, relation
  claims, and typed projections.
- The Rust navigation API answers service, requirement, evidence type,
  provider, form, and gap queries from discovery metadata.
- The route discovery layer preserves typed route status and does not promote
  hypotheses without evidence or review data.
- Registry Lab publishes `/metadata/cpsv-ap` and the live story uses the Atlas
  Rust navigation API or CLI.
- Lab smoke artifacts prove the service-first route end to end and include
  source evidence references.
- Focused tests and the closest practical broader checks pass in each touched
  repository.
- A final review confirms no `Partially Implemented` or `Still To Do` item
  remains except explicitly documented blockers.
- Atlas UI service-first views are either implemented in a later UI wave or
  tracked as deferred work outside this spec's completion criteria.
