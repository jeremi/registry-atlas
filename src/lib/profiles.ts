import type { Origin, Presence, ProfileId } from "./types";

export const PROFILE_LABELS: Record<ProfileId, string> = {
  "dcat-ap-3": "DCAT-AP 3.0.0",
  "dcat-ap-2": "DCAT-AP 2.1.1",
  "breg-dcat-ap": "BRegDCAT-AP",
  "registry-relay-publisher-profile": "Publisher-specific profile: Registry Relay",
};

export const STANDARD_URLS = {
  dcatAp3: "https://semiceu.github.io/DCAT-AP/releases/3.0.0/",
  dcatAp2: "https://semiceu.github.io/DCAT-AP/releases/2.1.1/",
  bregDcatAp: "https://semiceu.github.io/BRegDCAT-AP/",
  dcat: "https://www.w3.org/TR/vocab-dcat-3/",
  dcterms: "https://www.dublincore.org/specifications/dublin-core/dcmi-terms/",
  shacl: "https://www.w3.org/TR/shacl/",
  odrl: "https://www.w3.org/TR/odrl-model/",
  dqv: "https://www.w3.org/TR/vocab-dqv/",
  adms: "https://semiceu.github.io/ADMS/releases/2.00/",
  dpv: "https://w3c.github.io/dpv/2.0/dpv/",
  did: "https://www.w3.org/TR/did-core/",
  vcdm: "https://www.w3.org/TR/vc-data-model-2.0/",
  openApi: "https://spec.openapis.org/oas/latest.html",
  ogcRecords: "https://docs.ogc.org/is/20-004r1/20-004r1.html",
  ogcFeatures: "https://docs.ogc.org/is/17-069r4/17-069r4.html",
  registryRelay: "https://github.com/openscdp/registry-relay",
} as const;

export const SHAPE_REFS = {
  catalog: `${STANDARD_URLS.dcatAp3}#Catalogue_Shape`,
  dataset: `${STANDARD_URLS.dcatAp3}#Dataset_Shape`,
  distribution: `${STANDARD_URLS.dcatAp3}#Distribution_Shape`,
  dataService: `${STANDARD_URLS.dcatAp3}#Data_Service_Shape`,
  baseRegistry: `${STANDARD_URLS.bregDcatAp}#BaseRegistry_Shape`,
} as const;

export function presenceMicrocopy(presence: Presence): string {
  switch (presence) {
    case "found":
      return "This artifact was fetched and parsed.";
    case "missing":
      return "No usable artifact was retrieved from the current discovery chain.";
    case "invalid":
      return "The artifact was retrieved but could not be parsed by the current discovery engine.";
    case "auth-required":
      return "The endpoint requires credentials. Add a session token.";
  }
}

export function originMicrocopy(origin: Origin): string {
  switch (origin) {
    case "standard":
      return "Published through a recognized semantic metadata pattern.";
    case "publisher-specific":
      return "Publisher-specific metadata. It is shown separately from core semantic evidence.";
    case "unsupported":
      return "Kept as follow-up evidence, not treated as missing catalog metadata.";
  }
}
