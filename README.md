# Registry Atlas

Registry Atlas is a standards-first workbench for inspecting published
catalogue and registry discovery artifacts.

## Run

```bash
pnpm install
pnpm dev
```

The development server starts:

- UI: `http://127.0.0.1:5177`
- Fetch proxy: `http://127.0.0.1:3717`

Use `pnpm check` before frontend review. It runs lint, tests, and a production
build. Use `pnpm check:release` before release review. It adds the Rust
workspace tests and semantic discovery fixture checks.
Use [`HOW_TO_TEST_V0_1.md`](HOW_TO_TEST_V0_1.md) for the full release rehearsal
against Registry Relay.

## Workspace

- [`crates/semantic-asset-discovery`](crates/semantic-asset-discovery/README.md):
  bounded discovery client and facade over the core analyzer.
- [`crates/semantic-asset-discovery-core`](crates/semantic-asset-discovery-core/README.md):
  artifact parser, semantic asset model, profile packs, and report generation.
- [`crates/semantic-asset-discovery-cli`](crates/semantic-asset-discovery-cli/README.md):
  command-line discovery and report comparison tools.
- [`crates/semantic-asset-discovery-wasm`](crates/semantic-asset-discovery-wasm/README.md):
  WebAssembly wrapper used by the Atlas UI.
- [`crates/system-capability-discovery`](crates/system-capability-discovery/README.md):
  deterministic capability matching over semantic discovery reports.

## Registry Relay Demo

Start Registry Relay separately, then use the curated local demo in the atlas
top bar:

```text
http://127.0.0.1:4242/metadata
```

Bearer tokens are session-only. They are forwarded to the proxy request and are
never written to browser storage or server logs.

## Capability Queries

System capability discovery is strict in v0.1. It does not search the question
text, infer synonyms, or use AI. Put accepted machine terms in the query:

```rust
CapabilityQuery::new("social_protection_program")
    .need(InformationNeed::new("farmer_status")
        .requires_any([Term::label("Farmer")]))
    .need(InformationNeed::new("disability_status")
        .requires_all([
            Term::label("Disabled Person"),
            Term::field("disability_status"),
        ]))
    .need(InformationNeed::new("school_attendance")
        .requires_any([Term::field("attendance_rate")]));
```

Use `requires_any` for alternatives. Use `requires_all` when a field name is
too generic unless it appears on the same labelled asset or reviewed mapping.

CLI equivalent:

```bash
cargo run -q -p system-capability-discovery --bin system-capability-query -- \
  --envelope fixtures/system-capability/registry-relay-all-standards.envelope.json \
  --need-all disability_status label "Disabled Person" \
  --need-all disability_status field disability_status \
  --pretty
```

## semantic-asset-discovery

The v0.1 release-facing docs are:

- [`SEMANTIC_ASSET_DISCOVERY_README.md`](SEMANTIC_ASSET_DISCOVERY_README.md)
- [`SERVICE_FIRST_DISCOVERY_SPEC.md`](SERVICE_FIRST_DISCOVERY_SPEC.md)
- [`SEMANTIC_ASSET_DISCOVERY_FACADE_SPEC.md`](SEMANTIC_ASSET_DISCOVERY_FACADE_SPEC.md)
- [`SYSTEM_CAPABILITY_DISCOVERY_SPEC.md`](SYSTEM_CAPABILITY_DISCOVERY_SPEC.md)
- [`SEMANTIC_ASSET_DISCOVERY_V0.1_RELEASE.md`](SEMANTIC_ASSET_DISCOVERY_V0.1_RELEASE.md)
- [`STANDARDS_ASSUMPTIONS.md`](STANDARDS_ASSUMPTIONS.md): the boundary between
  standards evidence, Atlas-derived hypotheses, and claims that need review.
