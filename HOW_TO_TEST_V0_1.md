# How To Test v0.1

This is the release rehearsal for Registry Atlas, `semantic-asset-discovery`,
`system-capability-discovery`, and the Registry Relay demo integration.

The goal is to prove the v0.1 path end to end:

- Registry Relay publishes standards-shaped metadata at `/metadata/*`.
- Atlas harvests the metadata through `semantic-asset-discovery`.
- The harvested report validates.
- System capability discovery returns candidate routes with evidence, gaps, and
  review flags.
- The Atlas UI can load both the bundled fixture and a live Registry Relay demo.

## 1. Run The Atlas Release Gate

```bash
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-atlas
pnpm check:release
```

Expected result:

- Rust workspace tests pass.
- ESLint passes.
- Vitest passes.
- WASM build passes.
- Production UI build passes.
- Semantic discovery guard checks pass.

Known warnings:

- Vite may warn that the generated bundle is larger than 500 kB. That is not a
  v0.1 release blocker.
- `wasm-pack` may warn about optional Cargo metadata. That is not a v0.1 release
  blocker.

## 2. Run Registry Relay Tests

Focused metadata-core check:

```bash
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry_relay
cargo test -p registry-metadata-core --test metadata_core
```

Full all-features check:

```bash
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry_relay
just test
```

Expected result:

- All runnable tests pass.
- Tests requiring external Postgres or Zitadel may be ignored when their
  environment variables are not configured.

## 3. Start The Registry Relay Demo

Generate local demo keys once if `demo/.env.local` does not exist:

```bash
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry_relay
test -f demo/.env.local || just demo-keys env=demo/.env.local
```

Start the all-standards demo:

```bash
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry_relay
just demo-run demo/config/all_standards.yaml
```

Expected result:

- `http://127.0.0.1:4242/health` returns `{"status":"ok"}`.
- Leave this process running while performing the harvest and UI checks.

## 4. Harvest Live Metadata

Run this from a second shell. The command reads the demo bearer token from
Registry Relay's local env file without printing the token.

```bash
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-atlas
set -a
. /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry_relay/demo/.env.local
set +a

mkdir -p target/live-smoke
CATALOG_VIEWER_RAW="$CATALOG_VIEWER_RAW" \
  cargo run -q -p semantic-asset-discovery-cli -- harvest \
  --allow-private-network \
  --bearer-token-env CATALOG_VIEWER_RAW \
  http://127.0.0.1:4242/metadata \
  > target/live-smoke/registry-relay.envelope.json
```

Extract and validate the nested `DiscoveryReport`:

```bash
jq '.report' target/live-smoke/registry-relay.envelope.json \
  > target/live-smoke/registry-relay.report.json

cargo run -q -p semantic-asset-discovery-cli -- validate-report \
  target/live-smoke/registry-relay.report.json
```

Summarize the live harvest:

```bash
jq '{
  artifact_count: .report.summary.artifact_count,
  asset_count: .report.summary.asset_count,
  failed_artifact_count: .report.summary.failed_artifact_count,
  unsupported_artifact_count: .report.summary.unsupported_artifact_count,
  parse_error_count: .report.summary.parse_error_count
}' target/live-smoke/registry-relay.envelope.json

jq '[.report.assets[].kind] | group_by(.) | map({kind: .[0], count: length})' \
  target/live-smoke/registry-relay.envelope.json
```

Expected result for the current `all_standards` demo:

```json
{
  "artifact_count": 29,
  "asset_count": 168,
  "failed_artifact_count": 0,
  "unsupported_artifact_count": 0,
  "parse_error_count": 0
}
```

Expected asset kinds:

```json
[
  { "kind": "api_description", "count": 1 },
  { "kind": "catalog", "count": 3 },
  { "kind": "class", "count": 22 },
  { "kind": "data_service", "count": 44 },
  { "kind": "dataset", "count": 27 },
  { "kind": "distribution", "count": 44 },
  { "kind": "policy", "count": 27 }
]
```

## 5. Run The Capability Query

```bash
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-atlas
cargo run -q -p system-capability-discovery --bin system-capability-query -- \
  --envelope target/live-smoke/registry-relay.envelope.json \
  --demo-social-protection \
  --pretty \
  > target/live-smoke/social-protection-query.json
```

Summarize the result:

```bash
jq '.needs[] | {
  need_id,
  matches: [.matches[] | {
    confidence,
    route_url: .access.endpoint_url,
    access_kind: .access.kind,
    evidence_count: (.evidence | length),
    gaps,
    review_flags
  }]
}' target/live-smoke/social-protection-query.json
```

Expected route URLs for the current demo:

- `farmer_status`: `http://127.0.0.1:4242/datasets/farmer_registry/farmer`
- `disability_status`: `http://127.0.0.1:4242/datasets/disability_registry/disabled_person`
- `school_attendance`:
  `http://127.0.0.1:4242/datasets/education_registry/attendance_summary`

The output must be candidate-route shaped. It must include confidence, access
kind, evidence, gaps, and review flags. It must not collapse into a simple
key/value URL map.

## 6. Check Canonical Registry Relay Endpoints

```bash
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-atlas
set -a
. /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry_relay/demo/.env.local
set +a

for path in \
  /metadata \
  /metadata/catalog \
  /metadata/dcat/bregdcat-ap \
  /metadata/policies \
  /catalog/dcat-ap.jsonld
do
  http_code=$(/usr/bin/curl -sS -o /tmp/relay-smoke-body.json -w '%{http_code}' \
    -H "Authorization: Bearer $CATALOG_VIEWER_RAW" \
    "http://127.0.0.1:4242$path")
  bytes=$(/usr/bin/wc -c < /tmp/relay-smoke-body.json | /usr/bin/tr -d ' ')
  printf '%s %s bytes=%s\n' "$http_code" "$path" "$bytes"
done
```

Expected result:

```text
200 /metadata
200 /metadata/catalog
200 /metadata/dcat/bregdcat-ap
200 /metadata/policies
404 /catalog/dcat-ap.jsonld
```

The byte counts may change as demo metadata changes.

## 7. Browser Smoke

Start Atlas:

```bash
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-atlas
pnpm dev
```

Open:

```text
http://127.0.0.1:5177
```

Bundled fixture smoke:

1. Load `Bundled Registry Relay discovery`.
2. Confirm the overview shows semantic assets, access methods, recognized
   metadata, and follow-up evidence.
3. Open `Registry` and select a semantic asset.
4. Confirm the detail pane shows access methods for that asset.
5. Open `Capabilities`.
6. Confirm the social protection demo shows farmer status, disability status,
   and school attendance as candidate routes with evidence and review flags.
7. Open `Evidence`.
8. Confirm recognized metadata artifacts and publisher-specific metadata are
   separated.

Live Registry Relay smoke:

1. Keep Registry Relay running on `http://127.0.0.1:4242`.
2. Enter `http://127.0.0.1:4242/metadata`.
3. Paste the `CATALOG_VIEWER_RAW` value into the session bearer token field.
4. Run discovery.
5. Confirm the same UI structure appears for live metadata.

The UI must describe Atlas as a discovery and review workbench. It must not
claim that Registry Relay metadata is an authority decision, source-of-truth
certification, legal access grant, or row-level data query.

## 8. Cleanup

Stop Registry Relay and Atlas with `Ctrl-C`.

The live smoke writes only local files under:

```text
target/live-smoke/
```

Do not commit bearer tokens, raw environment files, terminal transcripts, or
screenshots that contain secrets.
