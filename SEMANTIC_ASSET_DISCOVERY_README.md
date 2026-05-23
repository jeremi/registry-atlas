# semantic-asset-discovery

`semantic-asset-discovery` is the reusable standards engine behind Dataspace
Atlas discovery. It analyzes fetched metadata artifacts, classifies semantic
assets, preserves evidence, and returns a typed `DiscoveryReport`.

The engine is intentionally host-neutral. It does not run a registry, persist
reports, schedule harvests, decide governance approval, or fetch from the
network inside the core or WASM wrapper.

## Supported Standards

v0.1 focuses on shallow, auditable discovery for published standards artifacts:

- DCAT and DCAT-AP JSON-LD catalogues, datasets, distributions, and services.
- BRegDCAT-AP profile claims when declared.
- PROF profiles and resource descriptors.
- SHACL artifacts in JSON-LD or Turtle, including body-content detection.
- SKOS concept schemes.
- JSON Schema, including Draft 2020-12 and common earlier drafts.
- OpenAPI 3.x service descriptions.
- OGC API Records and OGC API Features landing, conformance, and collection
  documents.
- Semantic model packages published through LinkML source, exported RDF or OWL,
  JSON-LD contexts, SHACL, JSON Schema, SKOS, alignments, crosswalks, or a
  `semantic-asset-package.v1.toml` manifest.

ODRL, DQV, ADMS, DPV, DID Web, and W3C Verifiable Credential artifacts are
recognized only where the current parser surface can classify or preserve them
as assets, claims, source hints, or findings. Deep validation and trust
assessment are outside v0.1.

## Boundaries

The project has four implementation surfaces:

```text
semantic-asset-discovery-core
  Pure Rust analysis library. No networking, storage, UI, or Atlas imports.

semantic-asset-discovery
  Ergonomic Rust host facade for bounded public-web discovery, safe fetching,
  the `DiscoveryRunEnvelope`, and report navigation views.

semantic-asset-discovery-wasm
  Browser-facing wrapper for already-fetched, sanitized artifact bundles.

semantic-asset-discovery-cli
  Native command-line wrapper for file analysis, bundle analysis, report
  validation, and bounded harvests.
```

Hosts own the unsafe edges:

- fetching artifacts;
- timeout, redirect, byte, depth, and artifact-count limits;
- SSRF and private-network policy;
- credential forwarding policy;
- stripping sensitive headers before constructing `FetchedArtifact`;
- persisting reports and indexing derived registry views;
- deriving cross-report systems and operational capabilities.

The core still redacts defensively, but sensitive request or response values
should not enter parser inputs.

## CLI

Build or run the CLI from the Registry Atlas workspace:

```bash
cargo run -p semantic-asset-discovery-cli -- --help
```

Analyze one or more local artifacts:

```bash
cargo run -p semantic-asset-discovery-cli -- analyze \
  --entry-url https://publisher.example/catalog.jsonld \
  fixtures/semantic-asset-discovery/dcat-ap/catalog.jsonld
```

Analyze a serialized `AnalyzeInput` bundle from a file:

```bash
cargo run -p semantic-asset-discovery-cli -- analyze-bundle bundle.json
```

Or from stdin:

```bash
cat bundle.json | cargo run -p semantic-asset-discovery-cli -- analyze-bundle
```

Harvest through the native host wrapper:

```bash
cargo run -p semantic-asset-discovery-cli -- harvest \
  --max-depth 1 \
  --max-fetches 20 \
  https://publisher.example/metadata
```

Private and local network targets are blocked by default during harvest. Use
`--allow-private-network` only for trusted local testing.

For standards interpretation boundaries, including what Atlas treats as a
discovered fact, a hypothesis, or a reviewed claim, see
[`STANDARDS_ASSUMPTIONS.md`](STANDARDS_ASSUMPTIONS.md).

Validate generated reports:

```bash
cargo run -p semantic-asset-discovery-cli -- validate-report fixtures/reports/*.json
```

Query a harvested report for strict capability evidence:

```bash
cargo run -q -p system-capability-discovery --bin system-capability-query -- \
  --envelope fixtures/system-capability/registry-relay-all-standards.envelope.json \
  --need-all disability_status label "Disabled Person" \
  --need-all disability_status field disability_status \
  --pretty
```

`--need` adds alternative terms (`requires_any`). `--need-all` adds conjunctive
terms that must match the same candidate route. This keeps v0.1 deterministic:
no question-text search, no fuzzy matching, and no AI expansion in the core
matcher.

CLI exit codes:

- `0`: command succeeded.
- `1`: analysis, fetch, parse, or validation error.
- `2`: invalid CLI usage or invalid options.

## Atlas WASM Integration

Atlas should call the WASM wrapper only after its proxy has fetched and
sanitized artifacts:

```text
Atlas URL input
  -> Atlas proxy fetches the entry artifact
  -> Atlas strips sensitive headers
  -> WASM analyzes the artifact bundle
  -> Atlas filters report.next_fetches
  -> Atlas fetches allowed candidates through the proxy
  -> WASM analyzes the expanded bundle
  -> Atlas stores and renders the final DiscoveryReport
```

The current WASM entry point is:

```ts
analyzeArtifacts(inputJson: string): string
```

`analyzeArtifacts` returns the result envelope from
[`SEMANTIC_ASSET_DISCOVERY_SCHEMA.md`](SEMANTIC_ASSET_DISCOVERY_SCHEMA.md).
It does not fetch. It enforces the default total body budget of
`16_777_216` bytes per call and returns `analyze.payload_too_large` when that
budget is exceeded.

The implementation spec also reserves a `version()` wrapper export for release
metadata.

Build the generated web package when Atlas needs to consume the WASM artifact:

```bash
pnpm build:wasm
```

The generated module exports `analyzeArtifacts(inputJson)` and `version()`.
Atlas code should load that module through `createSemanticAssetDiscoveryAnalyzer`
and pass proxy results through `analyzeProxyResultWithWasm`.

## Known Limitations

- Extraction is shallow. The core does not run SHACL validation, LinkML
  generation, SPARQL queries, or ontology reasoning.
- LinkML imports are emitted as fetch candidates. They are not resolved or
  merged inside the parser.
- `merge_reports` is not part of v0.1. Report history and deduplication are
  Atlas responsibilities.
- Host-rejected URLs must be preserved by the host as policy findings or host
  state. The core only reports links it can safely represent or reject at parse
  time.
- WASM uses a JSON string boundary in v0.1. Large harvests should be split into
  smaller bundles or run through the native CLI.
- The CLI harvest wrapper is a bounded utility for CI and local checks, not a
  production scheduler or central registry harvester.
- Fixture and profile names in release docs are intentionally generic. Product
  or publisher-specific behavior must come from standards evidence, not special
  cases.

## Reference Docs

- [`SEMANTIC_ASSET_DISCOVERY_SPEC.md`](SEMANTIC_ASSET_DISCOVERY_SPEC.md)
- [`SEMANTIC_ASSET_DISCOVERY_SCHEMA.md`](SEMANTIC_ASSET_DISCOVERY_SCHEMA.md)
- [`SEMANTIC_ASSET_DISCOVERY_FACADE_SPEC.md`](SEMANTIC_ASSET_DISCOVERY_FACADE_SPEC.md)
- [`SYSTEM_CAPABILITY_DISCOVERY_SPEC.md`](SYSTEM_CAPABILITY_DISCOVERY_SPEC.md)
- [`SEMANTIC_ASSET_DISCOVERY_V0.1_RELEASE.md`](SEMANTIC_ASSET_DISCOVERY_V0.1_RELEASE.md)
