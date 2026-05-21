import type { AtlasDiscoveryReportSummary, AtlasSemanticAssetSummary, DiscoveryFinding } from "./semanticAssetDiscovery";

export type CapabilityTerm = { kind: "label" | "field" | "iri"; value: string };

export interface CapabilityNeed {
  id: string;
  label: string;
  question: string;
  requiresAny: CapabilityTerm[];
  requiresAll?: CapabilityTerm[];
}

export interface CapabilityRoute {
  id: string;
  needId: string;
  role: "candidate_route" | "candidate_source" | "candidate_consumer_or_duplicate" | "unknown";
  label: string;
  sourceUrl?: string;
  accessKind: "metadata_only" | "api_description_available" | "dataset_distribution" | "rejected_or_gated";
  confidence: "high" | "medium" | "low";
  evidence: CapabilityEvidence[];
  gaps: string[];
  reviewFlags: string[];
}

export interface CapabilityEvidence {
  id: string;
  label: string;
  location: string;
  basis: "required_information" | "access_evidence";
}

export interface CapabilitySearchResult {
  queryId: string;
  needs: Array<{
    need: CapabilityNeed;
    routes: CapabilityRoute[];
  }>;
}

export const socialProtectionDemoNeeds: CapabilityNeed[] = [
  {
    id: "farmer_status",
    label: "Farmer registration",
    question: "Is the person registered as a farmer?",
    requiresAny: [{ kind: "label", value: "Farmer" }],
  },
  {
    id: "disability_status",
    label: "Disability status",
    question: "Does the person have a disability status?",
    requiresAny: [],
    requiresAll: [
      { kind: "label", value: "Disabled Person" },
      { kind: "field", value: "disability_status" },
    ],
  },
  {
    id: "school_attendance",
    label: "School attendance",
    question: "Are the person's children going to school?",
    requiresAny: [{ kind: "field", value: "attendance_rate" }],
  },
];

export function searchCapabilities(
  report: AtlasDiscoveryReportSummary,
  needs: CapabilityNeed[] = socialProtectionDemoNeeds,
): CapabilitySearchResult {
  const assetsByArtifact = new Map<string, AtlasSemanticAssetSummary[]>();
  for (const asset of report.assets) {
    const list = assetsByArtifact.get(asset.artifactId) ?? [];
    list.push(asset);
    assetsByArtifact.set(asset.artifactId, list);
  }

  return {
    queryId: "social_protection_program",
    needs: needs.map((need) => ({
      need,
      routes: routesForNeed(report, assetsByArtifact, need),
    })),
  };
}

function routesForNeed(
  report: AtlasDiscoveryReportSummary,
  assetsByArtifact: Map<string, AtlasSemanticAssetSummary[]>,
  need: CapabilityNeed,
): CapabilityRoute[] {
  const routes: CapabilityRoute[] = [];
  const seedTerms = [...need.requiresAny, ...(need.requiresAll ?? [])];

  for (const asset of report.assets) {
    const term = seedTerms.find((term) => term.kind === "label" && exactLabel(asset.label, term.value));
    if (!term) {
      continue;
    }
    routes.push(routeFromAsset(report, need, asset, term));
  }

  for (const finding of report.findings) {
    const propertyEvidence = propertyEvidenceFor(finding);
    if (!propertyEvidence) {
      continue;
    }
    const term = seedTerms.find((term) => term.kind === "field" && fieldMatches(propertyEvidence.fields, term.value));
    if (!term) {
      continue;
    }
    const asset = assetForPropertyEvidence(report, assetsByArtifact, propertyEvidence);
    routes.push(routeFromFinding(report, need, finding, asset, propertyEvidence.label, propertyEvidence.location));
  }

  routes.sort((left, right) => confidenceRank(right.confidence) - confidenceRank(left.confidence) || left.label.localeCompare(right.label));
  return preferCallableRoutes(dedupeRoutes(routes.filter((route) => satisfiesRequiredTerms(route, need))));
}

function satisfiesRequiredTerms(route: CapabilityRoute, need: CapabilityNeed): boolean {
  const hasAny = need.requiresAny.length === 0 || need.requiresAny.some((term) => routeHasTerm(route, term));
  const hasAll = (need.requiresAll ?? []).every((term) => routeHasTerm(route, term));
  return hasAny && hasAll;
}

function routeHasTerm(route: CapabilityRoute, term: CapabilityTerm): boolean {
  const expected = canonicalLabel(term.value);
  if (term.kind === "label") {
    return canonicalLabel(route.label) === expected || route.evidence.some((evidence) => canonicalLabel(evidence.label) === expected);
  }
  if (term.kind === "field") {
    return route.evidence.some((evidence) => fieldMatches([evidence.label, evidence.location], term.value));
  }
  return route.sourceUrl === term.value || route.evidence.some((evidence) => evidence.location === term.value);
}

function fieldMatches(fields: string[], expected: string): boolean {
  const normalizedExpected = canonicalField(expected);
  return fields.some((field) => canonicalField(field) === normalizedExpected);
}

function canonicalField(value: string): string {
  return value.trim().split(/[/:#]/).filter(Boolean).at(-1)?.replace(/[^a-z0-9]/gi, "").toLowerCase() ?? "";
}

interface PropertyEvidenceProjection {
  artifactId: string;
  fields: string[];
  label: string;
  location: string;
  assetLabel?: string;
}

function propertyEvidenceFor(finding: DiscoveryFinding): PropertyEvidenceProjection | undefined {
  const evidence = finding.evidence;
  if (!evidence) {
    return undefined;
  }
  if (evidence.source === "schema_property") {
    return {
      artifactId: evidence.artifact_id,
      fields: [evidence.property_name, evidence.property_path, evidence.schema_pointer],
      label: evidence.value ?? evidence.property_path,
      location: evidence.property_path,
    };
  }
  if (evidence.source === "shacl_property") {
    const assetLabel = labelFromIri(evidence.shape);
    return {
      artifactId: evidence.artifact_id,
      fields: [evidence.path, evidence.predicate, evidence.shape],
      label: assetLabel ?? evidence.value ?? evidence.path,
      location: evidence.path,
      assetLabel,
    };
  }
  return undefined;
}

function assetForPropertyEvidence(
  report: AtlasDiscoveryReportSummary,
  assetsByArtifact: Map<string, AtlasSemanticAssetSummary[]>,
  propertyEvidence: PropertyEvidenceProjection,
): AtlasSemanticAssetSummary | undefined {
  const direct = assetsByArtifact.get(propertyEvidence.artifactId)?.[0];
  if (direct && direct.kind !== "catalog") {
    return direct;
  }
  const assetLabel = propertyEvidence.assetLabel;
  if (assetLabel) {
    return report.assets.find((asset) => exactLabel(asset.label, assetLabel)) ?? direct;
  }
  return direct;
}

function labelFromIri(value: string): string | undefined {
  const segment = value.split(/[/#]/).filter(Boolean).at(-1);
  if (!segment) {
    return undefined;
  }
  return segment
    .replace(/Shape$/i, "")
    .replace(/[_-]+/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .trim();
}

function routeFromAsset(
  report: AtlasDiscoveryReportSummary,
  need: CapabilityNeed,
  asset: AtlasSemanticAssetSummary,
  term: CapabilityTerm,
): CapabilityRoute {
  const access = accessForAsset(report, asset);
  const signals = standardSignalsForAsset(report, asset);
  const role = routeRoleFromStandardSignals(signals);
  const evidence: CapabilityEvidence[] = [
    {
      id: `asset:${asset.id}`,
      label: term.value,
      location: asset.uri ?? asset.endpointUrl ?? report.entryUrl,
      basis: "required_information",
    },
  ];
  if (access.sourceUrl) {
    evidence.push({
      id: `access:${asset.id}`,
      label: "Access method",
      location: access.sourceUrl,
      basis: "access_evidence",
    });
  }
  evidence.push(...propertyEvidenceForAsset(report, asset));
  return {
    id: `${need.id}:${asset.id}`,
    needId: need.id,
    role,
    label: asset.label ?? asset.uri ?? asset.id,
    sourceUrl: access.sourceUrl,
    accessKind: access.kind,
    confidence: access.kind === "metadata_only" ? "medium" : "high",
    evidence,
    gaps: gapsFor(access.kind, signals, role),
    reviewFlags: reviewFlagsFor(need),
  };
}

function propertyEvidenceForAsset(report: AtlasDiscoveryReportSummary, asset: AtlasSemanticAssetSummary): CapabilityEvidence[] {
  return report.findings.flatMap((finding) => {
    const propertyEvidence = propertyEvidenceFor(finding);
    if (!propertyEvidence) {
      return [];
    }
    const evidenceAsset = report.assets.find((candidate) => candidate.artifactId === propertyEvidence.artifactId);
    if (!evidenceAsset || !exactLabel(evidenceAsset.label, asset.label)) {
      return [];
    }
    return [
      {
        id: `finding:${finding.id}`,
        label: propertyEvidence.label,
        location: propertyEvidence.location,
        basis: "required_information" as const,
      },
    ];
  });
}

function routeFromFinding(
  report: AtlasDiscoveryReportSummary,
  need: CapabilityNeed,
  finding: DiscoveryFinding,
  asset: AtlasSemanticAssetSummary | undefined,
  propertyLabel: string,
  propertyLocation: string,
): CapabilityRoute {
  const access = asset ? accessForAsset(report, asset) : { kind: "metadata_only" as const, sourceUrl: undefined };
  const signals = asset ? standardSignalsForAsset(report, asset) : emptyStandardSignals();
  const role = routeRoleFromStandardSignals(signals);
  const evidence: CapabilityEvidence[] = [
    {
      id: `finding:${finding.id}`,
      label: propertyLabel,
      location: propertyLocation,
      basis: "required_information",
    },
  ];
  return {
    id: `${need.id}:${finding.id}`,
    needId: need.id,
    role,
    label: asset?.label ?? propertyLocation,
    sourceUrl: access.sourceUrl ?? asset?.uri ?? asset?.endpointUrl ?? undefined,
    accessKind: access.kind,
    confidence: asset ? "medium" : "low",
    evidence,
    gaps: gapsFor(access.kind, signals, role),
    reviewFlags: reviewFlagsFor(need),
  };
}

function accessForAsset(
  report: AtlasDiscoveryReportSummary,
  asset: AtlasSemanticAssetSummary,
): { kind: CapabilityRoute["accessKind"]; sourceUrl?: string } {
  const direct = accessUrlsForAsset(asset);
  const rejected = report.rejectedFetches.find((item) => direct.includes(item.url));
  if (rejected) {
    return { kind: "rejected_or_gated", sourceUrl: rejected.url };
  }

  const endpoint = asset.endpointUrl ?? (isDistributionLike(asset) ? asset.uri : undefined);
  if (endpoint) {
    return { kind: accessKindForAsset(asset), sourceUrl: endpoint };
  }

  const related = relatedDistribution(report, asset);
  if (related) {
    return accessForAsset(report, related);
  }

  return { kind: "metadata_only" };
}

function accessUrlsForAsset(asset: AtlasSemanticAssetSummary): string[] {
  const urls = asset.endpointUrl ? [asset.endpointUrl] : [];
  if (asset.uri && (isDistributionLike(asset) || asset.kind === "api_description" || asset.kind === "data_service")) {
    urls.push(asset.uri);
  }
  return Array.from(new Set(urls));
}

function relatedDistribution(
  report: AtlasDiscoveryReportSummary,
  asset: AtlasSemanticAssetSummary,
): AtlasSemanticAssetSummary | undefined {
  if (isDistributionLike(asset)) {
    return undefined;
  }
  const label = canonicalAccessLabel(asset.label);
  const routeKey = assetRouteKey(asset);
  return report.assets.find((candidate) => {
    if (!isDistributionLike(candidate)) {
      return false;
    }
    return (
      (!!label && canonicalAccessLabel(candidate.label) === label) ||
      (!!routeKey && assetRouteKey(candidate) === routeKey)
    );
  });
}

function relatedDataset(
  report: AtlasDiscoveryReportSummary,
  asset: AtlasSemanticAssetSummary,
): AtlasSemanticAssetSummary | undefined {
  if (asset.kind === "dataset") {
    return asset;
  }
  const url = asset.endpointUrl ?? asset.uri;
  const datasetUrl = url ? datasetUrlPrefix(url) : undefined;
  return report.assets.find((candidate) => candidate.kind === "dataset" && candidate.uri === datasetUrl);
}

interface StandardSignals {
  predicates: Set<string>;
}

function emptyStandardSignals(): StandardSignals {
  return { predicates: new Set() };
}

function standardSignalsForAsset(
  report: AtlasDiscoveryReportSummary,
  asset: AtlasSemanticAssetSummary,
): StandardSignals {
  const signals = emptyStandardSignals();
  if (asset.publisher) {
    signals.predicates.add("dcterms:publisher");
  }
  const distribution = relatedDistribution(report, asset);
  const dataset = relatedDataset(report, asset) ?? (distribution ? relatedDataset(report, distribution) : undefined);
  const related = [asset, distribution, dataset].filter((item): item is AtlasSemanticAssetSummary => Boolean(item));
  const assetIds = new Set(related.map((item) => item.id));
  for (const finding of report.findings) {
    if (finding.code !== "semantic.standard_signal" || !finding.asset_id || !assetIds.has(finding.asset_id)) {
      continue;
    }
    if (finding.evidence?.source === "json_ld_predicate") {
      signals.predicates.add(finding.evidence.predicate);
    }
  }
  const relatedUrls = new Set(
    related.flatMap((item) =>
      [...accessUrlsForAsset(item), item.uri].filter((value): value is string => Boolean(value)),
    ),
  );
  for (const link of report.links) {
    if (link.predicate === "cpsv:produces" && urlSetContainsResolvedFragment(relatedUrls, link.toUrl)) {
      signals.predicates.add("cpsv:produces");
    }
  }
  return signals;
}

function routeRoleFromStandardSignals(signals: StandardSignals): CapabilityRoute["role"] {
  // This role is an Atlas interpretation, not a SEMIC/DCAT/BRegDCAT term.
  // We derive it only from standard predicates so the published metadata stays
  // portable while the UI can still explain how strongly a route is evidenced.
  return hasAnySignal(signals, ["cpsv:produces"]) && hasAuthoritySignal(signals) && hasLegalBasisSignal(signals)
    ? "candidate_source"
    : "candidate_route";
}

function gapsFor(
  accessKind: CapabilityRoute["accessKind"],
  signals: StandardSignals,
  role: CapabilityRoute["role"],
): string[] {
  const gaps = ["identifier unknown", "legal basis unknown", "authority unknown", "source of truth unknown", "freshness unknown"];
  if (accessKind === "metadata_only") {
    gaps.unshift("operation details unavailable");
    gaps.unshift("no callable access method");
  }
  return gaps.filter((gap) => {
    if (gap === "authority unknown") return !hasAuthoritySignal(signals);
    if (gap === "legal basis unknown") return !hasLegalBasisSignal(signals);
    if (gap === "freshness unknown") return !hasFreshnessSignal(signals);
    if (gap === "source of truth unknown") return role !== "candidate_source";
    return true;
  });
}

function hasAnySignal(signals: StandardSignals, predicates: string[]): boolean {
  return predicates.some((predicate) => signals.predicates.has(predicate));
}

function hasAuthoritySignal(signals: StandardSignals): boolean {
  return hasAnySignal(signals, ["dcterms:publisher", "dcterms:creator"]);
}

function hasLegalBasisSignal(signals: StandardSignals): boolean {
  return hasAnySignal(signals, ["dcatap:applicableLegislation"]);
}

function hasFreshnessSignal(signals: StandardSignals): boolean {
  return hasAnySignal(signals, ["dcterms:modified", "dcterms:issued", "dcterms:accrualPeriodicity", "adms:status", "dcatap:availability"]);
}

function urlSetContainsResolvedFragment(urls: Set<string>, candidate: string): boolean {
  if (urls.has(candidate)) {
    return true;
  }
  return [...urls].some((url) => url.startsWith("#") && candidate.endsWith(url));
}

function datasetUrlPrefix(url: string): string | undefined {
  const marker = "/datasets/";
  const index = url.indexOf(marker);
  if (index < 0) {
    return undefined;
  }
  const start = index + marker.length;
  const [dataset] = url.slice(start).split("/");
  return dataset ? `${url.slice(0, start)}${dataset}` : undefined;
}

function isDistributionLike(asset: AtlasSemanticAssetSummary): boolean {
  return asset.kind === "distribution" || asset.kind === "record_collection" || asset.kind === "feature_collection";
}

function accessKindForAsset(asset: AtlasSemanticAssetSummary): CapabilityRoute["accessKind"] {
  if (asset.kind === "data_service" || asset.kind === "api_description") {
    return "api_description_available";
  }
  if (isDistributionLike(asset)) {
    return "dataset_distribution";
  }
  return "metadata_only";
}

function reviewFlagsFor(need: CapabilityNeed): string[] {
  const requiredTerms = [...need.requiresAny, ...(need.requiresAll ?? [])];
  const text = `${need.id} ${need.label} ${requiredTerms.map((term) => term.value).join(" ")}`.toLowerCase();
  return /disability|attendance|child|school|health|income|eligibility|household|identity/.test(text)
    ? ["sensitive data", "policy review required"]
    : [];
}

function exactLabel(left: string | null | undefined, right: string): boolean {
  return canonicalLabel(left) === canonicalLabel(right);
}

function canonicalLabel(value: string | null | undefined): string {
  return (value ?? "").trim().replace(/\s+/g, " ").toLowerCase();
}

function canonicalAccessLabel(value: string | null | undefined): string {
  return canonicalLabel(value).replace(/\s+(api|service|distribution|endpoint)$/u, "");
}

function assetRouteKey(asset: AtlasSemanticAssetSummary): string | undefined {
  const url = asset.endpointUrl ?? asset.uri;
  if (!url) {
    return undefined;
  }
  return routeKeyFromUrl(url);
}

function routeKeyFromUrl(url: string): string | undefined {
  // Atlas-only projection: connect schema metadata and declared access
  // methods when a publisher uses the common `/datasets/{dataset}/{entity}`
  // and `/metadata/schema/{dataset}/{entity}/...` URL shapes. This is
  // evidence wiring, not a claim that the endpoint is callable or authorized.
  const datasets = firstTwoSegmentsAfter(url, "/datasets/");
  if (datasets) {
    return datasets;
  }
  return firstTwoSegmentsAfter(url, "/metadata/schema/");
}

function firstTwoSegmentsAfter(url: string, marker: string): string | undefined {
  const index = url.indexOf(marker);
  if (index < 0) {
    return undefined;
  }
  const [dataset, entity] = url.slice(index + marker.length).split("/").filter(Boolean);
  return dataset && entity ? `${dataset}/${entity}` : undefined;
}

function dedupeRoutes(routes: CapabilityRoute[]): CapabilityRoute[] {
  return Array.from(new Map(routes.map((route) => [route.sourceUrl ?? route.id, route])).values());
}

function preferCallableRoutes(routes: CapabilityRoute[]): CapabilityRoute[] {
  const callableRoutes = routes.filter((route) => route.accessKind !== "metadata_only");
  return callableRoutes.length > 0 ? callableRoutes : routes;
}

function confidenceRank(value: CapabilityRoute["confidence"]): number {
  return value === "high" ? 3 : value === "medium" ? 2 : 1;
}
