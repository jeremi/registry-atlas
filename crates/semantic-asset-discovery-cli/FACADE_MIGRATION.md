# CLI Facade Integration Notes

The CLI `harvest` command now uses the `semantic-asset-discovery` facade crate.
Online harvest output is a `DiscoveryRunEnvelope` with:

- `report`: the canonical `DiscoveryReport`;
- `fetched`: host fetch summary;
- `rejected_fetches`: host policy, authentication, fetch, and size rejections.

`analyze`, `analyze-bundle`, and `validate-report` remain raw
`DiscoveryReport` commands because they operate on local or pre-fetched core
inputs.

## Current Contract

- `harvest` routes through `DiscoveryClient::builder()`.
- `--allow-private-network` opts into `DiscoveryPolicy::local_development()`;
  public-web policy is the default.
- `--max-depth`, `--max-fetches`, `--max-body-bytes`, `--max-total-bytes`,
  `--max-concurrent-fetches`, `--timeout-ms`, and `--total-timeout-ms` map to
  facade request limits.
- `validate-report` validates raw `DiscoveryReport` JSON and rejects unsupported
  `schema_version` values.

## Accepted v0.1 Limit

The Rust facade currently executes the harvest queue sequentially while carrying
`max_concurrent_fetches` in the request and envelope contract. Parallel queue
scheduling is a v0.1.x optimization.
