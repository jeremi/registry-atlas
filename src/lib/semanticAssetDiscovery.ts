import type { ProxyFetchResult } from "./types";

export const SEMANTIC_ASSET_DISCOVERY_REPORT_SCHEMA_VERSION = "semantic-asset-discovery.report.v1" as const;
export const SEMANTIC_ASSET_DISCOVERY_SENSITIVE_HEADER_NAMES = [
  "authorization",
  "cookie",
  "proxy-authenticate",
  "set-cookie",
  "www-authenticate",
  "x-api-key",
  "x-auth-token",
  "proxy-authorization",
] as const;

type ForwardCompatibleString<T extends string> = T | (string & {});

export type ArtifactKind = ForwardCompatibleString<
  | "metadata_index"
  | "semantic_model_package"
  | "link_ml_schema"
  | "dcat_catalog"
  | "dcat_profile_catalog"
  | "prof_profile"
  | "prof_resource"
  | "shacl"
  | "skos"
  | "json_ld_context"
  | "owl_ontology"
  | "json_schema"
  | "open_api"
  | "ogc_records"
  | "ogc_features"
  | "ogc_landing"
  | "did_document"
  | "verifiable_credential"
  | "html_landing_page"
  | "unknown"
>;

export type ArtifactDiscoveryStatus = ForwardCompatibleString<
  | "fetched"
  | "failed"
  | "unsupported"
  | "skipped"
  | "auth_required"
  | "too_large"
  | "parse_error"
  | "disallowed_by_robots"
  | "unknown"
>;

export type SemanticAssetKind = ForwardCompatibleString<
  | "semantic_model_package"
  | "catalog"
  | "dataset"
  | "data_service"
  | "distribution"
  | "profile"
  | "vocabulary"
  | "vocabulary_term"
  | "class"
  | "property"
  | "shape_graph"
  | "concept_scheme"
  | "alignment"
  | "crosswalk"
  | "api_description"
  | "record_collection"
  | "feature_collection"
  | "policy"
  | "quality_measurement"
  | "lifecycle_status"
  | "privacy_basis"
  | "trust_artifact"
  | "unknown"
>;

export type LinkConfidence = ForwardCompatibleString<"declared" | "inferred" | "unknown">;
export type FindingSeverity = ForwardCompatibleString<"info" | "warning" | "error" | "unknown">;
export type DiscoveryPolicyName = ForwardCompatibleString<"public_web" | "local_development" | "unknown">;

export interface DiscoverySummary {
  artifact_count: number;
  asset_count: number;
  standard_count: number;
  profile_count: number;
  failed_artifact_count: number;
  unsupported_artifact_count: number;
  parse_error_count: number;
  next_fetch_count: number;
  truncated: boolean;
}

export interface HeaderPair {
  name: string;
  value: string;
}

export interface AnalyzeOptions {
  max_next_fetches: number;
  include_inferred_links: boolean;
  accepted_schemes: string[];
  enabled_profiles: string[];
}

export interface FetchedArtifact {
  url: string;
  final_url?: string | null;
  status: number;
  media_type?: string | null;
  request_accept?: string | null;
  redirect_chain: string[];
  headers: HeaderPair[];
  body: Uint8Array | number[] | string;
  fetched_at: string;
  depth: number;
  discovered_from?: string | null;
  discovered_by?: DiscoveryEvidence | null;
}

export interface AnalyzeInput {
  entry_url: string;
  analyzed_at?: string | null;
  artifacts: FetchedArtifact[];
  options: AnalyzeOptions;
}

export type DiscoveryEvidence =
  | {
      source: "http_header";
      artifact_id?: string | null;
      header_name: string;
      rel?: string | null;
      value?: string | null;
    }
  | {
      source: "json_ld_predicate";
      artifact_id?: string | null;
      predicate: string;
      pointer?: string | null;
      value?: string | null;
    }
  | {
      source: "json_pointer";
      artifact_id?: string | null;
      pointer: string;
      value?: string | null;
    }
  | {
      source: "schema_property";
      artifact_id: string;
      schema_pointer: string;
      property_path: string;
      property_name: string;
      value?: string | null;
    }
  | {
      source: "shacl_property";
      artifact_id: string;
      shape: string;
      path: string;
      predicate: string;
      value?: string | null;
    }
  | {
      source: "open_api_operation";
      artifact_id: string;
      path: string;
      method: string;
      operation_id?: string | null;
      summary?: string | null;
    }
  | {
      source: "ogc_collection";
      artifact_id: string;
      collection_id: string;
      title?: string | null;
    }
  | {
      source: "html_link";
      artifact_id?: string | null;
      rel: string;
      href: string;
      pointer?: string | null;
    }
  | {
      source: "url_pattern";
      artifact_id?: string | null;
      pattern: string;
      value: string;
    }
  | {
      source: "content_sniff";
      artifact_id?: string | null;
      detector: string;
      marker: string;
    }
  | {
      source: "host_policy";
      artifact_id?: string | null;
      policy: string;
      value?: string | null;
    };

export interface DiscoveredArtifact {
  id: string;
  url: string;
  final_url?: string | null;
  kind: ArtifactKind;
  status: ArtifactDiscoveryStatus;
  media_type?: string | null;
  http_status?: number | null;
  title?: string | null;
  description?: string | null;
  discovered_from?: string | null;
  discovered_by?: DiscoveryEvidence | null;
  byte_length?: number | null;
  hash?: string | null;
  error?: string | null;
  analyzed_at: string;
}

export interface DiscoverySourceHint {
  label: string;
  predicate?: string | null;
  path?: string | null;
  artifact_id: string;
}

export interface RawReference {
  artifact_id: string;
  pointer?: string | null;
  subject_iri?: string | null;
}

export interface SemanticAsset {
  id: string;
  kind: SemanticAssetKind;
  artifact_id: string;
  uri?: string | null;
  title?: string | null;
  description?: string | null;
  publisher?: string | null;
  endpoint_url?: string | null;
  conforms_to: string[];
  source_hints: DiscoverySourceHint[];
  raw_refs: RawReference[];
}

export interface DiscoveredLink {
  id: string;
  from_artifact_id?: string | null;
  from_url: string;
  to_url: string;
  rel?: string | null;
  predicate?: string | null;
  role?: string | null;
  confidence: LinkConfidence;
  discovered_by: DiscoveryEvidence;
}

export interface FetchCandidate {
  id: string;
  url: string;
  depth: number;
  priority: number;
  reason: string;
  discovered_from: string;
  discovered_by: DiscoveryEvidence;
}

export interface StandardClaim {
  id: string;
  iri: string;
  label?: string | null;
  version?: string | null;
  claimed_by_artifact_id: string;
  evidence: DiscoveryEvidence;
}

export interface ProfileClaim {
  id: string;
  iri: string;
  label?: string | null;
  version?: string | null;
  base_standard_iri?: string | null;
  claimed_by_artifact_id: string;
  evidence: DiscoveryEvidence;
}

export interface DiscoveryFinding {
  id: string;
  severity: FindingSeverity;
  code: string;
  message: string;
  artifact_id?: string | null;
  asset_id?: string | null;
  standard_iri?: string | null;
  evidence?: DiscoveryEvidence | null;
}

export interface DiscoveryReport {
  schema_version: typeof SEMANTIC_ASSET_DISCOVERY_REPORT_SCHEMA_VERSION | (string & {});
  run_id: string;
  entry_url: string;
  analyzed_at: string;
  summary: DiscoverySummary;
  artifacts: DiscoveredArtifact[];
  assets: SemanticAsset[];
  links: DiscoveredLink[];
  standards: StandardClaim[];
  profiles: ProfileClaim[];
  findings: DiscoveryFinding[];
  next_fetches: FetchCandidate[];
}

export interface DiscoveryRequest {
  entry_url: string;
  policy: DiscoveryPolicyName;
  max_depth: number;
  max_fetches: number;
  max_body_bytes: number;
  max_total_bytes: number;
  max_concurrent_fetches: number;
  timeout_ms: number;
  total_timeout_ms: number;
  user_agent?: string | null;
  accepted_schemes: string[];
  allowed_origins: string[];
}

export interface FetchSummary {
  entry_url: string;
  fetched_count: number;
  rejected_count: number;
  redirect_count: number;
  total_decompressed_bytes: number;
  max_total_bytes: number;
  max_concurrent_fetches: number;
  total_elapsed_ms: number;
}

export interface RejectedFetch {
  id: string;
  url: string;
  reason_code: string;
  discovered_from?: string | null;
  credential_sent: boolean;
}

export interface DiscoveryRunEnvelope {
  report: DiscoveryReport;
  fetched: FetchSummary;
  rejected_fetches: RejectedFetch[];
}

export interface WasmAnalyzeError {
  code: string;
  message: string;
}

export type WasmAnalyzeResult = { ok: true; report: DiscoveryReport } | { ok: false; error: WasmAnalyzeError };
export type AnalyzeArtifactsJson = (inputJson: string) => string;
export interface SemanticAssetDiscoveryWasmModule {
  analyzeArtifacts: AnalyzeArtifactsJson;
  version?: () => string;
}

export interface AtlasSemanticAssetSummary {
  id: string;
  kind: SemanticAssetKind;
  label: string;
  artifactId: string;
  artifactUrl?: string;
  artifactTitle?: string;
  uri?: string;
  description?: string;
  publisher?: string;
  endpointUrl?: string;
  conformsTo: string[];
  sourceHints: AtlasSourceHintSummary[];
  rawReferences: RawReference[];
}

export interface AtlasSourceHintSummary {
  label: string;
  artifactId: string;
  artifactUrl?: string;
  predicate?: string;
  path?: string;
}

export interface AtlasDiscoveredLinkSummary {
  id: string;
  fromUrl: string;
  toUrl: string;
  label: string;
  confidence: LinkConfidence;
  fromArtifactId?: string;
  fromArtifactTitle?: string;
  fromArtifactUrl?: string;
  rel?: string;
  predicate?: string;
  role?: string;
  evidence: string;
}

export interface AtlasDiscoveryReportSummary {
  schemaVersion: string;
  runId: string;
  entryUrl: string;
  analyzedAt: string;
  counts: DiscoverySummary;
  fetched?: FetchSummary;
  rejectedFetches: RejectedFetch[];
  assets: AtlasSemanticAssetSummary[];
  links: AtlasDiscoveredLinkSummary[];
  findings: DiscoveryFinding[];
  standards: StandardClaim[];
  profiles: ProfileClaim[];
  nextFetches: FetchCandidate[];
}

export type AtlasDiscoveryRunSummary = AtlasDiscoveryReportSummary & {
  fetched: FetchSummary;
  rejectedFetches: RejectedFetch[];
};

export function normalizeWasmAnalyzeResult(result: WasmAnalyzeResult): AtlasDiscoveryReportSummary | WasmAnalyzeError {
  return result.ok ? normalizeDiscoveryReport(result.report) : result.error;
}

export function analyzeProxyResultWithWasm(
  sourceUrl: string,
  result: ProxyFetchResult,
  analyzeArtifacts: AnalyzeArtifactsJson,
  options: Partial<AnalyzeOptions> = {},
): AtlasDiscoveryReportSummary | WasmAnalyzeError {
  const input: AnalyzeInput = {
    entry_url: sourceUrl,
    analyzed_at: new Date().toISOString(),
    artifacts: [buildSanitizedFetchedArtifact(result)],
    options: normalizeAnalyzeOptions(options),
  };
  const rawEnvelope = analyzeArtifacts(JSON.stringify(input));
  return normalizeWasmAnalyzeResult(parseWasmAnalyzeResult(rawEnvelope));
}

export async function createSemanticAssetDiscoveryAnalyzer(
  loadModule: () => Promise<SemanticAssetDiscoveryWasmModule>,
): Promise<AnalyzeArtifactsJson> {
  const module = await loadModule();
  if (typeof module.analyzeArtifacts !== "function") {
    throw new Error("semantic-asset-discovery WASM module is missing analyzeArtifacts.");
  }
  return module.analyzeArtifacts;
}

export function parseWasmAnalyzeResult(value: string): WasmAnalyzeResult {
  const parsed = JSON.parse(value) as unknown;
  if (!isRecord(parsed) || typeof parsed.ok !== "boolean") {
    throw new Error("semantic-asset-discovery WASM returned an invalid envelope.");
  }
  if (parsed.ok) {
    return { ok: true, report: parsed.report as DiscoveryReport };
  }
  return { ok: false, error: parsed.error as WasmAnalyzeError };
}

export function buildSanitizedFetchedArtifact(
  result: ProxyFetchResult,
  headers: HeaderPair[] = [],
  fetchedAt = new Date().toISOString(),
): FetchedArtifact {
  return {
    url: redactDiscoveryUrl(result.url),
    final_url: result.finalUrl ? redactDiscoveryUrl(result.finalUrl) : null,
    status: result.status,
    media_type: result.contentType ?? null,
    request_accept: null,
    redirect_chain: [],
    headers: stripSensitiveHeaders(headers),
    body: bytesFromText(result.body),
    fetched_at: fetchedAt,
    depth: 0,
    discovered_from: null,
    discovered_by: null,
  };
}

export function stripSensitiveHeaders(headers: HeaderPair[]): HeaderPair[] {
  const sensitiveNames = new Set<string>(SEMANTIC_ASSET_DISCOVERY_SENSITIVE_HEADER_NAMES);
  return headers
    .filter((header) => !sensitiveNames.has(header.name.toLowerCase()))
    .map((header) => ({ ...header }));
}

export function normalizeDiscoveryReport(report: DiscoveryReport): AtlasDiscoveryReportSummary {
  const artifactsById = new Map(report.artifacts.map((artifact) => [artifact.id, artifact]));

  return {
    schemaVersion: report.schema_version,
    runId: report.run_id,
    entryUrl: report.entry_url,
    analyzedAt: report.analyzed_at,
    counts: report.summary,
    rejectedFetches: [],
    assets: report.assets.map((asset) => summarizeAsset(asset, artifactsById)),
    links: report.links.map((link) => summarizeLink(link, artifactsById)),
    findings: report.findings,
    standards: report.standards,
    profiles: report.profiles,
    nextFetches: report.next_fetches,
  };
}

export function normalizeDiscoveryRunEnvelope(envelope: DiscoveryRunEnvelope): AtlasDiscoveryRunSummary {
  return {
    ...normalizeDiscoveryReport(envelope.report),
    fetched: { ...envelope.fetched },
    rejectedFetches: envelope.rejected_fetches.map((rejected) => ({ ...rejected })),
  };
}

export function redactDiscoveryUrl(value: string): string {
  try {
    const url = new URL(value);
    url.username = "";
    url.password = "";
    for (const key of Array.from(url.searchParams.keys())) {
      if (isSensitiveQueryName(key)) {
        url.searchParams.set(key, "REDACTED");
      }
    }
    return url.toString();
  } catch {
    return value;
  }
}

function summarizeAsset(asset: SemanticAsset, artifactsById: Map<string, DiscoveredArtifact>): AtlasSemanticAssetSummary {
  const artifact = artifactsById.get(asset.artifact_id);

  return {
    id: asset.id,
    kind: asset.kind,
    label: firstText(asset.title, asset.uri, artifact?.title, asset.id),
    artifactId: asset.artifact_id,
    artifactUrl: textOrUndefined(artifact?.url),
    artifactTitle: textOrUndefined(artifact?.title),
    uri: textOrUndefined(asset.uri),
    description: textOrUndefined(asset.description),
    publisher: textOrUndefined(asset.publisher),
    endpointUrl: textOrUndefined(asset.endpoint_url),
    conformsTo: asset.conforms_to,
    sourceHints: asset.source_hints.map((hint) => summarizeSourceHint(hint, artifactsById)),
    rawReferences: asset.raw_refs,
  };
}

function summarizeSourceHint(hint: DiscoverySourceHint, artifactsById: Map<string, DiscoveredArtifact>): AtlasSourceHintSummary {
  const artifact = artifactsById.get(hint.artifact_id);

  return {
    label: hint.label,
    artifactId: hint.artifact_id,
    artifactUrl: textOrUndefined(artifact?.url),
    predicate: textOrUndefined(hint.predicate),
    path: textOrUndefined(hint.path),
  };
}

function summarizeLink(link: DiscoveredLink, artifactsById: Map<string, DiscoveredArtifact>): AtlasDiscoveredLinkSummary {
  const artifact = link.from_artifact_id ? artifactsById.get(link.from_artifact_id) : undefined;
  const label = firstText(link.rel, link.predicate, link.role, "related");

  return {
    id: link.id,
    fromUrl: link.from_url,
    toUrl: link.to_url,
    label,
    confidence: link.confidence,
    fromArtifactId: textOrUndefined(link.from_artifact_id),
    fromArtifactTitle: textOrUndefined(artifact?.title),
    fromArtifactUrl: textOrUndefined(artifact?.url),
    rel: textOrUndefined(link.rel),
    predicate: textOrUndefined(link.predicate),
    role: textOrUndefined(link.role),
    evidence: evidenceLabel(link.discovered_by),
  };
}

function evidenceLabel(evidence: DiscoveryEvidence): string {
  switch (evidence.source) {
    case "http_header":
      return `HTTP ${evidence.header_name}${evidence.rel ? ` rel=${evidence.rel}` : ""}`;
    case "json_ld_predicate":
      return evidence.predicate;
    case "json_pointer":
      return evidence.pointer;
    case "schema_property":
      return evidence.property_path;
    case "shacl_property":
      return evidence.path;
    case "open_api_operation":
      return evidence.operation_id ?? `${evidence.method.toUpperCase()} ${evidence.path}`;
    case "ogc_collection":
      return evidence.title ?? evidence.collection_id;
    case "html_link":
      return `HTML link rel=${evidence.rel}`;
    case "url_pattern":
      return evidence.pattern;
    case "content_sniff":
      return evidence.detector;
    case "host_policy":
      return evidence.policy;
  }
}

function firstText(...values: Array<string | null | undefined>): string {
  return values.find((value) => textOrUndefined(value)) ?? "";
}

function textOrUndefined(value: string | null | undefined): string | undefined {
  return value && value.trim().length > 0 ? value : undefined;
}

function normalizeAnalyzeOptions(options: Partial<AnalyzeOptions>): AnalyzeOptions {
  return {
    max_next_fetches: options.max_next_fetches ?? 20,
    include_inferred_links: options.include_inferred_links ?? true,
    accepted_schemes: options.accepted_schemes ?? [],
    enabled_profiles: options.enabled_profiles ?? [],
  };
}

function bytesFromText(value: string): number[] {
  return Array.from(new TextEncoder().encode(value));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isSensitiveQueryName(name: string): boolean {
  const normalized = name.toLowerCase();
  return (
    normalized === "authorization" ||
    normalized === "access_token" ||
    normalized === "api_key" ||
    normalized === "apikey" ||
    normalized === "client_secret" ||
    normalized === "signature" ||
    normalized === "sig" ||
    normalized.includes("token") ||
    normalized.includes("secret") ||
    normalized.includes("password") ||
    normalized.endsWith("key")
  );
}
