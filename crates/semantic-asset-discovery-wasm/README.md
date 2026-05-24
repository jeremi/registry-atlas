# semantic-asset-discovery-wasm

WebAssembly wrapper for `semantic-asset-discovery-core`.

The Atlas UI uses this crate to analyze fetched artifact bundles in a browser
runtime while preserving the same report shape as the Rust core.

## What It Provides

- `version()` for the report schema version.
- `analyzeArtifacts(inputJson)` for JSON-in, JSON-out analysis.
- Panic isolation and structured error envelopes for invalid input, oversized
  payloads, and unexpected failures.

## Host Contract

Input is a JSON-serialized `AnalyzeInput`. Output is a JSON-serialized
`WasmAnalyzeResult` envelope:

```json
{ "ok": true, "report": {} }
```

or:

```json
{ "ok": false, "error": { "code": "analyze.invalid_input", "message": "..." } }
```

## Testing

```sh
cargo test -p semantic-asset-discovery-wasm
pnpm build:wasm
```

## License

MIT.
