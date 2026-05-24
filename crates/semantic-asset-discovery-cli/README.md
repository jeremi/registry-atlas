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
