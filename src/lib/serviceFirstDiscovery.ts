import type {
  AtlasDiscoveryReportSummary,
  AtlasSemanticAssetSummary,
  DiscoveryEvidence,
  RelationClaim,
  RelationEndpoint,
  SemanticRelation,
} from "./semanticAssetDiscovery";

export interface ServiceFirstDiscovery {
  services: PublicServiceDiscovery[];
  assetCounts: {
    publicServices: number;
    requirements: number;
    evidenceTypeLists: number;
    evidenceTypes: number;
    evidenceOfferings: number;
    evidenceProviders: number;
    accessServices: number;
    forms: number;
  };
  gaps: ServiceDiscoveryGap[];
}

export interface PublicServiceDiscovery {
  asset: AtlasSemanticAssetSummary;
  channels: RelationAssetView[];
  authorities: RelationAssetView[];
  requirements: RequirementDiscovery[];
  acceptedEvidenceTypes: EvidenceTypeDiscovery[];
  evidenceProviders: RelationAssetView[];
  accessServices: RelationAssetView[];
  supportingDataServices: RelationAssetView[];
  forms: RelationAssetView[];
  routes: EvidenceRoute[];
  gaps: ServiceDiscoveryGap[];
}

export interface RequirementDiscovery {
  asset: AtlasSemanticAssetSummary;
  provenance: RelationProvenance[];
  evidenceBundles: EvidenceBundle[];
  directEvidenceTypes: EvidenceTypeDiscovery[];
  gaps: ServiceDiscoveryGap[];
}

export interface EvidenceBundle {
  id: string;
  label: string;
  asset?: AtlasSemanticAssetSummary;
  evidenceTypes: EvidenceTypeDiscovery[];
  satisfiable: boolean;
  missingEvidenceTypeIds: string[];
  provenance: RelationProvenance[];
}

export interface EvidenceTypeDiscovery {
  asset: AtlasSemanticAssetSummary;
  offerings: EvidenceOfferingDiscovery[];
  providers: RelationAssetView[];
  accessServices: RelationAssetView[];
  hasAccessRoute: boolean;
  provenance: RelationProvenance[];
  gaps: ServiceDiscoveryGap[];
}

export interface EvidenceOfferingDiscovery {
  asset: AtlasSemanticAssetSummary;
  providers: RelationAssetView[];
  accessServices: RelationAssetView[];
  provenance: RelationProvenance[];
  gaps: ServiceDiscoveryGap[];
}

export interface EvidenceRoute {
  id: string;
  kind: "evidence_access_service" | "supporting_data_service" | "form";
  requirementId?: string;
  evidenceTypeId?: string;
  offering?: AtlasSemanticAssetSummary;
  provider?: AtlasSemanticAssetSummary;
  accessService?: AtlasSemanticAssetSummary;
  form?: AtlasSemanticAssetSummary;
  endpointUrl?: string;
  provenance: RelationProvenance[];
  gaps: ServiceDiscoveryGap[];
}

export interface RelationAssetView {
  asset: AtlasSemanticAssetSummary;
  provenance: RelationProvenance[];
}

export interface RelationProvenance {
  relationId: string;
  predicate: string;
  label: string;
  assertedByArtifactId?: string;
  evidence: string;
}

export interface ServiceDiscoveryGap {
  assetId: string;
  predicate: string;
  message: string;
}

const SERVICE_REQUIREMENT_PREDICATES = ["cv:holdsRequirement", "cccev:hasRequirement", "cpsv:holdsRequirement"];
const HAS_EVIDENCE_TYPE_LIST = ["cccev:hasEvidenceTypeList"];
const SPECIFIES_EVIDENCE_TYPE = ["cccev:specifiesEvidenceType"];
const DIRECT_EVIDENCE_TYPE = ["registry_manifest:acceptedEvidenceType"];
const OFFERING_EVIDENCE_TYPE = [
  "registry_manifest:evidenceType",
  "registry_manifest:offersEvidenceType",
  "registry_manifest:acceptedEvidenceType",
];
const PROVIDER_PREDICATES = [
  "registry_manifest:evidenceProvider",
  "registry_manifest:providedBy",
  "registry_manifest:issuingAuthority",
];
const ACCESS_SERVICE_PREDICATES = ["registry_manifest:evidenceService", "dcat:accessService"];
const FORM_PREDICATES = ["registry_manifest:form", "registry_manifest:hasForm"];

const PREDICATE_PREFIXES: Record<string, string> = {
  cccev: "http://data.europa.eu/m8g/",
  cpsv: "http://purl.org/vocab/cpsv#",
  cv: "http://data.europa.eu/m8g/",
  dcat: "http://www.w3.org/ns/dcat#",
  dcterms: "http://purl.org/dc/terms/",
  registry_manifest: "https://registry-manifest.dev/ns/v1#",
};

export function buildServiceFirstDiscovery(report?: AtlasDiscoveryReportSummary): ServiceFirstDiscovery | null {
  if (!report) {
    return null;
  }
  const graph = new ServiceFirstGraph(report);
  return graph.discovery();
}

class ServiceFirstGraph {
  private readonly report: AtlasDiscoveryReportSummary;
  private readonly assetsById = new Map<string, AtlasSemanticAssetSummary>();
  private readonly assetsByUri = new Map<string, AtlasSemanticAssetSummary>();
  private readonly outgoing = new Map<string, SemanticRelation[]>();
  private readonly incoming = new Map<string, SemanticRelation[]>();
  private readonly claimsByRelation = new Map<string, RelationClaim[]>();

  constructor(report: AtlasDiscoveryReportSummary) {
    this.report = report;
    for (const asset of report.assets) {
      this.assetsById.set(asset.id, asset);
      if (asset.uri) {
        this.assetsByUri.set(asset.uri, asset);
      }
    }
    for (const claim of report.relationClaims) {
      this.claimsByRelation.set(claim.relation_id, [...(this.claimsByRelation.get(claim.relation_id) ?? []), claim]);
    }
    for (const relation of report.relations) {
      const subject = this.assetForEndpoint(relation.subject);
      const object = this.assetForEndpoint(relation.object);
      if (subject) {
        this.outgoing.set(subject.id, [...(this.outgoing.get(subject.id) ?? []), relation]);
      }
      if (object) {
        this.incoming.set(object.id, [...(this.incoming.get(object.id) ?? []), relation]);
      }
    }
  }

  discovery(): ServiceFirstDiscovery {
    const services = this.assetsOfKind(["public_service"]).map((asset) => this.publicService(asset));
    const gaps =
      services.length === 0
        ? [
            {
              assetId: this.report.runId,
              predicate: "cpsv:PublicService",
              message: "No public service assets were discovered in the v2 semantic relation graph.",
            },
          ]
        : [];

    return {
      services,
      assetCounts: {
        publicServices: this.assetsOfKind(["public_service"]).length,
        requirements: this.assetsOfKind(["requirement", "information_requirement"]).length,
        evidenceTypeLists: this.assetsOfKind(["evidence_type_list"]).length,
        evidenceTypes: this.assetsOfKind(["evidence_type"]).length,
        evidenceOfferings: this.assetsOfKind(["evidence_offering"]).length,
        evidenceProviders: this.assetsOfKind(["evidence_provider", "public_organisation"]).length,
        accessServices: this.assetsOfKind(["data_service", "public_registry_service"]).length,
        forms: this.assetsOfKind(["form_definition"]).length,
      },
      gaps,
    };
  }

  private publicService(asset: AtlasSemanticAssetSummary): PublicServiceDiscovery {
    const channels = this.outgoingAssets(asset.id, ["cv:hasChannel"], ["channel"]);
    const authorities = this.outgoingAssets(asset.id, ["cv:hasCompetentAuthority"], ["evidence_provider", "public_organisation"]);
    const requirements = this.outgoingAssets(asset.id, SERVICE_REQUIREMENT_PREDICATES, ["requirement", "information_requirement"]).map(
      (requirement) => this.requirement(requirement.asset, requirement.provenance),
    );
    const acceptedEvidenceTypes = uniqueEvidenceTypes(requirements.flatMap((requirement) => [
      ...requirement.evidenceBundles.flatMap((bundle) => bundle.evidenceTypes),
      ...requirement.directEvidenceTypes,
    ]));
    const evidenceProviders = uniqueAssetViews(acceptedEvidenceTypes.flatMap((evidenceType) => evidenceType.providers));
    const accessServices = uniqueAssetViews(acceptedEvidenceTypes.flatMap((evidenceType) => evidenceType.accessServices));
    const supportingDataServices = this.outgoingAssets(asset.id, ["registry_manifest:usesDataService"], ["data_service", "public_registry_service"]);
    const forms = uniqueAssetViews([
      ...this.outgoingAssets(asset.id, FORM_PREDICATES, ["form_definition"]),
      ...this.incomingAssets(asset.id, ["registry_manifest:forPublicService"], ["form_definition"]),
      ...channels.flatMap((channel) => [
        ...this.outgoingAssets(channel.asset.id, FORM_PREDICATES, ["form_definition"], channel.provenance),
        ...this.incomingAssets(channel.asset.id, ["registry_manifest:forChannel"], ["form_definition"], channel.provenance),
      ]),
    ]);
    const routes = [
      ...requirements.flatMap((requirement) => this.evidenceRoutes(requirement)),
      ...supportingDataServices.map((service) => this.supportingDataServiceRoute(service)),
      ...forms.map((form) => this.formRoute(form)),
    ];
    const gaps = [
      ...(channels.length === 0
        ? [gap(asset.id, "cv:hasChannel", "Public service has no declared channel relation.")]
        : []),
      ...(requirements.length === 0
        ? [gap(asset.id, "cv:holdsRequirement", "Public service has no declared requirement relation.")]
        : []),
      ...requirements.flatMap((requirement) => requirement.gaps),
      ...acceptedEvidenceTypes.flatMap((evidenceType) => evidenceType.gaps),
    ];

    return {
      asset,
      channels,
      authorities,
      requirements,
      acceptedEvidenceTypes,
      evidenceProviders,
      accessServices,
      supportingDataServices,
      forms,
      routes,
      gaps,
    };
  }

  private requirement(asset: AtlasSemanticAssetSummary, provenance: RelationProvenance[]): RequirementDiscovery {
    const listAssets = this.outgoingAssets(asset.id, HAS_EVIDENCE_TYPE_LIST, ["evidence_type_list"]);
    const evidenceBundles = listAssets.map((list, index) => this.evidenceBundle(list.asset, list.provenance, index));
    const directEvidenceTypes = this.outgoingAssets(asset.id, DIRECT_EVIDENCE_TYPE, ["evidence_type"], provenance).map((evidenceType) =>
      this.evidenceType(evidenceType.asset, evidenceType.provenance),
    );
    const gaps =
      evidenceBundles.length === 0 && directEvidenceTypes.length === 0
        ? [gap(asset.id, "cccev:hasEvidenceTypeList", "Requirement has no declared evidence type list.")]
        : [];

    return {
      asset,
      provenance,
      evidenceBundles,
      directEvidenceTypes,
      gaps,
    };
  }

  private evidenceBundle(asset: AtlasSemanticAssetSummary, provenance: RelationProvenance[], index: number): EvidenceBundle {
    const evidenceTypes = this.outgoingAssets(asset.id, SPECIFIES_EVIDENCE_TYPE, ["evidence_type"], provenance).map((view) =>
      this.evidenceType(view.asset, view.provenance),
    );
    const missingEvidenceTypeIds = evidenceTypes
      .filter((evidenceType) => !evidenceType.hasAccessRoute)
      .map((evidenceType) => evidenceType.asset.id);

    return {
      id: asset.id,
      label: asset.label || `Alternative ${index + 1}`,
      asset,
      evidenceTypes,
      satisfiable: evidenceTypes.length > 0 && missingEvidenceTypeIds.length === 0,
      missingEvidenceTypeIds,
      provenance,
    };
  }

  private evidenceType(asset: AtlasSemanticAssetSummary, provenance: RelationProvenance[]): EvidenceTypeDiscovery {
    const offerings = this.incomingAssets(asset.id, OFFERING_EVIDENCE_TYPE, ["evidence_offering"], provenance).map((offering) =>
      this.evidenceOffering(offering.asset, offering.provenance),
    );
    const directProviders = this.outgoingAssets(asset.id, PROVIDER_PREDICATES, ["evidence_provider", "public_organisation"], provenance);
    const providers = uniqueAssetViews([...offerings.flatMap((offering) => offering.providers), ...directProviders]);
    const accessServices = uniqueAssetViews(offerings.flatMap((offering) => offering.accessServices));
    const gaps =
      offerings.length === 0
        ? [gap(asset.id, "registry_manifest:evidenceType", "Evidence type has no discovered evidence offering.")]
        : offerings.flatMap((offering) => offering.gaps);

    return {
      asset,
      offerings,
      providers,
      accessServices,
      hasAccessRoute: accessServices.length > 0,
      provenance,
      gaps,
    };
  }

  private evidenceOffering(asset: AtlasSemanticAssetSummary, provenance: RelationProvenance[]): EvidenceOfferingDiscovery {
    const providers = this.outgoingAssets(asset.id, PROVIDER_PREDICATES, ["evidence_provider", "public_organisation"], provenance);
    const accessServices = this.outgoingAssets(asset.id, ACCESS_SERVICE_PREDICATES, ["data_service", "public_registry_service"], provenance);
    const gaps = [
      ...(providers.length === 0
        ? [gap(asset.id, "registry_manifest:providedBy", "Evidence offering has no declared evidence provider.")]
        : []),
      ...(accessServices.length === 0
        ? [gap(asset.id, "registry_manifest:evidenceService", "Evidence offering has no declared access data service.")]
        : []),
    ];

    return {
      asset,
      providers,
      accessServices,
      provenance,
      gaps,
    };
  }

  private evidenceRoutes(requirement: RequirementDiscovery): EvidenceRoute[] {
    const routes: EvidenceRoute[] = [];
    for (const bundle of requirement.evidenceBundles) {
      for (const evidenceType of bundle.evidenceTypes) {
        for (const offering of evidenceType.offerings) {
          for (const accessService of offering.accessServices) {
            const endpoint = this.endpointUrlForAsset(accessService.asset.id);
            routes.push({
              id: `${requirement.asset.id}:${evidenceType.asset.id}:${offering.asset.id}:${accessService.asset.id}`,
              kind: "evidence_access_service",
              requirementId: requirement.asset.id,
              evidenceTypeId: evidenceType.asset.id,
              offering: offering.asset,
              provider: offering.providers[0]?.asset,
              accessService: accessService.asset,
              endpointUrl: endpoint?.url,
              provenance: [...bundle.provenance, ...evidenceType.provenance, ...offering.provenance, ...accessService.provenance, ...(endpoint?.provenance ?? [])],
              gaps: endpoint ? [] : [gap(accessService.asset.id, "dcat:endpointURL", "Access service has no declared endpoint URL relation.")],
            });
          }
        }
      }
    }
    return routes;
  }

  private supportingDataServiceRoute(service: RelationAssetView): EvidenceRoute {
    const endpoint = this.endpointUrlForAsset(service.asset.id);
    return {
      id: `supporting:${service.asset.id}`,
      kind: "supporting_data_service",
      accessService: service.asset,
      endpointUrl: endpoint?.url,
      provenance: [...service.provenance, ...(endpoint?.provenance ?? [])],
      gaps: endpoint ? [] : [gap(service.asset.id, "dcat:endpointURL", "Supporting data service has no declared endpoint URL relation.")],
    };
  }

  private formRoute(form: RelationAssetView): EvidenceRoute {
    return {
      id: `form:${form.asset.id}`,
      kind: "form",
      form: form.asset,
      provenance: form.provenance,
      gaps: [],
    };
  }

  private endpointUrlForAsset(assetId: string): { url: string; provenance: RelationProvenance[] } | undefined {
    const relation = this.outgoing.get(assetId)?.find((candidate) => predicateIn(candidate.predicate, ["dcat:endpointURL"]));
    if (!relation) {
      return undefined;
    }
    const url = endpointUri(relation.object);
    return url ? { url, provenance: this.provenanceForRelation(relation) } : undefined;
  }

  private outgoingAssets(
    assetId: string,
    predicates: string[],
    kinds: string[],
    priorProvenance: RelationProvenance[] = [],
  ): RelationAssetView[] {
    const seen = new Set<string>();
    return (this.outgoing.get(assetId) ?? [])
      .filter((relation) => predicateIn(relation.predicate, predicates))
      .map((relation) => ({ relation, asset: this.assetForEndpoint(relation.object) }))
      .filter((item): item is { relation: SemanticRelation; asset: AtlasSemanticAssetSummary } => Boolean(item.asset))
      .filter(({ asset }) => kinds.length === 0 || kinds.includes(asset.kind))
      .filter(({ asset }) => {
        if (seen.has(asset.id)) {
          return false;
        }
        seen.add(asset.id);
        return true;
      })
      .map(({ relation, asset }) => ({
        asset,
        provenance: [...priorProvenance, ...this.provenanceForRelation(relation)],
      }));
  }

  private incomingAssets(
    assetId: string,
    predicates: string[],
    kinds: string[],
    priorProvenance: RelationProvenance[] = [],
  ): RelationAssetView[] {
    const seen = new Set<string>();
    return (this.incoming.get(assetId) ?? [])
      .filter((relation) => predicateIn(relation.predicate, predicates))
      .map((relation) => ({ relation, asset: this.assetForEndpoint(relation.subject) }))
      .filter((item): item is { relation: SemanticRelation; asset: AtlasSemanticAssetSummary } => Boolean(item.asset))
      .filter(({ asset }) => kinds.length === 0 || kinds.includes(asset.kind))
      .filter(({ asset }) => {
        if (seen.has(asset.id)) {
          return false;
        }
        seen.add(asset.id);
        return true;
      })
      .map(({ relation, asset }) => ({
        asset,
        provenance: [...priorProvenance, ...this.provenanceForRelation(relation)],
      }));
  }

  private assetsOfKind(kinds: string[]): AtlasSemanticAssetSummary[] {
    return this.report.assets.filter((asset) => kinds.includes(asset.kind));
  }

  private assetForEndpoint(endpoint: RelationEndpoint): AtlasSemanticAssetSummary | undefined {
    if (endpoint.kind === "asset") {
      return this.assetsById.get(endpoint.asset_id) ?? (endpoint.uri ? this.assetsByUri.get(endpoint.uri) : undefined);
    }
    if (endpoint.kind === "external") {
      return this.assetsByUri.get(endpoint.uri);
    }
    return undefined;
  }

  private provenanceForRelation(relation: SemanticRelation): RelationProvenance[] {
    const claims = this.claimsByRelation.get(relation.id) ?? [];
    if (claims.length === 0) {
      return [
        {
          relationId: relation.id,
          predicate: relation.predicate,
          label: relation.label ?? relation.predicate,
          evidence: "unclaimed relation",
        },
      ];
    }
    return claims.map((claim) => ({
      relationId: relation.id,
      predicate: relation.predicate,
      label: relation.label ?? relation.predicate,
      assertedByArtifactId: claim.asserted_by_artifact_id,
      evidence: evidenceLabel(claim.evidence),
    }));
  }
}

function predicateIn(predicate: string, accepted: string[]): boolean {
  return accepted.some((candidate) => predicateMatches(predicate, candidate));
}

function predicateMatches(predicate: string, compact: string): boolean {
  if (predicate === compact) {
    return true;
  }
  const expanded = expandPredicate(compact);
  if (expanded && predicate === expanded) {
    return true;
  }
  const local = compact.split(":")[1];
  return Boolean(local && (predicate.endsWith(`#${local}`) || predicate.endsWith(`/${local}`)));
}

function expandPredicate(predicate: string): string | undefined {
  const [prefix, local] = predicate.split(":");
  const base = PREDICATE_PREFIXES[`${prefix}:`];
  return base && local ? `${base}${local}` : undefined;
}

function endpointUri(endpoint: RelationEndpoint): string | undefined {
  if (endpoint.kind === "external") {
    return endpoint.uri;
  }
  if (endpoint.kind === "asset") {
    return endpoint.uri ?? undefined;
  }
  return undefined;
}

function gap(assetId: string, predicate: string, message: string): ServiceDiscoveryGap {
  return { assetId, predicate, message };
}

function uniqueEvidenceTypes(items: EvidenceTypeDiscovery[]): EvidenceTypeDiscovery[] {
  return uniqueBy(items, (item) => item.asset.id);
}

function uniqueAssetViews(items: RelationAssetView[]): RelationAssetView[] {
  return uniqueBy(items, (item) => item.asset.id);
}

function uniqueBy<T>(items: T[], key: (item: T) => string): T[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    const itemKey = key(item);
    if (seen.has(itemKey)) {
      return false;
    }
    seen.add(itemKey);
    return true;
  });
}

function evidenceLabel(evidence: DiscoveryEvidence): string {
  switch (evidence.source) {
    case "http_header":
      return `HTTP ${evidence.header_name}${evidence.rel ? ` rel=${evidence.rel}` : ""}`;
    case "json_ld_predicate":
      return evidence.pointer ? `${evidence.predicate} at ${evidence.pointer}` : evidence.predicate;
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
