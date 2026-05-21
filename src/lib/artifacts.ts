import { getValues, hasType, isObject, stringValue, type JsonLdObject } from "./jsonld";
import { originMicrocopy, presenceMicrocopy } from "./profiles";
import type { ArtifactStatus, Origin, Presence } from "./types";

interface ArtifactInput {
  id: string;
  name: string;
  presence: Presence;
  origin: Origin;
  sourceStandard: string;
  url?: string;
  assessment?: ArtifactStatus["assessment"];
  error?: string;
}

export function artifactStatus(input: ArtifactInput): ArtifactStatus {
  const originNote = originMicrocopy(input.origin);
  const presenceNote = presenceMicrocopy(input.presence);
  return {
    id: input.id,
    name: input.name,
    presence: input.presence,
    origin: input.origin,
    url: input.url,
    microcopy: `${presenceNote} ${originNote}`,
    sourceStandard: input.sourceStandard,
    assessment: input.assessment,
    error: input.error,
  };
}

export function detectArtifactStatuses(params: {
  catalogUrl: string;
  catalog: JsonLdObject;
  nodes: JsonLdObject[];
  hasOpenApi: boolean;
}): ArtifactStatus[] {
  const { catalog, catalogUrl, hasOpenApi, nodes } = params;
  const hasShapes = hasEmbeddedShapes(catalog, nodes);
  const odrlState = detectOdrlState(nodes);
  const hasDqv = nodes.some((node) => hasType(node, ["dqv:QualityMeasurement", "dqv:QualityAnnotation"]));
  const hasAdms = nodes.some((node) => getValues(node, ["adms:status"]).length > 0);
  const hasDpv = nodes.some((node) => getValues(node, ["dpv:hasLegalBasis", "dpv:hasPersonalDataCategory"]).length > 0);
  const hasApplicableLegislation = nodes.some((node) => getValues(node, ["dcatap:applicableLegislation"]).length > 0);
  const hasAccessRights = nodes.some((node) => getValues(node, ["dcterms:accessRights", "dcterms:rights", "dcterms:license"]).length > 0);
  const hasTrust = nodes.some(
    (node) =>
      hasType(node, ["did:Service", "VerifiableCredential", "https://www.w3.org/2018/credentials#VerifiableCredential"]) ||
      getValues(node, ["prov:wasGeneratedBy", "prov:wasAttributedTo", "schema:identifier"]).some(isTrustLike),
  );
  const hasOgcFeatures = nodes.some((node) =>
    getValues(node, ["dcterms:conformsTo", "dcat:accessService"]).some((value) =>
      String(isObject(value) ? value["@id"] ?? "" : value).toLowerCase().includes("ogcapi-features"),
    ),
  );
  const hasOgcRecords = nodes.some((node) =>
    getValues(node, ["dcterms:conformsTo", "dcterms:type", "dcat:endpointURL", "dcat:landingPage"]).some((value) => {
      const text = String(isObject(value) ? value["@id"] ?? "" : value).toLowerCase();
      return text.includes("ogcapi-records") || text.includes("ogc api records") || text.includes("ogcapi-records-1");
    }),
  );
  const hasSpDciPublisherMetadata = nodes.some((node) => getValues(node, ["relay:syncRoute", "registry_relay:registryName"]).length > 0);

  return [
    artifactStatus({
      id: "dcat-ap-jsonld",
      name: "DCAT-AP JSON-LD catalog",
      presence: "found",
      origin: "standard",
      sourceStandard: "DCAT-AP",
      url: catalogUrl,
      assessment: "complete",
    }),
    artifactStatus({
      id: "breg-dcat-ap",
      name: "BRegDCAT-AP registry metadata",
      presence: nodes.some((node) => hasType(node, ["dcat:Dataset"]) && getValues(node, ["dcterms:type"]).some(isBaseRegistry))
        ? "found"
        : "missing",
      origin: "standard",
      sourceStandard: "BRegDCAT-AP",
      assessment: "partial",
    }),
    artifactStatus({
      id: "ogc-api-records",
      name: "OGC API Records",
      presence: hasOgcRecords ? "found" : "missing",
      origin: hasOgcRecords ? "standard" : "unsupported",
      sourceStandard: "OGC API Records",
      assessment: hasOgcRecords ? "partial" : "not-parsed",
    }),
    artifactStatus({
      id: "openapi",
      name: "OpenAPI service description",
      presence: hasOpenApi ? "found" : "missing",
      origin: "standard",
      sourceStandard: "OpenAPI",
      assessment: hasOpenApi ? "complete" : undefined,
    }),
    artifactStatus({
      id: "ogc-api-features",
      name: "OGC API Features for spatial collections",
      presence: hasOgcFeatures ? "found" : "missing",
      origin: hasOgcFeatures ? "standard" : "unsupported",
      sourceStandard: "OGC API Features",
      assessment: hasOgcFeatures ? "partial" : "not-parsed",
    }),
    artifactStatus({
      id: "shacl",
      name: "SHACL validation profile or embedded shapes",
      presence: hasShapes ? "found" : "missing",
      origin: "standard",
      sourceStandard: "SHACL",
      assessment: hasShapes ? "partial" : undefined,
    }),
    artifactStatus({
      id: "odrl",
      name: "ODRL policy or offer",
      presence: odrlState === "missing" ? "missing" : "found",
      origin: "standard",
      sourceStandard: "ODRL",
      assessment: odrlState === "thin" ? "partial" : odrlState === "found" ? "complete" : undefined,
    }),
    artifactStatus({
      id: "access-rights",
      name: "Access rights statement",
      presence: hasAccessRights ? "found" : "missing",
      origin: "standard",
      sourceStandard: "Dublin Core / DCAT-AP",
      assessment: hasAccessRights ? "complete" : undefined,
    }),
    artifactStatus({
      id: "dqv",
      name: "DQV validation metadata",
      presence: hasDqv ? "found" : "missing",
      origin: "standard",
      sourceStandard: "DQV",
      assessment: hasDqv ? "partial" : undefined,
    }),
    artifactStatus({
      id: "adms",
      name: "ADMS lifecycle/status metadata",
      presence: hasAdms ? "found" : "missing",
      origin: "standard",
      sourceStandard: "ADMS",
      assessment: hasAdms ? "complete" : undefined,
    }),
    artifactStatus({
      id: "dpv",
      name: "Legal basis or data protection metadata",
      presence: hasDpv || hasApplicableLegislation ? "found" : "missing",
      origin: "standard",
      sourceStandard: hasDpv ? "DPV" : "DCAT-AP",
      assessment: hasDpv || hasApplicableLegislation ? "complete" : undefined,
    }),
    artifactStatus({
      id: "trust",
      name: "DID, VCDM, or provenance/trust metadata",
      presence: hasTrust ? "found" : "missing",
      origin: "standard",
      sourceStandard: "DID/VCDM/PROV",
      assessment: hasTrust ? "partial" : undefined,
    }),
    artifactStatus({
      id: "sp-dci",
      name: "Publisher-specific integration: SP DCI sync",
      presence: hasSpDciPublisherMetadata ? "found" : "missing",
      origin: hasSpDciPublisherMetadata ? "publisher-specific" : "unsupported",
      sourceStandard: "Publisher-specific OpenAPI-visible pattern",
      assessment: "not-parsed",
    }),
  ];
}

function hasEmbeddedShapes(catalog: JsonLdObject, nodes: JsonLdObject[]): boolean {
  return (
    getValues(catalog, ["sh:shapesGraph"]).length > 0 ||
    nodes.some((node) => hasType(node, ["sh:NodeShape", "sh:PropertyShape"]))
  );
}

function detectOdrlState(nodes: JsonLdObject[]): "found" | "thin" | "missing" {
  const policyNodes = nodes.filter((node) => hasType(node, ["odrl:Policy", "odrl:Offer", "odrl:Set"]));
  if (policyNodes.length === 0) {
    return nodes.some((node) => getValues(node, ["odrl:hasPolicy"]).length > 0) ? "thin" : "missing";
  }

  const detailed = policyNodes.some(hasDetailedOdrlPolicy);
  return detailed ? "found" : "thin";
}

function hasDetailedOdrlPolicy(node: JsonLdObject): boolean {
  if (getValues(node, ["odrl:profile", "odrl:prohibition", "odrl:duty"]).length > 0) {
    return true;
  }

  return getValues(node, ["odrl:permission"]).some((value) => {
    if (!isObject(value)) {
      return false;
    }
    return hasDetailedOdrlRule(value);
  });
}

function hasDetailedOdrlRule(rule: JsonLdObject): boolean {
  if (getValues(rule, ["odrl:constraint", "odrl:duty", "odrl:assignee"]).length > 0) {
    return true;
  }

  const actions = getValues(rule, ["odrl:action"]).map((value) => stringValue(value)?.toLowerCase()).filter(Boolean);
  return actions.some((action) => action !== "odrl:use" && action !== "http://www.w3.org/ns/odrl/2/use");
}

function isBaseRegistry(value: unknown): boolean {
  return String(isObject(value) ? value["@id"] ?? "" : value).toLowerCase().includes("base-registry");
}

function isTrustLike(value: unknown): boolean {
  const text = String(isObject(value) ? value["@id"] ?? "" : value).toLowerCase();
  return text.includes("did:") || text.includes("credential") || text.includes("provenance");
}
