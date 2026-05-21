import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { parseDcatJsonLd, STANDARD_URLS } from "../src/lib";

const fixturePath = join(process.cwd(), "public/fixtures/registry-relay-dcat-ap.jsonld");
const fixture = JSON.parse(readFileSync(fixturePath, "utf8")) as unknown;

describe("Registration Readiness", () => {
  it("keeps validation not checked until real validation results are supplied", () => {
    const model = parseDcatJsonLd(fixture, {
      sourceUrl: "https://registry.example.gov/metadata/dcat/bregdcat-ap",
    });

    const validatable = model.readiness.find((category) => category.id === "validatable");
    const validationItem = model.missingItems.find((item) => item.id === "validation-profile");

    expect(model.validation.status).toBe("not-run");
    expect(validatable?.status).toBe("not-checked");
    expect(validationItem?.status).toBe("not-checked");
  });

  it("groups and ranks known versus missing evidence with standards links and shape refs", () => {
    const model = parseDcatJsonLd(fixture, {
      sourceUrl: "https://registry.example.gov/metadata/dcat/bregdcat-ap",
      openApi: { title: "Registry Relay", pathCount: 3, securitySchemes: ["bearerAuth"] },
    });

    expect(model.missingItems).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          group: "Identity",
          need: "Dataset identity",
          rank: "blocking",
          status: "known",
          standardUrl: STANDARD_URLS.dcat,
          shapeUrl: expect.stringContaining("Dataset_Shape"),
        }),
        expect.objectContaining({
          group: "Trust",
          need: "Trust evidence",
          rank: "blocking",
          status: "missing",
          standardUrl: STANDARD_URLS.vcdm,
        }),
        expect.objectContaining({
          group: "Policy",
          need: "Usage policy",
          status: "known",
          source: "ODRL",
        }),
        expect.objectContaining({
          group: "Policy",
          need: "Access rights statement",
          status: "known",
          source: "dcterms:accessRights",
        }),
        expect.objectContaining({
          group: "Policy",
          need: "Legal basis or data protection metadata",
          status: "known",
          source: "DPV or dcatap:applicableLegislation",
        }),
      ]),
    );

    const policy = model.readiness.find((category) => category.id === "policy");
    const trust = model.readiness.find((category) => category.id === "trust");

    expect(policy?.status).toBe("ready");
    expect(policy?.topMissingItems).toEqual([]);
    expect(trust?.status).toBe("missing");
    expect(trust?.topMissingItems[0]?.need).toBe("Trust evidence");
  });

  it("does not mark readiness categories partial from unrelated artifacts", () => {
    const model = parseDcatJsonLd(
      {
        "@context": {
          dcat: "http://www.w3.org/ns/dcat#",
          dcterms: "http://purl.org/dc/terms/",
        },
        "@id": "https://example.gov/catalog",
        "@type": "dcat:Catalog",
        "dcterms:title": "Bare catalog",
        "dcat:dataset": {
          "@id": "https://example.gov/datasets/one",
          "@type": "dcat:Dataset",
          "dcterms:title": "Dataset one",
        },
      },
      { sourceUrl: "https://example.gov/catalog" },
    );

    expect(model.readiness.find((category) => category.id === "policy")?.status).toBe("missing");
    expect(model.readiness.find((category) => category.id === "trust")?.status).toBe("missing");
    expect(model.readiness.find((category) => category.id === "lifecycle")?.status).toBe("missing");
  });
});
