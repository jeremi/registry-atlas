import { describe, expect, it } from "vitest";
import { buildServiceFirstDiscovery } from "../src/lib";
import type { AtlasDiscoveryReportSummary, AtlasSemanticAssetSummary, RelationClaim, SemanticRelation } from "../src/lib";

describe("service-first discovery helper", () => {
  it("preserves grouped CCCEV evidence options and route satisfaction", () => {
    const report = serviceFirstReport();

    const discovery = buildServiceFirstDiscovery(report);

    expect(discovery?.services).toHaveLength(1);
    const requirement = discovery?.services[0]?.requirements[0];
    expect(requirement?.evidenceBundles).toHaveLength(2);
    expect(requirement?.evidenceBundles[0]).toEqual(
      expect.objectContaining({
        label: "Birth and residence",
        satisfiable: true,
        missingEvidenceTypeIds: [],
      }),
    );
    expect(requirement?.evidenceBundles[0]?.evidenceTypes.map((item) => item.asset.label)).toEqual([
      "Birth certificate",
      "Proof of residence",
    ]);
    expect(requirement?.evidenceBundles[1]).toEqual(
      expect.objectContaining({
        label: "National card",
        satisfiable: false,
        missingEvidenceTypeIds: ["evidence:national-card"],
      }),
    );
    expect(discovery?.services[0]?.routes.map((route) => route.endpointUrl)).toEqual([
      "https://example.test/api/birth",
      "https://example.test/api/residence",
    ]);
  });
});

function serviceFirstReport(): AtlasDiscoveryReportSummary {
  const assets = [
    asset("service:family-benefit", "public_service", "Family benefit", "https://example.test/services/family-benefit"),
    asset("channel:online", "channel", "Online", "https://example.test/services/family-benefit/channels/online"),
    asset("requirement:child-proof", "requirement", "Child proof", "https://example.test/requirements/child-proof"),
    asset("list:birth-residence", "evidence_type_list", "Birth and residence", "https://example.test/requirements/child-proof#birth-residence"),
    asset("list:national-card", "evidence_type_list", "National card", "https://example.test/requirements/child-proof#national-card"),
    asset("evidence:birth", "evidence_type", "Birth certificate", "https://example.test/evidence-types/birth-certificate"),
    asset("evidence:residence", "evidence_type", "Proof of residence", "https://example.test/evidence-types/proof-of-residence"),
    asset("evidence:national-card", "evidence_type", "National card", "https://example.test/evidence-types/national-card"),
    asset("offering:birth", "evidence_offering", "Birth offering", "https://example.test/offerings/birth"),
    asset("offering:residence", "evidence_offering", "Residence offering", "https://example.test/offerings/residence"),
    asset("provider:civil", "evidence_provider", "Civil Registry", "https://example.test/providers/civil"),
    asset("service:birth-api", "data_service", "Birth API", "https://example.test/data-services/birth"),
    asset("service:residence-api", "data_service", "Residence API", "https://example.test/data-services/residence"),
  ];
  const relations = [
    relation("service:family-benefit", "cv:hasChannel", "channel:online"),
    relation("service:family-benefit", "cv:holdsRequirement", "requirement:child-proof"),
    relation("requirement:child-proof", "cccev:hasEvidenceTypeList", "list:birth-residence"),
    relation("requirement:child-proof", "cccev:hasEvidenceTypeList", "list:national-card"),
    relation("list:birth-residence", "cccev:specifiesEvidenceType", "evidence:birth"),
    relation("list:birth-residence", "cccev:specifiesEvidenceType", "evidence:residence"),
    relation("list:national-card", "cccev:specifiesEvidenceType", "evidence:national-card"),
    relation("offering:birth", "registry_manifest:evidenceType", "evidence:birth"),
    relation("offering:residence", "registry_manifest:evidenceType", "evidence:residence"),
    relation("offering:birth", "registry_manifest:providedBy", "provider:civil"),
    relation("offering:residence", "registry_manifest:providedBy", "provider:civil"),
    relation("offering:birth", "registry_manifest:evidenceService", "service:birth-api"),
    relation("offering:residence", "registry_manifest:evidenceService", "service:residence-api"),
    externalRelation("service:birth-api", "dcat:endpointURL", "https://example.test/api/birth"),
    externalRelation("service:residence-api", "dcat:endpointURL", "https://example.test/api/residence"),
  ];
  const relationClaims = relations.map(claim);

  return {
    schemaVersion: "semantic-asset-discovery.report.v2",
    runId: "run:test",
    entryUrl: "https://example.test/metadata/cpsv-ap",
    analyzedAt: "2026-05-25T00:00:00Z",
    counts: {
      artifact_count: 1,
      asset_count: assets.length,
      standard_count: 0,
      profile_count: 0,
      failed_artifact_count: 0,
      unsupported_artifact_count: 0,
      parse_error_count: 0,
      next_fetch_count: 0,
      truncated: false,
    },
    rejectedFetches: [],
    assets,
    relations,
    relationClaims,
    links: [],
    findings: [],
    standards: [],
    profiles: [],
    nextFetches: [],
  };
}

function asset(id: string, kind: AtlasSemanticAssetSummary["kind"], label: string, uri: string): AtlasSemanticAssetSummary {
  return {
    id,
    kind,
    label,
    artifactId: "artifact:cpsv",
    uri,
    conformsTo: [],
    sourceHints: [],
    rawReferences: [],
  };
}

function relation(subjectId: string, predicate: string, objectId: string): SemanticRelation {
  return {
    id: `relation:${subjectId}:${predicate}:${objectId}`,
    subject: { kind: "asset", asset_id: subjectId },
    predicate,
    object: { kind: "asset", asset_id: objectId },
  };
}

function externalRelation(subjectId: string, predicate: string, objectUri: string): SemanticRelation {
  return {
    id: `relation:${subjectId}:${predicate}:${objectUri}`,
    subject: { kind: "asset", asset_id: subjectId },
    predicate,
    object: { kind: "external", uri: objectUri },
  };
}

function claim(relation: SemanticRelation): RelationClaim {
  return {
    id: `claim:${relation.id}`,
    relation_id: relation.id,
    asserted_by_artifact_id: "artifact:cpsv",
    evidence: {
      source: "json_ld_predicate",
      artifact_id: "artifact:cpsv",
      predicate: relation.predicate,
      pointer: null,
      value: relation.id,
    },
    qualifiers: [],
    contradicts: [],
  };
}
