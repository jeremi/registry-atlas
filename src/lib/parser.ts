import { detectArtifactStatuses } from "./artifacts";
import { buildComparison } from "./comparison";
import { buildGraph } from "./graph";
import {
  asObject,
  contextualizeJsonLd,
  getFirstString,
  getId,
  getObjects,
  getStrings,
  getValues,
  flattenJsonLd,
  graphNodes,
  hasType,
  isPublisherSpecificKey,
  isObject,
  nodeId,
  stringValue,
  type JsonLdObject,
} from "./jsonld";
import { PROFILE_LABELS } from "./profiles";
import { buildMissingItems, buildReadiness } from "./readiness";
import type { AtlasModel, AtlasRecord, FieldValue, ProfileId, SourceHint, ValidationStatus } from "./types";

interface ParseOptions {
  sourceUrl: string;
  profile?: ProfileId;
  openApi?: AtlasModel["openApi"];
  validationStatus?: ValidationStatus;
}

const STANDARD_FIELD_TERMS = new Map<string, { label: string; terms: string[] }>([
  ["title", { label: "Title", terms: ["dcterms:title"] }],
  ["description", { label: "Description", terms: ["dcterms:description"] }],
  ["publisher", { label: "Publisher", terms: ["dcterms:publisher"] }],
  ["contactPoint", { label: "Contact point", terms: ["dcat:contactPoint"] }],
  ["accessRights", { label: "Access rights", terms: ["dcterms:accessRights"] }],
  ["applicableLegislation", { label: "Applicable legislation", terms: ["dcatap:applicableLegislation"] }],
  ["policy", { label: "Usage policy", terms: ["odrl:hasPolicy"] }],
  ["conformsTo", { label: "Conforms to", terms: ["dcterms:conformsTo"] }],
  ["theme", { label: "Theme", terms: ["dcat:theme"] }],
  ["keyword", { label: "Keyword", terms: ["dcat:keyword"] }],
  ["landingPage", { label: "Landing page", terms: ["dcat:landingPage"] }],
  ["endpointURL", { label: "Endpoint URL", terms: ["dcat:endpointURL"] }],
  ["endpointDescription", { label: "Endpoint description", terms: ["dcat:endpointDescription"] }],
  ["mediaType", { label: "Media type", terms: ["dcat:mediaType"] }],
  ["format", { label: "Format", terms: ["dcterms:format"] }],
  ["license", { label: "License", terms: ["dcterms:license"] }],
  ["status", { label: "Lifecycle status", terms: ["adms:status"] }],
  ["issued", { label: "Issued", terms: ["dcterms:issued"] }],
  ["modified", { label: "Modified", terms: ["dcterms:modified"] }],
]);

export function parseDcatJsonLd(document: unknown, options: ParseOptions): AtlasModel {
  const contextualDocument = contextualizeJsonLd(document);
  const root = asObject(contextualDocument);
  if (!root) {
    throw new Error("DCAT-AP JSON-LD parser expected a JSON object.");
  }

  const directNodes = graphNodes(contextualDocument);
  const catalog = findCatalog(root, directNodes);
  const nodes = mergeNodes(directNodes, flattenJsonLd(contextualDocument), [catalog]);
  const catalogId = nodeId(catalog, `${options.sourceUrl}#catalog`);
  const profile = options.profile ?? inferProfile(catalog);
  const catalogRecord = buildRecord(catalog, "catalog", catalogId, "Catalog", undefined, options.sourceUrl);
  const datasetRecords = collectDatasetRecords(catalog, nodes, catalogRecord.id, options.sourceUrl);
  const serviceRecords = collectServiceRecords(catalog, nodes, catalogRecord.id, options.sourceUrl);
  const distributionRecords = collectDistributionRecords(datasetRecords, nodes, options.sourceUrl);
  const records = [catalogRecord, ...datasetRecords, ...serviceRecords, ...distributionRecords];
  const missingItems = buildMissingItems(records, {
    hasOpenApi: Boolean(options.openApi),
    artifacts: detectArtifactStatuses({
      catalog,
      catalogUrl: options.sourceUrl,
      nodes,
      hasOpenApi: Boolean(options.openApi),
    }),
    validationStatus: options.validationStatus ?? "not-run",
  });
  const artifacts = detectArtifactStatuses({
    catalog,
    catalogUrl: options.sourceUrl,
    nodes,
    hasOpenApi: Boolean(options.openApi),
  });

  return {
    sourceUrl: options.sourceUrl,
    profile,
    catalogTitle: catalogRecord.name,
    participantId: getFirstString(catalog, ["dcterms:publisher", "foaf:maker"]),
    records,
    artifacts,
    missingItems,
    readiness: buildReadiness(missingItems, artifacts, options.validationStatus ?? "not-run"),
    graph: buildGraph(records),
    rawCatalog: contextualDocument,
    openApi: options.openApi,
    comparison: buildComparison(records),
    validation: {
      status: options.validationStatus ?? "not-run",
      message:
        options.validationStatus && options.validationStatus !== "not-run"
          ? "Validation results were supplied by a real validation run."
          : "Validation not yet run.",
    },
  };
}

export function inferProfile(catalog: JsonLdObject): ProfileId {
  const conformsTo = getStrings(catalog, ["dcterms:conformsTo"]).join(" ").toLowerCase();
  if (conformsTo.includes("bregdcat") || conformsTo.includes("base-registry")) {
    return "breg-dcat-ap";
  }
  if (conformsTo.includes("2.1.1")) {
    return "dcat-ap-2";
  }
  if (conformsTo.includes("dcat-ap") || conformsTo.includes("3.0.0")) {
    return "dcat-ap-3";
  }
  if (conformsTo.includes("registry-relay")) {
    return "registry-relay-publisher-profile";
  }
  return "dcat-ap-3";
}

function findCatalog(root: JsonLdObject, nodes: JsonLdObject[]): JsonLdObject {
  if (hasType(root, ["dcat:Catalog"])) {
    return root;
  }

  return nodes.find((node) => hasType(node, ["dcat:Catalog"])) ?? root;
}

function collectDatasetRecords(
  catalog: JsonLdObject,
  nodes: JsonLdObject[],
  catalogId: string,
  artifactId: string,
): AtlasRecord[] {
  const linkedDatasetIds = new Set(getValues(catalog, ["dcat:dataset"]).map(getId).filter(isDefined));
  const embeddedDatasets = getObjects(catalog, ["dcat:dataset"]);
  const datasetNodes = mergeNodes(
    nodes.filter((node) => hasType(node, ["dcat:Dataset"]) || linkedDatasetIds.has(nodeId(node, ""))),
    embeddedDatasets,
  );

  return datasetNodes.map((node, index) =>
    buildRecord(node, isBaseRegistry(node) ? "base-registry" : "dataset", nodeId(node, `${catalogId}/dataset/${index}`), "Dataset", catalogId, artifactId),
  );
}

function collectServiceRecords(
  catalog: JsonLdObject,
  nodes: JsonLdObject[],
  catalogId: string,
  artifactId: string,
): AtlasRecord[] {
  const linkedServiceIds = new Set(getValues(catalog, ["dcat:service"]).map(getId).filter(isDefined));
  const embeddedServices = getObjects(catalog, ["dcat:service"]);
  const serviceNodes = mergeNodes(
    nodes.filter((node) => hasType(node, ["dcat:DataService"]) || linkedServiceIds.has(nodeId(node, ""))),
    embeddedServices,
  );

  return serviceNodes.map((node, index) =>
    buildRecord(node, serviceType(node), nodeId(node, `${catalogId}/service/${index}`), "Data service", catalogId, artifactId),
  );
}

function collectDistributionRecords(datasetRecords: AtlasRecord[], nodes: JsonLdObject[], artifactId: string): AtlasRecord[] {
  const records: AtlasRecord[] = [];
  const nodeById = new Map(nodes.map((node) => [nodeId(node, ""), node]));

  for (const datasetRecord of datasetRecords) {
    const raw = asObject(datasetRecord.raw);
    if (!raw) {
      continue;
    }

    const distributionValues = getValues(raw, ["dcat:distribution"]);
    distributionValues.forEach((value, index) => {
      const embedded = asObject(value);
      const linked = getId(value) ? nodeById.get(getId(value) ?? "") : undefined;
      const node = embedded ?? linked;
      if (node) {
        const fallbackId = `${datasetRecord.id}/distribution/${index}`;
        records.push(buildRecord(node, distributionType(node), nodeId(node, fallbackId), "Distribution", datasetRecord.id, artifactId));
      }
    });
  }

  return records;
}

function buildRecord(
  node: JsonLdObject,
  type: AtlasRecord["type"],
  id: string,
  fallbackName: string,
  parentId: string | undefined,
  artifactId: string,
): AtlasRecord {
  const fields = buildStandardFields(node, artifactId, type);
  const publisherFields = buildPublisherFields(node, artifactId, type);
  const name =
    getFirstString(node, ["dcterms:title", "rdfs:label", "skos:prefLabel", "foaf:name"]) ??
    id.split(/[/#]/).filter(Boolean).at(-1) ??
    fallbackName;
  const accessRights = getFirstString(node, ["dcterms:accessRights"]);
  const conformsTo = getStrings(node, ["dcterms:conformsTo"]);

  return {
    id,
    type,
    name,
    publisher: getFirstString(node, ["dcterms:publisher", "foaf:maker"]),
    profile: profileLabel(conformsTo),
    accessRights,
    validation: "not-run",
    readiness: "not-checked",
    topMissingItem: undefined,
    serviceCount: getValues(node, ["dcat:service", "dcat:accessService"]).length,
    fields,
    publisherFields,
    raw: node,
    parentId,
    conformsTo,
  };
}

function buildStandardFields(node: JsonLdObject, artifactId: string, recordType: AtlasRecord["type"]): FieldValue[] {
  const fields: FieldValue[] = [];

  for (const [id, definition] of STANDARD_FIELD_TERMS.entries()) {
    for (const term of definition.terms) {
      const values = getValues(node, [term]).map(stringValue).filter(isDefined);
      values.forEach((value, index) => {
        fields.push({
          id: `${id}-${index}`,
          label: definition.label,
          value,
          source: sourceHint(recordType, term, artifactId),
        });
      });
    }
  }

  return fields;
}

function buildPublisherFields(node: JsonLdObject, artifactId: string, recordType: AtlasRecord["type"]): FieldValue[] {
  return Object.entries(node).flatMap(([key, value]) => {
    if (!isPublisherSpecificKey(key)) {
      return [];
    }
    return getValues({ [key]: value }, [key])
      .map(stringValue)
      .filter(isDefined)
      .map((fieldValue, index) => ({
        id: `${key}-${index}`,
        label: key,
        value: fieldValue,
        source: sourceHint(recordType, key, artifactId),
        publisherSpecific: true,
      }));
  });
}

function sourceHint(recordType: AtlasRecord["type"], term: string, artifactId: string): SourceHint {
  return {
    label: `${recordTypeLabel(recordType)} -> ${normalizeTerm(term)}`,
    term: normalizeTerm(term),
    artifactId,
  };
}

function normalizeTerm(term: string): string {
  return term.startsWith("dct:") ? term.replace("dct:", "dcterms:") : term;
}

function recordTypeLabel(recordType: AtlasRecord["type"]): string {
  switch (recordType) {
    case "catalog":
      return "dcat:Catalog";
    case "dataset":
    case "base-registry":
      return "dcat:Dataset";
    case "distribution":
      return "dcat:Distribution";
    case "service":
    case "ogc-feature-collection":
    case "ogc-record-collection":
      return "dcat:DataService";
    case "participant":
      return "foaf:Agent";
    case "operation-group":
      return "OpenAPI paths";
  }
}

function profileLabel(conformsTo: string[]): string | undefined {
  const text = conformsTo.join(" ").toLowerCase();
  if (text.includes("bregdcat") || text.includes("base-registry")) {
    return PROFILE_LABELS["breg-dcat-ap"];
  }
  if (text.includes("2.1.1")) {
    return PROFILE_LABELS["dcat-ap-2"];
  }
  if (text.includes("dcat-ap") || text.includes("3.0.0")) {
    return PROFILE_LABELS["dcat-ap-3"];
  }
  if (text.includes("registry-relay")) {
    return PROFILE_LABELS["registry-relay-publisher-profile"];
  }
  return undefined;
}

function isBaseRegistry(node: JsonLdObject): boolean {
  return getValues(node, ["dcterms:type", "dcterms:conformsTo"]).some((value) =>
    String(isObject(value) ? value["@id"] ?? "" : value).toLowerCase().includes("base-registry"),
  );
}

function serviceType(node: JsonLdObject): AtlasRecord["type"] {
  const text = getStrings(node, ["dcterms:conformsTo", "dcterms:type"]).join(" ").toLowerCase();
  if (text.includes("ogcapi-records") || text.includes("ogc api records")) {
    return "ogc-record-collection";
  }
  if (text.includes("ogcapi-features") || text.includes("ogc api features")) {
    return "ogc-feature-collection";
  }
  return "service";
}

function distributionType(node: JsonLdObject): AtlasRecord["type"] {
  const text = getStrings(node, ["dcterms:conformsTo", "dcat:mediaType", "dcterms:format"]).join(" ").toLowerCase();
  return text.includes("ogcapi-features") || text.includes("geo+json") ? "ogc-feature-collection" : "distribution";
}

function mergeNodes(...groups: JsonLdObject[][]): JsonLdObject[] {
  const byId = new Map<string, JsonLdObject>();
  const anonymous: JsonLdObject[] = [];
  for (const node of groups.flat()) {
    const id = typeof node["@id"] === "string" ? node["@id"] : undefined;
    if (id) {
      byId.set(id, { ...byId.get(id), ...node });
    } else {
      anonymous.push(node);
    }
  }
  return [...byId.values(), ...anonymous];
}

function isDefined<T>(value: T | undefined): value is T {
  return value !== undefined;
}
