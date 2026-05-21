export type ProfileId =
  | "dcat-ap-3"
  | "dcat-ap-2"
  | "breg-dcat-ap"
  | "registry-relay-publisher-profile";

export type Presence = "found" | "missing" | "invalid" | "auth-required";
export type Origin = "standard" | "publisher-specific" | "unsupported";
export type ReadinessStatus = "ready" | "partial" | "missing" | "not-checked";
export type ValidationStatus = "not-run" | "running" | "valid" | "warnings" | "invalid";
export type MissingRank = "blocking" | "recommended" | "nice-to-have";
export type RecordType =
  | "participant"
  | "catalog"
  | "dataset"
  | "base-registry"
  | "distribution"
  | "service"
  | "operation-group"
  | "ogc-record-collection"
  | "ogc-feature-collection";

export interface SourceHint {
  label: string;
  term: string;
  artifactId: string;
  url?: string;
}

export interface FieldValue {
  id: string;
  label: string;
  value: string;
  source: SourceHint;
  publisherSpecific?: boolean;
}

export interface AtlasRecord {
  id: string;
  type: RecordType;
  name: string;
  publisher?: string;
  profile?: string;
  accessRights?: string;
  validation: ValidationStatus;
  readiness: ReadinessStatus;
  topMissingItem?: string;
  serviceCount: number;
  fields: FieldValue[];
  publisherFields: FieldValue[];
  raw: unknown;
  parentId?: string;
  conformsTo?: string[];
}

export interface ArtifactStatus {
  id: string;
  name: string;
  presence: Presence;
  origin: Origin;
  url?: string;
  microcopy: string;
  sourceStandard: string;
  assessment?: "complete" | "partial" | "not-parsed";
  error?: string;
}

export interface MissingItem {
  id: string;
  group: "Identity" | "Access" | "Policy" | "Trust" | "Lifecycle" | "Services" | "Validation";
  need: string;
  rank: MissingRank;
  status: "known" | "missing" | "partial" | "not-checked";
  source: string;
  standardUrl: string;
  shapeUrl?: string;
  recordId?: string;
  publisherSpecific?: boolean;
}

export interface ReadinessCategory {
  id: "discoverable" | "validatable" | "policy" | "trust" | "lifecycle";
  label: string;
  status: ReadinessStatus;
  evidenceCount: number;
  topMissingItems: MissingItem[];
  terms: string[];
  score?: number;
}

export interface GraphNode {
  id: string;
  label: string;
  type: RecordType | "policy" | "trust-artifact" | "validation-issue";
}

export interface GraphEdge {
  id: string;
  from: string;
  to: string;
  label: string;
}

export interface AtlasModel {
  sourceUrl: string;
  profile: ProfileId;
  catalogTitle: string;
  participantId?: string;
  records: AtlasRecord[];
  artifacts: ArtifactStatus[];
  missingItems: MissingItem[];
  readiness: ReadinessCategory[];
  graph: {
    nodes: GraphNode[];
    edges: GraphEdge[];
    budget: number;
    summarized: boolean;
    summary: string[];
  };
  rawCatalog?: unknown;
  openApi?: {
    title?: string;
    pathCount: number;
    securitySchemes: string[];
  };
  comparison?: ComparisonModel;
  validation: {
    status: ValidationStatus;
    message: string;
  };
  semanticDiscovery?: import("./semanticAssetDiscovery").AtlasDiscoveryReportSummary;
  discoveryEngine?: "semantic-asset-discovery" | "legacy-dcat";
}

export interface ComparisonField {
  recordId: string;
  recordName: string;
  fieldId: string;
  label: string;
  value: string;
  source: SourceHint;
  affectsReadiness: boolean;
  reason: string;
}

export interface ComparisonModel {
  coreFieldCount: number;
  publisherFieldCount: number;
  publisherFields: ComparisonField[];
  readinessImpact: "none" | "operator-context-only" | "mapped-standard-evidence";
  summary: string;
}

export interface ProxyFetchResult {
  ok: boolean;
  status: number;
  statusText: string;
  url: string;
  finalUrl?: string;
  contentType?: string;
  body: string;
  json?: unknown;
  error?: string;
  errorCode?: string;
}
