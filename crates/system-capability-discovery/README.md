# system-capability-discovery

Deterministic system capability matching for Registry Atlas.

This crate indexes semantic discovery reports and answers explicit capability
queries. It is intentionally strict: callers must provide accepted machine terms
such as labels, fields, standards, or profiles.

## What It Provides

- `CapabilityIndex` for searching analyzed semantic assets.
- Query types for systems, information needs, and required terms.
- `requires_any` and `requires_all` matching semantics.
- Stable result structures suitable for CLI and UI display.

## Typical Use

```rust
use system_capability_discovery::{CapabilityIndex, CapabilityQuery, InformationNeed, Term};

fn query(index: &CapabilityIndex) {
    let query = CapabilityQuery::new("social_protection_program")
        .need(InformationNeed::new("farmer_status").requires_any([Term::label("Farmer")]));

    let result = index.search(query).expect("query is valid");
    println!("{}", result.needs.len());
}
```

## Boundary

The matcher does not search natural-language question text, infer synonyms, or
call AI services. Put reviewed labels, field names, standards, or profile terms
in the query.

## Testing

```sh
cargo test -p system-capability-discovery
```

## License

MIT.
