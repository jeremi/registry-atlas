# semantic-asset-discovery-core

Core parser and report model for Registry Atlas semantic asset discovery.

This crate is network-free. It analyzes fetched artifacts and produces the
semantic discovery report consumed by the Rust client, CLI, WASM wrapper, and
Atlas UI.

## What It Provides

- `analyze_artifacts` for turning fetched artifacts into a discovery report.
- Types for artifacts, semantic assets, standards claims, profile claims,
  graph edges, evidence, and condition status.
- `ServiceGraph` for service-first navigation across public services,
  requirements, evidence types, evidence providers, access services, forms,
  route paths, source relation claims, and explicit gaps.
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

## ServiceGraph API

`ServiceGraph::from_report` builds a borrowed graph over a v2 discovery report.
Every semantic relation must have a relation claim so downstream views can trace
paths back to source artifacts. From a public service IRI, callers can navigate:

- declared channels and requirements;
- accepted evidence types reached through requirement evidence type lists or
  direct accepted-evidence relations;
- evidence offerings, providers, and access data services;
- form definitions attached to the service or channel;
- typed route summaries through `routes_for_service` or an owned
  `PublicServiceProjection`.

```rust,no_run
use semantic_asset_discovery_core::{DiscoveryReport, ServiceGraph};

# fn existing_report() -> DiscoveryReport { unimplemented!() }
let report = existing_report();
let graph = ServiceGraph::from_report(&report)?;
let service = graph.public_service("https://example.test/services/permit")?;

for route in graph.routes_for_service(service.id()) {
    println!("{:?} -> {}", route.route_kind, route.target.id);
}

let projection = service.projection();
assert_eq!(projection.service_iri.as_deref(), Some("https://example.test/services/permit"));
# Ok::<(), semantic_asset_discovery_core::ServiceGraphError>(())
```

`service.gaps()` reports absent declared metadata in the analyzed report. A gap
does not prove the real-world service, provider, access route, form, or evidence
does not exist. It means the expected relation was not declared in the artifacts
available to discovery.

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
