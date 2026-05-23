import { FormEvent, ReactNode, useMemo, useState } from "react";
import {
  AlertTriangle,
  Archive,
  Braces,
  CheckCircle2,
  ChevronDown,
  CircleDashed,
  Database,
  Eye,
  FileJson,
  GitCompare,
  Globe2,
  KeyRound,
  Languages,
  Link2,
  ListChecks,
  Loader2,
  Network,
  PanelRightOpen,
  Play,
  Search,
  ShieldCheck,
  Table2,
  Undo2,
  Waypoints,
  XCircle,
} from "lucide-react";
import localDemoFixtureText from "./fixtures/registry-relay-dcat-ap.jsonld?raw";
import localDemoRunEnvelope from "./fixtures/registry-relay-system-capability.envelope.json";
import { discoverAtlas, discoveryRunEnvelopeToAtlasModel, getId, getValues, parseDcatJsonLd, searchCapabilities } from "./lib";
import type { AtlasDiscoveryReportSummary } from "./lib";
import type { CapabilitySearchResult } from "./lib";
import type { DiscoveryRunEnvelope } from "./lib";
import type {
  ArtifactStatus,
  AtlasModel,
  AtlasRecord,
  FieldValue,
  MissingItem,
  ProfileId,
  ReadinessStatus,
  ValidationStatus,
} from "./lib/types";

type DiscoverMode = "core" | "publisher";
type ViewState =
  | "empty"
  | "loading"
  | "ready"
  | "fetch-error"
  | "parse-error"
  | "auth-required"
  | "unsupported";
type WorkspaceTab = "overview" | "registry" | "capabilities" | "evidence";
type RegistryViewMode = "list" | "map";
type ComparisonMode = "core" | "publisher" | "diff";

interface AppDiscoveryRequest {
  url: string;
  bearerToken: string;
  profile: ProfileId;
  includePublisherMetadata: boolean;
  language: string;
}

interface Notice {
  title: string;
  message: string;
}

const profileLabels: Record<ProfileId, string> = {
  "dcat-ap-3": "DCAT-AP 3.0.0",
  "dcat-ap-2": "DCAT-AP 2.1.1",
  "breg-dcat-ap": "BRegDCAT-AP",
  "registry-relay-publisher-profile": "Publisher-specific profile: Registry Relay",
};

const validationLabels: Record<ValidationStatus, string> = {
  "not-run": "Validation not yet run",
  running: "Validation running",
  valid: "Valid",
  warnings: "Warnings",
  invalid: "Invalid",
};

const readinessLabels: Record<ReadinessStatus, string> = {
  ready: "Ready",
  partial: "Partial",
  missing: "Missing",
  "not-checked": "Not checked",
};

const curatedDemos = [
  {
    label: "Bundled Registry Relay discovery",
    url: "fixture:registry-relay-dcat-ap",
    note: "Semantic discovery fixture",
  },
  {
    label: "Registry Relay metadata index",
    url: "http://127.0.0.1:4242/metadata",
    note: "Publisher: Registry Relay",
  },
  {
    label: "OpenAPI service description",
    url: "http://127.0.0.1:4242/openapi.json",
    note: "Publisher: Registry Relay",
  },
  {
    label: "OGC API landing page",
    url: "http://127.0.0.1:4242/ogc/v1",
    note: "Spatial access method",
  },
];

let sessionRecentCatalogues: string[] = [];

async function discoverAtlasModel(request: AppDiscoveryRequest): Promise<AtlasModel> {
  if (request.url === "fixture:registry-relay-dcat-ap") {
    return discoveryRunEnvelopeToAtlasModel(localDemoRunEnvelope as DiscoveryRunEnvelope, request.profile);
  }
  const url = resolveCatalogueUrl(request.url);
  try {
    return await discoverAtlas(url, {
      profile: request.profile,
      bearerToken: request.bearerToken || undefined,
    });
  } catch (error) {
    const localDemoUnavailable =
      /^https?:\/\/(127\.0\.0\.1|localhost)(:\d+)?\/metadata(\/dcat(\/bregdcat-ap)?)?$/i.test(url) &&
      error instanceof Error;
    if (localDemoUnavailable) {
      return loadLocalDemoFixture(request);
    }
    throw error;
  }
}

function resolveCatalogueUrl(url: string): string {
  if (url === "fixture:registry-relay-dcat-ap") {
    return new URL("/fixtures/registry-relay-dcat-ap.jsonld", window.location.origin).toString();
  }
  return url;
}

function rememberCatalogue(url: string) {
  sessionRecentCatalogues = [url, ...sessionRecentCatalogues.filter((item) => item !== url)].slice(0, 5);
}

async function loadLocalDemoFixture(request: AppDiscoveryRequest): Promise<AtlasModel> {
  const document = await loadLocalDemoDocument();
  return parseDcatJsonLd(document, {
    sourceUrl: request.url,
    profile: request.profile,
    openApi: {
      title: "Source-provided OpenAPI",
      pathCount: 42,
      securitySchemes: ["bearerAuth"],
    },
  });
}

async function loadLocalDemoDocument(): Promise<unknown> {
  try {
    const response = await fetch("/fixtures/registry-relay-dcat-ap.jsonld");
    if (response.ok) {
      return (await response.json()) as unknown;
    }
  } catch {
    return JSON.parse(localDemoFixtureText) as unknown;
  }
  return JSON.parse(localDemoFixtureText) as unknown;
}

function statusTone(status: string) {
  if (["found", "ready", "valid", "known", "complete"].includes(status)) return "positive";
  if (["partial", "warnings", "not-checked", "not-run"].includes(status)) return "warning";
  if (["auth-required", "invalid", "missing"].includes(status)) return "danger";
  return "neutral";
}

function formatJson(value: unknown) {
  try {
    return JSON.stringify(value ?? {}, null, 2);
  } catch {
    return "Raw artifact could not be serialized.";
  }
}

function isCatalogRecord(record: AtlasRecord): boolean {
  return record.type === "catalog";
}

function isDatasetRecord(record: AtlasRecord): boolean {
  return record.type === "dataset" || record.type === "base-registry";
}

function isAccessMethodRecord(record: AtlasRecord): boolean {
  return (
    record.type === "service" ||
    record.type === "distribution" ||
    record.type === "ogc-feature-collection" ||
    record.type === "ogc-record-collection" ||
    record.type === "operation-group"
  );
}

function recordTypeLabel(record: AtlasRecord): string {
  switch (record.type) {
    case "catalog":
      return "catalog";
    case "dataset":
      return "dataset";
    case "base-registry":
      return "base registry";
    case "distribution":
      return "distribution";
    case "service":
      return "data service";
    case "ogc-feature-collection":
      return "OGC feature collection";
    case "ogc-record-collection":
      return "OGC records collection";
    case "operation-group":
      return "API operation group";
    case "participant":
      return "participant";
  }
}

function accessMethodTerm(record: AtlasRecord): string {
  switch (record.type) {
    case "distribution":
      return "dcat:Distribution";
    case "service":
    case "ogc-feature-collection":
    case "ogc-record-collection":
      return "dcat:DataService";
    case "operation-group":
      return "OpenAPI paths";
    default:
      return recordTypeLabel(record);
  }
}

function getAccessServiceIds(record: AtlasRecord): string[] {
  const raw =
    record.raw && typeof record.raw === "object" && !Array.isArray(record.raw)
      ? (record.raw as Parameters<typeof getValues>[0])
      : undefined;
  return getValues(raw, ["dcat:accessService"]).map(getId).filter((value): value is string => Boolean(value));
}

function getAccessMethodsForScope(records: AtlasRecord[], scope: AtlasRecord | null): AtlasRecord[] {
  if (!scope) {
    return [];
  }

  const directChildren = records.filter((record) => record.parentId === scope.id && isAccessMethodRecord(record));
  const linkedServiceIds = new Set(directChildren.flatMap(getAccessServiceIds));
  const linkedServices = records.filter((record) => linkedServiceIds.has(record.id));

  if (isCatalogRecord(scope)) {
    return uniqueRecords([
      ...directChildren,
      ...records.filter((record) => record.parentId === scope.id && record.type === "service"),
      ...linkedServices,
    ]);
  }

  if (isDatasetRecord(scope)) {
    return uniqueRecords([...directChildren, ...linkedServices]);
  }

  return [];
}

function uniqueRecords(records: AtlasRecord[]): AtlasRecord[] {
  return Array.from(new Map(records.map((record) => [record.id, record])).values());
}

export function App() {
  const [catalogUrl, setCatalogUrl] = useState("");
  const [bearerToken, setBearerToken] = useState("");
  const [profile, setProfile] = useState<ProfileId>("dcat-ap-3");
  const [mode, setMode] = useState<DiscoverMode>("core");
  const [language, setLanguage] = useState("en");
  const [viewState, setViewState] = useState<ViewState>("empty");
  const [notice, setNotice] = useState<Notice>({
    title: "Start from published catalogue metadata",
    message: "Paste a semantic metadata URL, use a curated demo, or open a recent in-memory session.",
  });
  const [model, setModel] = useState<AtlasModel | null>(null);
  const [activeTab, setActiveTab] = useState<WorkspaceTab>("overview");
  const [registryViewMode, setRegistryViewMode] = useState<RegistryViewMode>("list");
  const [catalogueScopeId, setCatalogueScopeId] = useState<string | null>(null);
  const [comparisonMode, setComparisonMode] = useState<ComparisonMode>("diff");
  const [selectedRecordId, setSelectedRecordId] = useState<string | null>(null);
  const [rawOpen, setRawOpen] = useState(false);
  const [recentCatalogues, setRecentCatalogues] = useState(sessionRecentCatalogues);

  const includePublisherMetadata = mode === "publisher";
  const selectedRecord = useMemo(
    () => model?.records.find((record) => record.id === selectedRecordId) ?? model?.records[0] ?? null,
    [model, selectedRecordId],
  );
  const selectedAccessMethods = useMemo(
    () => {
      if (!selectedRecord) {
        return [];
      }
      if (isCatalogRecord(selectedRecord) && catalogueScopeId !== selectedRecord.id) {
        return [];
      }
      return getAccessMethodsForScope(model?.records ?? [], selectedRecord);
    },
    [catalogueScopeId, model, selectedRecord],
  );

  const visibleRecords = useMemo(() => model?.records ?? [], [model]);
  const visibleArtifacts = useMemo(
    () => (includePublisherMetadata ? model?.artifacts : model?.artifacts.filter((artifact) => artifact.origin !== "publisher-specific")) ?? [],
    [includePublisherMetadata, model],
  );
  const visibleMissing = useMemo(
    () => (includePublisherMetadata ? model?.missingItems : model?.missingItems.filter((item) => !item.publisherSpecific)) ?? [],
    [includePublisherMetadata, model],
  );
  const capabilityResult = useMemo(
    () => (model?.semanticDiscovery ? searchCapabilities(model.semanticDiscovery) : null),
    [model],
  );

  async function handleDiscover(event?: FormEvent) {
    event?.preventDefault();
    const url = catalogUrl.trim();
    if (!url) {
      setViewState("empty");
      setNotice({
        title: "Catalogue URL required",
        message: "Enter a published semantic metadata, catalogue, OpenAPI, or OGC landing URL to begin discovery.",
      });
      return;
    }

    setViewState("loading");
    setNotice({
      title: "Discovering published artifacts",
      message: "Fetching through the discovery adapter. Bearer token stays in memory for this browser session only.",
    });

    try {
      const discovered = await discoverAtlasModel({
        url,
        bearerToken,
        profile,
        includePublisherMetadata,
        language,
      });
      rememberCatalogue(url);
      setRecentCatalogues([...sessionRecentCatalogues]);
      setModel(discovered);
      setSelectedRecordId(discovered.records[0]?.id ?? null);
      setActiveTab("overview");
      setRegistryViewMode("list");
      setCatalogueScopeId(null);
      setRawOpen(false);
      setViewState("ready");
      setNotice({
        title: discovered.catalogTitle,
        message: discovered.validation.message,
      });
    } catch (error) {
      const err = error instanceof Error ? error : new Error("Discovery failed.");
      const nextState: ViewState =
        err.name === "AuthRequired"
          ? "auth-required"
          : err.name === "ParseError"
            ? "parse-error"
            : err.name === "UnsupportedArtifact"
              ? "unsupported"
              : "fetch-error";
      setViewState(nextState);
      setNotice({ title: stateTitle(nextState), message: err.message });
    }
  }

  function loadUrl(url: string) {
    setCatalogUrl(url);
  }

  const validationStatus = model?.validation.status ?? "not-run";

  return (
    <div className="atlas-shell">
      <header className="topbar" aria-label="Discovery controls">
        <div className="brand-block">
          <div className="brand-mark">
            <Globe2 size={18} aria-hidden="true" />
          </div>
          <div>
            <p className="eyebrow">Registry Atlas</p>
            <h1>Semantic discovery workbench</h1>
          </div>
        </div>

        <form className="discovery-form" onSubmit={handleDiscover}>
          <label className="control control-url">
            <span>Catalogue URL</span>
            <div className="input-with-icon">
              <Link2 size={15} aria-hidden="true" />
              <input
                value={catalogUrl}
                onChange={(event) => setCatalogUrl(event.target.value)}
                placeholder="https://example.gov/metadata"
                aria-label="Catalogue URL"
              />
            </div>
          </label>

          <label className="control control-token">
            <span>Session bearer token</span>
            <div className="input-with-icon">
              <KeyRound size={15} aria-hidden="true" />
              <input
                value={bearerToken}
                onChange={(event) => setBearerToken(event.target.value)}
                type="password"
                autoComplete="off"
                placeholder="Memory only"
                aria-label="Session-only bearer token"
              />
            </div>
          </label>

          <label className="control">
            <span>Profile</span>
            <select value={profile} onChange={(event) => setProfile(event.target.value as ProfileId)} aria-label="Profile">
              {Object.entries(profileLabels).map(([id, label]) => (
                <option key={id} value={id}>
                  {label}
                </option>
              ))}
            </select>
          </label>

          <fieldset className="segmented" aria-label="Discovery mode">
            <legend>Mode</legend>
            <button
              type="button"
              className={mode === "core" ? "selected" : ""}
              onClick={() => setMode("core")}
            >
              Core metadata
            </button>
            <button
              type="button"
              className={mode === "publisher" ? "selected" : ""}
              onClick={() => setMode("publisher")}
            >
              Publisher metadata
            </button>
          </fieldset>

          <label className="control control-language">
            <span>Language</span>
            <div className="select-with-icon">
              <Languages size={15} aria-hidden="true" />
              <select value={language} onChange={(event) => setLanguage(event.target.value)} aria-label="Language">
                <option value="en">EN</option>
                <option value="fr">FR</option>
                <option value="es">ES</option>
              </select>
            </div>
          </label>

          <button className="discover-button" type="submit" disabled={viewState === "loading"}>
            {viewState === "loading" ? <Loader2 size={16} aria-hidden="true" /> : <Search size={16} aria-hidden="true" />}
            Discover
          </button>
        </form>

        <div className={`validation-badge ${statusTone(validationStatus)}`} aria-label="Validation badge">
          <ShieldCheck size={16} aria-hidden="true" />
          {validationLabels[validationStatus]}
        </div>
      </header>

      <section className="source-strip" aria-label="Catalog source shortcuts">
        <div className="source-strip-group">
          <span className="source-strip-label">Recents</span>
          <div className="source-chip-row">
            {recentCatalogues.length > 0 ? (
              recentCatalogues.map((url) => (
                <button key={url} className="source-chip" type="button" onClick={() => loadUrl(url)}>
                  <Archive size={13} aria-hidden="true" />
                  <span>{url}</span>
                </button>
              ))
            ) : (
              <span className="source-empty">Session-only history appears after discovery.</span>
            )}
          </div>
        </div>

        <div className="source-strip-group demos">
          <span className="source-strip-label">Demos</span>
          <div className="source-chip-row">
            {curatedDemos.map((demo) => (
              <button key={demo.url} className="source-chip demo" type="button" onClick={() => loadUrl(demo.url)}>
                <Database size={13} aria-hidden="true" />
                <span>{demo.label}</span>
                <small>{demo.note}</small>
              </button>
            ))}
          </div>
        </div>
      </section>

      <main className="workbench">
        <section className="workspace" aria-label="Center workspace">
          <div className="workspace-header">
            <div>
              <p className="eyebrow">Discovery result</p>
              <h2>{model?.catalogTitle ?? "No catalogue loaded"}</h2>
            </div>
            <div className="source-line">
              <FileJson size={15} aria-hidden="true" />
              {model?.sourceUrl ?? "Published artifact URL not selected"}
              {model?.discoveryEngine === "semantic-asset-discovery" ? <span className="engine-badge">semantic engine</span> : null}
            </div>
          </div>

          <nav className="tabs" aria-label="Workspace tabs">
            <TabButton
              id="overview"
              activeTab={activeTab}
              setActiveTab={setActiveTab}
              icon={<CheckCircle2 size={15} />}
              label="Overview"
            />
            <TabButton
              id="registry"
              activeTab={activeTab}
              setActiveTab={setActiveTab}
              icon={<Table2 size={15} />}
              label="Semantic assets"
            />
            <TabButton
              id="capabilities"
              activeTab={activeTab}
              setActiveTab={setActiveTab}
              icon={<Waypoints size={15} />}
              label="Capabilities"
            />
            <TabButton id="evidence" activeTab={activeTab} setActiveTab={setActiveTab} icon={<ListChecks size={15} />} label="Evidence" />
          </nav>

          <div className="workspace-body">
            {viewState !== "ready" || !model ? (
              <StateBlock state={viewState} notice={notice} />
            ) : activeTab === "overview" ? (
              <OverviewView model={model} missingItems={visibleMissing} includePublisherMetadata={includePublisherMetadata} />
            ) : activeTab === "registry" ? (
              <RegistryView
                model={model}
                records={visibleRecords}
                selectedId={selectedRecord?.id}
                scopeId={catalogueScopeId}
                mode={registryViewMode}
                setMode={setRegistryViewMode}
                onSelect={setSelectedRecordId}
                onScopeChange={setCatalogueScopeId}
              />
            ) : activeTab === "capabilities" ? (
              <CapabilitiesView result={capabilityResult} />
            ) : (
              <EvidenceView
                artifacts={visibleArtifacts}
                model={model}
                semanticReport={model.semanticDiscovery}
                records={visibleRecords}
                mode={comparisonMode}
                setMode={setComparisonMode}
                includePublisherMetadata={includePublisherMetadata}
                state={viewState}
                notice={notice}
              />
            )}
          </div>
        </section>

        <aside className="detail-panel" aria-label="Inspector">
          <div className="detail-header">
            <SectionTitle icon={<PanelRightOpen size={15} />} label="Inspector" />
            {selectedRecord ? <span className={`pill ${statusTone(selectedRecord.readiness)}`}>{readinessLabels[selectedRecord.readiness]}</span> : null}
          </div>

          {selectedRecord ? (
            <RecordDetail
              record={selectedRecord}
              accessMethods={selectedAccessMethods}
              includePublisherMetadata={includePublisherMetadata}
              rawOpen={rawOpen}
              setRawOpen={setRawOpen}
              onSelect={setSelectedRecordId}
            />
          ) : (
            <StateBlock state={viewState} notice={notice} compact />
          )}
        </aside>
      </main>
    </div>
  );
}

function SectionTitle({ icon, label }: { icon: ReactNode; label: string }) {
  return (
    <div className="section-title">
      {icon}
      <span>{label}</span>
    </div>
  );
}

function TabButton({
  id,
  activeTab,
  setActiveTab,
  icon,
  label,
}: {
  id: WorkspaceTab;
  activeTab: WorkspaceTab;
  setActiveTab: (tab: WorkspaceTab) => void;
  icon: ReactNode;
  label: string;
}) {
  return (
    <button type="button" className={activeTab === id ? "active" : ""} onClick={() => setActiveTab(id)}>
      {icon}
      {label}
    </button>
  );
}

function ArtifactCard({ artifact }: { artifact: ArtifactStatus }) {
  return (
    <article className={`artifact-card ${artifact.origin === "publisher-specific" ? "publisher-specific" : ""}`}>
      <div className="artifact-title">
        <span className={`evidence-dot ${statusTone(artifact.presence)}`} aria-hidden="true" />
        <strong>{artifact.name}</strong>
      </div>
      <div className="axis-pills">
        <span className={`pill ${statusTone(artifact.presence)}`}>{artifact.presence.replace("-", " ")}</span>
        <span className={`pill ${statusTone(artifact.origin)}`}>{artifactOriginLabel(artifact.origin)}</span>
      </div>
      {artifact.presence === "found" ? null : <p>{artifact.microcopy}</p>}
      <small>{artifact.sourceStandard}</small>
    </article>
  );
}

function artifactOriginLabel(origin: ArtifactStatus["origin"]): string {
  switch (origin) {
    case "standard":
      return "Recognized metadata";
    case "publisher-specific":
      return "Publisher-specific";
    case "unsupported":
      return "Follow-up evidence";
  }
}

function StateBlock({ state, notice, compact = false }: { state: ViewState; notice: Notice; compact?: boolean }) {
  const icon = stateIcon(state);
  return (
    <div className={`state-block ${compact ? "compact" : ""} ${state}`}>
      {icon}
      <h3>{notice.title}</h3>
      <p>{notice.message}</p>
      {state === "empty" ? (
        <p className="muted">Atlas reads published semantic metadata first. It does not browse protected row-level data.</p>
      ) : null}
      {state === "auth-required" ? (
        <p className="muted">Add a session bearer token in the top bar and run discovery again.</p>
      ) : null}
      {state === "unsupported" ? (
        <p className="muted">Unparsed artifacts stay visible as follow-up evidence instead of being counted as missing catalog metadata.</p>
      ) : null}
    </div>
  );
}

function stateTitle(state: ViewState) {
  const titles: Record<ViewState, string> = {
    empty: "Start from published catalogue metadata",
    loading: "Discovering published artifacts",
    ready: "Catalogue loaded",
    "fetch-error": "Fetch error",
    "parse-error": "Parse error",
    "auth-required": "Authentication required",
    unsupported: "Unsupported artifact",
  };
  return titles[state];
}

function stateIcon(state: ViewState) {
  if (state === "loading") return <Loader2 size={26} aria-hidden="true" />;
  if (state === "fetch-error" || state === "parse-error") return <XCircle size={26} aria-hidden="true" />;
  if (state === "auth-required") return <KeyRound size={26} aria-hidden="true" />;
  if (state === "unsupported") return <CircleDashed size={26} aria-hidden="true" />;
  return <Play size={26} aria-hidden="true" />;
}

function RecordCardList({
  records,
  selectedId,
  onSelect,
  onScopeChange,
  compact = false,
}: {
  records: AtlasRecord[];
  selectedId?: string;
  onSelect: (id: string) => void;
  onScopeChange?: (id: string) => void;
  compact?: boolean;
}) {
  if (records.length === 0) {
    return <p className="muted">No records published for this section.</p>;
  }

  return (
    <div className={`record-card-list ${compact ? "compact" : ""}`}>
      {records.map((record) => (
        <button
          key={record.id}
          type="button"
          aria-label={record.name}
          className={`record-card node-${record.type} ${selectedId === record.id ? "selected" : ""}`}
          onClick={() => {
            onSelect(record.id);
            onScopeChange?.(record.id);
          }}
        >
          <Network size={14} aria-hidden="true" />
          <span>{record.name}</span>
          <small>{recordTypeLabel(record)}</small>
        </button>
      ))}
    </div>
  );
}

function GraphView({
  model,
  records,
  selectedId,
  scope,
  accessMethods,
  onSelect,
  onScopeChange,
}: {
  model: AtlasModel;
  records: AtlasRecord[];
  selectedId?: string;
  scope: AtlasRecord | null;
  accessMethods: AtlasRecord[];
  onSelect: (id: string) => void;
  onScopeChange: (id: string) => void;
}) {
  const tooLarge = model.graph.summarized || model.graph.nodes.length > model.graph.budget;
  const catalogRecords = records.filter(isCatalogRecord);
  const datasetRecords = records.filter(isDatasetRecord);
  const columns = [
    {
      id: "catalogues",
      label: "Catalog",
      records: catalogRecords,
    },
    {
      id: "datasets",
      label: "Registerable semantic assets",
      records: datasetRecords,
    },
    ...(scope
      ? [
          {
            id: "access-methods",
            label: "Access methods",
            records: accessMethods,
          },
        ]
      : []),
  ];
  const visibleRecordIds = new Set(columns.flatMap((column) => column.records.map((record) => record.id)));
  const visibleEdges = model.graph.edges
    .filter((edge) => visibleRecordIds.has(edge.from) && visibleRecordIds.has(edge.to))
    .slice(0, 12);
  const hiddenEdgeCount = Math.max(
    model.graph.edges.filter((edge) => visibleRecordIds.has(edge.from) && visibleRecordIds.has(edge.to)).length - visibleEdges.length,
    0,
  );

  if (tooLarge) {
    return (
      <div className="graph-fallback">
        <AlertTriangle size={24} aria-hidden="true" />
        <h3>Graph budget exceeded</h3>
        <p>Graph rendering is scoped to about {model.graph.budget.toLocaleString()} nodes. This catalogue is summarized instead.</p>
        <div className="summary-grid">
          {model.graph.summary.map((item) => (
            <span key={item}>{item}</span>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="graph-view">
      <div className="graph-summary">
        <span>{model.records.length} records</span>
        <span>{model.graph.edges.length} relationships</span>
        <span>Budget {model.graph.budget.toLocaleString()} nodes</span>
      </div>

      <div className="graph-canvas" aria-label="Scoped semantic asset graph">
        {columns.map((column) => (
          <section key={column.id} className="graph-column">
            <div className="graph-column-header">
              <span>{column.label}</span>
              <strong>{column.records.length}</strong>
            </div>
            <div className="graph-stack">
              {column.records.slice(0, 14).map((record) => (
                <button
                  key={record.id}
                  type="button"
                  aria-label={record.name}
                  className={`graph-node node-${record.type} ${selectedId === record.id ? "selected" : ""}`}
                  onClick={() => {
                    onSelect(record.id);
                    if (isCatalogRecord(record) || isDatasetRecord(record)) {
                      onScopeChange(record.id);
                    }
                  }}
                  title={record.name}
                >
                  <Network size={14} aria-hidden="true" />
                  <span>{record.name}</span>
                  <small>{recordTypeLabel(record)}</small>
                </button>
              ))}
              {column.records.length > 14 ? <span className="graph-more">+{column.records.length - 14} more</span> : null}
            </div>
          </section>
        ))}
      </div>
      <div className="edge-list">
        {visibleEdges.map((edge) => (
          <span key={edge.id}>
            {shortenGraphId(edge.from)}
            {" -> "}
            {shortenGraphId(edge.to)}
            <strong>{edge.label}</strong>
          </span>
        ))}
        {hiddenEdgeCount > 0 ? <span>+{hiddenEdgeCount} more relationships</span> : null}
      </div>
    </div>
  );
}

function shortenGraphId(value: string): string {
  const normalized = value.replace(/[#/?=&]+/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  return parts.at(-1) ?? value;
}

function RegistryView({
  model,
  records,
  selectedId,
  scopeId,
  mode,
  setMode,
  onSelect,
  onScopeChange,
}: {
  model: AtlasModel;
  records: AtlasRecord[];
  selectedId?: string;
  scopeId: string | null;
  mode: RegistryViewMode;
  setMode: (mode: RegistryViewMode) => void;
  onSelect: (id: string) => void;
  onScopeChange: (id: string | null) => void;
}) {
  const catalogRecords = records.filter(isCatalogRecord);
  const datasetRecords = records.filter(isDatasetRecord);
  const accessMethodRecords = records.filter(isAccessMethodRecord);
  const catalogRecord = catalogRecords[0] ?? null;
  const scopedRecord = scopeId ? records.find((record) => record.id === scopeId) ?? null : null;
  const scopedDataset = scopedRecord && isDatasetRecord(scopedRecord) ? scopedRecord : null;
  const accessMethods = getAccessMethodsForScope(records, scopedRecord);
  const selectedAccessMethod =
    selectedId && accessMethods.some((record) => record.id === selectedId)
      ? records.find((record) => record.id === selectedId) ?? null
      : null;

  function clearScope() {
    onScopeChange(null);
    if (catalogRecord) {
      onSelect(catalogRecord.id);
    }
  }

  return (
    <div className="registry-view">
      <header className="registry-overview">
        <div className="catalogue-context">
          <p className="eyebrow">Semantic assets</p>
          <h3>{catalogRecord?.name ?? model.catalogTitle}</h3>
          <p>{catalogRecord?.publisher ? `${catalogRecord.publisher} metadata publication` : "Published catalogue metadata"}</p>
          {catalogRecord ? (
            <button type="button" className="inline-button" onClick={clearScope}>
              Inspect source metadata
            </button>
          ) : null}
        </div>

        <div className="view-metrics" aria-label="Semantic asset summary">
          <span>
            <strong>{records.length}</strong>
            records
          </span>
          <span>
            <strong>{datasetRecords.length}</strong>
            registerable assets
          </span>
          <span>
            <strong>{accessMethodRecords.length}</strong>
            access entries
          </span>
        </div>

        <div className="view-actions">
          {scopedDataset ? (
            <button type="button" className="secondary-button" onClick={clearScope}>
              <Undo2 size={14} aria-hidden="true" />
              Overview
            </button>
          ) : null}
          <fieldset className="segmented local" aria-label="Semantic asset view">
            <legend>Semantic asset view</legend>
            <button type="button" className={mode === "list" ? "selected" : ""} onClick={() => setMode("list")}>
              List
            </button>
            <button type="button" className={mode === "map" ? "selected" : ""} onClick={() => setMode("map")}>
              Map
            </button>
          </fieldset>
        </div>
      </header>

      <div className="drilldown-path" aria-label="Selected semantic asset path">
        <Waypoints size={15} aria-hidden="true" />
        <span>Semantic assets</span>
        {scopedDataset ? (
          <>
            <strong>{scopedDataset.name}</strong>
            {selectedAccessMethod ? <strong>{selectedAccessMethod.name}</strong> : null}
          </>
        ) : (
          <strong>Select a semantic asset to see its access methods</strong>
        )}
      </div>

      {mode === "list" ? (
        <div className="registry-browser">
          <section className="catalogue-pane">
            <div className="catalogue-section-header">
              <h3>Registerable semantic assets</h3>
              <span>{datasetRecords.length}</span>
            </div>
            <RecordCardList records={datasetRecords} selectedId={selectedId} onSelect={onSelect} onScopeChange={onScopeChange} />
          </section>

          <section className={`catalogue-pane access-method-pane ${scopedDataset ? "active" : ""}`}>
            <div className="catalogue-section-header">
              <h3>Access methods</h3>
              <span>{scopedDataset ? accessMethods.length : "Select a dataset"}</span>
            </div>
            {scopedDataset ? (
              <>
                <p className="muted">
                  Published DCAT distributions and data services for <strong>{scopedDataset.name}</strong>.
                </p>
                <RecordCardList records={accessMethods} selectedId={selectedId} onSelect={onSelect} compact />
              </>
            ) : (
              <div className="empty-pane">
                <Network size={24} aria-hidden="true" />
                <h3>No dataset selected</h3>
                <p>Choose a semantic asset on the left. Atlas will show only the access methods that belong to it.</p>
              </div>
            )}
          </section>
        </div>
      ) : (
        <GraphView
          model={model}
          records={records}
          selectedId={selectedId}
          scope={scopedDataset}
          accessMethods={accessMethods}
          onSelect={onSelect}
          onScopeChange={onScopeChange}
        />
      )}
    </div>
  );
}

function CapabilitiesView({ result }: { result: CapabilitySearchResult | null }) {
  if (!result) {
    return (
      <div className="capability-view">
        <StateBlock
          state="empty"
          notice={{
            title: "Capability discovery needs semantic evidence",
            message: "Run semantic discovery first. The strict matcher uses accepted terms only.",
          }}
          compact
        />
      </div>
    );
  }

  return (
    <div className="capability-view">
      <header className="view-intro">
        <div>
          <p className="eyebrow">Strict capability discovery</p>
          <h3>Candidate answer routes</h3>
        </div>
        <p>Question text is shown for context only. Matches come from accepted terms and machine-verifiable metadata evidence.</p>
      </header>

      <div className="capability-grid">
        {result.needs.map(({ need, routes }) => (
          <section key={need.id} className="capability-need" aria-label={`${need.label} capability routes`}>
            <header>
              <div>
                <p className="eyebrow">{need.id}</p>
                <h3>{need.label}</h3>
              </div>
              <span className={`pill ${routes.length > 0 ? "positive" : "warning"}`}>
                {routes.length} route{routes.length === 1 ? "" : "s"}
              </span>
            </header>
            <p className="question-context">{need.question}</p>
            <div className="accepted-terms">
              {[...need.requiresAny, ...(need.requiresAll ?? [])].map((term) => (
                <span key={`${term.kind}:${term.value}`}>{term.kind}: {term.value}</span>
              ))}
            </div>
            {routes.length > 0 ? (
              <div className="capability-routes">
                {routes.slice(0, 4).map((route) => (
                  <article key={route.id} className="capability-route">
                    <div>
                      <h4>{route.label}</h4>
                      <span className={`pill ${route.confidence === "high" ? "positive" : route.confidence === "medium" ? "warning" : "neutral"}`}>
                        {route.confidence}
                      </span>
                    </div>
                    <p>{route.role.replaceAll("_", " ")} / {route.accessKind.replaceAll("_", " ")}</p>
                    {route.sourceUrl ? <code>{route.sourceUrl}</code> : null}
                    <dl>
                      <dt>Evidence</dt>
                      <dd>{route.evidence.map((item) => item.location).slice(0, 2).join(", ")}</dd>
                      <dt>Gaps</dt>
                      <dd>{route.gaps.slice(0, 3).join(", ")}</dd>
                    </dl>
                    {route.reviewFlags.length > 0 ? (
                      <div className="review-flags">
                        {route.reviewFlags.map((flag) => (
                          <span key={flag}>{flag}</span>
                        ))}
                      </div>
                    ) : null}
                  </article>
                ))}
              </div>
            ) : (
              <p className="muted">No route matched the accepted terms. The question text was not searched.</p>
            )}
          </section>
        ))}
      </div>
    </div>
  );
}

function EvidenceView({
  artifacts,
  model,
  semanticReport,
  records,
  mode,
  setMode,
  includePublisherMetadata,
  state,
  notice,
}: {
  artifacts: ArtifactStatus[];
  model: AtlasModel;
  semanticReport?: AtlasDiscoveryReportSummary;
  records: AtlasRecord[];
  mode: ComparisonMode;
  setMode: (mode: ComparisonMode) => void;
  includePublisherMetadata: boolean;
  state: ViewState;
  notice: Notice;
}) {
  const retrievedCount = artifacts.filter((artifact) => artifact.presence === "found").length;
  const followUpCount = artifacts.filter((artifact) => artifact.presence !== "found").length;
  const hiddenPublisherArtifactCount = model.artifacts.filter((artifact) => artifact.origin === "publisher-specific").length;
  const recognizedArtifacts = artifacts.filter((artifact) => artifact.origin === "standard");
  const publisherArtifacts = includePublisherMetadata ? model.artifacts.filter((artifact) => artifact.origin === "publisher-specific") : [];
  const unsupportedArtifacts = artifacts.filter((artifact) => artifact.origin === "unsupported");

  return (
    <div className="evidence-view">
      <header className="view-intro">
        <div>
          <p className="eyebrow">Discovery trail</p>
          <h3>Semantic evidence</h3>
        </div>
        <p>Published artifacts, extracted links, and parser findings used to build the semantic asset view.</p>
      </header>

      <section className="evidence-panel">
        <SectionTitle icon={<ListChecks size={15} />} label="Published metadata trail" />
        <div className="evidence-summary" aria-label="Evidence source counts">
          <span>
            <strong>{retrievedCount}</strong>
            retrieved
          </span>
          <span>
            <strong>{followUpCount}</strong>
            follow-up
          </span>
        </div>

        {artifacts.length > 0 ? (
          <div className="evidence-groups">
            <EvidenceGroup title="Recognized metadata artifacts" artifacts={recognizedArtifacts} emptyText="No recognized metadata artifacts are visible." />
            <EvidenceGroup
              title="Publisher-specific metadata"
              artifacts={publisherArtifacts}
              emptyText={includePublisherMetadata ? "No publisher-specific metadata was discovered." : "Publisher-specific metadata is hidden in core metadata mode."}
            />
            <EvidenceGroup
              title="Follow-up or unparsed artifacts"
              artifacts={unsupportedArtifacts}
              emptyText="No unparsed follow-up artifacts are visible."
            />
          </div>
        ) : (
          <StateBlock state={state} notice={notice} compact />
        )}
        {!includePublisherMetadata && hiddenPublisherArtifactCount > 0 ? (
          <p className="publisher-hidden">
            <Eye size={14} aria-hidden="true" />
            {hiddenPublisherArtifactCount} publisher-specific artifact{hiddenPublisherArtifactCount === 1 ? "" : "s"} hidden in core metadata mode.
          </p>
        ) : null}
      </section>

      <section className="evidence-panel" aria-label="Semantic asset coverage">
        <SectionTitle icon={<GitCompare size={15} />} label="Semantic asset coverage" />
        <ComparisonView records={records} mode={mode} setMode={setMode} includePublisherMetadata={includePublisherMetadata} />
      </section>

      {semanticReport ? <SemanticDiscoveryView report={semanticReport} /> : null}
    </div>
  );
}

function SemanticDiscoveryView({ report }: { report: AtlasDiscoveryReportSummary }) {
  const visibleFindings = report.findings.slice(0, 6);
  const visibleLinks = report.links.slice(0, 6);
  const visibleFetches = report.nextFetches.slice(0, 6);
  const visibleRejectedFetches = report.rejectedFetches.slice(0, 6);

  return (
    <section className="evidence-panel technical-report" aria-label="Semantic discovery report">
      <details>
        <summary>
          <Braces size={15} aria-hidden="true" />
          <span>Semantic discovery report</span>
          <small>Semantic assets, profiles, standards claims, links, fetch plan, and parser findings</small>
        </summary>

        <div className="semantic-summary" aria-label="Semantic discovery counts">
          <span>
            <strong>{report.counts.artifact_count}</strong>
            artifacts
          </span>
          <span>
            <strong>{report.counts.asset_count}</strong>
            semantic assets
          </span>
          <span>
            <strong>{report.counts.profile_count}</strong>
            profiles
          </span>
          <span>
            <strong>{report.counts.next_fetch_count}</strong>
            fetch candidates
          </span>
        </div>

        <div className="semantic-grid">
          <SemanticList
            title="Profiles"
            items={report.profiles.map((profile) => ({
              id: profile.id,
              label: profile.label ?? profile.iri,
              detail: profile.base_standard_iri ?? profile.iri,
            }))}
            emptyText="No profile claims were declared."
          />
          <SemanticList
            title="Standards"
            items={report.standards.map((standard) => ({
              id: standard.id,
              label: standard.label ?? standard.iri,
              detail: standard.version ?? standard.iri,
            }))}
            emptyText="No standalone standards claims were declared."
          />
        </div>

        <div className="semantic-grid">
          <SemanticList
            title="Declared links"
            items={visibleLinks.map((link) => ({
              id: link.id,
              label: link.label,
              detail: link.toUrl,
            }))}
            emptyText="No declared links were extracted."
          />
          <SemanticList
            title="Fetch candidates"
            items={visibleFetches.map((candidate) => ({
              id: candidate.id,
              label: candidate.reason,
              detail: candidate.url,
            }))}
            emptyText="No additional fetch candidates remain."
          />
        </div>

        {report.fetched || visibleRejectedFetches.length > 0 ? (
          <div className="semantic-grid">
            <SemanticList
              title="Fetch summary"
              items={
                report.fetched
                  ? [
                      {
                        id: "fetch-summary",
                        label: `${report.fetched.fetched_count} fetched, ${report.fetched.rejected_count} rejected`,
                        detail: `${report.fetched.total_decompressed_bytes.toLocaleString()} bytes in ${report.fetched.total_elapsed_ms} ms`,
                      },
                    ]
                  : []
              }
              emptyText="No facade fetch summary was attached."
            />
            <SemanticList
              title="Rejected fetches"
              items={visibleRejectedFetches.map((rejected) => ({
                id: rejected.id,
                label: rejected.reason_code,
                detail: rejected.url,
              }))}
              emptyText="No host-rejected fetches were recorded."
            />
          </div>
        ) : null}

        <SemanticList
          title="Findings"
          items={visibleFindings.map((finding) => ({
            id: finding.id,
            label: `${finding.severity}: ${finding.code}`,
            detail: finding.message,
          }))}
          emptyText="No parser or policy findings were reported."
        />
      </details>
    </section>
  );
}

function SemanticList({
  title,
  items,
  emptyText,
}: {
  title: string;
  items: Array<{ id: string; label: string; detail: string }>;
  emptyText: string;
}) {
  return (
    <section className="semantic-list">
      <div className="evidence-group-header">
        <h3>{title}</h3>
        <span>{items.length}</span>
      </div>
      {items.length > 0 ? (
        <div className="semantic-list-items">
          {items.map((item) => (
            <article key={item.id}>
              <strong>{item.label}</strong>
              <span>{item.detail}</span>
            </article>
          ))}
        </div>
      ) : (
        <p className="muted">{emptyText}</p>
      )}
    </section>
  );
}

function EvidenceGroup({ title, artifacts, emptyText }: { title: string; artifacts: ArtifactStatus[]; emptyText: string }) {
  return (
    <section className="evidence-group">
      <div className="evidence-group-header">
        <h3>{title}</h3>
        <span>{artifacts.length}</span>
      </div>
      {artifacts.length > 0 ? (
        <div className="artifact-list compact">
          {artifacts.map((artifact) => (
            <ArtifactCard key={artifact.id} artifact={artifact} />
          ))}
        </div>
      ) : (
        <p className="muted">{emptyText}</p>
      )}
    </section>
  );
}

function ComparisonView({
  records,
  mode,
  setMode,
  includePublisherMetadata,
}: {
  records: AtlasRecord[];
  mode: ComparisonMode;
  setMode: (mode: ComparisonMode) => void;
  includePublisherMetadata: boolean;
}) {
  const publisherFieldCount = records.reduce((count, record) => count + record.publisherFields.length, 0);
  return (
    <div className="comparison-view">
      <fieldset className="segmented local" aria-label="Coverage lens">
        <legend>Coverage lens</legend>
        <button type="button" className={mode === "core" ? "selected" : ""} onClick={() => setMode("core")}>
          Core metadata
        </button>
        <button type="button" className={mode === "publisher" ? "selected" : ""} onClick={() => setMode("publisher")}>
          Publisher metadata
        </button>
        <button type="button" className={mode === "diff" ? "selected" : ""} onClick={() => setMode("diff")}>
          Combined
        </button>
      </fieldset>

      <div className="comparison-summary">
        <div>
          <strong>{records.reduce((count, record) => count + record.fields.length, 0)}</strong>
          <span>core fields</span>
        </div>
        <div>
          <strong>{publisherFieldCount}</strong>
          <span>publisher fields</span>
        </div>
        <div>
          <strong>{includePublisherMetadata ? "Visible" : "Hidden"}</strong>
          <span>publisher metadata lens</span>
        </div>
      </div>

      <div className="diff-list">
        {records.map((record) => (
          <article key={record.id} className="diff-row">
            <h3>{record.name}</h3>
            {(mode === "core" || mode === "diff") && (
              <p>{record.fields.length} core semantic field{record.fields.length === 1 ? "" : "s"} available to the semantic asset view.</p>
            )}
            {(mode === "publisher" || mode === "diff") && (
              <div className="publisher-fields">
                {record.publisherFields.length > 0 ? (
                  record.publisherFields.map((item) => (
                    <span key={item.id}>publisher:{item.label} - contextual metadata</span>
                  ))
                ) : (
                  <span>No publisher-specific fields discovered.</span>
                )}
              </div>
            )}
          </article>
        ))}
      </div>
    </div>
  );
}

function MissingSummary({ items }: { items: MissingItem[] }) {
  const groups = items.reduce<Record<string, MissingItem[]>>((acc, item) => {
    acc[item.group] = [...(acc[item.group] ?? []), item];
    return acc;
  }, {});

  if (items.length === 0) {
    return (
      <section className="missing-view">
        <h3>Readiness checks</h3>
        <p className="muted">No readiness issues are visible for the current lens.</p>
      </section>
    );
  }

  return (
    <div className="missing-view">
      <h3>Readiness checks</h3>
      {Object.entries(groups).map(([group, groupItems]) => (
        <section key={group}>
          <h3>{group}</h3>
          <div className="missing-table">
            {groupItems.map((item) => (
              <article key={item.id}>
                <div>
                  <strong>{item.need}</strong>
                  <span>{item.source}</span>
                </div>
                <span className={`pill ${statusTone(item.rank)}`}>{item.rank}</span>
                <span className={`pill ${statusTone(item.status)}`}>{item.status.replace("-", " ")}</span>
                <a href={item.standardUrl} target="_blank" rel="noreferrer">
                  source term
                </a>
              </article>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

function OverviewView({
  model,
  missingItems,
  includePublisherMetadata,
}: {
  model: AtlasModel;
  missingItems: MissingItem[];
  includePublisherMetadata: boolean;
}) {
  const semanticAssets = model.records.filter((record) => isCatalogRecord(record) || isDatasetRecord(record));
  const accessMethods = model.records.filter(isAccessMethodRecord);
  const recognizedArtifacts = model.artifacts.filter((artifact) => artifact.origin === "standard" && artifact.presence === "found");
  const publisherArtifactCount = model.artifacts.filter((artifact) => artifact.origin === "publisher-specific").length;
  const publisherFieldCount = model.records.reduce((count, record) => count + record.publisherFields.length, 0);
  const credentialGatedCount = model.artifacts.filter((artifact) => artifact.presence === "auth-required").length;
  const followUpCount = model.artifacts.filter((artifact) => artifact.origin === "unsupported" || artifact.presence === "invalid").length;
  const blockingChecks = missingItems.filter((item) => item.rank === "blocking" && item.status !== "known");

  return (
    <div className="overview-view">
      <header className="overview-intro">
        <div>
          <p className="eyebrow">Decision view</p>
          <h3>Semantic asset overview</h3>
        </div>
        <p>
          Shows which semantic assets Atlas can register, what is usable from published metadata, and which readiness checks still need attention.
        </p>
      </header>

      <section className="overview-summary" aria-label="Semantic asset overview summary">
        <OverviewMetric label="Semantic assets" value={semanticAssets.length} detail="catalogs, datasets, and semantic models" />
        <OverviewMetric label="Access methods" value={accessMethods.length} detail="services, APIs, and distributions" />
        <OverviewMetric label="Recognized metadata" value={recognizedArtifacts.length} detail="retrieved semantic artifacts" />
        <OverviewMetric label="Follow-up evidence" value={followUpCount} detail="unparsed or incomplete artifacts" tone={followUpCount > 0 ? "warning" : "positive"} />
        <OverviewMetric label="Credential-gated" value={credentialGatedCount} detail="requires viewer context" tone={credentialGatedCount > 0 ? "warning" : "neutral"} />
        <OverviewMetric
          label="Publisher-specific"
          value={publisherArtifactCount + publisherFieldCount}
          detail={includePublisherMetadata ? "visible in current lens" : "hidden from core readiness"}
          tone={publisherArtifactCount + publisherFieldCount > 0 ? "publisher" : "neutral"}
        />
      </section>

      <section className="overview-panel" aria-label="Semantic asset readiness">
        <div className="overview-section-header">
          <div>
            <p className="eyebrow">Readiness</p>
            <h3>Can this become a semantic asset entry?</h3>
          </div>
          <span className={`pill ${blockingChecks.length > 0 ? "danger" : "positive"}`}>
            {blockingChecks.length > 0 ? `${blockingChecks.length} blocking` : "No blocking gaps"}
          </span>
        </div>
        <div className="readiness-grid">
          {model.readiness.map((category) => (
            <article key={category.id} className="readiness-card">
              <div>
                <h3>{category.label}</h3>
                <span className={`pill ${statusTone(category.status)}`}>{readinessLabels[category.status]}</span>
              </div>
              <p>{category.evidenceCount} semantic evidence item{category.evidenceCount === 1 ? "" : "s"} found.</p>
              <div className="term-list">
                {category.terms.map((term) => (
                  <span key={term}>{term}</span>
                ))}
              </div>
              <div className="action-cards">
                {category.topMissingItems.slice(0, 2).map((item) => (
                  <a key={item.id} href={item.standardUrl} target="_blank" rel="noreferrer">
                    <strong>{item.need}</strong>
                    <span>{item.rank} - {item.source}</span>
                  </a>
                ))}
                {category.topMissingItems.length === 0 ? <span className="muted">No top missing item for this category.</span> : null}
              </div>
            </article>
          ))}
        </div>
      </section>

      <section className="overview-panel readiness-missing" aria-label="Readiness checks">
        <MissingSummary items={missingItems} />
      </section>

      {!includePublisherMetadata ? <p className="publisher-hidden">Publisher-specific metadata is excluded from this readiness summary.</p> : null}
    </div>
  );
}

function OverviewMetric({
  label,
  value,
  detail,
  tone = "neutral",
}: {
  label: string;
  value: number;
  detail: string;
  tone?: "neutral" | "positive" | "warning" | "publisher";
}) {
  return (
    <article className={`overview-metric ${tone}`}>
      <strong>{value}</strong>
      <span>{label}</span>
      <small>{detail}</small>
    </article>
  );
}

function RecordDetail({
  record,
  accessMethods,
  includePublisherMetadata,
  rawOpen,
  setRawOpen,
  onSelect,
}: {
  record: AtlasRecord;
  accessMethods: AtlasRecord[];
  includePublisherMetadata: boolean;
  rawOpen: boolean;
  setRawOpen: (open: boolean) => void;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="record-detail">
      <div className="record-heading">
        <p className="eyebrow">{recordTypeLabel(record)}</p>
        <h2>{record.name}</h2>
        <p>
          <span>Source title</span>
          {record.publisher ? ` from ${record.publisher}` : " from published metadata"}
        </p>
      </div>

      <FieldList fields={record.fields} />

      {accessMethods.length > 0 ? (
        <section className="inspector-access-methods">
          <h3>Access methods</h3>
          <p className="muted">DCAT distributions and data services linked from this record.</p>
          <div className="access-method-list">
            {accessMethods.map((method) => (
              <button key={method.id} type="button" aria-label={method.name} onClick={() => onSelect(method.id)}>
                <span>{method.name}</span>
                <small>{accessMethodTerm(method)}</small>
              </button>
            ))}
          </div>
        </section>
      ) : null}

      {includePublisherMetadata ? (
        <div className="publisher-field-group">
          <h3>Publisher-specific fields</h3>
          <FieldList fields={record.publisherFields} publisherSpecific />
        </div>
      ) : record.publisherFields.length > 0 ? (
        <p className="publisher-hidden">
          <Eye size={14} aria-hidden="true" />
          {record.publisherFields.length} publisher-specific field{record.publisherFields.length === 1 ? "" : "s"} hidden.
        </p>
      ) : null}

      <button className="raw-toggle" type="button" onClick={() => setRawOpen(!rawOpen)}>
        <Braces size={15} aria-hidden="true" />
        Raw RDF / JSON-LD
        <ChevronDown size={15} aria-hidden="true" />
      </button>
      {rawOpen ? <pre className="raw-drawer">{formatJson(record.raw)}</pre> : null}
    </div>
  );
}

function FieldList({ fields, publisherSpecific = false }: { fields: FieldValue[]; publisherSpecific?: boolean }) {
  if (fields.length === 0) {
    return <p className="muted">No {publisherSpecific ? "publisher-specific" : "core metadata"} fields are available for this record.</p>;
  }

  return (
    <dl className={`field-list ${publisherSpecific ? "publisher-specific" : ""}`}>
      {fields.map((item) => (
        <div key={item.id} className="field-row">
          <dt>{publisherSpecific ? `publisher:${item.label}` : item.label}</dt>
          <dd>
            <strong>{item.value}</strong>
            <span>
              Source: {item.source.label}
              {" -> "}
              {item.source.term}
            </span>
          </dd>
        </div>
      ))}
    </dl>
  );
}
