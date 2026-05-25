import { describe, expect, it } from "vitest";
import {
  analyzeProxyResultWithWasm,
  buildSanitizedFetchedArtifact,
  createSemanticAssetDiscoveryAnalyzer,
  normalizeDiscoveryRunEnvelope,
  normalizeDiscoveryReport,
  normalizeWasmAnalyzeResult,
  redactDiscoveryUrl,
  stripSensitiveHeaders,
  SEMANTIC_ASSET_DISCOVERY_REPORT_SCHEMA_VERSION,
} from "../src/lib";
import type { DiscoveryReport, DiscoveryRequest, DiscoveryRunEnvelope, WasmAnalyzeResult } from "../src/lib";

const analyzedAt = "2026-05-19T00:00:00Z";

describe("semantic asset discovery adapter", () => {
  it("normalizes semantic assets without publisher-specific assumptions", () => {
    const report: DiscoveryReport = minimalReport({
      artifacts: [
        {
          id: "artifact:package",
          url: "https://example.org/publicschema/semantic-asset-package.v1.toml",
          kind: "semantic_model_package",
          status: "fetched",
          title: "PublicSchema package",
        },
      ],
      assets: [
        {
          id: "asset:package",
          kind: "semantic_model_package",
          artifact_id: "artifact:package",
          uri: "https://example.org/publicschema",
          title: "PublicSchema Release",
          description: "A package of model artifacts.",
          publisher: "Example Standards Office",
          endpoint_url: "https://example.org/publicschema",
          conforms_to: ["https://www.w3.org/TR/shacl/"],
          source_hints: [
            {
              label: "Package manifest",
              predicate: "manifest",
              path: "/package",
              artifact_id: "artifact:package",
            },
          ],
          raw_refs: [{ artifact_id: "artifact:package", pointer: "/package", subject_iri: "https://example.org/publicschema" }],
        },
      ],
    });

    const normalized = normalizeDiscoveryReport(report);

    expect(normalized.assets).toEqual([
      expect.objectContaining({
        id: "asset:package",
        kind: "semantic_model_package",
        label: "PublicSchema Release",
        artifactId: "artifact:package",
        artifactUrl: "https://example.org/publicschema/semantic-asset-package.v1.toml",
        artifactTitle: "PublicSchema package",
        publisher: "Example Standards Office",
        conformsTo: ["https://www.w3.org/TR/shacl/"],
      }),
    ]);
    expect(normalized.assets[0]?.sourceHints).toEqual([
      {
        label: "Package manifest",
        artifactId: "artifact:package",
        artifactUrl: "https://example.org/publicschema/semantic-asset-package.v1.toml",
        predicate: "manifest",
        path: "/package",
      },
    ]);
  });

  it("normalizes discovered links with generic evidence labels", () => {
    const report: DiscoveryReport = minimalReport({
      artifacts: [
        {
          id: "artifact:catalog",
          url: "https://catalog.example/data.jsonld",
          kind: "dcat_catalog",
          status: "fetched",
          title: "Research catalog",
        },
      ],
      links: [
        {
          id: "link:shapes",
          from_artifact_id: "artifact:catalog",
          from_url: "https://catalog.example/data.jsonld",
          to_url: "https://catalog.example/shapes.ttl",
          rel: "describedby",
          predicate: "sh:shapesGraph",
          role: "validation",
          confidence: "declared",
          discovered_by: {
            source: "json_ld_predicate",
            artifact_id: "artifact:catalog",
            predicate: "sh:shapesGraph",
            pointer: "/@graph/0/sh:shapesGraph",
            value: "https://catalog.example/shapes.ttl",
          },
        },
      ],
    });

    const normalized = normalizeDiscoveryReport(report);

    expect(normalized.links).toEqual([
      {
        id: "link:shapes",
        fromUrl: "https://catalog.example/data.jsonld",
        toUrl: "https://catalog.example/shapes.ttl",
        label: "describedby",
        confidence: "declared",
        fromArtifactId: "artifact:catalog",
        fromArtifactTitle: "Research catalog",
        fromArtifactUrl: "https://catalog.example/data.jsonld",
        rel: "describedby",
        predicate: "sh:shapesGraph",
        role: "validation",
        evidence: "sh:shapesGraph",
      },
    ]);
  });

  it("reports WASM envelopes as normalized reports or typed errors", () => {
    const okEnvelope: WasmAnalyzeResult = { ok: true, report: minimalReport() };
    const errEnvelope: WasmAnalyzeResult = {
      ok: false,
      error: { code: "analyze.payload_too_large", message: "Analyze input exceeds the WASM body budget" },
    };

    expect(normalizeWasmAnalyzeResult(okEnvelope)).toEqual(expect.objectContaining({ schemaVersion: SEMANTIC_ASSET_DISCOVERY_REPORT_SCHEMA_VERSION }));
    expect(normalizeWasmAnalyzeResult(okEnvelope)).toEqual(expect.objectContaining({ rejectedFetches: [] }));
    expect(normalizeWasmAnalyzeResult(errEnvelope)).toEqual({
      code: "analyze.payload_too_large",
      message: "Analyze input exceeds the WASM body budget",
    });
  });

  it("normalizes the facade DiscoveryRunEnvelope with host fetch state", () => {
    const request: DiscoveryRequest = {
      entry_url: "https://publisher.example/catalog.jsonld",
      policy: "public_web",
      max_depth: 2,
      max_fetches: 50,
      max_body_bytes: 8_388_608,
      max_total_bytes: 67_108_864,
      max_concurrent_fetches: 8,
      timeout_ms: 10_000,
      total_timeout_ms: 120_000,
      user_agent: "registry-atlas-test/0.1",
      accepted_schemes: ["http", "https"],
      allowed_origins: ["https://publisher.example"],
    };
    const envelope: DiscoveryRunEnvelope = {
      report: minimalReport({ entry_url: request.entry_url }),
      fetched: {
        entry_url: request.entry_url,
        fetched_count: 1,
        rejected_count: 1,
        redirect_count: 0,
        total_decompressed_bytes: 512,
        max_total_bytes: request.max_total_bytes,
        max_concurrent_fetches: request.max_concurrent_fetches,
        total_elapsed_ms: 42,
      },
      rejected_fetches: [
        {
          id: "rejected:shape",
          url: redactDiscoveryUrl("https://user:secret@publisher.example/shapes.ttl?access_token=abc123&ok=true"),
          reason_code: "auth.required",
          discovered_from: request.entry_url,
          credential_sent: false,
        },
      ],
    };

    const normalized = normalizeDiscoveryRunEnvelope(envelope);

    expect(normalized.fetched).toEqual(envelope.fetched);
    expect(normalized.rejectedFetches).toEqual([
      {
        id: "rejected:shape",
        url: "https://publisher.example/shapes.ttl?access_token=REDACTED&ok=true",
        reason_code: "auth.required",
        discovered_from: request.entry_url,
        credential_sent: false,
      },
    ]);
    expect(normalized.rejectedFetches[0]?.url).not.toContain("user:secret");
    expect(normalized.rejectedFetches[0]?.url).not.toContain("abc123");
  });

  it("builds sanitized WASM inputs before handoff", () => {
    const artifact = buildSanitizedFetchedArtifact(
      {
        ok: true,
        status: 200,
        statusText: "OK",
        url: "https://user:secret@example.org/catalog.jsonld?api_key=secret&ok=true",
        finalUrl: "https://example.org/catalog.jsonld?access_token=secret&ok=true",
        contentType: "application/ld+json",
        body: "{}",
      },
      [
        { name: "authorization", value: "Bearer secret" },
        { name: "content-type", value: "application/ld+json" },
      ],
      analyzedAt,
    );

    expect(artifact.headers).toEqual([{ name: "content-type", value: "application/ld+json" }]);
    expect(artifact.url).toBe("https://example.org/catalog.jsonld?api_key=REDACTED&ok=true");
    expect(artifact.final_url).toBe("https://example.org/catalog.jsonld?access_token=REDACTED&ok=true");
    expect(artifact.body).toEqual([123, 125]);
    expect(JSON.stringify(artifact)).not.toContain("secret");
    expect(stripSensitiveHeaders([{ name: "Cookie", value: "secret" }])).toEqual([]);
  });

  it("normalizes reports returned by a WASM analyze function", () => {
    const result = analyzeProxyResultWithWasm(
      "https://example.org/catalog.jsonld",
      {
        ok: true,
        status: 200,
        statusText: "OK",
        url: "https://example.org/catalog.jsonld",
        contentType: "application/ld+json",
        body: "{}",
      },
      (inputJson) => {
        const input = JSON.parse(inputJson) as { artifacts: Array<{ headers: unknown[]; body: number[] }> };
        expect(input.artifacts[0]?.headers).toEqual([]);
        expect(input.artifacts[0]?.body).toEqual([123, 125]);
        return JSON.stringify({ ok: true, report: minimalReport() satisfies DiscoveryReport });
      },
    );

    expect(result).toEqual(expect.objectContaining({ schemaVersion: SEMANTIC_ASSET_DISCOVERY_REPORT_SCHEMA_VERSION }));
  });

  it("loads the generated WASM analyze function through a small runtime adapter", async () => {
    const analyzeArtifacts = await createSemanticAssetDiscoveryAnalyzer(async () => ({
      analyzeArtifacts: (inputJson) => JSON.stringify({ ok: true, report: minimalReport({ entry_url: JSON.parse(inputJson).entry_url }) }),
    }));

    expect(analyzeArtifacts(JSON.stringify({ entry_url: "https://example.org/from-wasm" }))).toContain(
      "https://example.org/from-wasm",
    );
    await expect(createSemanticAssetDiscoveryAnalyzer(async () => ({ analyzeArtifacts: undefined as never }))).rejects.toThrow(
      "missing analyzeArtifacts",
    );
  });
});

function minimalReport(overrides: Partial<DiscoveryReport> = {}): DiscoveryReport {
  return {
    schema_version: SEMANTIC_ASSET_DISCOVERY_REPORT_SCHEMA_VERSION,
    run_id: "run:test",
    entry_url: "https://example.org/entry",
    analyzed_at: analyzedAt,
    summary: {
      artifact_count: overrides.artifacts?.length ?? 0,
      asset_count: overrides.assets?.length ?? 0,
      standard_count: overrides.standards?.length ?? 0,
      profile_count: overrides.profiles?.length ?? 0,
      failed_artifact_count: 0,
      unsupported_artifact_count: 0,
      parse_error_count: 0,
      next_fetch_count: overrides.next_fetches?.length ?? 0,
      truncated: false,
    },
    artifacts: [],
    assets: [],
    relations: [],
    relation_claims: [],
    links: [],
    standards: [],
    profiles: [],
    findings: [],
    next_fetches: [],
    ...overrides,
  };
}
