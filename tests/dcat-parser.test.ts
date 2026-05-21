import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { parseDcatJsonLd, PROFILE_LABELS } from "../src/lib";

const fixturePath = join(process.cwd(), "public/fixtures/registry-relay-dcat-ap.jsonld");
const fixture = JSON.parse(readFileSync(fixturePath, "utf8")) as unknown;

describe("DCAT-AP JSON-LD parser", () => {
  it("parses catalogs, datasets, services, distributions, embedded shapes, ODRL presence, and publisher-specific fields", () => {
    const model = parseDcatJsonLd(fixture, {
      sourceUrl: "https://registry.example.gov/metadata/dcat/bregdcat-ap",
    });

    expect(PROFILE_LABELS["dcat-ap-3"]).toBe("DCAT-AP 3.0.0");
    expect(PROFILE_LABELS["dcat-ap-2"]).toBe("DCAT-AP 2.1.1");
    expect(PROFILE_LABELS["breg-dcat-ap"]).toBe("BRegDCAT-AP");
    expect(PROFILE_LABELS["registry-relay-publisher-profile"]).toBe("Publisher-specific profile: Registry Relay");
    expect(model.profile).toBe("dcat-ap-3");
    expect(model.records.map((record) => record.name)).toEqual(
      expect.arrayContaining([
        "Government Demo Registry Relay (All Standards Demo)",
        "Benefits Casework",
        "Public Works Projects",
        "Benefit Case REST access service",
      ]),
    );
    expect(model.records.some((record) => record.type === "distribution")).toBe(true);

    const service = model.records.find((record) => record.name === "Benefit Case REST access service");
    expect(service?.fields.some((field) => field.source.term === "dcat:endpointURL")).toBe(true);

    expect(model.artifacts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: "shacl", presence: "found", origin: "standard" }),
        expect.objectContaining({ id: "odrl", presence: "found", assessment: "complete" }),
        expect.objectContaining({ id: "access-rights", presence: "found", assessment: "complete" }),
        expect.objectContaining({ id: "dpv", presence: "found", sourceStandard: "DCAT-AP" }),
        expect.objectContaining({ id: "adms", presence: "found", origin: "standard" }),
      ]),
    );
    expect(model.validation).toEqual({ status: "not-run", message: "Validation not yet run." });
  });

  it("resolves simple JSON-LD context aliases for core metadata fields", () => {
    const model = parseDcatJsonLd(
      {
        "@context": {
          dcat: "http://www.w3.org/ns/dcat#",
          dcterms: "http://purl.org/dc/terms/",
          title: "dcterms:title",
          dataset: "dcat:dataset",
          Catalog: "dcat:Catalog",
          Dataset: "dcat:Dataset",
        },
        "@id": "https://example.gov/catalog",
        "@type": "Catalog",
        title: "Aliased catalog",
        dataset: {
          "@id": "https://example.gov/datasets/one",
          "@type": "Dataset",
          title: "Aliased dataset",
        },
      },
      { sourceUrl: "https://example.gov/catalog" },
    );

    expect(model.catalogTitle).toBe("Aliased catalog");
    expect(model.records.map((record) => record.name)).toContain("Aliased dataset");
    expect(model.missingItems.find((item) => item.id === "identity-title")?.status).toBe("known");
  });

  it("reports OGC API Records when a catalog advertises records conformance", () => {
    const model = parseDcatJsonLd(
      {
        "@context": {
          dcat: "http://www.w3.org/ns/dcat#",
          dcterms: "http://purl.org/dc/terms/",
        },
        "@id": "https://example.gov/catalog",
        "@type": "dcat:Catalog",
        "dcterms:title": "Records catalog",
        "dcat:service": {
          "@id": "https://example.gov/records",
          "@type": "dcat:DataService",
          "dcterms:title": "Records API",
          "dcat:endpointURL": "https://example.gov/records",
          "dcterms:conformsTo": "http://www.opengis.net/spec/ogcapi-records-1/1.0/conf/core",
        },
      },
      { sourceUrl: "https://example.gov/catalog" },
    );

    expect(model.artifacts).toEqual(
      expect.arrayContaining([expect.objectContaining({ id: "ogc-api-records", presence: "found", origin: "standard" })]),
    );
    expect(model.records).toEqual(expect.arrayContaining([expect.objectContaining({ type: "ogc-record-collection" })]));
  });

  it("treats default ODRL use offers as thin policy evidence", () => {
    const model = parseDcatJsonLd(
      {
        "@context": {
          dcat: "http://www.w3.org/ns/dcat#",
          dcterms: "http://purl.org/dc/terms/",
          odrl: "http://www.w3.org/ns/odrl/2/",
        },
        "@id": "https://example.gov/metadata/dcat",
        "@type": "dcat:Catalog",
        "dcterms:title": "Default policy catalog",
        "dcat:dataset": {
          "@id": "https://example.gov/datasets/farmers",
          "@type": "dcat:Dataset",
          "dcterms:title": "Farmers",
          "odrl:hasPolicy": {
            "@id": "https://example.gov/datasets/farmers#offer",
            "@type": "odrl:Offer",
            "odrl:uid": "https://example.gov/datasets/farmers#offer",
            "odrl:assigner": { "@id": "https://example.gov" },
            "odrl:permission": {
              "odrl:action": { "@id": "odrl:use" },
              "odrl:assigner": { "@id": "https://example.gov" },
              "odrl:target": { "@id": "https://example.gov/datasets/farmers" },
            },
          },
        },
      },
      { sourceUrl: "https://example.gov/metadata/dcat" },
    );

    expect(model.artifacts).toEqual(
      expect.arrayContaining([expect.objectContaining({ id: "odrl", presence: "found", assessment: "partial" })]),
    );
    expect(model.missingItems.find((item) => item.id === "policy-usage")?.status).toBe("partial");
  });

  it("counts DCAT-AP applicable legislation as legal-basis evidence", () => {
    const model = parseDcatJsonLd(
      {
        "@context": {
          dcat: "http://www.w3.org/ns/dcat#",
          dcatap: "http://data.europa.eu/r5r/",
          dcterms: "http://purl.org/dc/terms/",
          odrl: "http://www.w3.org/ns/odrl/2/",
        },
        "@id": "https://example.gov/metadata/dcat",
        "@type": "dcat:Catalog",
        "dcterms:title": "Policy catalog",
        "dcat:dataset": {
          "@id": "https://example.gov/datasets/farmers",
          "@type": "dcat:Dataset",
          "dcterms:title": "Farmers",
          "dcterms:accessRights": { "@id": "http://publications.europa.eu/resource/authority/access-right/NON_PUBLIC" },
          "dcatap:applicableLegislation": { "@id": "https://example.gov/legislation/data-sharing" },
          "odrl:hasPolicy": {
            "@id": "https://example.gov/datasets/farmers#offer",
            "@type": "odrl:Offer",
            "odrl:uid": "https://example.gov/datasets/farmers#offer",
            "odrl:assigner": { "@id": "https://example.gov" },
            "odrl:profile": [{ "@id": "https://example.gov/odrl/profile/data-sharing" }],
            "odrl:permission": {
              "odrl:action": { "@id": "odrl:use" },
              "odrl:target": { "@id": "https://example.gov/datasets/farmers" },
              "odrl:constraint": {
                "odrl:leftOperand": { "@id": "odrl:purpose" },
                "odrl:operator": { "@id": "odrl:isA" },
                "odrl:rightOperand": { "@id": "https://example.gov/purpose/social-protection" },
              },
            },
          },
        },
      },
      { sourceUrl: "https://example.gov/metadata/dcat" },
    );

    expect(model.artifacts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: "odrl", presence: "found", assessment: "complete" }),
        expect.objectContaining({ id: "access-rights", presence: "found" }),
        expect.objectContaining({ id: "dpv", presence: "found", sourceStandard: "DCAT-AP" }),
      ]),
    );
    expect(model.missingItems.find((item) => item.id === "policy-legal-basis")?.status).toBe("known");
    expect(model.readiness.find((category) => category.id === "policy")?.status).toBe("ready");
  });

  it("builds core versus publisher-specific coverage data", () => {
    const model = parseDcatJsonLd(fixture, {
      sourceUrl: "https://registry.example.gov/metadata/dcat/bregdcat-ap",
    });

    expect(model.comparison.coreFieldCount).toBeGreaterThan(0);
    expect(model.comparison.publisherFieldCount).toBeGreaterThan(0);
    expect(model.comparison.readinessImpact).toBe("operator-context-only");
    expect(model.comparison.publisherFields.every((field) => field.affectsReadiness === false)).toBe(true);
  });
});
