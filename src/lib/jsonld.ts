export type JsonLdObject = Record<string, unknown>;

const PREFIXES: Record<string, string> = {
  adms: "http://www.w3.org/ns/adms#",
  dcat: "http://www.w3.org/ns/dcat#",
  dcatap: "http://data.europa.eu/r5r/",
  dcterms: "http://purl.org/dc/terms/",
  dct: "http://purl.org/dc/terms/",
  dpv: "https://w3id.org/dpv#",
  dqv: "http://www.w3.org/ns/dqv#",
  foaf: "http://xmlns.com/foaf/0.1/",
  locn: "http://www.w3.org/ns/locn#",
  odrl: "http://www.w3.org/ns/odrl/2/",
  owl: "http://www.w3.org/2002/07/owl#",
  prov: "http://www.w3.org/ns/prov#",
  rdf: "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
  rdfs: "http://www.w3.org/2000/01/rdf-schema#",
  schema: "https://schema.org/",
  sh: "http://www.w3.org/ns/shacl#",
  skos: "http://www.w3.org/2004/02/skos/core#",
  vcard: "http://www.w3.org/2006/vcard/ns#",
};

const STANDARD_PREFIXES = new Set([
  "@",
  "adms",
  "dcat",
  "dcatap",
  "dct",
  "dcterms",
  "dpv",
  "dqv",
  "foaf",
  "locn",
  "odrl",
  "owl",
  "prov",
  "rdf",
  "rdfs",
  "schema",
  "sh",
  "skos",
  "vcard",
]);

export function asArray(value: unknown): unknown[] {
  if (value === undefined || value === null) {
    return [];
  }
  return Array.isArray(value) ? value : [value];
}

export function asObject(value: unknown): JsonLdObject | undefined {
  return isObject(value) ? value : undefined;
}

export function isObject(value: unknown): value is JsonLdObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function expandTerm(term: string): string {
  const [prefix, local] = term.split(":");
  if (!local || !PREFIXES[prefix]) {
    return term;
  }
  return `${PREFIXES[prefix]}${local}`;
}

export function termMatches(actual: string, expected: string): boolean {
  return actual === expected || actual === expandTerm(expected);
}

export function getValues(node: JsonLdObject | undefined, terms: string[]): unknown[] {
  if (!node) {
    return [];
  }

  const values: unknown[] = [];
  for (const [key, value] of Object.entries(node)) {
    if (terms.some((term) => termMatches(key, term))) {
      values.push(...asArray(value));
    }
  }
  return values;
}

export function contextualizeJsonLd(document: unknown): unknown {
  const root = asObject(document);
  if (!root) {
    return document;
  }
  return applyContext(document, {});
}

export function getObjects(node: JsonLdObject | undefined, terms: string[]): JsonLdObject[] {
  return getValues(node, terms).flatMap((value) => {
    const object = asObject(value);
    return object ? [object] : [];
  });
}

export function getStrings(node: JsonLdObject | undefined, terms: string[]): string[] {
  return getValues(node, terms)
    .map(stringValue)
    .filter((value): value is string => Boolean(value));
}

export function getFirstString(node: JsonLdObject | undefined, terms: string[]): string | undefined {
  return getStrings(node, terms)[0];
}

export function getId(value: unknown): string | undefined {
  if (typeof value === "string") {
    return value;
  }
  const object = asObject(value);
  const id = object?.["@id"];
  return typeof id === "string" ? id : undefined;
}

export function stringValue(value: unknown): string | undefined {
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }

  const object = asObject(value);
  if (!object) {
    return undefined;
  }

  const literal = object["@value"];
  if (typeof literal === "string" || typeof literal === "number" || typeof literal === "boolean") {
    return String(literal);
  }

  const id = object["@id"];
  if (typeof id === "string") {
    return id;
  }

  const label = getFirstString(object, ["rdfs:label", "skos:prefLabel", "dcterms:title", "foaf:name", "vcard:fn"]);
  return label;
}

export function hasType(node: JsonLdObject | undefined, terms: string[]): boolean {
  if (!node) {
    return false;
  }

  return asArray(node["@type"]).some((type) => {
    if (typeof type !== "string") {
      return false;
    }
    return terms.some((term) => termMatches(type, term));
  });
}

export function flattenJsonLd(document: unknown): JsonLdObject[] {
  const nodes: JsonLdObject[] = [];

  function visit(value: unknown): void {
    const object = asObject(value);
    if (!object) {
      if (Array.isArray(value)) {
        value.forEach(visit);
      }
      return;
    }

    nodes.push(object);
    for (const child of Object.values(object)) {
      if (Array.isArray(child)) {
        child.forEach(visit);
      } else if (isObject(child)) {
        visit(child);
      }
    }
  }

  visit(document);
  return dedupeNodes(nodes);
}

export function graphNodes(document: unknown): JsonLdObject[] {
  const root = asObject(document);
  const graph = root ? getValues(root, ["@graph"]) : [];
  const nodes = graph.length > 0 ? graph.flatMap((value) => asArray(value)) : asArray(document);
  return nodes.flatMap((value) => {
    const object = asObject(value);
    return object ? [object] : [];
  });
}

export function nodeId(node: JsonLdObject, fallback: string): string {
  const id = node["@id"];
  return typeof id === "string" && id.length > 0 ? id : fallback;
}

export function isPublisherSpecificKey(key: string): boolean {
  if (key.startsWith("@")) {
    return false;
  }
  const [prefix] = key.split(":");
  if (prefix && STANDARD_PREFIXES.has(prefix)) {
    return false;
  }
  if (Object.values(PREFIXES).some((iri) => key.startsWith(iri))) {
    return false;
  }
  return key.includes(":") || key.startsWith("x-") || key.includes("registryRelay");
}

function dedupeNodes(nodes: JsonLdObject[]): JsonLdObject[] {
  const seen = new Set<JsonLdObject>();
  const unique: JsonLdObject[] = [];
  for (const node of nodes) {
    if (!seen.has(node)) {
      seen.add(node);
      unique.push(node);
    }
  }
  return unique;
}

type ContextMap = Record<string, string>;

function applyContext(value: unknown, inheritedContext: ContextMap): unknown {
  if (Array.isArray(value)) {
    return value.map((item) => applyContext(item, inheritedContext));
  }

  const object = asObject(value);
  if (!object) {
    return value;
  }

  const context = { ...inheritedContext, ...contextTerms(object["@context"]) };
  const rewritten: JsonLdObject = {};

  for (const [key, child] of Object.entries(object)) {
    const contextualKey = key === "@context" ? key : context[key] ?? key;
    rewritten[contextualKey] = applyContext(child, context);
  }

  return rewritten;
}

function contextTerms(context: unknown): ContextMap {
  if (Array.isArray(context)) {
    return Object.assign({}, ...context.map(contextTerms)) as ContextMap;
  }

  const object = asObject(context);
  if (!object) {
    return {};
  }

  const terms: ContextMap = {};
  for (const [alias, definition] of Object.entries(object)) {
    if (alias.startsWith("@")) {
      continue;
    }

    if (typeof definition === "string") {
      if (definition.includes(":")) {
        terms[alias] = definition;
      }
      continue;
    }

    const record = asObject(definition);
    const id = record?.["@id"];
    if (typeof id === "string") {
      terms[alias] = id;
    }
  }
  return terms;
}
