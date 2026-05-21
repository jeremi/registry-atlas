import { describe, expect, it } from "vitest";
import fixture from "../fixtures/system-capability/registry-relay-all-standards.envelope.json";
import { normalizeDiscoveryRunEnvelope, searchCapabilities } from "../src/lib";
import type { DiscoveryRunEnvelope } from "../src/lib";

describe("system capability discovery", () => {
  it("binds access evidence to the matched Registry Relay asset", () => {
    const report = normalizeDiscoveryRunEnvelope(fixture as DiscoveryRunEnvelope);
    const result = searchCapabilities(report);

    const farmer = result.needs.find((need) => need.need.id === "farmer_status");
    const disability = result.needs.find((need) => need.need.id === "disability_status");
    const attendance = result.needs.find((need) => need.need.id === "school_attendance");

    expect(farmer?.routes).toContainEqual(
      expect.objectContaining({
        sourceUrl: "http://127.0.0.1:4242/datasets/farmer_registry/farmer",
        accessKind: "dataset_distribution",
        role: "candidate_route",
      }),
    );
    expect(farmer?.routes[0]?.gaps).toEqual(
      expect.arrayContaining([
        "identifier unknown",
        "legal basis unknown",
        "authority unknown",
        "source of truth unknown",
        "freshness unknown",
      ]),
    );
    expect(disability?.routes.map((route) => route.sourceUrl)).toEqual([
      "http://127.0.0.1:4242/datasets/disability_registry/disabled_person",
    ]);
    expect(attendance?.routes).toContainEqual(
      expect.objectContaining({
        sourceUrl: "http://127.0.0.1:4242/datasets/education_registry/attendance_summary",
        accessKind: "dataset_distribution",
        role: "candidate_route",
      }),
    );
  });
});
