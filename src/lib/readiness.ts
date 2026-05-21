import { STANDARD_URLS, SHAPE_REFS } from "./profiles";
import type { ArtifactStatus, AtlasRecord, MissingItem, ReadinessCategory, ValidationStatus } from "./types";

interface MissingContext {
  artifacts: ArtifactStatus[];
  hasOpenApi: boolean;
  validationStatus: ValidationStatus;
}

export function buildMissingItems(records: AtlasRecord[], context: MissingContext): MissingItem[] {
  const catalog = records.find((record) => record.type === "catalog");
  const datasets = records.filter((record) => record.type === "dataset" || record.type === "base-registry");
  const services = records.filter((record) => record.type === "service" || record.type === "ogc-feature-collection");
  const hasPolicy = artifact(context.artifacts, "odrl")?.presence === "found";
  const hasAccessRights = artifact(context.artifacts, "access-rights")?.presence === "found" || records.some((record) => hasField(record, "dcterms:accessRights"));
  const hasLifecycle = records.some((record) => hasField(record, "adms:status"));
  const hasTrust = artifact(context.artifacts, "trust")?.presence === "found";
  const hasDpv = artifact(context.artifacts, "dpv")?.presence === "found";

  return [
    item("identity-catalog", "Identity", "Catalogue identity", "blocking", catalog ? "known" : "missing", "dcat:Catalog", STANDARD_URLS.dcat, SHAPE_REFS.catalog, catalog?.id),
    item(
      "identity-dataset",
      "Identity",
      "Dataset identity",
      "blocking",
      datasets.length > 0 ? "known" : "missing",
      "dcat:Dataset URI",
      STANDARD_URLS.dcat,
      SHAPE_REFS.dataset,
      datasets[0]?.id,
    ),
    item(
      "identity-title",
      "Identity",
      "Title",
      "blocking",
      records.some((record) => hasField(record, "dcterms:title")) ? "known" : "missing",
      "dcterms:title",
      STANDARD_URLS.dcterms,
      SHAPE_REFS.dataset,
    ),
    item(
      "identity-publisher",
      "Identity",
      "Publisher",
      "blocking",
      records.some((record) => hasField(record, "dcterms:publisher")) ? "known" : "missing",
      "dcterms:publisher",
      STANDARD_URLS.dcterms,
      SHAPE_REFS.catalog,
    ),
    item(
      "access-endpoint",
      "Access",
      "Endpoint URL",
      "blocking",
      services.some((record) => hasField(record, "dcat:endpointURL")) ? "known" : "missing",
      "dcat:DataService, dcat:endpointURL",
      STANDARD_URLS.dcat,
      SHAPE_REFS.dataService,
      services[0]?.id,
    ),
    item(
      "access-openapi",
      "Access",
      "API operations",
      "recommended",
      context.hasOpenApi ? "known" : "missing",
      "OpenAPI paths",
      STANDARD_URLS.openApi,
    ),
    item(
      "policy-usage",
      "Policy",
      "Usage policy",
      "recommended",
      hasPolicy ? policyStatus(context.artifacts) : "missing",
      "ODRL",
      STANDARD_URLS.odrl,
    ),
    item(
      "policy-access-rights",
      "Policy",
      "Access rights statement",
      "recommended",
      hasAccessRights ? "known" : "missing",
      "dcterms:accessRights",
      STANDARD_URLS.dcterms,
    ),
    item(
      "policy-legal-basis",
      "Policy",
      "Legal basis or data protection metadata",
      "recommended",
      hasDpv ? "known" : "missing",
      "DPV or dcatap:applicableLegislation",
      STANDARD_URLS.dcatAp3,
    ),
    item(
      "trust-evidence",
      "Trust",
      "Trust evidence",
      "blocking",
      hasTrust ? "known" : "missing",
      "DID, VCDM, or provenance metadata",
      STANDARD_URLS.vcdm,
    ),
    item(
      "lifecycle-status",
      "Lifecycle",
      "Lifecycle status",
      "recommended",
      hasLifecycle ? "known" : "missing",
      "adms:status",
      STANDARD_URLS.adms,
      SHAPE_REFS.dataset,
    ),
    item(
      "lifecycle-contact",
      "Lifecycle",
      "Contact point",
      "recommended",
      records.some((record) => hasField(record, "dcat:contactPoint")) ? "known" : "missing",
      "dcat:contactPoint",
      STANDARD_URLS.dcat,
      SHAPE_REFS.catalog,
    ),
    item(
      "validation-profile",
      "Validation",
      "Validation results",
      "blocking",
      context.validationStatus === "not-run" ? "not-checked" : validationItemStatus(context.validationStatus),
      "SHACL validation run",
      STANDARD_URLS.shacl,
    ),
    item(
      "validation-timestamp",
      "Validation",
      "Validation timestamp",
      "recommended",
      artifact(context.artifacts, "dqv")?.presence === "found" ? "known" : "missing",
      "DQV",
      STANDARD_URLS.dqv,
    ),
  ];
}

export function buildReadiness(
  missingItems: MissingItem[],
  _artifacts: ArtifactStatus[],
  validationStatus: ValidationStatus,
): ReadinessCategory[] {
  const categoryDefs: Array<Omit<ReadinessCategory, "status" | "evidenceCount" | "topMissingItems" | "score">> = [
    { id: "discoverable", label: "Discoverable", terms: ["dcat:Catalog", "dcat:Dataset", "dcat:DataService", "dcat:endpointURL"] },
    { id: "validatable", label: "Validatable", terms: ["sh:shapesGraph", "SHACL validation run", "DQV"] },
    { id: "policy", label: "Policy", terms: ["dcterms:accessRights", "ODRL", "DPV"] },
    { id: "trust", label: "Trust", terms: ["DID", "VCDM 2.0", "prov:wasGeneratedBy"] },
    { id: "lifecycle", label: "Lifecycle", terms: ["adms:status", "dcat:contactPoint", "dcterms:modified"] },
  ];

  return categoryDefs.map((definition) => {
    const related = relatedItems(definition.id, missingItems);
    const knownCount = related.filter((item) => item.status === "known").length;
    const blockingMissing = related.some((item) => item.rank === "blocking" && item.status === "missing");
    const notChecked = definition.id === "validatable" && validationStatus === "not-run";
    const score = related.length === 0 ? 0 : Math.round((knownCount / related.length) * 100);
    const status: ReadinessCategory["status"] = notChecked
      ? "not-checked"
      : blockingMissing
        ? "missing"
        : knownCount === related.length
          ? "ready"
          : knownCount > 0 || related.some((item) => item.status === "partial")
            ? "partial"
            : "missing";

    return {
      ...definition,
      status,
      evidenceCount: knownCount,
      topMissingItems: related
        .filter((item) => item.status !== "known")
        .sort(rankSort)
        .slice(0, 3),
      score,
    };
  });
}

function relatedItems(category: ReadinessCategory["id"], items: MissingItem[]): MissingItem[] {
  switch (category) {
    case "discoverable":
      return items.filter((item) => ["Identity", "Access", "Services"].includes(item.group));
    case "validatable":
      return items.filter((item) => item.group === "Validation");
    case "policy":
      return items.filter((item) => item.group === "Policy");
    case "trust":
      return items.filter((item) => item.group === "Trust");
    case "lifecycle":
      return items.filter((item) => item.group === "Lifecycle");
  }
}

function item(
  id: string,
  group: MissingItem["group"],
  need: string,
  rank: MissingItem["rank"],
  status: MissingItem["status"],
  source: string,
  standardUrl: string,
  shapeUrl?: string,
  recordId?: string,
): MissingItem {
  return { id, group, need, rank, status, source, standardUrl, shapeUrl, recordId };
}

function hasField(record: AtlasRecord, term: string): boolean {
  return record.fields.some((field) => field.source.term === term);
}

function artifact(artifacts: ArtifactStatus[], id: string): ArtifactStatus | undefined {
  return artifacts.find((candidate) => candidate.id === id);
}

function policyStatus(artifacts: ArtifactStatus[]): MissingItem["status"] {
  return artifact(artifacts, "odrl")?.assessment === "partial" ? "partial" : "known";
}

function validationItemStatus(status: ValidationStatus): MissingItem["status"] {
  return status === "valid" || status === "warnings" ? "known" : status === "invalid" ? "partial" : "not-checked";
}

function rankSort(left: MissingItem, right: MissingItem): number {
  const weight: Record<MissingItem["rank"], number> = {
    blocking: 0,
    recommended: 1,
    "nice-to-have": 2,
  };
  return weight[left.rank] - weight[right.rank];
}
