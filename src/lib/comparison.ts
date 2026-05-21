import type { AtlasRecord, ComparisonField, ComparisonModel } from "./types";

export function buildComparison(records: AtlasRecord[]): ComparisonModel {
  const publisherFields: ComparisonField[] = records.flatMap((record) =>
    record.publisherFields.map((field) => ({
      recordId: record.id,
      recordName: record.name,
      fieldId: field.id,
      label: field.label,
      value: field.value,
      source: field.source,
      affectsReadiness: false,
      reason: "Publisher-specific fields improve operator context but do not satisfy core semantic readiness unless explicitly mapped.",
    })),
  );
  const coreFieldCount = records.reduce((count, record) => count + record.fields.length, 0);

  return {
    coreFieldCount,
    publisherFieldCount: publisherFields.length,
    publisherFields,
    readinessImpact: publisherFields.length > 0 ? "operator-context-only" : "none",
    summary:
      publisherFields.length > 0
        ? "Publisher-specific fields add operator context and are tracked separately from core semantic metadata."
        : "No publisher-specific fields were discovered.",
  };
}
