# semantic-asset-discovery-cli

Command-line tools for semantic asset discovery.

The crate builds the `semantic-asset-discovery` binary used by release checks,
fixture review, and offline comparison of discovery reports.

## What It Provides

- Network harvesting from an entry URL.
- Offline bundle analysis from JSON.
- Report comparison for expected discovery output.
- Flags for policy, limits, credentials, output paths, and pretty JSON.

## Typical Use

```sh
cargo run -p semantic-asset-discovery-cli --bin semantic-asset-discovery -- \
  harvest https://example.test/metadata/catalog.json
```

Analyze an existing bundle:

```sh
cargo run -p semantic-asset-discovery-cli --bin semantic-asset-discovery -- \
  analyze-bundle target/discovery-bundle.json
```

Render a service-first explainability view for one public service:

```sh
cargo run -p semantic-asset-discovery-cli --bin semantic-asset-discovery -- \
  service-view https://example.test/services/permit \
  --report target/discovery-report.json
```

`service-view` emits `semantic-asset-discovery.service-view.v1` JSON with:

- the public service asset and declared channels;
- requirements and accepted evidence types;
- evidence providers, form definitions, and route summaries;
- source evidence references for each relation path;
- explicit gaps for missing declared metadata.

Gap entries describe absence in the discovered metadata, not absence in reality.
For example, a missing `registry_manifest:evidenceService` relation means the
report did not declare an access data service for an evidence offering. It does
not prove that no access service exists outside the analyzed artifacts.

## Security Notes

- Treat tokens passed to the CLI as secrets.
- Prefer local fixture bundles for reproducible release checks.
- Use local-development policy only against trusted loopback services.

## Testing

```sh
cargo test -p semantic-asset-discovery-cli
```

## License

MIT.
