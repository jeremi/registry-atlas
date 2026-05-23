# Registry Atlas Specification

## Purpose

Registry Atlas is a visual discovery and validation explorer for registry
dataspaces. It builds its view from published standards artifacts, not from
private application configuration.

The first demo should answer:

- What catalogues, datasets, registries, and services are published?
- Which standards describe them?
- What can be discovered and validated from those standards alone?
- What is missing before a registry is ready for decentralized registration?

The product is a standards microscope: it makes published interoperability
metadata visible, inspectable, and useful.

## Product Positioning

Registry Atlas is a standards-first catalogue and registry discovery tool.

It is not a registry admin console in v0. It does not approve participants,
mutate registries, browse protected rows, or replace governance. It helps data
stewards, interoperability reviewers, and digital public infrastructure teams
inspect what a participant has published and understand readiness gaps.

Atlas can become a central registry by storing and reviewing generic discovery
reports. The reusable Rust and WebAssembly discovery layer is specified
separately in
[`SEMANTIC_ASSET_DISCOVERY_SPEC.md`](SEMANTIC_ASSET_DISCOVERY_SPEC.md). That
library is publisher-neutral and must not be specific to Registry Relay.
Standards interpretation assumptions are tracked in
[`STANDARDS_ASSUMPTIONS.md`](STANDARDS_ASSUMPTIONS.md).

The UI should feel dense, calm, and operational. The memorable screenshot should
be a **Registration Readiness** summary backed by standards evidence.

## Core Principle

Registry Atlas must consume the same published artifacts that another dataspace
participant could consume.

For Registry Relay, private YAML config may generate those artifacts, but the
Atlas should prefer public discovery surfaces:

- `GET /metadata`
- `GET /metadata/dcat`
- `GET /metadata/policies`
- `GET /datasets`
- `GET /datasets/{dataset_id}`
- `GET /openapi.json`
- OGC API Records endpoints when available
- OGC API Features endpoints only for genuinely geospatial feature collections
- provenance, DID, ODRL, DQV, DPV, or policy artifacts when present

Local-only developer inputs, such as Registry Relay config files or audit logs,
may exist later, but must be visually labelled as non-standard and must not
count toward standards-only readiness.

## Standards Anchor

The v0 target profile is **DCAT-AP 3.0.0**.

Registry Atlas should include a fallback parser for **DCAT-AP 2.1.1** because
existing catalogues may still publish it. Registry Relay now publishes
standards-facing `/metadata/*` artifacts, including base DCAT and BRegDCAT-AP
JSON-LD. The Registry Relay demo remains useful ground truth, but Atlas should
continue to treat it as one publisher fixture rather than as a private
integration contract.

Use SEMIC-style source hints. Prefer `dcterms:` in UI labels and documentation,
not the longer Dublin Core namespace URL.

Supported or planned standards and vocabularies:

- **DCAT-AP 3.0.0** for catalogues, datasets, distributions, and data services.
  A fallback parser for DCAT-AP 2.1.1 catalogs is supported for legacy inputs.
- **BRegDCAT-AP** for base registry metadata and interconnection use cases.
- **OGC API Records** for catalogue and registry discovery (DCAT-aligned record
  collections). This is the primary OGC profile for the Atlas.
- **OGC API Features** for spatial data access on datasets that have geometry
  (kept secondary to Records).
- **OpenAPI** for HTTP operation discovery and security scheme hints.
- **SHACL** for profile validation, ideally server-side in v0.
- **DQV** for validation quality metadata and validation timestamps.
- **ADMS** for lifecycle/status metadata such as active, deprecated, or
  withdrawn.
- **ODRL** for usage policy and offers.
- **DPV** (Data Privacy Vocabulary) alongside ODRL for legal basis and
  personal-data classification semantics where available.
- **DID Web** for the minimal participant identity artifact. **Verifiable
  Credentials (W3C VCDM 2.0)** issued under a named trust framework (EBSI,
  EUDI ARF, or DSSC trust anchor) where a stronger registry-legitimacy signal
  is published.
- **vCard** via `dcat:contactPoint` for steward/contact information.

**SP DCI** integration patterns (OpenAPI-based) for social protection registry
sync services when advertised. Treated as a cross-agency convention, not a peer
standard to DCAT-AP/OGC.

## OGC Scope Decision

OGC API Records is the right OGC profile for catalogue and registry discovery.
OGC API Features is spatial and should only appear when a dataset publishes
feature collections, such as facilities or public works assets with geometry.

Registry Relay can expose OGC API Records and OGC API Features when the
corresponding feature gates are enabled. Atlas should still keep their meaning
separate:

- OGC API Records: catalogue and registry discovery evidence.
- OGC API Features: geospatial feature service evidence attached to spatial
  datasets.

## Technical Constraints

### Fetching Remote Catalogues

"Paste any catalogue URL" requires a same-origin fetch proxy. Browser CORS will
block many real catalogues, and relying on permissive CORS would make the Atlas
look standards-based while only working for friendly local demos.

v0 must include a small server-side fetch proxy.

Proxy requirements:

- Fetch DCAT-AP JSON-LD, OpenAPI, OGC landing documents, and linked service
  descriptors.
- Apply request timeouts, response size limits, content-type checks, and
  redirect limits.
- Redact credentials from logs.
- In production, block private-network targets by default to reduce SSRF risk.
- In local demo mode, explicitly allow `localhost` and `127.0.0.1`.
- Never persist bearer tokens.

### Auth-Gated Discovery

The top bar should include a session-only bearer token input. It is kept in
memory for the current browser session and never written to local storage,
IndexedDB, server storage, or logs.

When a fetch returns `401` or `403`, the artifact should show
`Presence: Auth required` and a microcopy prompt to add a session token.

### Validation

In-browser SHACL is not a safe v0 assumption. Production-grade browser SHACL
support is thin, and partial validation would be misleading.

v0 should expose validation state honestly:

- server-side SHACL validation when a real validator has run;
- remote SEMIC/ITB validation integration when configured;
- `Validation not yet run` when no validator has run.

The UI must never report `Valid` unless a real validation adapter completed.

### Scale

Real public catalogues can contain tens of thousands of datasets. The primary
workspace cannot be graph-first.

The center workspace should be tabbed:

- **Table**: default view for all catalogs.
- **Tree**: catalogue -> publisher/theme -> dataset -> service.
- **Graph**: opt-in scoped view for selected subsets.

Graph rendering should have an explicit budget:

- render up to about 1,500 nodes with Cytoscape or equivalent;
- warn before graphing large selections;
- above the budget, summarize by publisher, theme, profile, service type, or
  validation status.

## Initial UX

The first version is a single workbench with a top bar, left artifact rail,
center workspace, and right detail panel.

### Top Bar

Controls:

- Catalogue URL input.
- Session-only bearer token input, sent only to the host the user typed, never
  persisted.
- Profile multi-selector with versions: `DCAT-AP 3.0.0`, `DCAT-AP 2.1.1`,
  `BRegDCAT-AP`, `Registry Relay (extension)`. A catalog may declare
  conformance to more than one profile.
- Standards toggle: `Standards only` / `Include extensions`.
- Discover action.
- Validation badge: `Not yet run`, `Running`, `Valid`, `Warnings`, `Invalid`,
  with a freshness affordance next to it (e.g. "Validated 4s ago / Revalidate").
- Recent catalogs dropdown (session-only, no persistence by default).
- Language selector or i18n-ready locale shell.

First-visit state:

- Recent catalogues rail, if any exist in memory for the session.
- Curated demo shortcuts, including local Registry Relay.
- A short empty-state prompt explaining that the Atlas starts from published
  catalogue metadata.

### Left Panel: Published Artifacts

Show discovered artifacts with two separate axes.

Presence:

- Found: "This artifact was fetched and parsed."
- Missing: "No link or file was found from the current discovery chain."
- Invalid: "The artifact exists but failed parsing or profile validation."
- Auth required: "The endpoint requires credentials. Add a session token."

Origin:

- Standard: "Defined by the selected standards profile."
- Extension: "Useful non-standard metadata. Declared by a Registry Relay or
  other non-standard layer. Excluded from standards-only readiness."
- Unsupported: "Known pattern, but the Atlas does not yet parse this artifact."

Each combined state has a short "what to do next" microcopy line in the UI, so
the user is never left with a badge and no recourse.

Initial artifact checklist:

- DCAT-AP JSON-LD catalog.
- BRegDCAT-AP registry metadata.
- OGC API Records landing page (catalogue discovery).
- OGC API Records collections.
- OpenAPI service description.
- OGC API Features landing page (spatial data access, secondary).
- OGC API Features collections (when geometry is present).
- SHACL validation profile or embedded shapes.
- ODRL policy or offer.
- DQV validation metadata.
- ADMS lifecycle/status metadata.
- DPV legal basis or data protection metadata.
- DID, VCDM, or provenance/trust metadata.
- SP DCI sync services, shown as unsupported or extension unless parsed through
  OpenAPI.

### Center Workspace

The center panel is a tabbed workspace over the same underlying graph model.

- **Table** (default): sortable, filterable list of catalog entries. The first
  view a data steward sees.
- **Tree**: catalog -> dataset -> distribution/service hierarchy.
- **Graph**: node-link rendering, opt-in, scoped to the current selection plus
  one or two hops.

The Atlas commits to a node-count budget of approximately 1,500 nodes in the
Graph view before it switches to a grouped or clustered fallback (by publisher,
theme, or profile). Above this ceiling the inspection-focused posture stops
paying off and a summary view replaces the hairball.

Table view columns:

- Name
- Type
- Publisher/contact
- Profile
- Access rights
- Service count
- Validation
- Readiness
- Top missing item

Tree view groups the catalog by catalogue, publisher/theme, dataset, and
service.

Graph view renders a selected subset only. Suggested node types:

- Participant
- Catalog
- Dataset
- Base registry
- Data service
- Distribution
- API operation group
- Record collection (OGC API Records)
- Feature collection (OGC API Features)
- Policy
- Validation issue

Suggested edges:

- catalog contains dataset
- service serves dataset
- dataset has distribution
- service has endpoint URL
- service conforms to standard
- dataset conforms to profile
- policy applies to dataset or service
- validation issue affects artifact

The graph should show standards relationships, not inferred implementation
details. Internal table relationships, row-level joins, and private config
belong in extension views only.

### Right Panel: What We Know

Clicking any node opens a detail panel showing human-readable fields with their
source hints displayed inline. Every field carries its provenance next to its
value, so users do not have to flip tabs to confirm where a value came from.

Fields shown:

- Name and description
- Publisher or owner
- Access rights
- Sensitivity, if published
- Standards conformance
- Service endpoint
- Validation status
- Missing recommended metadata

Each field is annotated with its source, for example:

```text
Endpoint URL
https://example.gov/api
Source: dcat:DataService -> dcat:endpointURL
```

Use `dcterms:` source labels where appropriate:

```text
Title
Benefits Casework
Source: dcat:Dataset -> dcterms:title
```

An expandable `Raw RDF / JSON-LD` drawer at the bottom reveals the source
fragment, RDF class or profile class, raw identifiers and URIs, and links to
the relevant JSON-LD or service descriptor. It is collapsed by default so the
human view stays calm.

Extension fields should be visually separated:

- left rule-line;
- `ext:` prefix on field labels;
- grouped below standards fields;
- excluded from standards-only missing counts;
- hidden when the top-bar toggle is `Standards only`.

### Comparison View

The Atlas needs an explicit comparison surface for standards-only versus
extension-enriched discovery.

Comparison modes:

- Standards-only view.
- Include extensions view.
- Diff view showing fields added by extensions and whether they affect
  readiness.

The diff should make clear that extensions can improve operator usability but
do not satisfy DCAT-AP, BRegDCAT-AP, or SEMIC profile requirements unless a
standard maps to them.

## Core Demo Flow

The initial demo should be a five-minute story.

1. Start from a published catalog URL.
2. Discover linked artifacts through the fetch proxy.
3. Show artifact presence and origin.
4. Validate the catalog against the selected profile, or clearly show
   `Validation not yet run`.
5. Inspect datasets and services in the Table view.
6. Open a scoped Graph view for one participant or a handful of datasets.
7. Click a dataset or service to show standards-backed fields with source
   hints.
8. Open the Registration Readiness summary: a one-screen score across
   Discoverable, Validatable, Trust, and Policy, with the top three missing
   items as actionable cards. Each card links back to the standard that names
   the field, so the reviewer leaves knowing exactly what to fix next.

The payoff is the Registration Readiness summary:

- Discoverable
- Validatable
- Policy
- Trust
- Lifecycle

Each category should show a score or state and the top three missing items as
action cards. Each action card links back to the standards term, SHACL shape
where available, and source artifact that explains the requirement.

## Known Versus Missing View

Fields are grouped by capability (Identity, Access, Policy, Trust, Lifecycle)
and ranked within each group: blocking-for-registration first,
recommended-by-profile second, nice-to-have last. Every "Missing" row
deep-links to the SHACL shape and the DCAT-AP or BRegDCAT-AP specification
section that names the field, so the table does not just report the gap, it
teaches what would close it.

The known/missing report should be grouped, ranked, and linked.

Ranking:

- Blocking: prevents discovery, validation, trust assessment, or registration.
- Recommended: important for operational use but not always mandatory.
- Nice-to-have: improves review quality but does not block readiness.

Groups:

- Identity
- Access
- Policy
- Trust
- Lifecycle
- Services
- Validation

Example rows:

| Group | Need | Rank | Status | Source |
| --- | --- | --- | --- | --- |
| Identity | Dataset identity | Blocking | Known | `dcat:Dataset` URI |
| Identity | Title | Blocking | Known | `dcterms:title` |
| Identity | Publisher | Blocking | Known | `dcterms:publisher` |
| Access | Endpoint URL | Blocking | Known | `dcat:DataService`, `dcat:endpointURL` |
| Access | API operations | Recommended | Known if linked | OpenAPI paths |
| Access | Auth scheme | Recommended | Partial | OpenAPI security schemes |
| Policy | Usage policy | Recommended | Missing or partial | ODRL |
| Policy | Legal basis | Recommended | Missing or partial | DPV |
| Trust | Trust evidence | Blocking for registration | Missing or known | VCDM, DID, certificate metadata |
| Lifecycle | Lifecycle status | Recommended | Missing or known | `adms:status` |
| Validation | Validation timestamp | Recommended | Missing or known | DQV |

Every missing row should deep-link to the standard section or SHACL shape that
names the field when available.

## Registration Readiness

Registration Readiness is a standards-based summary, not a governance decision.

Categories:

- **Discoverable**: required catalogue and service metadata can be fetched and
  parsed.
- **Validatable**: selected profile validation can run and results are
  available.
- **Policy**: usage policy, access rights, legal basis, and data protection
  metadata are present enough for review.
- **Trust**: participant identity and trust evidence are present.
- **Lifecycle**: status, contact, and change-management metadata are present.

Each category should show:

- status: Ready, Partial, Missing, Not checked;
- evidence count;
- top missing items;
- standard terms that define the evidence.

The Atlas must not mark a participant as approved or registered. It only says
what published metadata supports.

## Registry Relay Demo Mapping

For the local Registry Relay demo, Registry Atlas should discover:

- Catalog and participant from `/metadata` and `/metadata/dcat`.
- Datasets from DCAT catalog entries.
- Data services from DCAT service metadata.
- Native entity APIs from `/openapi.json`.
- OGC API Records services from `/ogc/v1/records` and record collection links
  (primary).
- OGC API Features services from `/ogc/v1` and feature collection links
  (secondary, only for datasets with geometry).
- SP DCI sync services from OpenAPI or catalog metadata as extension or
  unsupported v0 rows.
- Claim verification routes from OpenAPI, if visible to the supplied token.
- Embedded SHACL shapes from the DCAT-AP JSON-LD artifact, where present.

Current ground truth:

- Registry Relay publishes `/metadata` as the canonical discovery entry point,
  with links to catalog, DCAT, BRegDCAT-AP, SHACL, JSON Schema, and policy
  artifacts visible to the caller.
- Registry Relay implements OpenAPI.
- Registry Relay implements feature-gated OGC API Records and OGC API Features.
- Registry Relay has feature-gated SP DCI sync routes.
- Registry Relay publishes dataset-scoped ODRL Offers. Default offers are thin
  governance evidence; configured offers with purpose, duties, prohibitions, or
  other constraints are stronger policy evidence. Neither grants access.
- Registry Relay supports DID Web and provenance only when configured in the
  appropriate mode. Atlas should often show this as Missing.
- Registry Relay can publish SHACL shapes and JSON Schema artifacts for visible
  entities.

Useful initial demo datasets:

- Benefits Casework
- Education Registry
- Clinic Capacity
- Public Works Projects
- Subject Registry
- Disability Registry, when the standards demo is enabled

The Atlas should avoid exposing synthetic row-level data by default. It should
show discovery metadata and registry capabilities first.

## SEMIC And EU Compatibility

Registry Atlas should work in a SEMIC-oriented environment by treating
DCAT-AP 3.0.0 and BRegDCAT-AP as first-class inputs.

Initial SEMIC-compatible behavior:

- Accept a DCAT-AP JSON-LD document as input.
- Preserve RDF identifiers and profile conformance URIs.
- Parse catalogues, datasets, distributions, and data services.
- Show BRegDCAT-AP registry concepts when present.
- Run or display SHACL validation results.
- Distinguish profile-conformant metadata from custom extensions.
- Use `dcterms:` source hints in UI copy.

The UI should never imply that a Registry Relay-specific extension is part of
DCAT-AP or BRegDCAT-AP. Extensions are useful, but visibly separate.

Governance and publication:

- SEMIC governance is referenced as "Interoperable Europe" (regulation
  EU 2024/903) and the Interoperable Europe Portal, which has replaced the
  former ISA² Joinup catalogues for solution discovery.
- Any Atlas-defined extension fields are published as a proper profile (SHACL
  shapes graph plus a human-readable profile spec) following the SEMIC Style
  Guide, not as ad-hoc JSON-LD context entries.

BRegDCAT-AP support surfaces these registry-specific concepts when present:

- `RegisteredOrganization` or `Agent` as publisher
- `Concept` and `ConceptScheme` for registered entity types
- `PublicService` linkage (CPSV-AP)
- Legal basis and license semantics tied to a registry authority

## Decentralized Registration Concept

Registry Atlas can later support a registration workflow, but v0 should only
preview readiness.

Future workflow:

1. Enter participant discovery URL.
2. Fetch published catalog and service descriptors.
3. Validate against selected profiles.
4. Show discovered datasets, services, policies, trust artifacts, and
   lifecycle metadata.
5. Produce a Registration Readiness report.
6. Export a registration package or pointer set for a governance process.

The first version should not directly mutate registries, approve participants,
or write to a shared dataspace directory.

## Profile Extension Policy

Most candidate fields are already covered by existing standards. The Atlas
reuses these rather than defining a parallel vocabulary:

- **Participant lifecycle status**: covered by `adms:status` (ADMS) with the
  ADMS status concept scheme.
- **Last validation timestamp and validator profile**: covered by DQV
  (`dqv:QualityMeasurement`) or PROV-O.
- **Trust evidence** (signature, DID, certificate, issuer, governance
  authority): covered by W3C VCDM 2.0 (`proof`, `issuer`).
- **Contact and steward role**: covered by `dcat:contactPoint` with vCard
  properties (`vcard:role`).
- **Policy summary**: covered by `odrl:Policy` with `dcterms:description`.
- **Data protection classification and legal basis**: covered by DPV
  (`dpv:hasLegalBasis`, `dpv:hasPersonalDataCategory`).

Only two fields lack existing standards coverage and justify an Atlas-defined
extension profile:

- **Onboarding endpoint**: the URL a participant uses to begin a registration
  handshake. Proposed property name: `atlas:onboardingEndpoint` (subject to
  alignment with DSSC onboarding flows if a property emerges there).
- **Change notification mechanism**: how a participant signals catalog refresh
  or registration-state change. Candidate underlying mechanisms: WebSub or
  ActivityPub.

Both extensions ship with a published SHACL shapes graph and a human-readable
profile document.

Any extension must be namespaced, source-linked, and excluded from standards-only
readiness unless mapped to a selected profile.

## MVP Scope

Version 0 should support:

- Paste or enter a catalog URL.
- Use a server-side fetch proxy.
- Add a session-only bearer token.
- Load a local Registry Relay demo URL.
- Select `DCAT-AP 3.0.0`, `DCAT-AP 2.1.1`, `BRegDCAT-AP`, or
  publisher-specific metadata visibility.
- Parse DCAT-AP JSON-LD enough to render catalog, dataset, service, and
  distribution records.
- Show Table, Tree, and scoped Graph views.
- Follow links to OpenAPI when advertised.
- Show OGC API Records when available.
- Show OGC API Features only as spatial feature services.
- Show artifact Presence and Origin separately.
- Show a known versus missing report.
- Show Registration Readiness.
- Label publisher-specific fields as extensions.
- Include an i18n wrapper from day one, even if only English strings exist.
  SEMIC audiences are multilingual; retrofitting i18n later is expensive.
- Run SHACL validation via the SEMIC ITB validator service by default, with a
  pySHACL self-hosted fallback behind one `ValidatorAdapter` interface. No
  in-browser SHACL engine in v0.
- Provide a keyboard-navigable fallback for the Graph view, so the inspection
  workflow is usable with assistive tech.

Version 0 should skip or mark unsupported:

- In-browser SHACL engine (deferred until the JS SHACL ecosystem matures).
- Direct editing or approval of registrations.
- Persistent user accounts.
- Server-side saved catalog history.
- IndexedDB or local persistent catalog storage.
- Multi-user workflows.
- Full RDF editing.
- Row-level data browsing.
- Live audit playback.
- SP DCI-specific parsing beyond OpenAPI-visible endpoints (surfaced as
  Unsupported until needed).
- ODRL, DID, and VCDM deep parsing (surfaced as Missing rows in the Known vs
  Missing table, not parsed in v0).
- Complete BRegDCAT-AP coverage beyond display and validation hooks.

Recommended state model:

- Stateless server.
- Session-scoped in-memory browser cache.
- No persisted credentials.
- Recent catalogues may live in memory only for the current session in v0.

## Design Direction

Visual rules:

- Use a light neutral canvas.
- Use color for node type, validation state, and readiness state only.
- Prefer tables, side panels, and compact lists over decorative cards.
- Keep graph lines thin and graph views scoped.
- Show standards source terms inline.
- Make missing metadata visible and actionable.
- Separate extensions with a consistent visual treatment.

Interaction states to define in implementation:

- Empty state.
- Loading and long-running validation state.
- Fetch error.
- Parse error.
- Auth required.
- Validation not yet run.
- Too many nodes for graph.
- Unsupported artifact.
- Extension hidden by standards-only mode.

Interaction state behaviour:

- Discovery and validation can take seconds for large catalogs. Each panel
  renders a skeleton during fetch, and discovered artifacts appear in the left
  panel as they resolve (streaming, not batch).
- Empty states show 3-4 curated demo catalog URLs (local Registry Relay plus
  public SEMIC and EU catalogs).
- "Auth required" resolves through an inline session-only token modal, scoped
  to the host the user typed. The token is never persisted.

Extension visual separation:

- Extension fields render with a left rule-line and an `ext:` prefix on their
  labels.
- Extension fields group at the bottom of any list they appear in.
- Extension fields never count toward "Missing" in the standards-only view.
- A top-bar toggle (`Standards only` / `Include extensions`) switches between
  the two views, so reviewers can see the catalog as a non-Relay participant
  would.

## Success Criteria

The first prototype succeeds if a reviewer can:

- Start from a published catalog URL despite CORS limitations.
- Supply a session-only bearer token for protected discovery.
- See discovered datasets and services without reading raw JSON-LD.
- Understand which standards produced each field.
- Inspect profile conformance or clearly see validation was not run.
- Compare standards-only discovery with publisher-specific metadata.
- See a Registration Readiness summary with top missing items.
- Explain what metadata is still missing for decentralized registration.

## Open Decisions

Resolved (recorded for traceability):

- BRegDCAT-AP is an optional profile, auto-enabled when the catalog declares
  `dcterms:conformsTo` to a BRegDCAT-AP version.
- SHACL validation runs against the SEMIC ITB validator by default, with a
  pySHACL self-hosted fallback. No in-browser SHACL engine in v0.
- The Atlas is stateless across sessions in v0. A session-scoped in-memory
  cache is the only persistence.
- Auth-gated discovery is supported in v0 via a single optional bearer token
  in the top bar, in-memory only, sent only to the host the user typed.
- The minimum trust artifact is a DID Web document. The credible artifact is a
  Verifiable Credential (VCDM 2.0) issued under a named trust framework
  (EBSI, EUDI ARF, or DSSC trust anchor).

Still open:

- Which two or three public EU/SEMIC catalogs are the v0 acceptance gate
  alongside Registry Relay? (Recommendation: pick in week 1 and run the
  discover-and-render flow against them before UI polish.)
- What is the exact publication shape of the two Atlas extension profiles
  (onboarding endpoint, change notification)? Hosted where, versioned how?
- How should the Atlas surface DCAT-AP 3.0 features that have no equivalent in
  2.1.1 fallback mode (e.g. `dcat:DatasetSeries`)?
- Where does the same-origin fetch proxy live: shipped with the Atlas SPA, or
  expected to be deployed by the operator?

## Implementation Plan

Implementation should proceed in waves with parallel workers assigned to
separate ownership areas. Each wave must end with review, working software, and
an unambiguous definition of done.

### Wave 0: Project Skeleton And Delivery Rules

Parallel ownership:

- App shell worker: frontend scaffold, routing, layout frame, i18n wrapper.
- Service worker: server scaffold, fetch proxy shape, health endpoint.
- Quality worker: lint, typecheck, test runner, CI commands, review checklist.

Definition of done:

- The app starts locally with one documented command.
- Lint, typecheck, and tests run in CI-equivalent local commands.
- No credentials are persisted or logged.
- A reviewer can open the empty Atlas workbench and see the URL input, profile
  selector, token input, artifact rail, workspace, and detail panel shell.

Review gate:

- Code review after skeleton merge.
- Security review of fetch proxy constraints before remote fetching is enabled.

### Wave 1: Standards Ingestion Core

Parallel ownership:

- RDF/DCAT worker: JSON-LD fetch, RDF parsing, DCAT-AP 3.0.0 and 2.1.1 mapping.
- Proxy/auth worker: same-origin proxy, session-only bearer forwarding, CORS
  handling, size/time/redirect limits.
- Fixture worker: local Registry Relay catalog fixtures, SEMIC-style sample
  fixtures, parser tests.

Definition of done:

- A DCAT-AP JSON-LD URL can be fetched through the proxy and parsed into typed
  catalog, dataset, distribution, and data-service records.
- Auth-required, missing, invalid, and found states are covered by tests.
- The app can load at least one local Registry Relay catalog artifact and one
  static DCAT-AP fixture.
- Unsupported or partially parsed ODRL, DID/VCDM, and SP DCI artifacts appear
  as presence rows without false claims of deep parsing.

Review gate:

- Standards review of term mappings and `dcterms:` source hints.
- Code review focused on parser correctness and proxy safety.

### Wave 2: Workbench UX

Parallel ownership:

- Artifact rail worker: Presence x Origin model, statuses, microcopy.
- Table/tree worker: default table view, tree grouping, empty/loading/error/auth
  states.
- Detail worker: integrated "What We Know" panel, inline source hints, raw
  RDF/JSON-LD drawer, extension separation.

Definition of done:

- The user can discover a catalog and inspect records without reading raw
  JSON-LD.
- Every displayed field either has a standards source hint or is clearly marked
  as `ext:`.
- Standards-only mode hides extension fields and excludes them from missing
  counts.
- Empty, loading, fetch error, parse error, auth required, unsupported artifact,
  and validation-not-run states are implemented and reviewed.

Review gate:

- UX review using screenshots at desktop and narrow widths.
- Standards review confirming extension fields are visually and semantically
  separated.

### Wave 3: Readiness And Validation

Parallel ownership:

- Readiness worker: Discoverable, Validatable, Policy, Trust, and Lifecycle
  scoring.
- Missing-fields worker: grouped/ranked known-versus-missing report with links
  to standards terms and SHACL shapes where available.
- Validation worker: server-side SHACL validation or explicit `Not yet run`
  implementation if validation is deferred.

Definition of done:

- Registration Readiness produces deterministic Ready, Partial, Missing, or Not
  checked states for each category.
- The top three missing items are shown as actionable cards with standards
  references.
- Validation never reports `Valid` unless a real validator has run.
- Unit tests cover readiness scoring and missing-field ranking.

Review gate:

- Product review of readiness language to ensure it does not imply approval.
- Standards review of ADMS, DQV, ODRL, DPV, VCDM/DID, and vCard mappings.

### Wave 4: Scoped Graph And Comparison

Parallel ownership:

- Graph worker: scoped graph rendering, node budget, large-catalog fallback.
- Comparison worker: standards-only versus include-extensions diff view.
- Demo worker: curated local demos and recent-session catalog shortcuts.

Definition of done:

- Table remains the default view for all catalogs.
- Graph rendering is opt-in, scoped, and blocked or summarized above the node
  budget.
- Comparison view clearly identifies fields added by extensions and whether
  they affect standards-only readiness.
- Curated demo entries load without manual setup beyond documented Registry
  Relay startup steps.

Review gate:

- UX review of graph fallback behavior on a large synthetic catalog.
- Code review focused on performance and no layout overlap.

### Wave 5: Hardening And Release Candidate

Parallel ownership:

- Accessibility/i18n worker: keyboard navigation, labels, locale string
  coverage.
- Security worker: proxy SSRF protections, credential redaction, token handling
  tests.
- Verification worker: end-to-end browser tests, fixture regression suite,
  release checklist.

Definition of done:

- All documented lint, typecheck, unit, integration, and browser checks pass.
- A reviewer can run the five-minute demo flow from a clean checkout.
- The release notes list supported standards, unsupported artifacts, and known
  Registry Relay gaps.
- No v0 item remains partially implemented unless it is explicitly listed as
  unsupported in the UI and release notes.

Review gate:

- Final code review by at least one reviewer outside each workstream's ownership
  area.
- Final product walkthrough against the Success Criteria section.
- Final standards accuracy pass before tagging the prototype.
