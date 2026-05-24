# semantic-asset-discovery-core

Core parser and report model for Registry Atlas semantic asset discovery.

This crate is network-free. It analyzes fetched artifacts and produces the
semantic discovery report consumed by the Rust client, CLI, WASM wrapper, and
Atlas UI.

## What It Provides

- `analyze_artifacts` for turning fetched artifacts into a discovery report.
- Types for artifacts, semantic assets, standards claims, profile claims,
  graph edges, evidence, and condition status.
- Built-in profile pack support.
- Deterministic report serialization for fixture and release checks.

## Typical Use

```rust
use semantic_asset_discovery_core::{analyze_artifacts, AnalyzeInput};

fn analyze(input: AnalyzeInput) -> Result<(), semantic_asset_discovery_core::AnalyzeError> {
    let report = analyze_artifacts(input)?;
    println!("{}", report.schema_version);
    Ok(())
}
```

## Boundary

This crate does not fetch URLs, hold credentials, call AI services, or infer
synonyms from user questions. It parses evidence already present in artifacts
and profile packs.

## Testing

```sh
cargo test -p semantic-asset-discovery-core
```

## License

MIT.
