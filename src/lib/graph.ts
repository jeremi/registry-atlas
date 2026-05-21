import type { AtlasRecord, GraphEdge, GraphNode } from "./types";

export const GRAPH_NODE_BUDGET = 1500;

export function buildGraph(records: AtlasRecord[], budget = GRAPH_NODE_BUDGET): {
  nodes: GraphNode[];
  edges: GraphEdge[];
  budget: number;
  summarized: boolean;
  summary: string[];
} {
  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];

  for (const record of records) {
    if (nodes.length >= budget) {
      break;
    }
    nodes.push({ id: record.id, label: record.name, type: record.type });
    if (record.parentId) {
      edges.push({
        id: `${record.parentId}->${record.id}`,
        from: record.parentId,
        to: record.id,
        label: edgeLabel(record.type),
      });
    }
  }

  const summarized = records.length > budget;
  const summary = summarized
    ? summarizeRecords(records, budget)
    : [`Rendered ${nodes.length} standards relationship nodes within the ${budget} node budget.`];

  return {
    nodes,
    edges: edges.filter((edge) => nodes.some((node) => node.id === edge.from) && nodes.some((node) => node.id === edge.to)),
    budget,
    summarized,
    summary,
  };
}

function edgeLabel(type: AtlasRecord["type"]): string {
  switch (type) {
    case "dataset":
    case "base-registry":
      return "catalog contains dataset";
    case "distribution":
    case "ogc-feature-collection":
      return "dataset has distribution";
    case "service":
    case "ogc-record-collection":
      return "catalog advertises service";
    case "operation-group":
      return "service has API operation group";
    case "participant":
    case "catalog":
      return "contains";
  }
}

function summarizeRecords(records: AtlasRecord[], budget: number): string[] {
  const byType = records.reduce<Record<string, number>>((counts, record) => {
    counts[record.type] = (counts[record.type] ?? 0) + 1;
    return counts;
  }, {});

  return [
    `Graph budget is ${budget} nodes; ${records.length} records were discovered.`,
    ...Object.entries(byType).map(([type, count]) => `${type}: ${count}`),
  ];
}

