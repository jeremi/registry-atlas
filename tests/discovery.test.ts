import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it, vi } from "vitest";
import { buildGraph, discoverAtlas, GRAPH_NODE_BUDGET, proxyFetch } from "../src/lib";
import type { AtlasRecord } from "../src/lib";

const fixturePath = join(process.cwd(), "public/fixtures/registry-relay-dcat-ap.jsonld");
const fixtureText = readFileSync(fixturePath, "utf8");

describe("discovery helper", () => {
  it("uses the semantic analyzer and follows declared next fetches", async () => {
    const semanticAnalyzer = vi.fn((inputJson: string) => {
      const input = JSON.parse(inputJson) as { artifacts: unknown[] };
      return JSON.stringify({
        ok: true,
        report: semanticReport(input.artifacts.length),
      });
    });
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const url = decodeURIComponent(String(input));
      if (url.includes("dataset.jsonld")) {
        return proxyResponse("https://example.gov/dataset.jsonld", { "@id": "https://example.gov/datasets/one" });
      }
      return proxyResponse("https://example.gov/catalog.jsonld", { "@id": "https://example.gov/catalog" });
    });

    const model = await discoverAtlas("https://example.gov/catalog.jsonld", {
      fetcher,
      semanticAnalyzer,
      maxSemanticFetches: 2,
    });

    expect(semanticAnalyzer).toHaveBeenCalledTimes(2);
    expect(fetcher).toHaveBeenCalledWith(expect.stringContaining(encodeURIComponent("https://example.gov/dataset.jsonld")), expect.anything());
    expect(model.catalogTitle).toBe("Semantic test catalog");
    expect(model.records).toEqual(expect.arrayContaining([expect.objectContaining({ name: "Semantic test dataset", type: "dataset" })]));
    expect((model as { semanticDiscovery?: unknown }).semanticDiscovery).toBeDefined();
  });

  it("keeps semantic discovery results when a follow-up fetch fails", async () => {
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("missing-dataset.jsonld")) {
        throw new Error("offline");
      }
      return proxyResponse("https://registry.example.gov/metadata/dcat/bregdcat-ap", JSON.parse(fixtureText) as unknown);
    });
    const semanticAnalyzer = vi.fn((inputJson: string) => {
      const input = JSON.parse(inputJson) as { artifacts: Array<{ status: number; url: string }> };
      return JSON.stringify({
        ok: true,
        report: semanticReport(input.artifacts.length, input.artifacts),
      });
    });

    const model = await discoverAtlas("https://registry.example.gov/metadata/dcat/bregdcat-ap", {
      fetcher,
      semanticAnalyzer,
      bearerToken: "secret-token",
      maxSemanticFetches: 1,
    });

    expect(fetcher).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining(encodeURIComponent("https://registry.example.gov/metadata/dcat/bregdcat-ap")),
      expect.objectContaining({ headers: { "x-atlas-bearer": "secret-token" } }),
    );
    expect(fetcher).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining(encodeURIComponent("https://example.gov/missing-dataset.jsonld")),
      expect.objectContaining({ headers: undefined }),
    );
    expect(model.catalogTitle).toBe("Semantic test catalog");
    expect(model.artifacts).toEqual(expect.arrayContaining([expect.objectContaining({ id: "artifact-follow-up", presence: "invalid" })]));
    expect(model.semanticDiscovery?.rejectedFetches).toEqual([
      expect.objectContaining({
        url: "https://example.gov/missing-dataset.jsonld",
        reason_code: "fetch.failed",
        credential_sent: false,
      }),
    ]);
  });

  it("preserves proxy policy rejection reason for rejected semantic follow-ups", async () => {
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("missing-dataset.jsonld")) {
        return jsonResponse({
          ok: false,
          status: 400,
          statusText: "",
          url: "https://private.example/metadata?api_key=REDACTED",
          finalUrl: "https://private.example/metadata?api_key=REDACTED",
          error: {
            code: "private_network_blocked",
            message: "Private-network targets are blocked by this proxy.",
          },
        });
      }
      return proxyResponse("https://registry.example.gov/metadata/dcat/bregdcat-ap?api_key=REDACTED", JSON.parse(fixtureText) as unknown);
    });
    const semanticAnalyzer = vi.fn((inputJson: string) => {
      const input = JSON.parse(inputJson) as { entry_url: string; artifacts: Array<{ url: string }> };
      expect(input.entry_url).not.toContain("secret");
      expect(input.artifacts.every((artifact) => !artifact.url.includes("secret"))).toBe(true);
      return JSON.stringify({
        ok: true,
        report: semanticReport(input.artifacts.length, [
          { status: 200, url: "https://registry.example.gov/metadata/dcat/bregdcat-ap" },
        ]),
      });
    });

    const model = await discoverAtlas("https://registry.example.gov/metadata/dcat/bregdcat-ap?api_key=secret", {
      fetcher,
      semanticAnalyzer,
      maxSemanticFetches: 1,
    });

    expect(model.semanticDiscovery?.rejectedFetches).toEqual([
      expect.objectContaining({
        reason_code: "private_network_blocked",
        url: "https://example.gov/missing-dataset.jsonld",
      }),
    ]);
  });

  it("marks auth-gated catalog artifacts honestly", async () => {
    const fetcher = vi.fn(async () => new Response("denied", { status: 403, statusText: "Forbidden" }));

    const model = await discoverAtlas("https://registry.example.gov/metadata/dcat/bregdcat-ap", { fetcher });

    expect(model.artifacts).toEqual(
      expect.arrayContaining([expect.objectContaining({ id: "dcat-ap-jsonld", presence: "auth-required" })]),
    );
  });

  it("returns proxy fetch metadata without logging or persisting tokens", async () => {
    const fetcher = vi.fn(async () => jsonResponse({ ok: true }));

    const result = await proxyFetch("https://example.gov/catalog", { bearerToken: "secret-token", fetcher });

    expect(fetcher).toHaveBeenCalledWith(
      "/api/proxy?url=https%3A%2F%2Fexample.gov%2Fcatalog",
      expect.objectContaining({ headers: { "x-atlas-bearer": "secret-token" } }),
    );
    expect(result.json).toEqual({ ok: true });
    expect(result.body).not.toContain("secret-token");
  });

  it("uses detailed semantic policy signals for policy readiness", async () => {
    const fetcher = vi.fn(async () =>
      proxyResponse("https://registry.example.gov/metadata/dcat/bregdcat-ap", { "@id": "https://registry.example.gov/catalog" }),
    );
    const semanticAnalyzer = vi.fn(() =>
      JSON.stringify({
        ok: true,
        report: policySemanticReport(),
      }),
    );

    const model = await discoverAtlas("https://registry.example.gov/metadata/dcat/bregdcat-ap", {
      fetcher,
      semanticAnalyzer,
      maxSemanticFetches: 0,
    });

    expect(model.artifacts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: "odrl", presence: "found", assessment: "complete" }),
        expect.objectContaining({ id: "access-rights", presence: "found", assessment: "complete" }),
        expect.objectContaining({ id: "dpv", presence: "found", sourceStandard: "DCAT-AP" }),
      ]),
    );
    expect(model.missingItems).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: "policy-access-rights", status: "known" }),
        expect.objectContaining({ id: "policy-legal-basis", status: "known" }),
      ]),
    );
    expect(model.readiness.find((category) => category.id === "policy")?.status).toBe("ready");
  });
});

describe("graph budget", () => {
  it("summarizes instead of rendering above the 1,500 node budget", () => {
    const records: AtlasRecord[] = Array.from({ length: GRAPH_NODE_BUDGET + 2 }, (_, index) => ({
      id: `record-${index}`,
      type: index === 0 ? "catalog" : "dataset",
      name: `Record ${index}`,
      validation: "not-run",
      readiness: "not-checked",
      serviceCount: 0,
      fields: [],
      publisherFields: [],
      raw: {},
      parentId: index === 0 ? undefined : "record-0",
    }));

    const graph = buildGraph(records);

    expect(graph.nodes).toHaveLength(GRAPH_NODE_BUDGET);
    expect(graph.summarized).toBe(true);
    expect(graph.summary[0]).toContain(`${GRAPH_NODE_BUDGET}`);
  });
});

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    statusText: "OK",
    headers: { "content-type": "application/json" },
  });
}

function proxyResponse(url: string, upstreamJson: unknown): Response {
  return jsonResponse({
    ok: true,
    status: 200,
    statusText: "OK",
    url,
    finalUrl: url,
    contentType: "application/ld+json",
    body: JSON.stringify(upstreamJson),
    json: upstreamJson,
  });
}

function semanticReport(artifactCount: number, artifacts: Array<{ status: number; url: string }> = []): unknown {
  const nextFetches =
    artifactCount === 1
      ? [
          {
            id: "fetch-dataset",
            url: artifacts.length > 0 ? "https://example.gov/missing-dataset.jsonld" : "https://example.gov/dataset.jsonld",
            depth: 1,
            priority: 10,
            reason: "dcat:dataset",
            discovered_from: "https://example.gov/catalog.jsonld",
            discovered_by: {
              source: "json_ld_predicate",
              artifact_id: "artifact-catalog",
              predicate: "dcat:dataset",
              value: "https://example.gov/dataset.jsonld",
            },
          },
        ]
      : [];

  return {
    schema_version: "semantic-asset-discovery.report.v1",
    run_id: "run-test",
    entry_url: "https://example.gov/catalog.jsonld",
    analyzed_at: "2026-05-19T00:00:00Z",
    summary: {
      artifact_count: artifactCount,
      asset_count: 2,
      standard_count: 0,
      profile_count: 1,
      failed_artifact_count: 0,
      unsupported_artifact_count: 0,
      parse_error_count: 0,
      next_fetch_count: nextFetches.length,
      truncated: false,
    },
    artifacts: [
      {
        id: "artifact-catalog",
        url: "https://example.gov/catalog.jsonld",
        kind: "dcat_catalog",
        status: "fetched",
        title: "Semantic test catalog",
        analyzed_at: "2026-05-19T00:00:00Z",
      },
      ...(artifactCount > 1
        ? [
            {
              id: "artifact-follow-up",
              url: artifacts[1]?.url ?? "https://example.gov/missing-dataset.jsonld",
              kind: "unknown",
              status: artifacts[1]?.status === 0 ? "failed" : "fetched",
              title: "Follow-up artifact",
              error: "offline",
              analyzed_at: "2026-05-19T00:00:00Z",
            },
          ]
        : []),
    ],
    assets: [
      {
        id: "asset-catalog",
        kind: "catalog",
        artifact_id: "artifact-catalog",
        uri: "https://example.gov/catalog",
        title: "Semantic test catalog",
        conforms_to: ["https://semiceu.github.io/DCAT-AP/releases/3.0.0/"],
        source_hints: [],
        raw_refs: [],
      },
      {
        id: "asset-dataset",
        kind: "dataset",
        artifact_id: "artifact-catalog",
        uri: "https://example.gov/datasets/one",
        title: "Semantic test dataset",
        conforms_to: [],
        source_hints: [],
        raw_refs: [],
      },
    ],
    links: [],
    standards: [],
    profiles: [
      {
        id: "profile-dcat-ap",
        iri: "https://semiceu.github.io/DCAT-AP/releases/3.0.0/",
        label: "DCAT-AP",
        claimed_by_artifact_id: "artifact-catalog",
        evidence: {
          source: "json_ld_predicate",
          artifact_id: "artifact-catalog",
          predicate: "dcterms:conformsTo",
          value: "https://semiceu.github.io/DCAT-AP/releases/3.0.0/",
        },
      },
    ],
    findings: [],
    next_fetches: nextFetches,
  };
}

function policySemanticReport(): unknown {
  return {
    schema_version: "semantic-asset-discovery.report.v1",
    run_id: "run-policy-test",
    entry_url: "https://registry.example.gov/metadata/dcat/bregdcat-ap",
    analyzed_at: "2026-05-19T00:00:00Z",
    summary: {
      artifact_count: 1,
      asset_count: 3,
      standard_count: 0,
      profile_count: 0,
      failed_artifact_count: 0,
      unsupported_artifact_count: 0,
      parse_error_count: 0,
      next_fetch_count: 0,
      truncated: false,
    },
    artifacts: [
      {
        id: "artifact-catalog",
        url: "https://registry.example.gov/metadata/dcat/bregdcat-ap",
        kind: "dcat_catalog",
        status: "fetched",
        title: "Policy catalog",
        analyzed_at: "2026-05-19T00:00:00Z",
      },
    ],
    assets: [
      {
        id: "asset-catalog",
        kind: "catalog",
        artifact_id: "artifact-catalog",
        uri: "https://registry.example.gov/catalog",
        title: "Policy catalog",
        conforms_to: [],
        source_hints: [],
        raw_refs: [],
      },
      {
        id: "asset-dataset",
        kind: "dataset",
        artifact_id: "artifact-catalog",
        uri: "https://registry.example.gov/datasets/farmers",
        title: "Farmers",
        conforms_to: [],
        source_hints: [],
        raw_refs: [],
      },
      {
        id: "asset-policy",
        kind: "policy",
        artifact_id: "artifact-catalog",
        uri: "https://registry.example.gov/datasets/farmers#offer",
        title: "Access policy",
        conforms_to: ["https://example.gov/odrl/profile/government-data-sharing"],
        source_hints: [
          { artifact_id: "artifact-catalog", label: "odrl:prohibition", predicate: "odrl:prohibition" },
          { artifact_id: "artifact-catalog", label: "odrl:constraint", predicate: "odrl:constraint" },
        ],
        raw_refs: [],
      },
    ],
    links: [],
    standards: [],
    profiles: [],
    findings: [
      standardSignal("dcterms:accessRights", "http://publications.europa.eu/resource/authority/access-right/NON_PUBLIC"),
      standardSignal("dcatap:applicableLegislation", "https://example.gov/legislation/data-sharing"),
    ],
    next_fetches: [],
  };
}

function standardSignal(predicate: string, value: string): unknown {
  return {
    id: `finding-${predicate}`,
    severity: "info",
    code: "semantic.standard_signal",
    message: `Standard semantic signal ${predicate}`,
    artifact_id: "artifact-catalog",
    asset_id: "asset-dataset",
    standard_iri: null,
    evidence: {
      source: "json_ld_predicate",
      artifact_id: "artifact-catalog",
      predicate,
      pointer: null,
      value,
    },
  };
}
