# Standards Assumptions

Registry Atlas discovers and interprets semantic metadata. This document keeps
the line clear between evidence found in standards-based metadata and the
derived roles, gaps, and confidence scores that Atlas presents to users.

## Scope

Atlas may consume metadata from:

- DCAT and DCAT-AP catalogues;
- BRegDCAT-AP profile metadata;
- CPSV public service metadata;
- SHACL, JSON Schema, SKOS, OGC API Records, and related semantic assets;
- Registry Relay or any other publisher that exposes similar standards-based
  metadata.

Atlas is not specific to Registry Relay. Registry Relay is only one test
publisher.

## Facts, Hypotheses, And Review Claims

Atlas uses three levels of statement:

- **Discovered fact**: a machine-readable artifact, predicate, schema property,
  link, header, or rejected fetch was observed in a `DiscoveryReport` or
  `DiscoveryRunEnvelope`.
- **Atlas hypothesis**: Atlas derived a route, gap, confidence level, or review
  flag from discovered facts using deterministic rules.
- **Reviewed claim**: a human or governance workflow asserted something that
  cannot be proven from metadata alone, such as source-of-truth status, legal
  authority, production readiness, or sufficient legal basis.

The v0.1 UI and APIs should label hypotheses as candidate routes, gaps, or
review flags. They must not present hypotheses as reviewed claims.

## Standard Evidence We Consume

The following predicates are treated as standards evidence when present:

- `dcterms:publisher`
- `dcterms:creator`
- `dcterms:source`
- `dcterms:provenance`
- `dcterms:accessRights`
- `dcterms:rights`
- `dcterms:license`
- `dcterms:modified`
- `dcterms:issued`
- `dcterms:accrualPeriodicity`
- `adms:status`
- `dcatap:availability`
- `dcatap:applicableLegislation`
- `dcat:distribution`
- `dcat:accessService`
- `dcat:servesDataset`
- `cpsv:PublicService`
- `cpsv:produces`

These predicates are evidence. They are not automatic proof of authority,
legal access, operational readiness, or complete coverage.

`dcterms:conformsTo` is conformance or profile evidence. Atlas should preserve
it, but it is not a fetch target and it is not source-of-truth evidence by
itself.

## Atlas-Derived Interpretation

Atlas derives user-facing route roles from evidence:

- `candidate_route`: a route or asset appears relevant to a capability need.
- `candidate_source`: a stronger route where Atlas found standard evidence for
  publisher or creator, applicable legislation, and a CPSV production relation.
- gaps such as `LegalBasisUnknown`, `SourceOfTruthUnknown`,
  `AuthorityUnknown`, `FreshnessUnknown`, and `RequiredIdentifierUnknown`.
- confidence scores based on strict matches, access evidence, gaps, and review
  flags.

These are Atlas interpretations. They are not standards predicates and should
not be published back into a dataset catalogue as if they were source metadata.

`DatasetDistribution` means Atlas found a declared distribution, data service,
collection, feed, or queryable route associated with the matching metadata. It
does not mean that the current user is authorized, that the route accepts a
specific lookup identifier, or that the route has been tested as a production
integration endpoint.

`candidate_source` is deliberately conservative. Atlas may only derive it from
standard-facing authority, legal-basis, and production-relation evidence or from
reviewed mappings. A matching field name or entity label is enough for
`candidate_route`, but not for `candidate_source`.

## Publisher-Neutral Discovery Hypotheses

The current implementation assumes that many publishers expose a small entry
document that links to richer metadata artifacts. Registry Relay uses
`/metadata`; another publisher may use a DCAT catalogue URL, a `describedby`
link, an OGC landing page, or a static metadata bundle.

Discovery therefore treats these as equivalent patterns when the evidence is
machine-readable:

- a metadata index with typed links to DCAT, SHACL, JSON Schema, ODRL, OpenAPI,
  or OGC artifacts;
- a DCAT catalogue with `dcat:dataset`, `dcat:distribution`, and
  `dcat:accessService`;
- a semantic package that includes schemas, shapes, contexts, vocabularies, or
  alignments without requiring DCAT.

This is a discovery hypothesis about common publication patterns, not a new
standard. A publisher that uses a different pattern should still work if it
exposes equivalent standard links and artifacts.

## Policy And ODRL Hypotheses

Atlas treats ODRL Offers, `dcterms:accessRights`,
`dcatap:applicableLegislation`, and similar policy metadata as governance
evidence. They can reduce gaps or raise review flags, but they do not grant
access.

Policy readiness is measured against the discovery use case, not against a
publisher-specific implementation spec. Atlas treats policy evidence as ready
only when it can see:

- a machine-readable usage policy, such as an ODRL Offer;
- enough ODRL structure to distinguish a default use marker from richer policy
  terms, such as profile, constraints, duties, prohibitions, or assignees;
- an access-rights signal, such as `dcterms:accessRights`,
  `dcterms:rights`, or `dcterms:license`;
- a legal-basis or data-protection signal, such as
  `dcatap:applicableLegislation` or DPV metadata.

This is still a metadata-readiness statement. It means the catalogue exposes
reviewable policy evidence for discovery. It does not mean the policy is legally
adequate for the user's concrete program.

Atlas must not infer that:

- an ODRL Offer has been accepted;
- a Dataspace Protocol contract exists;
- all duties or constraints have been satisfied;
- a legal basis is sufficient for a concrete use case;
- a user or system is authorized to call the route.

Those require reviewed claims or a separate contract, authorization, or policy
enforcement layer.

## What Atlas Must Not Infer

Atlas must not infer that:

- a dataset is legally accessible just because it has an access URL;
- a dataset is the source of truth just because it contains a matching field;
- a publisher is the legal authority for every field in a dataset;
- a public service relation proves that all operational access conditions are
  satisfied;
- a matched route is production-ready without identifier, authorization, and
  governance review.

For example, a student registry may contain a `disability_status` field. That is
a valid match for discovery, but it is not the same as discovering the disability
registry as the source for disability registration.

## Demo Assumptions

The Registry Relay demo is intentionally mixed:

- `farmer_registry` and `disability_registry` include stronger standard evidence
  through `dcatap:applicableLegislation` and `cpsv:produces`.
- `education_registry` and `benefits_casework` may contain related fields, but
  they remain candidate routes unless stronger standard evidence is present.

This is deliberate. It tests that Atlas can distinguish "contains a relevant
field" from "appears to be a candidate source."

The demo datasets are not claims about real OpenCRVS, OpenSPP, SP DCI, SEMIC,
or PublicSchema deployments. They are hypothetical registry examples built to
exercise standards discovery. Proper project-specific profiles should be added
only from reviewed source artifacts and maintainer input.

## AI Boundary

The first version uses strict matching. It does not use AI to decide that a
dataset matches a need.

AI may later help users rewrite a question into explicit terms, propose reviewed
mappings, or inspect unmatched metadata. The strict matcher must still be able to
verify the final terms against machine-readable evidence.

## Version Assumptions

Atlas is designed around the DCAT-AP and BRegDCAT-AP profile family, plus CPSV
evidence for public services.

Known caveat: the local `third_party/semic-shacl-validator` bundle includes
BRegDCAT-AP 2.x shapes. Some Registry Relay demo manifests currently claim
`bregdcat-ap` 3.0. Until exact BRegDCAT-AP 3.0 shapes are pinned, SEMIC validation
should be advisory and version-specific.

## Validation Assumptions

Atlas tests verify deterministic parsing, strict matching, and route derivation.
They do not prove legal authority or policy readiness.

Recommended validation layers:

- local parser and matcher tests;
- fixture-based tests against a running Registry Relay demo;
- report schema validation for harvested discovery reports;
- optional SEMIC SHACL validation for profile conformance;
- human governance review for legal basis, access conditions, and source
  authority.
