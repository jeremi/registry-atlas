import { artifactStatus } from "./artifacts";
import { buildComparison } from "./comparison";
import { buildGraph } from "./graph";
import { parseDcatJsonLd } from "./parser";
import { buildMissingItems, buildReadiness } from "./readiness";
import {
  buildSanitizedFetchedArtifact,
  normalizeDiscoveryRunEnvelope,
  parseWasmAnalyzeResult,
  redactDiscoveryUrl,
  SEMANTIC_ASSET_DISCOVERY_REPORT_SCHEMA_VERSION,
  type AnalyzeArtifactsJson,
  type DiscoveryEvidence,
  type DiscoveryFinding,
  type DiscoveryRunEnvelope,
  type FetchedArtifact,
  type FetchSummary,
  type DiscoveredArtifact,
  type DiscoveryReport,
  type FetchCandidate,
  type RejectedFetch,
  type SemanticAsset,
  type SemanticAssetKind,
} from "./semanticAssetDiscovery";
import type {
  AtlasModel,
  AtlasRecord,
  FieldValue,
  ProfileId,
  ProxyFetchResult,
  RecordType,
} from "./types";

const DEFAULT_ATLAS_SEMANTIC_FETCHES = 64;
const DEFAULT_ATLAS_SEMANTIC_CANDIDATES = 128;

export interface DiscoveryOptions {
  profile?: ProfileId;
  bearerToken?: string;
  fetcher?: typeof fetch;
  semanticAnalyzer?: AnalyzeArtifactsJson;
  maxSemanticFetches?: number;
}

export async function discoverAtlas(sourceUrl: string, options: DiscoveryOptions = {}): Promise<AtlasModel> {
  const fetcher = options.fetcher ?? fetch;
  const catalogResult = await proxyFetch(sourceUrl, { bearerToken: options.bearerToken, fetcher });
  if (catalogResult.status === 401 || catalogResult.status === 403) {
    const model = parseDcatJsonLd(emptyCatalog(sourceUrl), {
      sourceUrl,
      profile: options.profile,
    });
    model.artifacts = model.artifacts.map((artifact) =>
      artifact.id === "dcat-ap-jsonld"
        ? {
            ...artifact,
            presence: "auth-required",
            microcopy: "The endpoint requires credentials. Add a session token. Defined by the selected standards profile.",
          }
        : artifact,
    );
    return model;
  }

  if (!catalogResult.ok) {
    throw new Error(catalogResult.error ?? `Failed to fetch ${sourceUrl}: ${catalogResult.status} ${catalogResult.statusText}`);
  }

  return await discoverSemanticAtlas(sourceUrl, catalogResult, options, fetcher);
}

async function discoverSemanticAtlas(
  sourceUrl: string,
  initialResult: ProxyFetchResult,
  options: DiscoveryOptions,
  fetcher: typeof fetch,
): Promise<AtlasModel> {
  const startedAt = Date.now();
  const analyzer = options.semanticAnalyzer ?? (await loadDefaultSemanticAnalyzer());
  const artifacts = [buildSanitizedFetchedArtifact(initialResult)];
  const rejectedFetches: RejectedFetch[] = [];
  const seenUrls = new Set<string>([initialResult.url, initialResult.finalUrl ?? initialResult.url]);
  const maxFetches = options.maxSemanticFetches ?? DEFAULT_ATLAS_SEMANTIC_FETCHES;
  let report = analyzeWithSemanticEngine(sourceUrl, artifacts, analyzer);

  for (let index = 0; index < maxFetches; index += 1) {
    const next = nextFetch(report.next_fetches, seenUrls);
    if (!next) {
      break;
    }
    seenUrls.add(next.url);
    const candidateResult = await fetchSemanticCandidate(next, sourceUrl, options.bearerToken, fetcher);
    artifacts.push(candidateResult.artifact);
    if (candidateResult.rejectedFetch) {
      rejectedFetches.push(candidateResult.rejectedFetch);
    }
    report = analyzeWithSemanticEngine(sourceUrl, artifacts, analyzer);
  }

  return semanticReportToAtlasModel(
    {
      report,
      fetched: buildFetchSummary(sourceUrl, artifacts, rejectedFetches, Date.now() - startedAt),
      rejected_fetches: rejectedFetches,
    },
    options.profile,
  );
}

interface SemanticCandidateFetchResult {
  artifact: FetchedArtifact;
  rejectedFetch?: RejectedFetch;
}

async function fetchSemanticCandidate(
  candidate: FetchCandidate,
  entryUrl: string,
  bearerToken: string | undefined,
  fetcher: typeof fetch,
): Promise<SemanticCandidateFetchResult> {
  const scopedBearerToken = credentialForSemanticCandidate(entryUrl, candidate.url, bearerToken);
  const credentialSent = Boolean(scopedBearerToken);
  try {
    const fetched = await proxyFetch(candidate.url, { bearerToken: scopedBearerToken, fetcher });
    const artifact = {
      ...buildSanitizedFetchedArtifact(fetched),
      discovered_from: candidate.discovered_from,
      discovered_by: candidate.discovered_by,
      depth: candidate.depth,
    };
    const rejectedFetch = rejectedFetchForProxyResult(candidate, fetched, credentialSent);
    return rejectedFetch ? { artifact, rejectedFetch } : { artifact };
  } catch (error) {
    return {
      artifact: failedFetchedArtifact(candidate, error),
      rejectedFetch: buildRejectedFetch(candidate, "fetch.failed", credentialSent),
    };
  }
}

function failedFetchedArtifact(candidate: FetchCandidate, error: unknown): FetchedArtifact {
  return {
    url: candidate.url,
    final_url: null,
    status: 0,
    media_type: "application/problem+json",
    request_accept: null,
    redirect_chain: [],
    headers: [],
    body: Array.from(new TextEncoder().encode(JSON.stringify({ ok: false, error: errorMessage(error) }))),
    fetched_at: new Date().toISOString(),
    depth: candidate.depth,
    discovered_from: candidate.discovered_from,
    discovered_by: candidate.discovered_by as DiscoveryEvidence,
  };
}

function rejectedFetchForProxyResult(
  candidate: FetchCandidate,
  result: ProxyFetchResult,
  credentialSent: boolean,
): RejectedFetch | undefined {
  if (result.status === 401) {
    return buildRejectedFetch(candidate, "auth.required", credentialSent);
  }
  if (result.status === 403) {
    return buildRejectedFetch(candidate, "auth.rejected", credentialSent);
  }
  if (!result.ok) {
    return buildRejectedFetch(candidate, result.errorCode ?? "fetch.failed", credentialSent);
  }
  return undefined;
}

function buildRejectedFetch(candidate: FetchCandidate, reasonCode: string, credentialSent: boolean): RejectedFetch {
  return {
    id: `rejected:${candidate.id}`,
    url: redactDiscoveryUrl(candidate.url),
    reason_code: reasonCode,
    discovered_from: candidate.discovered_from,
    credential_sent: credentialSent,
  };
}

function credentialForSemanticCandidate(entryUrl: string, candidateUrl: string, bearerToken: string | undefined): string | undefined {
  if (!bearerToken) {
    return undefined;
  }
  try {
    return new URL(entryUrl).origin === new URL(candidateUrl).origin ? bearerToken : undefined;
  } catch {
    return undefined;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function analyzeWithSemanticEngine(sourceUrl: string, artifacts: FetchedArtifact[], analyzer: AnalyzeArtifactsJson): DiscoveryReport {
  const envelope = parseWasmAnalyzeResult(
    analyzer(
      JSON.stringify({
        entry_url: redactDiscoveryUrl(sourceUrl),
        analyzed_at: new Date().toISOString(),
        artifacts,
        options: {
          max_next_fetches: DEFAULT_ATLAS_SEMANTIC_CANDIDATES,
          include_inferred_links: true,
          accepted_schemes: [],
          enabled_profiles: [],
        },
      }),
    ),
  );
  if (!envelope.ok) {
    const error = new Error(envelope.error.message);
    error.name = envelope.error.code;
    throw error;
  }
  return envelope.report;
}

function nextFetch(candidates: FetchCandidate[], seenUrls: Set<string>): FetchCandidate | undefined {
  return candidates
    .filter((candidate) => !seenUrls.has(candidate.url))
    .sort((left, right) => left.priority - right.priority || left.url.localeCompare(right.url))[0];
}

let analyzerPromise: Promise<AnalyzeArtifactsJson> | undefined;

async function loadDefaultSemanticAnalyzer(): Promise<AnalyzeArtifactsJson> {
  analyzerPromise ??= import("../wasm/semantic-asset-discovery/semantic_asset_discovery.js").then(async (module) => {
    await module.default();
    const version = module.version();
    if (version !== SEMANTIC_ASSET_DISCOVERY_REPORT_SCHEMA_VERSION) {
      throw new Error(`semantic-asset-discovery version mismatch: ${version}`);
    }
    return module.analyzeArtifacts;
  });
  return analyzerPromise;
}

function semanticReportToAtlasModel(runEnvelope: DiscoveryRunEnvelope, requestedProfile?: ProfileId): AtlasModel {
  const report = runEnvelope.report;
  const normalizedReport = normalizeDiscoveryRunEnvelope(runEnvelope);
  const records = semanticAssetsToRecords(report.assets);
  const artifacts = [...report.artifacts.map(semanticArtifactStatus), ...semanticAssetEvidenceStatuses(report.assets, report.findings)];
  const openApi = semanticOpenApiSummary(report.assets);
  const missingItems = buildMissingItems(records, {
    artifacts,
    hasOpenApi: Boolean(openApi),
    validationStatus: "not-run",
  });
  const profile = requestedProfile ?? inferSemanticProfile(report);
  const catalog = records.find((record) => record.type === "catalog") ?? records[0];
  const catalogTitle = catalog?.name ?? report.entry_url;

  return {
    sourceUrl: report.entry_url,
    profile,
    catalogTitle,
    participantId: catalog?.publisher,
    records,
    artifacts,
    missingItems,
    readiness: buildReadiness(missingItems, artifacts, "not-run"),
    graph: buildGraph(records),
    rawCatalog: report,
    openApi,
    comparison: buildComparison(records),
    validation: {
      status: "not-run",
      message: `Semantic discovery analyzed ${report.summary.artifact_count} artifact${report.summary.artifact_count === 1 ? "" : "s"} and found ${report.summary.asset_count} semantic asset${report.summary.asset_count === 1 ? "" : "s"}.`,
    },
    semanticDiscovery: normalizedReport,
    discoveryEngine: "semantic-asset-discovery",
  };
}

export function discoveryRunEnvelopeToAtlasModel(runEnvelope: DiscoveryRunEnvelope, requestedProfile?: ProfileId): AtlasModel {
  return semanticReportToAtlasModel(runEnvelope, requestedProfile);
}

function buildFetchSummary(
  entryUrl: string,
  artifacts: FetchedArtifact[],
  rejectedFetches: RejectedFetch[],
  totalElapsedMs: number,
): FetchSummary {
  return {
    entry_url: entryUrl,
    fetched_count: artifacts.filter((artifact) => artifact.status > 0).length,
    rejected_count: rejectedFetches.length,
    redirect_count: artifacts.reduce((count, artifact) => count + artifact.redirect_chain.length, 0),
    total_decompressed_bytes: artifacts.reduce((count, artifact) => count + artifactBodyLength(artifact), 0),
    max_total_bytes: 67_108_864,
    max_concurrent_fetches: 1,
    total_elapsed_ms: totalElapsedMs,
  };
}

function artifactBodyLength(artifact: FetchedArtifact): number {
  if (typeof artifact.body === "string") {
    return new TextEncoder().encode(artifact.body).length;
  }
  return artifact.body.length;
}

function semanticAssetsToRecords(assets: SemanticAsset[]): AtlasRecord[] {
  const catalogAsset = assets.find((asset) => asset.kind === "catalog") ?? assets.find((asset) => asset.kind === "semantic_model_package");
  const catalogId = catalogAsset ? semanticRecordId(catalogAsset) : undefined;
  const datasetIdsByUri = new Map(
    assets
      .filter((asset) => asset.kind === "dataset" || asset.kind === "class")
      .map((asset) => [asset.uri, semanticRecordId(asset)] as const)
      .filter((entry): entry is [string, string] => Boolean(entry[0])),
  );
  const records = assets
    .map((asset) => semanticAssetToRecord(asset, catalogId, datasetIdsByUri))
    .filter((record): record is AtlasRecord => Boolean(record));

  if (records.length > 0 && !records.some((record) => record.type === "catalog")) {
    return [
      {
        id: `${records[0].id}/catalog`,
        type: "catalog",
        name: "Semantic asset package",
        validation: "not-run",
        readiness: "not-checked",
        serviceCount: records.filter((record) => isAccessType(record.type)).length,
        fields: [],
        publisherFields: [],
        raw: {},
      },
      ...records,
    ];
  }

  return records.map((record) =>
    record.type === "catalog"
      ? {
          ...record,
          serviceCount: records.filter((candidate) => isAccessType(candidate.type)).length,
        }
      : record,
  );
}

function semanticAssetToRecord(asset: SemanticAsset, catalogId: string | undefined, datasetIdsByUri: Map<string, string>): AtlasRecord | undefined {
  const type = semanticRecordType(asset.kind);
  if (!type) {
    return undefined;
  }
  const id = semanticRecordId(asset);
  const parentId = semanticParentId(asset, type, catalogId, datasetIdsByUri);
  const fields = semanticFields(asset);

  return {
    id,
    type,
    name: asset.title ?? asset.uri ?? id,
    publisher: asset.publisher ?? undefined,
    profile: asset.conforms_to[0],
    validation: "not-run",
    readiness: "not-checked",
    serviceCount: 0,
    fields,
    publisherFields: [],
    raw: asset,
    parentId: parentId === id ? undefined : parentId,
    conformsTo: asset.conforms_to,
  };
}

function semanticParentId(
  asset: SemanticAsset,
  type: RecordType,
  catalogId: string | undefined,
  datasetIdsByUri: Map<string, string>,
): string | undefined {
  if (type === "dataset" || type === "base-registry") {
    return catalogId;
  }
  if (isAccessType(type)) {
    return nearestDatasetId(asset, datasetIdsByUri) ?? catalogId;
  }
  return undefined;
}

function nearestDatasetId(asset: SemanticAsset, datasetIdsByUri: Map<string, string>): string | undefined {
  const candidates = [asset.uri, asset.endpoint_url].filter((value): value is string => Boolean(value));
  let best: { uri: string; id: string } | undefined;
  for (const [uri, id] of datasetIdsByUri.entries()) {
    if (candidates.some((candidate) => candidate === uri || candidate.startsWith(`${uri}/`) || candidate.startsWith(`${uri}#`))) {
      if (!best || uri.length > best.uri.length) {
        best = { uri, id };
      }
    }
  }
  return best?.id;
}

function semanticRecordType(kind: SemanticAssetKind): RecordType | undefined {
  switch (kind) {
    case "catalog":
    case "semantic_model_package":
      return "catalog";
    case "dataset":
    case "class":
      return "dataset";
    case "data_service":
      return "service";
    case "distribution":
      return "distribution";
    case "record_collection":
      return "ogc-record-collection";
    case "feature_collection":
      return "ogc-feature-collection";
    case "api_description":
      return "operation-group";
    default:
      return undefined;
  }
}

function semanticRecordId(asset: SemanticAsset): string {
  return asset.id;
}

function semanticFields(asset: SemanticAsset): FieldValue[] {
  const values: Array<[string, string, string | undefined, string]> = [
    ["uri", "URI", asset.uri ?? undefined, "semantic:uri"],
    ["title", "Title", asset.title ?? undefined, "dcterms:title"],
    ["description", "Description", asset.description ?? undefined, "dcterms:description"],
    ["publisher", "Publisher", asset.publisher ?? undefined, "dcterms:publisher"],
    ["endpoint", "Endpoint URL", asset.endpoint_url ?? undefined, "dcat:endpointURL"],
    ["conformsTo", "Conforms to", asset.conforms_to.join(", ") || undefined, "dcterms:conformsTo"],
  ];

  return values
    .filter(([, , value]) => Boolean(value))
    .map(([id, label, value, term]) => ({
      id,
      label,
      value: value ?? "",
      source: {
        label: asset.source_hints[0]?.label ?? "Semantic discovery",
        term,
        artifactId: asset.artifact_id,
        url: asset.uri ?? undefined,
      },
    }));
}

function semanticArtifactStatus(artifact: DiscoveredArtifact) {
  const presence = artifact.status === "fetched" ? "found" : artifact.status === "auth_required" ? "auth-required" : artifact.status === "parse_error" ? "invalid" : artifact.status === "failed" ? "invalid" : "missing";
  return artifactStatus({
    id: artifact.id,
    name: artifact.title ?? artifactKindLabel(artifact.kind),
    presence,
    origin: artifact.kind === "unknown" || artifact.status === "unsupported" ? "unsupported" : "standard",
    sourceStandard: artifactKindLabel(artifact.kind),
    url: artifact.url,
    assessment: artifact.status === "fetched" ? "complete" : artifact.status === "parse_error" ? "not-parsed" : "partial",
    error: artifact.error ?? undefined,
  });
}

function semanticAssetEvidenceStatuses(assets: SemanticAsset[], findings: DiscoveryFinding[]) {
  const statuses = [];
  if (assets.some((asset) => asset.kind === "policy")) {
    statuses.push(artifactStatus({
      id: "odrl",
      name: "ODRL policy or offer",
      presence: "found",
      origin: "standard",
      sourceStandard: "ODRL",
      assessment: hasDetailedPolicySignal(assets, findings) ? "complete" : "partial",
    }));
  }
  if (hasStandardSignal(findings, ["dcterms:accessRights", "dcterms:rights", "dcterms:license"])) {
    statuses.push(artifactStatus({
      id: "access-rights",
      name: "Access rights statement",
      presence: "found",
      origin: "standard",
      sourceStandard: "Dublin Core / DCAT-AP",
      assessment: "complete",
    }));
  }
  if (assets.some((asset) => asset.kind === "shape_graph")) {
    statuses.push(artifactStatus({
      id: "shacl",
      name: "SHACL validation profile or embedded shapes",
      presence: "found",
      origin: "standard",
      sourceStandard: "SHACL",
      assessment: "partial",
    }));
  }
  if (assets.some((asset) => asset.kind === "quality_measurement")) {
    statuses.push(artifactStatus({
      id: "dqv",
      name: "DQV validation metadata",
      presence: "found",
      origin: "standard",
      sourceStandard: "DQV",
      assessment: "partial",
    }));
  }
  if (assets.some((asset) => asset.kind === "lifecycle_status")) {
    statuses.push(artifactStatus({
      id: "adms",
      name: "ADMS lifecycle/status metadata",
      presence: "found",
      origin: "standard",
      sourceStandard: "ADMS",
      assessment: "partial",
    }));
  }
  if (assets.some((asset) => asset.kind === "privacy_basis") || hasStandardSignal(findings, ["dcatap:applicableLegislation"])) {
    statuses.push(artifactStatus({
      id: "dpv",
      name: "Legal basis or data protection metadata",
      presence: "found",
      origin: "standard",
      sourceStandard: assets.some((asset) => asset.kind === "privacy_basis") ? "DPV" : "DCAT-AP",
      assessment: "complete",
    }));
  }
  if (assets.some((asset) => asset.kind === "trust_artifact")) {
    statuses.push(artifactStatus({
      id: "trust",
      name: "DID, VCDM, or provenance/trust metadata",
      presence: "found",
      origin: "standard",
      sourceStandard: "DID/VCDM/PROV",
      assessment: "partial",
    }));
  }
  return statuses;
}

function hasDetailedPolicySignal(assets: SemanticAsset[], findings: DiscoveryFinding[]): boolean {
  const detailedPredicates = new Set([
    "odrl:profile",
    "odrl:prohibition",
    "odrl:constraint",
    "odrl:duty",
    "odrl:assignee",
  ]);
  const hasDetailedHint = assets
    .filter((asset) => asset.kind === "policy")
    .some(
      (asset) =>
        asset.conforms_to.length > 0 ||
        asset.source_hints.some((hint) => hint.predicate ? detailedPredicates.has(hint.predicate) : false),
    );
  return hasDetailedHint || hasStandardSignal(findings, [...detailedPredicates]);
}

function hasStandardSignal(findings: DiscoveryFinding[], predicates: string[]): boolean {
  const predicateSet = new Set(predicates);
  return findings.some((finding) => {
    const evidence = finding.evidence;
    return evidence?.source === "json_ld_predicate" && predicateSet.has(evidence.predicate);
  });
}

function artifactKindLabel(kind: string): string {
  return kind
    .split("_")
    .map((part) => part.toUpperCase() === "API" ? "API" : part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function semanticOpenApiSummary(assets: SemanticAsset[]): AtlasModel["openApi"] | undefined {
  const apiAssets = assets.filter((asset) => asset.kind === "api_description");
  if (apiAssets.length === 0) {
    return undefined;
  }
  return {
    title: apiAssets[0].title ?? "OpenAPI description",
    pathCount: 0,
    securitySchemes: [],
  };
}

function inferSemanticProfile(report: DiscoveryReport): ProfileId {
  const text = [...report.profiles.map((profile) => profile.iri), ...report.standards.map((standard) => standard.iri)].join(" ").toLowerCase();
  if (text.includes("breg") || text.includes("base-registry")) {
    return "breg-dcat-ap";
  }
  if (text.includes("2.1.1")) {
    return "dcat-ap-2";
  }
  return "dcat-ap-3";
}

function isAccessType(type: RecordType): boolean {
  return type === "service" || type === "distribution" || type === "ogc-feature-collection" || type === "ogc-record-collection" || type === "operation-group";
}

export async function proxyFetch(
  targetUrl: string,
  options: { bearerToken?: string; fetcher?: typeof fetch } = {},
): Promise<ProxyFetchResult> {
  const fetcher = options.fetcher ?? fetch;
  const response = await fetcher(`/api/proxy?url=${encodeURIComponent(targetUrl)}`, {
    headers: options.bearerToken ? { "x-atlas-bearer": options.bearerToken } : undefined,
  });
  const text = await response.text();
  const contentType = response.headers.get("content-type") ?? undefined;
  const parsed = parseJson(text);
  const envelope = proxyEnvelope(parsed);

  if (envelope) {
    return {
      ok: envelope.ok,
      status: envelope.status,
      statusText: envelope.statusText,
      url: envelope.url ?? targetUrl,
      finalUrl: envelope.finalUrl,
      contentType: envelope.contentType,
      body: envelope.body,
      json: envelope.json,
      error: envelope.error,
      errorCode: envelope.errorCode,
    };
  }

  return {
    ok: response.ok,
    status: response.status,
    statusText: response.statusText,
    url: targetUrl,
    finalUrl: response.url,
    contentType,
    body: text,
    json: parsed,
    error: response.ok ? undefined : text,
  };
}

export function summarizeOpenApi(document: unknown): AtlasModel["openApi"] {
  const record = asRecord(document);
  const info = asRecord(record.info);
  const components = asRecord(record.components);
  const securitySchemes = asRecord(components.securitySchemes);
  const paths = asRecord(record.paths);

  return {
    title: typeof info.title === "string" ? info.title : undefined,
    pathCount: Object.keys(paths).length,
    securitySchemes: Object.keys(securitySchemes),
  };
}

function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
}

function parseJson(text: string): unknown | undefined {
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return undefined;
  }
}

function proxyEnvelope(value: unknown): ProxyFetchResult | undefined {
  const record = asRecord(value);
  if (!("ok" in record)) {
    return undefined;
  }
  if (!("status" in record) && !("body" in record) && !("error" in record) && !("url" in record) && !("finalUrl" in record)) {
    return undefined;
  }

  const error = record.error;
  return {
    ok: Boolean(record.ok),
    status: typeof record.status === "number" ? record.status : 0,
    statusText: typeof record.statusText === "string" ? record.statusText : "",
    url: typeof record.url === "string" ? record.url : "",
    finalUrl: typeof record.finalUrl === "string" ? record.finalUrl : undefined,
    contentType: typeof record.contentType === "string" ? record.contentType : undefined,
    body: typeof record.body === "string" ? record.body : JSON.stringify(value),
    json: record.json,
    error:
      typeof error === "string"
        ? error
        : error && typeof error === "object" && "message" in error
          ? String((error as { message: unknown }).message)
          : undefined,
    errorCode:
      error && typeof error === "object" && "code" in error
        ? String((error as { code: unknown }).code)
        : undefined,
  };
}

function emptyCatalog(sourceUrl: string): Record<string, unknown> {
  return {
    "@context": { dcat: "http://www.w3.org/ns/dcat#", dcterms: "http://purl.org/dc/terms/" },
    "@id": sourceUrl,
    "@type": "dcat:Catalog",
    "dcterms:title": "Protected catalog",
  };
}
