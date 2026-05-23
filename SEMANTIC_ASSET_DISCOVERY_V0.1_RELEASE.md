# semantic-asset-discovery v0.1 Release Notes And Checklist

## Release Summary

v0.1 introduces `semantic-asset-discovery` as a reusable Rust discovery engine
for Registry Atlas and other hosts. It accepts already-fetched metadata
artifacts, classifies semantic assets, extracts standards and profile claims,
records tagged evidence, emits fetch candidates, and returns a typed
`DiscoveryReport` using schema version
`semantic-asset-discovery.report.v1`.

The release is standards-first and publisher-neutral. Fixture names and profile
examples are generic. Publisher-specific behavior is out of scope unless it is
expressed through supported standards artifacts.

## Included Surfaces

- `semantic-asset-discovery-core`: network-free Rust analysis crate.
- `semantic-asset-discovery`: ergonomic host facade for bounded public-web
  discovery, rejected fetches, run envelopes, and navigation views.
- `semantic-asset-discovery-wasm`: browser-facing JSON envelope wrapper for
  Atlas and other WASM consumers.
- `semantic-asset-discovery-cli`: native commands for analysis, bundle
  analysis, bounded harvest, and report validation.
- v1 report schema, tagged evidence model, deterministic report object IDs, and
  defensive sensitive-header redaction.
- Fixture coverage for generic catalogue, profile, schema, API, geospatial, and
  semantic model package examples.

## Supported In v0.1

- DCAT and DCAT-AP shallow JSON-LD extraction.
- BRegDCAT-AP profile claims when declared.
- PROF profile and resource links.
- SHACL classification and shape graph asset detection for JSON-LD and Turtle.
- SKOS concept scheme detection.
- JSON Schema classification and shallow asset extraction.
- OpenAPI 3.x classification and external link extraction.
- OGC API Records and OGC API Features classification from landing,
  conformance, and collection documents.
- Semantic model package detection through LinkML source, package manifests,
  JSON-LD contexts, RDF or OWL exports, SHACL, JSON Schema, SKOS, alignments,
  and crosswalks.
- Fetch candidates from declared links, with rejected core links represented as
  findings instead of silent drops.

## Not Included In v0.1

- Central registry storage, scheduling, review workflow, or governance
  decisions.
- Network access from the core or WASM wrapper.
- SHACL validation execution.
- LinkML import resolution, schema merging, or code generation.
- SPARQL query or update behavior.
- Deep ODRL, DQV, ADMS, DPV, DID Web, or Verifiable Credential validation.
- Report merge semantics across harvest runs.
- Production harvest orchestration beyond the bounded CLI wrapper.

## Release Definition Of Done

v0.1 is releasable only when every required item below is checked and the
verification command passes from a clean checkout.

Pass/fail gates:

- [x] All CLI checklist items are checked.
- [x] All WASM and Atlas checklist items are checked.
- [x] All standards and fixture checklist items are checked.
- [x] Every reviewer sign-off row has reviewer, date, status, and notes.
- [x] No reviewer sign-off row is `Blocked` or `Changes requested`.
- [x] No P0 or P1 review finding remains unresolved.
- [x] `pnpm check:release` passes.
- [x] The release notes list exactly what is supported, unsupported, and
  shallow-only in v0.1.
- [x] Every supported standard or artifact family listed in this file has a
  passing fixture or test.
- [x] Every unsupported standard or feature listed in this file is represented
  by a finding, unsupported-state test, known limitation, or explicit non-goal.
- [x] Sensitive request or response values are absent from golden reports, CLI
  output fixtures, WASM envelope fixtures, UI test fixtures, and logs captured
  by tests.
- [x] The core crate has no networking dependency.
- [x] Core source contains no Registry Relay names outside fixtures or tests.
- [x] Registry Relay, generic DCAT-AP, SEMIC-style, OGC, OpenAPI, JSON Schema,
  and semantic model package fixtures all run through the same standards
  discovery path.
- [x] The release can be reproduced without manual data edits, hidden local
  services, or uncommitted fixture changes.

The release is not done if any checklist item is unchecked. Accepted residual
risks are allowed only for P2 or lower issues, must be written in the reviewer
sign-off notes, and must not violate the core/facade/WASM security boundaries.

## CLI Checklist

- [x] `semantic-asset-discovery analyze --entry-url <url> <artifact>...`
  produces a valid report for a single local artifact and multi-artifact input.
- [x] `semantic-asset-discovery analyze-bundle <bundle.json>` produces a valid
  report from serialized `AnalyzeInput`.
- [x] `semantic-asset-discovery analyze-bundle` reads serialized `AnalyzeInput`
  from stdin.
- [x] `semantic-asset-discovery harvest --max-depth 1 --max-fetches 20 <url>`
  follows declared links through the host fetch loop.
- [x] Private-network harvest candidates are blocked by default and preserved as
  auditable host policy findings.
- [x] `semantic-asset-discovery validate-report fixtures/reports/*.json`
  accepts supported reports.
- [x] `validate-report` rejects an unsupported `schema_version`.
- [x] CLI output does not contain sensitive request or response header values.

## WASM And Atlas Checklist

- [x] `analyzeArtifacts(inputJson)` returns `{ "ok": true, "report": ... }`
  for valid input.
- [x] `analyzeArtifacts(inputJson)` returns `{ "ok": false, "error": ... }`
  for invalid input, payloads above the body budget, and caught panics.
- [x] Atlas passes only sanitized `FetchedArtifact` bundles into WASM.
- [x] Atlas owns proxy fetches, URL filtering, credential policy, persistence,
  indexing, and UI state.
- [x] TypeScript consumers use checked declarations or schema coverage instead
  of duplicating standards parsing logic outside Rust.

## Standards And Fixture Checklist

- [x] Generic DCAT-AP catalogue fixture.
- [x] Generic BRegDCAT-AP-style catalogue fixture.
- [x] PROF profile fixture with resource descriptors.
- [x] SEMIC-style Turtle SHACL fixture.
- [x] JSON Schema fixture.
- [x] OpenAPI fixture.
- [x] OGC API Records fixture.
- [x] OGC API Features fixture.
- [x] Generic standards publisher bundle fixture.
- [x] Generic semantic model package fixture with LinkML, JSON-LD context,
  SHACL, JSON Schema, SKOS, alignment, crosswalk, and package manifest coverage.

## Verification Commands

Run these before tagging v0.1:

```bash
pnpm check:release
```

Then run the live Registry Relay rehearsal in
[`HOW_TO_TEST_V0_1.md`](HOW_TO_TEST_V0_1.md). That rehearsal is the
cross-project proof that Registry Relay publishes the canonical `/metadata/*`
surfaces, Atlas can harvest them, the generated report validates, the social
protection capability query returns candidate routes with evidence, and the UI
can load both bundled and live metadata.

`pnpm check:release` runs the app checks and the semantic discovery release
checks. The semantic checks include Rust formatting, the full Rust workspace
test suite, report validation, the core no-network dependency guard, the core
publisher-neutrality guard, and a `wasm32-unknown-unknown` build of the WASM
crate.

Before v0.1 sign-off, `pnpm check:release` MUST fail if any required release
crate is missing or untested, including `semantic-asset-discovery-core`,
`semantic-asset-discovery`, `semantic-asset-discovery-cli`, and
`semantic-asset-discovery-wasm`.

Record the command date, result, reviewer, and any accepted residual risk before
closing the release.

## Known Limitations

- Parser coverage is intentionally shallow and evidence-oriented.
- Discovery reports are not governance decisions and do not certify publisher
  readiness.
- Atlas must store raw reports and derived indexes if it wants durable registry
  history, diffs, review state, or reprocessing.
- Large browser analyses must be split or moved to the native CLI because WASM
  enforces a body budget.
- Unsupported standards must appear as findings, unsupported states, or release
  notes. They must not disappear silently.
- The Rust facade currently executes the harvest queue sequentially while
  preserving `max_concurrent_fetches` in the request/envelope contract. Parallel
  scheduling is a v0.1.x optimization, not a v0.1 correctness requirement.

## Reviewer Sign-Off

Allowed status values are `Approved`, `Approved with P2 risk`, `Changes
requested`, and `Blocked`.

| Area | Reviewer | Date | Status | Notes |
| --- | --- | --- | --- | --- |
| Core boundary and no-network dependency guard | Codex reviewer | 2026-05-20 | Approved | `semantic-asset-discovery-core` has no networking dependency and passes the Registry Relay source guard. |
| Schema compatibility and report validation | Codex reviewer | 2026-05-20 | Approved | `DiscoveryReport`, `DiscoveryRunEnvelope`, WASM envelope, and CLI report validation are covered by tests. |
| Standards mapping and fixture coverage | Codex reviewer | 2026-05-20 | Approved | Generic DCAT-AP, BRegDCAT-AP-style, PROF, SEMIC SHACL Turtle, JSON Schema, OpenAPI, OGC, standards publisher, and semantic package fixtures pass through the same Rust core path. |
| WASM envelope and Atlas handoff | Codex reviewer | 2026-05-20 | Approved | Atlas sanitizes WASM inputs, preserves fetch summaries and rejected fetches, and keeps credential forwarding same-origin. |
| CLI harvest security and redaction | Codex reviewer | 2026-05-20 | Approved with P2 risk | CLI harvest emits `DiscoveryRunEnvelope`; public-web policy blocks private-network targets. Accepted P2: native Rust facade harvest queue is sequential despite the concurrency budget field. |
| Release documentation | Codex reviewer | 2026-05-20 | Approved with P2 risk | Checklist and limitations reflect the implemented v0.1 scope and accepted sequential-harvest P2 risk. |
