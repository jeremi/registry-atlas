# semantic-asset-discovery Specification

## Status

This document is the implementation spec for `semantic-asset-discovery`.

Normative words use RFC 2119 meaning:

- **MUST** means required.
- **MUST NOT** means forbidden.
- **SHOULD** means recommended unless there is a documented reason.
- **MAY** means optional.

Detailed report and API schemas live in
[`SEMANTIC_ASSET_DISCOVERY_SCHEMA.md`](SEMANTIC_ASSET_DISCOVERY_SCHEMA.md).
This spec defines boundaries, invariants, acceptance criteria, and implementation
waves.

## Purpose

`semantic-asset-discovery` is a small Rust standards engine for discovering
semantic assets from published metadata.

It is designed to be reused from:

- Registry Atlas through WebAssembly;
- native CLI and CI checks;
- future central registry harvest jobs;
- publisher test suites, including Registry Relay, without special treatment.

The core promise is:

> Given fetched metadata artifacts, classify them, extract semantic assets,
> discover standards links, preserve evidence, and return a typed report.

The library is not specific to Registry Relay. Registry Relay is one possible
publisher. Registry Atlas is one possible registry application built on top.

The library MUST discover semantic model packages, not only catalogues. A
semantic model package can include LinkML source or exported RDF, JSON-LD
contexts, SHACL shapes, JSON Schema, SKOS concept schemes, profile metadata,
provenance metadata, and crosswalk or alignment artifacts. This is required for
SEMIC-style vocabulary environments and for PublicSchema.

## Product Direction

Registry Atlas can become a central registry by storing, indexing, comparing,
and reviewing discovery reports produced by this library.

The reusable library stays smaller:

```text
Publisher URL
  -> host fetch loop
  -> semantic-asset-discovery core
  -> DiscoveryReport
  -> Registry Atlas storage, search, review, and UI
```

The important boundary is:

```text
Rust core:      analyze fetched artifacts
Host runtime:   fetch, filter, persist, schedule, secure, review, display
```

## Architectural Boundary

The project is split into crates:

```text
crates/
  semantic-asset-discovery-core/
    Pure Rust library. No networking, no app state.

  semantic-asset-discovery/
    Ergonomic Rust facade. Fetch loop, policy, sanitization, and navigation
    views.

  semantic-asset-discovery-cli/
    Thin command-line wrapper. Uses the facade for online harvest and the core
    for offline analysis, bundle analysis, and report validation.

  semantic-asset-discovery-wasm/
    wasm-bindgen wrapper for Registry Atlas and browser consumers.
```

The core crate MUST compile standalone:

```bash
cargo build -p semantic-asset-discovery-core
```

The core crate MUST NOT depend on:

- Atlas TypeScript code;
- Atlas server code;
- Registry Relay crates;
- `reqwest`, browser fetch APIs, or any networking client;
- application storage libraries;
- UI libraries.

CI MUST include a dependency check that fails if
`semantic-asset-discovery-core` gains a networking dependency. A simple first
gate is acceptable:

```bash
cargo tree -p semantic-asset-discovery-core | rg 'reqwest|hyper|ureq|isahc' && exit 1 || true
```

The first implementation MAY live inside the Registry Atlas repository, but
the crate boundary MUST be real. Atlas may depend on the WASM package and schema
types, not on hidden internal parser modules.

## Non-Goals

The library does not:

- run a central registry;
- persist harvest results;
- schedule recurring crawls;
- own Atlas UI state;
- approve participants;
- mutate source systems;
- perform source-specific ETL;
- require GraphDB, Virtuoso, or another triplestore;
- run SPARQL UPDATE;
- infer private implementation details from public metadata;
- depend on Registry Relay code, config, or endpoint names.

These concerns belong to applications built on top of the library.

## Core And Host Responsibilities

### Core Responsibilities

The core crate MUST:

- analyze caller-provided fetched artifacts;
- classify artifacts;
- classify semantic model packages and their component artifacts;
- extract shallow semantic assets;
- extract standards and profile claims;
- extract outgoing links as auditable fetch candidates;
- normalize compact IRIs using known prefixes and local context data;
- produce partial reports for malformed artifacts;
- preserve evidence for claims, links, and findings;
- expose a stable report schema version constant.

The core crate MUST NOT:

- fetch URLs;
- open sockets;
- read environment variables;
- persist files;
- decide SSRF policy;
- decide credential forwarding policy;
- know about Atlas storage;
- know about Registry Relay configuration;
- decide governance approval.

### Host Responsibilities

The host runtime MUST:

- fetch artifacts;
- strip sensitive headers before handing artifacts to the core;
- enforce timeout, redirect, byte, and artifact-count limits;
- enforce private-network and SSRF policy;
- decide whether discovered URLs are allowed;
- decide whether inferred links are allowed;
- persist or discard reports;
- run validation tools;
- render UI or produce CLI output.

Sensitive header stripping is a host responsibility because tokens should not
enter WASM memory or parser inputs. The core MAY publish a
`SENSITIVE_HEADER_NAMES` constant for host convenience, but callers MUST strip
or redact those headers before constructing `FetchedArtifact`.

The sensitive header set MUST include:

- `authorization`;
- `cookie`;
- `proxy-authenticate`;
- `set-cookie`;
- `www-authenticate`;
- `x-api-key`;
- `x-auth-token`;
- `proxy-authorization`.

The core still performs defensive redaction if a caller violates this boundary,
but that defense is not the primary security control.

## Host-Driven Harvest Loop

Harvesting is orchestration around the core.

Pseudo-flow:

```text
1. Host fetches the entry URL.
2. Host strips sensitive headers and applies fetch policy.
3. Host passes fetched artifacts to core analyze.
4. Core returns DiscoveryReport with next_fetches.
5. Host filters each FetchCandidate and records rejected candidates.
6. Host fetches allowed candidates.
7. Host repeats until depth, count, policy, or time budget is reached.
8. Host stores or displays the final report.
```

This gives two modes:

- **Analyze mode**: already-fetched artifacts in, report out.
- **Harvest mode**: entry URL in, host fetch loop plus core analysis, report
  out.

The CLI exposes both:

```bash
semantic-asset-discovery analyze fixtures/catalog.jsonld
semantic-asset-discovery harvest https://publisher.example/metadata
```

The WASM wrapper starts with analyze mode. Atlas fetches through its proxy and
passes sanitized artifacts to WASM.

## URL Filtering And Auditability

The core MUST NOT silently drop extracted URLs.

When the core extracts a URL, it MUST produce either:

- a `FetchCandidate`, when the URL is syntactically valid and uses an accepted
  scheme; or
- a finding with code `link.rejected_by_core`, when the URL is malformed or uses
  a scheme the core cannot represent safely.

The host remains responsible for security filtering. When the host rejects a
candidate, it MUST preserve an auditable reason in host state or a report
finding, for example:

- `security.disallowed_host`;
- `security.private_network_blocked`;
- `security.credentials_not_forwarded`;
- `limit.max_depth_reached`;
- `limit.max_fetches_reached`.

`FetchCandidate` MUST include enough evidence for the host to explain why a URL
was discovered and why it was accepted or rejected.

## Core API

The core API is intentionally small:

```rust
pub fn analyze_artifacts(input: AnalyzeInput) -> Result<DiscoveryReport, AnalyzeError>;
```

`analyze_artifacts` receives the current artifact bundle and returns a full
report for that bundle. The host can call it repeatedly with expanded bundles.

The core API intentionally omits `merge_reports` in v1. Merge semantics across
runs are an application concern until we have durable storage use cases.

`AnalyzeError` is reserved for errors that prevent analysis from starting:

- invalid entry URL;
- invalid options;
- invalid UTF-8 at the JSON/WASM boundary;
- schema deserialization failure;
- internal invariant failure caused by a core bug with no safe partial report.

Parser failures, unknown artifacts, malformed JSON-LD, unsupported media types,
and partial extraction failures MUST become `DiscoveryFinding` entries, not
`AnalyzeError`.

`AnalyzeError::InternalInvariant` MUST NOT be used for input-shape problems,
parser discomfort, unsupported standards, malformed RDF, malformed JSON-LD, or
missing optional fields. Those cases MUST become findings. Internal invariant
means a panic-equivalent core bug in release builds.

## Analyze Option Defaults

When callers omit options or provide empty option fields, the core MUST apply
these defaults:

- `max_next_fetches`: `20`
- `include_inferred_links`: `true`
- `accepted_schemes`: `["http", "https"]`
- `enabled_profiles`: all built-in profile packs

An empty `accepted_schemes` list means use the default schemes. An empty
`enabled_profiles` list means enable all built-in profile packs. To disable all
profiles, callers must pass an explicit sentinel value:
`["none"]`.

## Deterministic IDs

All report object IDs MUST be deterministic within a report and stable across
repeated analysis of the same inputs.

The default ID strategy is:

```text
artifact:<sha256(normalized_url)>[:16]
asset:<sha256(artifact_id + kind + uri_or_pointer)>[:16]
link:<sha256(from_url + to_url + rel_or_predicate)>[:16]
finding:<sha256(code + artifact_id + evidence_pointer)>[:16]
claim:<sha256(claimed_by_artifact_id + iri)>[:16]
```

Random UUIDs MUST NOT be used for report object IDs in core output.

`run_id` is different: it identifies the analysis run for audit and storage. It
MAY be host-provided and non-deterministic. Consumers MUST NOT use `run_id` for
deduplicating artifacts, assets, links, findings, claims, or reports.

## Evidence Model

`DiscoveryEvidence` MUST be a tagged enum, not a stringly typed bag.

Each evidence variant MUST contain only fields that make sense for that source.
For example:

- HTTP header evidence includes `header_name` and optional `rel`;
- JSON-LD predicate evidence includes `predicate` and optional JSON pointer;
- HTML link evidence includes `rel`, `href`, and optional element location;
- content sniff evidence includes a detector name and matched marker.

The schema file defines the current enum shape. Parsers MUST NOT invent ad-hoc
evidence objects.

## WASM Result Envelope

The WASM wrapper MUST return a discriminated JSON envelope:

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
    "message": "..."
  }
}
```

`analyzeArtifacts(inputJson: string): string` returns the serialized envelope,
not a raw report or panic string.

The wrapper MUST catch panics and convert them to an `ok: false` envelope with a
redacted message.

## Report Schema Rules

The schema is defined in
[`SEMANTIC_ASSET_DISCOVERY_SCHEMA.md`](SEMANTIC_ASSET_DISCOVERY_SCHEMA.md).

Rules:

- `REPORT_SCHEMA_VERSION` MUST be a public constant.
- Timestamps MUST be RFC 3339 strings.
- Numeric fields crossing the WASM or JSON boundary MUST use fixed-width integer
  types such as `u8`, `u16`, `u32`, or `u64`. They MUST NOT use `usize` or
  `isize`.
- Byte lengths and count fields MUST use `u64` to avoid WASM32/native
  ambiguity.
- Growing enums MUST include an unknown or other variant for forward
  compatibility.
- `schema_version` MUST deserialize through a validating type or equivalent
  validation path. Hand-written reports with unknown schema versions MUST fail
  `validate-report`.
- `ArtifactKind` SHOULD describe fetchable document categories.
- Embedded semantics such as ODRL, DQV, ADMS, and DPV SHOULD be modeled as
  assets or claims inside a DCAT artifact unless they are actually served as
  standalone artifacts.
- OGC landing pages that declare multiple conformance classes MUST choose a
  primary artifact kind using the priority rule in this spec, and MUST preserve
  every conformance IRI as a standard claim or finding.

OGC primary kind priority uses the highest-priority declared conformance class:

1. OGC API Records.
2. OGC API Features.
3. Unknown OGC landing page with conformance claims.

## Standards Coverage

### Wave 1 Required

The first core slice MUST support:

- JSON and JSON-LD artifact classification;
- DCAT/DCAT-AP shallow extraction;
- deterministic IDs;
- tagged evidence;
- fetch candidates from HTTP `Link` headers supplied by the host;
- fetch candidates from JSON-LD links;
- report findings for parse failures and rejected URLs.

### Wave 2a Required

Wave 2a MUST support:

- BRegDCAT-AP profile claims when declared;
- PROF profiles and resource descriptors;
- Turtle parsing for canonical SEMIC SHACL and profile artifacts;
- SHACL body-content detection;
- SKOS concept scheme detection;
- JSON Schema Draft 2020-12 and common earlier drafts;
- OpenAPI 3.x documents;
- OGC API Records landing pages, collections, and records;
- OGC API Features landing pages and collections.

Turtle support is not optional. SEMIC DCAT-AP 3.0.0 SHACL shapes are
Turtle-first, so canonical SEMIC fixtures cannot be an acceptance gate without
it.

Implementation choice:

- Use a Rust RDF/Turtle parser that compiles for native and WASM targets.
- Keep RDF parsing behind parser modules.
- Do not expose RDF library types in the public report schema.

The exact crate can be changed during implementation if needed, but Wave 2a is
not complete until canonical SEMIC Turtle SHACL fixtures parse.

### Wave 2b Required

Wave 2b MUST support:

- semantic model package discovery for LinkML source or exported RDF bundles;
- `semantic-asset-package.v1.toml` manifest detection;
- shallow LinkML package metadata extraction;
- JSON-LD context artifact detection;
- OWL/RDF export artifact detection;
- PublicSchema-style package fixture coverage;
- alignment and crosswalk asset extraction.

### Later Shallow Coverage

Later parser modules MAY add:

- ODRL policy extraction;
- DQV quality measurements;
- ADMS lifecycle status;
- DPV legal basis and personal-data classification hints;
- DID Web document classification;
- W3C Verifiable Credential classification.

### Semantic Model Packages

The library MUST treat semantic model packages as first-class discovery targets.

Minimum package components:

- LinkML schema source;
- exported RDF or OWL;
- JSON-LD context;
- SHACL shapes;
- JSON Schema;
- SKOS concept schemes;
- profile metadata;
- provenance metadata;
- crosswalk or alignment artifacts.

Examples:

- SEMIC Core Vocabularies and application profiles;
- PublicSchema vocabulary releases;
- registry metadata profile packs;
- domain vocabularies that publish SHACL and SKOS without a DCAT catalogue.

The core SHOULD classify a package from either:

- a package manifest;
- an HTML landing page with typed links;
- HTTP `Link` headers;
- LinkML schema metadata;
- PROF resource descriptors;
- DCAT/DCAT-AP records pointing to the package artifacts.

Semantic model package support MUST NOT require a DCAT catalogue. A package can
be discoverable from its own landing page or manifest.

A package manifest is a fetched JSON, YAML, or TOML document that explicitly
declares a semantic model package and links its component artifacts. The v1
manifest convention is `semantic-asset-package.v1.toml`. Other manifests MAY be
classified when profile packs define them.

LinkML support in v1 is shallow:

- extract schema identity, version, prefixes, imports, classes, slots, enums,
  annotations, and mappings from the fetched root schema;
- emit imports as fetch candidates with evidence;
- do not resolve imports inside the parser;
- do not merge imported LinkML schemas into the root schema;
- do not run LinkML generation inside the core.

Import resolution is a host-driven harvest behavior. If the host fetches imported
schemas and passes them in a later bundle, the core analyzes them as separate
artifacts with their own evidence.

These SHOULD usually be represented as semantic assets, claims, or findings
inside their source artifact, not as standalone artifact kinds unless the
publisher serves separate documents.

## Required Link Extraction

The core MUST extract candidate links from:

- HTTP `Link` headers passed in `FetchedArtifact.headers`;
- HTML `<link>` elements with `alternate`, `canonical`, `describedby`, or
  `profile`;
- LinkML schema imports, prefixes, and annotations that point to published
  artifacts;
- JSON-LD predicates:
  - `dcat:catalog`;
  - `dcat:dataset`;
  - `dcat:service`;
  - `dcat:landingPage`;
  - `dcat:distribution`;
  - `dcat:downloadURL`;
  - `dcat:accessURL`;
  - `dcat:endpointURL`;
  - `dcat:endpointDescription`;
  - `dcat:hasPart`;
  - `dcterms:hasPart`;
  - `dcterms:conformsTo`;
  - `prof:hasResource`;
  - `prof:hasArtifact`;
  - `prof:isProfileOf`;
  - `sh:shapesGraph`;
- JSON Schema `$id` and `$schema`;
- OpenAPI `externalDocs` and `servers`;
- OGC API landing, conformance, and collection links;
- metadata index documents when a publisher exposes one.

`prof:isProfileOf` feeds `ProfileClaim.base_standard_iri`.

## Property-Level Evidence

The core MUST preserve property-level and operation-level evidence so higher
layers can perform strict matching without reparsing source artifacts.

At minimum, v0.1 MUST emit these `DiscoveryEvidence` variants when the source
artifact contains the corresponding structure:

- `SchemaProperty` for JSON Schema properties, including the schema JSON Pointer
  and property path;
- `ShaclProperty` for SHACL property paths, including the shape IRI when known;
- `OpenApiOperation` for OpenAPI paths and methods, including `operationId` and
  summary when present;
- `OgcCollection` for OGC API collection ids and titles.

These evidence records are not fuzzy search tokens. They are source locations
and extracted labels or identifiers that downstream strict matchers can compare
against accepted terms, IRIs, fields, or reviewed mappings.

## Built-In Prefix Seed Set

The core MUST seed compact IRI expansion with at least:

| Prefix | IRI |
| --- | --- |
| `adms` | `http://www.w3.org/ns/adms#` |
| `dcat` | `http://www.w3.org/ns/dcat#` |
| `dct` | `http://purl.org/dc/terms/` |
| `dcterms` | `http://purl.org/dc/terms/` |
| `dpv` | `https://w3id.org/dpv#` |
| `dqv` | `http://www.w3.org/ns/dqv#` |
| `foaf` | `http://xmlns.com/foaf/0.1/` |
| `odrl` | `http://www.w3.org/ns/odrl/2/` |
| `owl` | `http://www.w3.org/2002/07/owl#` |
| `prof` | `http://www.w3.org/ns/dx/prof/` |
| `rdf` | `http://www.w3.org/1999/02/22-rdf-syntax-ns#` |
| `rdfs` | `http://www.w3.org/2000/01/rdf-schema#` |
| `sh` | `http://www.w3.org/ns/shacl#` |
| `skos` | `http://www.w3.org/2004/02/skos/core#` |
| `vcard` | `http://www.w3.org/2006/vcard/ns#` |
| `xsd` | `http://www.w3.org/2001/XMLSchema#` |

The core MAY also use local JSON-LD contexts and profile-pack prefix maps.

The core MUST NOT dereference remote vocabularies by default.

## SHACL Detection

SHACL detection MUST support both:

- link discovery through `sh:shapesGraph`;
- body-content detection for files containing SHACL terms.

Minimum body-content markers:

- `sh:NodeShape`;
- `sh:PropertyShape`;
- `sh:targetClass`;
- `sh:property`;
- `http://www.w3.org/ns/shacl#NodeShape`;
- `http://www.w3.org/ns/shacl#PropertyShape`.

SHACL detection MUST work for JSON-LD and Turtle fixtures by Wave 2a.

## Content Negotiation And Body Sniffing

The host records request and response metadata. `FetchedArtifact` MUST include:

- requested URL;
- final URL;
- response status;
- response media type if available;
- request `Accept` value or content-negotiation profile if the host has it;
- redirect chain if the host followed redirects;
- fetched timestamp.

The core MUST NOT trust media type alone. It MUST use conservative body sniffing
when a server returns `text/html` for a JSON-LD or RDF URL, or when a server
returns missing or incorrect content types.

Body sniffing MUST produce evidence with source `content-sniff`.

## Profile Packs

Profile packs MUST be data files from v1, not hardcoded Rust structs.

Initial format:

```text
profile-packs/*.toml
```

The core MAY embed built-in packs with `include_str!`, but parsing should use
the same data path as external packs.

Initial built-in packs:

- DCAT-AP;
- BRegDCAT-AP;
- OGC API Records;
- PROF;
- SHACL;
- JSON Schema;
- OpenAPI.

This keeps the library standards-aware without making the compiled Rust code the
only place where profile knowledge can live.

## Registry Relay Neutrality

Registry Relay may be used as a fixture publisher, but core logic MUST NOT
contain:

- endpoint names that only Registry Relay supports;
- Registry Relay config field names;
- Registry Relay extension vocabulary as a built-in standard;
- special readiness bonuses for Registry Relay metadata;
- assumptions about Registry Relay authentication or deployment.

If a Registry Relay fixture exposes DCAT-AP, SHACL, JSON Schema, OpenAPI, OGC
Records, or other standards artifacts, the core discovers those artifacts
through the same generic rules used for any publisher.

CI MUST include a grep-style guard that fails if Registry Relay names appear in
core source outside fixtures or tests:

```bash
rg -n 'Registry Relay|registry-relay|registry_relay' \
  crates/semantic-asset-discovery-core/src
```

This command should return no matches.

## CLI Wrapper

The CLI owns native fetching and file workflows.

Commands:

```bash
semantic-asset-discovery analyze <artifact-file>
semantic-asset-discovery analyze-bundle <bundle-json>
semantic-asset-discovery harvest <entry-url>
semantic-asset-discovery validate-report <report-file>
```

Useful flags:

```bash
--max-depth 2
--max-fetches 20
--max-body-bytes 2000000
--max-total-bytes 64000000
--timeout-ms 10000
--total-timeout-ms 120000
--output report.json
--format json
--no-inferred-links
--allow-host publisher.example
```

Exit codes:

- `0`: report or validation result produced with no error findings;
- `1`: report produced but contains error findings, validation failed, or
  harvest could not produce a safe partial report;
- `2`: invalid CLI usage or invalid options.

## WASM Wrapper

The WASM wrapper exposes:

```ts
analyzeArtifacts(inputJson: string): string
version(): string
```

`analyzeArtifacts` MUST return the result envelope defined in the schema.

The wrapper MUST NOT fetch. Atlas owns fetch and proxy behavior.

The wrapper MUST ship generated TypeScript declarations or a checked JSON
schema, and tests MUST verify that Atlas can consume the envelope.

The WASM wrapper MUST enforce a default total body budget per
`analyzeArtifacts` call:

```text
DEFAULT_WASM_BODY_BUDGET_BYTES = 16_777_216
```

If the sum of `FetchedArtifact.body` byte lengths exceeds that budget, the
wrapper MUST return an `ok: false` envelope with error code
`analyze.payload_too_large`. Hosts can split analysis into smaller bundles or
use the native CLI for larger harvests. This guardrail can be revisited when a
typed `serde-wasm-bindgen` boundary replaces JSON strings.

## Registry Atlas Integration

Atlas should call the WASM wrapper from its discovery path:

```text
Atlas URL input
  -> Atlas proxy fetches entry artifact
  -> Atlas strips sensitive headers
  -> WASM analyzes artifact bundle
  -> Atlas filters report.next_fetches
  -> Atlas fetches allowed candidates through proxy
  -> WASM analyzes expanded bundle
  -> Atlas stores final DiscoveryReport
  -> Atlas renders registry UI
```

Atlas maps the generic report into product views:

- artifact rail from `artifacts`;
- standards and profile badges from `standards` and `profiles`;
- table, tree, and graph records from `assets`;
- source hints from `source_hints`;
- known-versus-missing rows from findings plus Atlas readiness rules;
- freshness, diffs, and history from stored reports.

Atlas becomes the central registry by adding durable application concepts:

- sources;
- harvest runs;
- report snapshots;
- semantic asset indexes;
- finding indexes;
- review state;
- governance decisions;
- notifications.

The central registry database should store the raw discovery report alongside
derived indexes so Atlas can reprocess old reports when readiness rules change.

## Acceptance Criteria

### Core Mechanical Checks

Wave 1 is not complete until these pass:

```bash
cargo build -p semantic-asset-discovery-core
cargo test -p semantic-asset-discovery-core
cargo tree -p semantic-asset-discovery-core | rg 'reqwest|hyper|ureq|isahc' && exit 1 || true
rg -n 'Registry Relay|registry-relay|registry_relay' crates/semantic-asset-discovery-core/src && exit 1 || true
```

The actual CI commands may wrap these checks, but the checks must exist.

### Schema Compatibility

Every wave that changes report shape MUST update:

- Rust schema types;
- WASM envelope tests;
- TypeScript declarations or JSON schema;
- at least one golden report fixture.

`validate-report` MUST reject reports with unsupported schema versions unless
explicit compatibility code exists.

### Wave 2a Fixture Acceptance

Wave 2a is not complete until fixture tests cover:

- generic DCAT-AP JSON-LD catalog;
- BRegDCAT-AP-style catalog;
- PROF profile with resource descriptors;
- canonical or canonical-derived SEMIC Turtle SHACL shapes;
- JSON Schema document;
- OpenAPI document;
- OGC Records landing document;
- OGC Features landing document;
- Registry Relay-published metadata bundle used only as a standards fixture.

### Wave 2b Fixture Acceptance

Wave 2b is not complete until fixture tests cover:

- PublicSchema-style semantic model package with LinkML source, JSON-LD context,
  SHACL, JSON Schema, SKOS, and alignment metadata;
- `semantic-asset-package.v1.toml` package manifest;
- LinkML imports emitted as fetch candidates;
- alignment and crosswalk assets with tagged evidence.

### Harvest Acceptance

Wave 5 is not complete until:

- CLI harvest against a mock HTTP server follows declared links;
- host filtering rejects at least one private-network candidate with an
  auditable finding;
- sensitive headers are not present in any generated report;
- exit codes match the CLI section;
- report output validates against the schema.

## Implementation Waves

### Wave 1: Rust Core Skeleton

Deliver:

- `semantic-asset-discovery-core` crate;
- standalone build with no networking dependency;
- schema types from `SEMANTIC_ASSET_DISCOVERY_SCHEMA.md`;
- `analyze_artifacts`;
- deterministic IDs;
- tagged evidence;
- result and error types;
- redaction constants;
- unit tests for partial reports, deterministic IDs, and defensive redaction.

Done when:

- all Core Mechanical Checks pass;
- malformed artifacts return findings, not panics;
- no Registry Relay names appear in core source outside fixtures or tests.

### Wave 2a: SEMIC And Catalogue Standards Parsers

Deliver:

- JSON-LD shallow parsing;
- Turtle parsing for SEMIC SHACL fixtures;
- DCAT/DCAT-AP traversal links;
- BRegDCAT-AP claims;
- PROF `isProfileOf` and resource descriptors;
- SHACL body-content detection;
- SKOS detection;
- JSON Schema detection;
- OpenAPI detection;
- OGC Records and Features detection.

Done when:

- all Wave 2a Fixture Acceptance checks pass;
- each classification includes tagged evidence;
- profile packs are loaded from TOML data.

### Wave 2b: Semantic Model Package Parsers

Deliver:

- semantic model package classification;
- `semantic-asset-package.v1.toml` manifest detection;
- shallow LinkML package metadata extraction;
- JSON-LD context artifact detection;
- OWL/RDF export artifact detection;
- PublicSchema-style package fixture coverage;
- alignment and crosswalk asset extraction.

Done when:

- PublicSchema-style packages are discoverable without a DCAT catalogue;
- LinkML imports are emitted as fetch candidates, not resolved inside the
  parser;
- package fixture tests prove extraction of schema identity, version, prefixes,
  classes, slots, enums, annotations, mappings, alignments, and crosswalks;
- each extracted package field has tagged evidence.

### Wave 3: Fetch Candidate Extraction

Deliver:

- HTTP `Link` extraction;
- HTML link extraction;
- JSON-LD link extraction;
- OpenAPI external-doc links;
- OGC conformance and collection links;
- deterministic candidate ordering;
- no silent URL drops.

Done when:

- fixture bundles yield expected `next_fetches`;
- duplicate links collapse with first evidence preserved;
- inferred links can be disabled;
- rejected core links become findings.

### Wave 4: WASM Wrapper

Deliver:

- `semantic-asset-discovery-wasm` crate;
- `analyzeArtifacts(inputJson)` export;
- `version()` export;
- result envelope;
- TypeScript declarations or checked schema;
- Atlas-compatible tests.

Done when:

- Atlas can call the WASM wrapper with sanitized fetched artifacts;
- no network behavior exists in WASM;
- success and failure envelopes are tested.

### Wave 5: CLI Harvest

Deliver:

- `semantic-asset-discovery-cli` crate;
- native fetcher;
- host-side sensitive header stripping;
- host-side URL policy findings;
- `analyze`, `analyze-bundle`, `harvest`, and `validate-report` commands;
- CI-friendly exit codes.

Done when:

- all Harvest Acceptance checks pass;
- output is redacted;
- report output validates against schema.

### Wave 6: Atlas Central Registry Integration

Deliver:

- Atlas discovery path uses WASM report analysis;
- Atlas proxy owns fetching and security policy;
- Atlas strips sensitive headers before WASM handoff;
- Atlas stores or can store `DiscoveryReport` snapshots;
- UI derives artifacts, assets, claims, and findings from the generic report;
- Registry Relay demo works through standards discovery only.

Done when:

- Atlas can inspect a non-Relay fixture and a Registry Relay fixture through
  the same path;
- tests prove there is no Registry Relay special case in the discovery core;
- Atlas README describes the new discovery model.

## Closed Decisions

- Core is Rust, host-neutral, and network-free.
- Atlas uses the core through WASM.
- Fetching, SSRF policy, credential forwarding, and header stripping are host
  responsibilities.
- `merge_reports` is out of v1 core.
- Evidence is a tagged enum.
- WASM returns a result envelope.
- IDs are deterministic content-derived strings.
- Profile packs are TOML data files embedded or loaded by the core.
- Turtle support is required in Wave 2a.
- Semantic model package and LinkML support are split into Wave 2b.
- ODRL, DQV, ADMS, and DPV are usually semantic assets or claims, not primary
  artifact kinds.
- Semantic model packages are first-class discovery targets and do not require
  a DCAT catalogue.

## Non-Blocking Open Decisions

These decisions MUST NOT block v0.1 implementation. If they are still open at
release time, the release checklist must record the current choice and residual
risk.

- Which public EU/SEMIC fixtures are the acceptance gate alongside local
  fixtures? v0.1 implementation uses checked-in canonical-derived fixtures;
  public URLs can be added as non-blocking smoke tests because public upstream
  repositories may change independently of this crate.
- Should the WASM wrapper return raw JSON strings only long term, or also expose
  typed JS objects through `serde-wasm-bindgen` after v1?
- When should the crates move from Atlas into a shared workspace or independent
  repository?
- Before public extraction, what is the crate license, release authority,
  ownership model, and contribution policy?

## Parallel Implementation Plan

Implementation should run in waves with parallel workers, disjoint ownership,
and review gates at the end of every wave. Workers must not edit the same files
in parallel unless one worker is explicitly integrating the others' completed
patches.

### Global Definition Of Done

The canonical release sign-off lives in
[`SEMANTIC_ASSET_DISCOVERY_V0.1_RELEASE.md`](SEMANTIC_ASSET_DISCOVERY_V0.1_RELEASE.md).
The implementation is done only when every release checklist item is checked and
all of these are true:

- All wave definitions of done are satisfied.
- All review gates have a dated reviewer note in the release checklist.
- All required commands in this section pass from a clean checkout.
- `pnpm check:release` passes from the Registry Atlas workspace.
- No wave item is marked partial.
- No test is skipped to make the release pass unless the skip is documented in
  the release checklist with a non-P0/P1 residual risk.
- No unsupported behavior is silent. Each unsupported behavior has a failing
  fixture converted into an explicit unsupported-state test, finding, or UI
  state.
- The same discovery path handles a SEMIC fixture, a PublicSchema fixture, a
  generic DCAT-AP fixture, and a Registry Relay standards fixture.
- The core crate has no networking dependencies.
- The core source contains no Registry Relay names outside fixtures or tests.
- Sensitive headers are absent from every golden report, CLI output fixture,
  and WASM envelope fixture.
- Report output validates against the v1 schema.
- Atlas can consume the WASM result envelope without special-casing success or
  failure shapes.
- Release notes list supported standards, unsupported standards, fixture
  coverage, and known limitations.

The implementation is not done if any of these are true:

- A wave definition of done is incomplete.
- A P0 or P1 review finding is unresolved.
- A fixture only works through manual data edits or hidden local setup.
- A supported standard path is implemented only in Atlas TypeScript when it
  belongs in the Rust core.
- Sensitive request or response values appear in a report, fixture, WASM
  envelope, CLI output, UI test fixture, or log snapshot.
- Registry Relay behavior is special-cased in core source.
- The release checklist has an unchecked required item.
- The release notes describe support that is not covered by a passing fixture
  or test.

Required final commands:

```bash
cargo build -p semantic-asset-discovery-core
cargo test -p semantic-asset-discovery-core
cargo test -p semantic-asset-discovery
cargo test -p semantic-asset-discovery-cli
cargo test -p semantic-asset-discovery-wasm
cargo tree -p semantic-asset-discovery-core | rg 'reqwest|hyper|ureq|isahc' && exit 1 || true
rg -n 'Registry Relay|registry-relay|registry_relay' crates/semantic-asset-discovery-core/src && exit 1 || true
semantic-asset-discovery validate-report fixtures/reports/*.json
pnpm check
pnpm check:semantic
pnpm check:release
```

If a command name changes during implementation, the replacement command must
be documented in the release checklist before the wave can close.

### Operating Rules

- Each wave starts with a short kickoff note naming workers, owned paths,
  expected tests, and integration order.
- Each worker owns a narrow slice and writes code directly in that slice.
- Each worker must include focused tests for new behavior before handing off.
- The integration owner reviews worker patches before merge, checks for
  boundary violations, and runs the wave verification commands.
- A reviewer who did not write the slice performs a code review after each wave.
- A wave is not complete until its definition of done is met, review findings
  are resolved, and verification commands pass.
- Review gates are pass/fail. A wave cannot close with unresolved P0 or P1
  review findings.
- Each wave must add or update a release checklist entry with commands run,
  fixture coverage, reviewer, and unresolved risks.
- If a feature is only partially implemented, it is not marked done. It either
  remains in the wave with a blocker, or it is explicitly moved to a later wave
  with a test proving the current unsupported behavior.

### Wave A: Core Skeleton And Schema

Parallel workers:

- Core types worker owns `crates/semantic-asset-discovery-core/src/types.rs`,
  schema constants, deterministic IDs, and `AnalyzeError`.
- Core analysis worker owns `analyze.rs`, partial-report behavior, artifact
  status handling, and finding creation.
- Quality worker owns crate wiring, CI checks, no-network dependency guard, and
  golden report fixture scaffolding.

Definition of done:

- `cargo build -p semantic-asset-discovery-core` passes.
- `cargo test -p semantic-asset-discovery-core` passes.
- The dependency guard command returns success only when no forbidden networking
  crate is present:
  `cargo tree -p semantic-asset-discovery-core | rg 'reqwest|hyper|ureq|isahc' && exit 1 || true`.
- The Registry Relay guard command returns success:
  `rg -n 'Registry Relay|registry-relay|registry_relay' crates/semantic-asset-discovery-core/src && exit 1 || true`.
- Malformed artifacts produce findings, not panics.
- A golden report fixture validates against `REPORT_SCHEMA_VERSION`.
- Unit tests prove deterministic IDs are identical across two runs with the
  same input.
- Unit tests prove parser failures return `DiscoveryFinding`, not
  `AnalyzeError`.
- Unit tests prove sensitive header values are defensively redacted if a host
  violates the handoff contract.

Review gate:

- Reviewer must approve the crate boundary, deterministic ID scheme, tagged
  evidence model, and host/core security split.
- Review is blocked if core contains networking dependencies, Atlas imports, or
  Registry Relay source references.

### Wave B1: SEMIC And Catalogue Standards Parsers

Parallel workers:

- RDF worker owns Turtle and JSON-LD parsing, built-in prefixes, SHACL body
  detection, and SEMIC fixture support.
- Catalog worker owns DCAT/DCAT-AP traversal, BRegDCAT-AP claims, PROF
  `isProfileOf`, and OGC Records/Features classification.
- Standards fixture worker owns DCAT-AP, BRegDCAT-AP, PROF, SEMIC Turtle SHACL,
  OpenAPI, OGC, and Registry Relay standards fixtures.

Definition of done:

- Fixture tests pass for these named fixture groups:
  `fixtures/dcat-ap`, `fixtures/breg-dcat-ap`, `fixtures/prof`,
  `fixtures/semic-shacl-turtle`, `fixtures/json-schema`, `fixtures/openapi`,
  `fixtures/ogc-records`, `fixtures/ogc-features`, and
  `fixtures/registry-relay-standards`.
- Every classification has tagged evidence.
- Profile packs load from TOML data.
- SEMIC Turtle SHACL fixtures produce `Shacl` artifacts and `ShapeGraph` assets.
- RDF parser types do not appear in public report types.

Review gate:

- Standards reviewer must approve the SEMIC and DCAT-family fixture mappings.
- Code reviewer must approve parser isolation and confirm no RDF library types
  leak into the public schema.
- Review is blocked if any fixture group is skipped without an unsupported-state
  test.

### Wave B2: Semantic Model Package Parsers

Parallel workers:

- Package worker owns semantic model package detection, package manifest
  parsing, and package-level asset creation.
- LinkML worker owns shallow LinkML metadata extraction and import candidate
  emission.
- PublicSchema fixture worker owns PublicSchema-style fixtures, JSON-LD context,
  JSON Schema, SHACL, SKOS, alignment, and crosswalk expectations.

Definition of done:

- Fixture tests pass for `fixtures/publicschema-package`.
- PublicSchema-style packages are discoverable without a DCAT catalogue.
- PublicSchema fixtures produce `SemanticModelPackage`, `LinkMlSchema`,
  `JsonLdContext`, `JsonSchema`, `ShapeGraph`, `ConceptScheme`, `Alignment`, or
  `Crosswalk` outputs as applicable.
- LinkML extraction is shallow: imports are emitted as fetch candidates and are
  not resolved inside the parser.
- Package fixture tests prove extraction of schema identity, version, prefixes,
  classes, slots, enums, annotations, mappings, alignments, and crosswalks.
- Each extracted package field has tagged evidence.

Review gate:

- Standards reviewer must approve the PublicSchema fixture mapping.
- Code reviewer must approve that LinkML import resolution remains host-driven.
- Review is blocked if semantic model packages require a DCAT catalogue.

### Wave C: Link Extraction And Harvest Semantics

Parallel workers:

- Link worker owns HTTP `Link`, HTML link, JSON-LD link, OpenAPI, OGC, and
  LinkML link extraction.
- Candidate worker owns candidate normalization, ordering, duplicate collapse,
  inferred-link toggling, and rejected-link findings.
- Fixture worker owns multi-artifact bundle tests and golden `next_fetches`
  expectations.

Definition of done:

- No extracted URL is silently dropped.
- Duplicate links collapse with first evidence preserved.
- Inferred links can be disabled.
- Host-rejected URL examples produce auditable findings in harvest tests.
- Golden `next_fetches` fixtures are stable.
- Tests cover HTTP `Link`, HTML `alternate`, HTML `canonical`, HTML
  `describedby`, HTML `profile`, JSON-LD DCAT traversal, PROF, OpenAPI,
  OGC, and LinkML extraction.
- Tests prove malformed URLs become `link.rejected_by_core` findings.
- Tests prove private-network URL rejection is host-side and auditable.

Review gate:

- Reviewer must approve auditability, deterministic candidate ordering, and
  SSRF boundary clarity.
- Review is blocked if any extracted URL can disappear without a candidate or
  finding.

### Wave D: WASM And Atlas Adapter

Parallel workers:

- WASM worker owns `semantic-asset-discovery-wasm`, panic handling,
  `analyzeArtifacts`, `version`, and result envelope tests.
- TypeScript worker owns generated declarations or JSON schema checks and the
  thin Atlas wrapper.
- Atlas integration worker owns proxy-to-WASM handoff, sensitive header
  stripping before WASM, and fixture-driven discovery flow.

Definition of done:

- WASM success and error envelopes are tested.
- Atlas can analyze sanitized fetched artifacts through WASM.
- No network behavior exists in WASM.
- TypeScript consumers have a checked schema or generated declarations.
- Existing Atlas tests pass.
- Tests prove `analyzeArtifacts` returns `{ "ok": true, "report": ... }` on
  success and `{ "ok": false, "error": ... }` on invalid input.
- Tests prove panic handling returns a redacted error envelope.
- Tests prove Atlas strips sensitive headers before WASM handoff.
- Tests prove inputs above `DEFAULT_WASM_BODY_BUDGET_BYTES` return
  `analyze.payload_too_large`.
- `pnpm check` passes.

Review gate:

- Reviewer must approve the WASM boundary, redaction behavior, result envelope,
  and TypeScript wrapper thinness.
- Review is blocked if TypeScript duplicates standards parsing logic that
  belongs in Rust.

### Wave E: CLI Harvest And Release Candidate

Parallel workers:

- CLI worker owns `analyze`, `analyze-bundle`, `harvest`, `validate-report`,
  native fetch limits, and exit codes.
- Security worker owns host-side header stripping, URL policy, private-network
  rejection tests, and redacted output.
- Verification worker owns mock-server harvest tests, report validation,
  README updates, and release checklist.

Definition of done:

- CLI harvest against a mock HTTP server follows declared links.
- Private-network candidates are rejected with auditable findings.
- Sensitive headers never appear in reports or CLI output.
- Exit codes match the spec.
- `validate-report` rejects unsupported schema versions.
- Atlas can inspect both non-Relay and Registry Relay fixtures through the same
  standards path.
- All documented commands pass.
- `semantic-asset-discovery analyze` produces a valid report for a single file.
- `semantic-asset-discovery analyze-bundle` produces a valid report for a
  bundle directory.
- `semantic-asset-discovery harvest` produces a valid report from the mock
  server fixture.
- `semantic-asset-discovery validate-report fixtures/reports/*.json` passes for
  supported reports and fails for an unsupported-version fixture.
- Release notes include exact supported standards, unsupported standards,
  fixture coverage, and known limitations.

Review gate:

- Final reviewer must approve correctness, security, release documentation, and
  unresolved spec gaps.
- Release is blocked if any wave item is still partial without an explicit
  unsupported-state test and release note.

## Short Implementation Execution Plan

This is the operating plan for implementing the spec with parallel workers.

### Working Model

- Work happens in a dedicated worktree.
- Each wave starts with a kickoff note naming worker lanes, owned paths, tests,
  and integration order.
- Workers run in parallel only when their file ownership is disjoint.
- The integration owner reviews and merges worker output, resolves conflicts,
  and runs the wave verification commands.
- A separate reviewer performs code review after each wave. The reviewer must
  not be the primary author of the reviewed slice.
- Finished workers are closed after their results are integrated. New workers
  are opened for the next independent slice.

### Wave Plan

1. **Core contract wave**
   - Workers: schema/types, analyzer behavior, quality gates.
   - Exit gate: core builds standalone, core tests pass, deterministic IDs and
     defensive redaction are tested, parser failures become findings, and the
     no-network and publisher-neutrality guards pass.

2. **Standards parser wave**
   - Workers: RDF/Turtle/SHACL, DCAT/PROF/OGC/OpenAPI/JSON Schema, fixtures.
   - Exit gate: named fixtures pass, every classification has tagged evidence,
     profile packs load from TOML, and no parser library type leaks into the
     public schema.

3. **Semantic package wave**
   - Workers: package manifest detection, LinkML shallow extraction,
     PublicSchema-style fixtures.
   - Exit gate: semantic model packages work without DCAT, LinkML imports are
     emitted as fetch candidates, package fields have tagged evidence, and
     import resolution remains host-driven.

4. **Link and harvest semantics wave**
   - Workers: link extraction, candidate normalization, host rejection
     fixtures.
   - Exit gate: no URL disappears silently, duplicate links collapse
     deterministically, malformed URLs become findings, and private-network
     rejection is host-side and auditable.

5. **Facade, WASM, and Atlas wave**
   - Workers: Rust facade, WASM envelope, TypeScript/Atlas adapter.
   - Exit gate: `DiscoveryClient` and `DiscoveryRunEnvelope` work end to end,
     WASM success/error/payload-budget paths are tested, Atlas strips sensitive
     headers before WASM, and `pnpm check` passes.

6. **CLI and release wave**
   - Workers: CLI commands, security/redaction tests, release checklist.
   - Exit gate: analyze, analyze-bundle, harvest, and validate-report all
     produce valid behavior; sensitive values are absent from outputs; release
     notes match tested support; `pnpm check:release` passes.

### Non-Negotiable Definition Of Done

The implementation is done only when:

- every wave exit gate above is satisfied;
- every required item in
  [`SEMANTIC_ASSET_DISCOVERY_V0.1_RELEASE.md`](SEMANTIC_ASSET_DISCOVERY_V0.1_RELEASE.md)
  is checked;
- every review gate is approved or approved only with documented P2-or-lower
  risk;
- no P0 or P1 finding remains open;
- no unsupported behavior is silent;
- no supported claim appears in release notes without a passing fixture or
  test;
- the same standards path handles SEMIC-style, PublicSchema-style, generic
  DCAT-AP, OGC, OpenAPI, JSON Schema, and Registry Relay standards fixtures;
- sensitive values are absent from reports, fixtures, WASM envelopes, CLI
  output, UI test fixtures, and captured logs;
- the core crate has no networking dependency and no Registry Relay source
  reference outside fixtures or tests;
- `pnpm check:release` passes from a clean checkout.

If any item above is false, the release is not done.
