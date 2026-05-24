# semantic-asset-discovery

Bounded semantic asset discovery client for Registry Atlas.

This crate fetches standards-facing registry artifacts, applies a discovery
policy, and passes fetched artifacts to `semantic-asset-discovery-core` for
analysis.

## What It Provides

- `DiscoveryRequest` and `DiscoveryClientBuilder` for configuring discovery.
- `DiscoveryPolicy::public_web` and `DiscoveryPolicy::local_development`.
- `Credentials` for bearer or custom API-key headers with origin restrictions.
- `DiscoveryFetcher` for replacing network fetches in tests or host runtimes.
- `DiscoveryBundle` for offline analysis of already-fetched artifacts.
- Report views for registry assets, graph edges, evidence, conditions, and
  substrate layers.

## Typical Use

```rust
use semantic_asset_discovery::{DiscoveryClient, DiscoveryRequest};

async fn discover(entry_url: &str) -> Result<(), semantic_asset_discovery::DiscoveryError> {
    let client = DiscoveryClient::builder().build()?;
    let run = client
        .discover_request(DiscoveryRequest::new(entry_url))
        .await?;

    for dataset in run.registry().datasets() {
        println!("{} {:?}", dataset.id(), dataset.title());
    }

    Ok(())
}
```

## Policy Notes

- Public-web discovery is the default.
- Local-development discovery is for loopback demos and tests.
- Discovery enforces depth, fetch count, per-body byte, total byte,
  concurrency, redirect, and timeout limits.
- Credential forwarding is origin-scoped. Do not use bearer tokens with a broad
  origin allowlist.

## Testing

```sh
cargo test -p semantic-asset-discovery
```

## License

MIT.
