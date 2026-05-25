use crate::types::{
    DiscoveryEvidence, DiscoveryReport, RelationClaim, RelationEndpoint, SemanticAsset,
    SemanticAssetKind, SemanticRelation,
};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ServiceGraphError {
    #[error("semantic relation `{relation_id}` has no relation claim")]
    UnclaimedRelation { relation_id: String },
    #[error("public service `{iri}` was not found")]
    PublicServiceNotFound { iri: String },
    #[error("evidence type `{iri}` was not found")]
    EvidenceTypeNotFound { iri: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceGraphGap {
    pub asset_id: String,
    pub predicate: String,
    pub message: String,
}

/// Navigation API for service-first discovery metadata in a v2 discovery report.
///
/// `ServiceGraph` indexes the assets, semantic relations, and relation claims
/// already declared in a [`DiscoveryReport`]. It does not infer missing real
/// world services. A gap means expected metadata was absent from the report,
/// not that the provider, route, form, or evidence does not exist.
///
/// ```rust,no_run
/// use semantic_asset_discovery_core::{DiscoveryReport, ServiceGraph};
///
/// # fn existing_report() -> DiscoveryReport { unimplemented!() }
/// let report = existing_report();
/// let graph = ServiceGraph::from_report(&report)?;
/// let service = graph.public_service("https://example.test/services/permit")?;
///
/// for evidence_type in service.accepted_evidence_types() {
///     println!("{}", evidence_type.asset.id);
/// }
///
/// let projection = service.projection();
/// println!("{} declared gaps", projection.gaps.len());
/// # Ok::<(), semantic_asset_discovery_core::ServiceGraphError>(())
/// ```
pub struct ServiceGraph<'a> {
    report: &'a DiscoveryReport,
    assets_by_id: HashMap<&'a str, &'a SemanticAsset>,
    assets_by_uri: HashMap<&'a str, &'a SemanticAsset>,
    outgoing: HashMap<&'a str, Vec<&'a SemanticRelation>>,
    incoming: HashMap<&'a str, Vec<&'a SemanticRelation>>,
    claims_by_relation: HashMap<&'a str, Vec<&'a RelationClaim>>,
}

impl<'a> ServiceGraph<'a> {
    /// Builds a graph over a report whose semantic relations all have source claims.
    ///
    /// The graph is intentionally evidence-first: every relation must be backed
    /// by a relation claim so callers can trace view paths back to source
    /// artifacts.
    pub fn from_report(report: &'a DiscoveryReport) -> Result<Self, ServiceGraphError> {
        let assets_by_id = report
            .assets
            .iter()
            .map(|asset| (asset.id.as_str(), asset))
            .collect::<HashMap<_, _>>();
        let assets_by_uri = report
            .assets
            .iter()
            .filter_map(|asset| asset.uri.as_deref().map(|uri| (uri, asset)))
            .collect::<HashMap<_, _>>();
        let mut claims_by_relation: HashMap<&str, Vec<&RelationClaim>> = HashMap::new();
        for claim in &report.relation_claims {
            claims_by_relation
                .entry(claim.relation_id.as_str())
                .or_default()
                .push(claim);
        }
        for relation in &report.relations {
            if !claims_by_relation.contains_key(relation.id.as_str()) {
                return Err(ServiceGraphError::UnclaimedRelation {
                    relation_id: relation.id.clone(),
                });
            }
        }

        let mut outgoing: HashMap<&str, Vec<&SemanticRelation>> = HashMap::new();
        let mut incoming: HashMap<&str, Vec<&SemanticRelation>> = HashMap::new();
        for relation in &report.relations {
            if let Some(subject_id) =
                endpoint_asset_id(&relation.subject, &assets_by_id, &assets_by_uri)
            {
                outgoing.entry(subject_id).or_default().push(relation);
            }
            if let Some(object_id) =
                endpoint_asset_id(&relation.object, &assets_by_id, &assets_by_uri)
            {
                incoming.entry(object_id).or_default().push(relation);
            }
        }

        Ok(Self {
            report,
            assets_by_id,
            assets_by_uri,
            outgoing,
            incoming,
            claims_by_relation,
        })
    }

    pub fn report(&self) -> &'a DiscoveryReport {
        self.report
    }

    pub fn public_service(&'a self, iri: &str) -> Result<PublicServiceView<'a>, ServiceGraphError> {
        let asset = self
            .assets_by_uri
            .get(iri)
            .copied()
            .filter(|asset| asset.kind == SemanticAssetKind::PublicService)
            .ok_or_else(|| ServiceGraphError::PublicServiceNotFound {
                iri: iri.to_string(),
            })?;
        Ok(PublicServiceView { graph: self, asset })
    }

    pub fn evidence_type(&'a self, iri: &str) -> Result<EvidenceTypeView<'a>, ServiceGraphError> {
        let asset = self
            .assets_by_uri
            .get(iri)
            .copied()
            .filter(|asset| asset.kind == SemanticAssetKind::EvidenceType)
            .ok_or_else(|| ServiceGraphError::EvidenceTypeNotFound {
                iri: iri.to_string(),
            })?;
        Ok(EvidenceTypeView {
            graph: self,
            asset,
            edge_path: Vec::new(),
        })
    }

    pub fn routes_for_service(&'a self, service_id: &str) -> Vec<ServiceRouteView<'a>> {
        let Some(service) = self.assets_by_id.get(service_id).copied() else {
            return Vec::new();
        };
        let service = PublicServiceView {
            graph: self,
            asset: service,
        };
        let mut routes = Vec::new();
        for evidence_type in service.accepted_evidence_types() {
            routes.push(ServiceRouteView {
                graph: self,
                service: service.asset,
                target: evidence_type.asset,
                edges: evidence_type.edge_path,
                route_kind: ServiceRouteKind::EvidenceType,
            });
        }
        for provider in service.evidence_providers() {
            routes.push(ServiceRouteView {
                graph: self,
                service: service.asset,
                target: provider.asset,
                edges: provider.edge_path,
                route_kind: ServiceRouteKind::EvidenceProvider,
            });
        }
        for data_service in service.data_services() {
            routes.push(ServiceRouteView {
                graph: self,
                service: service.asset,
                target: data_service.asset,
                edges: data_service.edge_path,
                route_kind: ServiceRouteKind::SupportingDataService,
            });
        }
        for evidence_type in service.accepted_evidence_types() {
            for offering in evidence_type.evidence_offerings() {
                for access_service in offering.access_services() {
                    routes.push(ServiceRouteView {
                        graph: self,
                        service: service.asset,
                        target: access_service.asset,
                        edges: access_service.edge_path,
                        route_kind: ServiceRouteKind::EvidenceAccessService,
                    });
                }
            }
        }
        for form in service.forms() {
            routes.push(ServiceRouteView {
                graph: self,
                service: service.asset,
                target: form.asset,
                edges: form.edge_path,
                route_kind: ServiceRouteKind::Form,
            });
        }
        routes
    }

    fn asset_for_endpoint(&self, endpoint: &RelationEndpoint) -> Option<&'a SemanticAsset> {
        match endpoint {
            RelationEndpoint::Asset { asset_id, .. } => {
                self.assets_by_id.get(asset_id.as_str()).copied()
            }
            RelationEndpoint::External { uri } => self.assets_by_uri.get(uri.as_str()).copied(),
            RelationEndpoint::BlankNode { .. } => None,
        }
    }

    fn outgoing_assets(
        &'a self,
        asset_id: &str,
        predicates: &[&str],
        kinds: &[SemanticAssetKind],
    ) -> Vec<AssetPathView<'a>> {
        let mut seen = BTreeSet::new();
        self.outgoing
            .get(asset_id)
            .into_iter()
            .flatten()
            .filter(|relation| predicates.contains(&relation.predicate.as_str()))
            .filter_map(|relation| {
                let asset = self.asset_for_endpoint(&relation.object)?;
                if !kinds.is_empty() && !kinds.contains(&asset.kind) {
                    return None;
                }
                seen.insert(asset.id.clone()).then_some(AssetPathView {
                    asset,
                    edge_path: vec![relation],
                })
            })
            .collect()
    }

    fn incoming_assets(
        &'a self,
        asset_id: &str,
        predicates: &[&str],
        kinds: &[SemanticAssetKind],
    ) -> Vec<AssetPathView<'a>> {
        let mut seen = BTreeSet::new();
        self.incoming
            .get(asset_id)
            .into_iter()
            .flatten()
            .filter(|relation| predicates.contains(&relation.predicate.as_str()))
            .filter_map(|relation| {
                let asset = self.asset_for_endpoint(&relation.subject)?;
                if !kinds.is_empty() && !kinds.contains(&asset.kind) {
                    return None;
                }
                seen.insert(asset.id.clone()).then_some(AssetPathView {
                    asset,
                    edge_path: vec![relation],
                })
            })
            .collect()
    }

    fn relation_claims(&'a self, relation_id: &str) -> Vec<&'a RelationClaim> {
        self.claims_by_relation
            .get(relation_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn endpoint_url_for_asset(&'a self, asset_id: &str) -> Option<EndpointProjection<'a>> {
        self.outgoing
            .get(asset_id)?
            .iter()
            .find(|relation| relation.predicate == "dcat:endpointURL")
            .and_then(|relation| match &relation.object {
                RelationEndpoint::External { uri } => Some(EndpointProjection {
                    url: uri.as_str(),
                    relation_id: relation.id.as_str(),
                }),
                RelationEndpoint::Asset { uri: Some(uri), .. } => Some(EndpointProjection {
                    url: uri.as_str(),
                    relation_id: relation.id.as_str(),
                }),
                _ => None,
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointProjection<'a> {
    pub url: &'a str,
    pub relation_id: &'a str,
}

/// Owned, ID-only view of a public service and its declared service-first paths.
///
/// This projection is useful at API boundaries where callers do not want to
/// hold borrowed graph view structs. It preserves route kinds and explicit
/// gaps, but leaves richer asset labels and source evidence on the borrowed
/// views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicServiceProjection {
    pub service_id: String,
    pub service_iri: Option<String>,
    pub channel_ids: Vec<String>,
    pub requirement_ids: Vec<String>,
    pub evidence_requirements: Vec<RequirementEvidenceProjection>,
    pub accepted_evidence_type_ids: Vec<String>,
    pub evidence_provider_ids: Vec<String>,
    pub data_service_ids: Vec<String>,
    pub form_ids: Vec<String>,
    pub routes: Vec<ServiceRouteProjection>,
    pub gaps: Vec<ServiceGraphGap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRouteProjection {
    pub kind: ServiceRouteKind,
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementEvidenceProjection {
    pub requirement_id: String,
    pub option_groups: Vec<EvidenceOptionProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceOptionProjection {
    pub evidence_type_list_id: String,
    pub evidence_type_ids: Vec<String>,
    pub satisfiable: bool,
    pub missing_evidence_type_ids: Vec<String>,
}

#[derive(Clone)]
struct AssetPathView<'a> {
    asset: &'a SemanticAsset,
    edge_path: Vec<&'a SemanticRelation>,
}

/// Borrowed view over one public service and the metadata declared around it.
pub struct PublicServiceView<'a> {
    pub asset: &'a SemanticAsset,
    graph: &'a ServiceGraph<'a>,
}

impl<'a> PublicServiceView<'a> {
    pub fn id(&self) -> &'a str {
        self.asset.id.as_str()
    }

    pub fn channels(&self) -> Vec<ChannelView<'a>> {
        self.graph
            .outgoing_assets(
                &self.asset.id,
                &["cv:hasChannel"],
                &[SemanticAssetKind::Channel],
            )
            .into_iter()
            .map(|path| ChannelView {
                graph: self.graph,
                asset: path.asset,
                edge_path: path.edge_path,
            })
            .collect()
    }

    pub fn requirements(&self) -> Vec<RequirementView<'a>> {
        self.graph
            .outgoing_assets(
                &self.asset.id,
                &["cv:holdsRequirement", "cccev:hasRequirement"],
                &[SemanticAssetKind::Requirement],
            )
            .into_iter()
            .map(|path| RequirementView {
                graph: self.graph,
                asset: path.asset,
                edge_path: path.edge_path,
            })
            .collect()
    }

    pub fn accepted_evidence_types(&self) -> Vec<EvidenceTypeView<'a>> {
        let mut seen = BTreeSet::new();
        let mut evidence_types = Vec::new();
        for requirement in self.requirements() {
            for evidence_type in requirement.accepted_evidence_types() {
                if seen.insert(evidence_type.asset.id.clone()) {
                    evidence_types.push(evidence_type);
                }
            }
        }
        for direct in self.graph.outgoing_assets(
            &self.asset.id,
            &["registry_manifest:acceptedEvidenceType"],
            &[SemanticAssetKind::EvidenceType],
        ) {
            if seen.insert(direct.asset.id.clone()) {
                evidence_types.push(EvidenceTypeView {
                    graph: self.graph,
                    asset: direct.asset,
                    edge_path: direct.edge_path,
                });
            }
        }
        evidence_types
    }

    pub fn evidence_providers(&self) -> Vec<EvidenceProviderView<'a>> {
        let mut seen = BTreeSet::new();
        let mut providers = Vec::new();
        for evidence_type in self.accepted_evidence_types() {
            for provider in evidence_type.providers() {
                if seen.insert(provider.asset.id.clone()) {
                    providers.push(provider);
                }
            }
        }
        providers
    }

    pub fn data_services(&self) -> Vec<DataServiceView<'a>> {
        self.graph
            .outgoing_assets(
                &self.asset.id,
                &["registry_manifest:usesDataService"],
                &[SemanticAssetKind::DataService],
            )
            .into_iter()
            .map(|service| DataServiceView {
                graph: self.graph,
                asset: service.asset,
                edge_path: service.edge_path,
            })
            .collect()
    }

    pub fn forms(&self) -> Vec<FormDefinitionView<'a>> {
        let mut seen = BTreeSet::new();
        let mut forms = Vec::new();
        for direct in self.graph.outgoing_assets(
            &self.asset.id,
            &["registry_manifest:form", "registry_manifest:hasForm"],
            &[SemanticAssetKind::FormDefinition],
        ) {
            if seen.insert(direct.asset.id.clone()) {
                forms.push(FormDefinitionView {
                    graph: self.graph,
                    asset: direct.asset,
                    edge_path: direct.edge_path,
                });
            }
        }
        for channel in self.channels() {
            for form in self.graph.outgoing_assets(
                &channel.asset.id,
                &["registry_manifest:form", "registry_manifest:hasForm"],
                &[SemanticAssetKind::FormDefinition],
            ) {
                if seen.insert(form.asset.id.clone()) {
                    let mut edge_path = channel.edge_path.clone();
                    edge_path.extend(form.edge_path);
                    forms.push(FormDefinitionView {
                        graph: self.graph,
                        asset: form.asset,
                        edge_path,
                    });
                }
            }
            for form in self.graph.incoming_assets(
                &channel.asset.id,
                &["registry_manifest:forChannel"],
                &[SemanticAssetKind::FormDefinition],
            ) {
                if seen.insert(form.asset.id.clone()) {
                    let mut edge_path = channel.edge_path.clone();
                    edge_path.extend(form.edge_path);
                    forms.push(FormDefinitionView {
                        graph: self.graph,
                        asset: form.asset,
                        edge_path,
                    });
                }
            }
        }
        for form in self.graph.incoming_assets(
            &self.asset.id,
            &["registry_manifest:forPublicService"],
            &[SemanticAssetKind::FormDefinition],
        ) {
            if seen.insert(form.asset.id.clone()) {
                forms.push(FormDefinitionView {
                    graph: self.graph,
                    asset: form.asset,
                    edge_path: form.edge_path,
                });
            }
        }
        forms
    }

    /// Returns explicit gaps where expected service-first metadata is absent.
    ///
    /// These gaps report absent declared relations in the analyzed metadata.
    /// They are not proof that a provider, access service, form, channel, or
    /// evidence path does not exist outside the discovered artifacts.
    pub fn gaps(&self) -> Vec<ServiceGraphGap> {
        let mut gaps = Vec::new();
        if self.channels().is_empty() {
            gaps.push(ServiceGraphGap {
                asset_id: self.asset.id.clone(),
                predicate: "cv:hasChannel".to_string(),
                message: "Public service has no declared channel relation.".to_string(),
            });
        }
        if self.requirements().is_empty() {
            gaps.push(ServiceGraphGap {
                asset_id: self.asset.id.clone(),
                predicate: "cv:holdsRequirement".to_string(),
                message: "Public service has no declared requirement relation.".to_string(),
            });
        }
        for requirement in self.requirements() {
            if requirement.accepted_evidence_types().is_empty() {
                gaps.push(ServiceGraphGap {
                    asset_id: requirement.asset.id.clone(),
                    predicate: "cccev:hasEvidenceTypeList".to_string(),
                    message: "Requirement has no declared evidence type list.".to_string(),
                });
            }
        }
        for evidence_type in self.accepted_evidence_types() {
            let offerings = evidence_type.evidence_offerings();
            if offerings.is_empty() {
                gaps.push(ServiceGraphGap {
                    asset_id: evidence_type.asset.id.clone(),
                    predicate: "registry_manifest:evidenceType".to_string(),
                    message: "Evidence type has no discovered evidence offering.".to_string(),
                });
                continue;
            }
            for offering in offerings {
                if offering.providers().is_empty() {
                    gaps.push(ServiceGraphGap {
                        asset_id: offering.asset.id.clone(),
                        predicate: "registry_manifest:providedBy".to_string(),
                        message: "Evidence offering has no declared evidence provider.".to_string(),
                    });
                }
                if offering.access_services().is_empty() {
                    gaps.push(ServiceGraphGap {
                        asset_id: offering.asset.id.clone(),
                        predicate: "registry_manifest:evidenceService".to_string(),
                        message: "Evidence offering has no declared access data service."
                            .to_string(),
                    });
                }
            }
        }
        gaps
    }

    pub fn projection(&self) -> PublicServiceProjection {
        PublicServiceProjection {
            service_id: self.asset.id.clone(),
            service_iri: self.asset.uri.clone(),
            channel_ids: self
                .channels()
                .into_iter()
                .map(|channel| channel.asset.id.clone())
                .collect(),
            requirement_ids: self
                .requirements()
                .into_iter()
                .map(|requirement| requirement.asset.id.clone())
                .collect(),
            evidence_requirements: self
                .requirements()
                .into_iter()
                .map(|requirement| requirement.evidence_projection())
                .collect(),
            accepted_evidence_type_ids: self
                .accepted_evidence_types()
                .into_iter()
                .map(|evidence_type| evidence_type.asset.id.clone())
                .collect(),
            evidence_provider_ids: self
                .evidence_providers()
                .into_iter()
                .map(|provider| provider.asset.id.clone())
                .collect(),
            data_service_ids: self
                .data_services()
                .into_iter()
                .map(|service| service.asset.id.clone())
                .collect(),
            form_ids: self
                .forms()
                .into_iter()
                .map(|form| form.asset.id.clone())
                .collect(),
            routes: self
                .graph
                .routes_for_service(self.id())
                .into_iter()
                .map(|route| ServiceRouteProjection {
                    kind: route.route_kind,
                    target_id: route.target.id.clone(),
                })
                .collect(),
            gaps: self.gaps(),
        }
    }
}

pub struct ChannelView<'a> {
    graph: &'a ServiceGraph<'a>,
    pub asset: &'a SemanticAsset,
    edge_path: Vec<&'a SemanticRelation>,
}

impl<'a> ChannelView<'a> {
    pub fn relations(&self) -> &[&'a SemanticRelation] {
        &self.edge_path
    }

    pub fn claims(&self) -> Vec<&'a RelationClaim> {
        claims_for_edges(self.graph, &self.edge_path)
    }

    pub fn evidence(&self) -> Vec<&'a DiscoveryEvidence> {
        evidence_for_edges(self.graph, &self.edge_path)
    }
}

pub struct RequirementView<'a> {
    pub asset: &'a SemanticAsset,
    graph: &'a ServiceGraph<'a>,
    edge_path: Vec<&'a SemanticRelation>,
}

impl<'a> RequirementView<'a> {
    pub fn relations(&self) -> &[&'a SemanticRelation] {
        &self.edge_path
    }

    pub fn claims(&self) -> Vec<&'a RelationClaim> {
        claims_for_edges(self.graph, &self.edge_path)
    }

    pub fn evidence(&self) -> Vec<&'a DiscoveryEvidence> {
        evidence_for_edges(self.graph, &self.edge_path)
    }

    pub fn accepted_evidence_types(&self) -> Vec<EvidenceTypeView<'a>> {
        let mut seen = BTreeSet::new();
        let mut evidence_types = Vec::new();
        for direct in self.graph.outgoing_assets(
            &self.asset.id,
            &["registry_manifest:acceptedEvidenceType"],
            &[SemanticAssetKind::EvidenceType],
        ) {
            if seen.insert(direct.asset.id.clone()) {
                let mut edge_path = self.edge_path.clone();
                edge_path.extend(direct.edge_path);
                evidence_types.push(EvidenceTypeView {
                    graph: self.graph,
                    asset: direct.asset,
                    edge_path,
                });
            }
        }
        for list in self.graph.outgoing_assets(
            &self.asset.id,
            &["cccev:hasEvidenceTypeList"],
            &[SemanticAssetKind::EvidenceTypeList],
        ) {
            for evidence_type in self.graph.outgoing_assets(
                &list.asset.id,
                &["cccev:specifiesEvidenceType"],
                &[SemanticAssetKind::EvidenceType],
            ) {
                if seen.insert(evidence_type.asset.id.clone()) {
                    let mut edge_path = self.edge_path.clone();
                    edge_path.extend(list.edge_path.clone());
                    edge_path.extend(evidence_type.edge_path);
                    evidence_types.push(EvidenceTypeView {
                        graph: self.graph,
                        asset: evidence_type.asset,
                        edge_path,
                    });
                }
            }
        }
        evidence_types
    }

    pub fn evidence_options(&self) -> Vec<EvidenceOptionView<'a>> {
        let mut options = Vec::new();
        let mut seen = BTreeSet::new();
        for list in self.graph.outgoing_assets(
            &self.asset.id,
            &["cccev:hasEvidenceTypeList"],
            &[SemanticAssetKind::EvidenceTypeList],
        ) {
            if seen.insert(list.asset.id.clone()) {
                let mut edge_path = self.edge_path.clone();
                edge_path.extend(list.edge_path);
                options.push(EvidenceOptionView {
                    graph: self.graph,
                    asset: list.asset,
                    edge_path,
                });
            }
        }

        let direct_evidence = self.graph.outgoing_assets(
            &self.asset.id,
            &["registry_manifest:acceptedEvidenceType"],
            &[SemanticAssetKind::EvidenceType],
        );
        if !direct_evidence.is_empty() {
            let direct_asset = self.asset;
            let mut edge_path = self.edge_path.clone();
            for evidence_type in &direct_evidence {
                edge_path.extend(evidence_type.edge_path.clone());
            }
            options.push(EvidenceOptionView {
                graph: self.graph,
                asset: direct_asset,
                edge_path,
            });
        }

        options
    }

    pub fn satisfiable_evidence_options(&self) -> Vec<EvidenceOptionView<'a>> {
        self.evidence_options()
            .into_iter()
            .filter(EvidenceOptionView::is_satisfiable)
            .collect()
    }

    pub fn evidence_projection(&self) -> RequirementEvidenceProjection {
        RequirementEvidenceProjection {
            requirement_id: self.asset.id.clone(),
            option_groups: self
                .evidence_options()
                .into_iter()
                .map(|option| option.projection())
                .collect(),
        }
    }
}

pub struct EvidenceOptionView<'a> {
    pub asset: &'a SemanticAsset,
    graph: &'a ServiceGraph<'a>,
    edge_path: Vec<&'a SemanticRelation>,
}

impl<'a> EvidenceOptionView<'a> {
    pub fn relations(&self) -> &[&'a SemanticRelation] {
        &self.edge_path
    }

    pub fn claims(&self) -> Vec<&'a RelationClaim> {
        claims_for_edges(self.graph, &self.edge_path)
    }

    pub fn evidence(&self) -> Vec<&'a DiscoveryEvidence> {
        evidence_for_edges(self.graph, &self.edge_path)
    }

    pub fn evidence_types(&self) -> Vec<EvidenceTypeView<'a>> {
        let mut evidence_types = Vec::new();
        let mut seen = BTreeSet::new();
        if self.asset.kind == SemanticAssetKind::EvidenceTypeList {
            for evidence_type in self.graph.outgoing_assets(
                &self.asset.id,
                &["cccev:specifiesEvidenceType"],
                &[SemanticAssetKind::EvidenceType],
            ) {
                if seen.insert(evidence_type.asset.id.clone()) {
                    let mut edge_path = self.edge_path.clone();
                    edge_path.extend(evidence_type.edge_path);
                    evidence_types.push(EvidenceTypeView {
                        graph: self.graph,
                        asset: evidence_type.asset,
                        edge_path,
                    });
                }
            }
        } else if self.asset.kind == SemanticAssetKind::Requirement {
            for evidence_type in self.graph.outgoing_assets(
                &self.asset.id,
                &["registry_manifest:acceptedEvidenceType"],
                &[SemanticAssetKind::EvidenceType],
            ) {
                if seen.insert(evidence_type.asset.id.clone()) {
                    let mut edge_path = self.edge_path.clone();
                    edge_path.extend(evidence_type.edge_path);
                    evidence_types.push(EvidenceTypeView {
                        graph: self.graph,
                        asset: evidence_type.asset,
                        edge_path,
                    });
                }
            }
        }
        evidence_types
    }

    pub fn missing_evidence_types(&self) -> Vec<EvidenceTypeView<'a>> {
        self.evidence_types()
            .into_iter()
            .filter(|evidence_type| !evidence_type.has_access_route())
            .collect()
    }

    pub fn is_satisfiable(&self) -> bool {
        let evidence_types = self.evidence_types();
        !evidence_types.is_empty()
            && evidence_types
                .iter()
                .all(EvidenceTypeView::has_access_route)
    }

    pub fn projection(&self) -> EvidenceOptionProjection {
        let evidence_type_ids = self
            .evidence_types()
            .into_iter()
            .map(|evidence_type| evidence_type.asset.id.clone())
            .collect::<Vec<_>>();
        let missing_evidence_type_ids = self
            .missing_evidence_types()
            .into_iter()
            .map(|evidence_type| evidence_type.asset.id.clone())
            .collect::<Vec<_>>();
        EvidenceOptionProjection {
            evidence_type_list_id: self.asset.id.clone(),
            evidence_type_ids,
            satisfiable: self.is_satisfiable(),
            missing_evidence_type_ids,
        }
    }
}

pub struct EvidenceTypeView<'a> {
    pub asset: &'a SemanticAsset,
    graph: &'a ServiceGraph<'a>,
    edge_path: Vec<&'a SemanticRelation>,
}

impl<'a> EvidenceTypeView<'a> {
    pub fn relations(&self) -> &[&'a SemanticRelation] {
        &self.edge_path
    }

    pub fn claims(&self) -> Vec<&'a RelationClaim> {
        claims_for_edges(self.graph, &self.edge_path)
    }

    pub fn evidence(&self) -> Vec<&'a DiscoveryEvidence> {
        evidence_for_edges(self.graph, &self.edge_path)
    }

    pub fn providers(&self) -> Vec<EvidenceProviderView<'a>> {
        let mut providers = Vec::new();
        let mut seen = BTreeSet::new();
        for offering in self.evidence_offerings() {
            for provider in offering.providers() {
                if seen.insert(provider.asset.id.clone()) {
                    providers.push(provider);
                }
            }
        }
        for direct in self.graph.outgoing_assets(
            &self.asset.id,
            &[
                "registry_manifest:evidenceProvider",
                "registry_manifest:providedBy",
                "registry_manifest:issuingAuthority",
            ],
            &[
                SemanticAssetKind::EvidenceProvider,
                SemanticAssetKind::PublicOrganisation,
            ],
        ) {
            if seen.insert(direct.asset.id.clone()) {
                let mut edge_path = self.edge_path.clone();
                edge_path.extend(direct.edge_path);
                providers.push(EvidenceProviderView {
                    graph: self.graph,
                    asset: direct.asset,
                    edge_path,
                });
            }
        }
        if let Some(incoming) = self.graph.incoming.get(self.asset.id.as_str()) {
            for relation in incoming.iter().filter(|relation| {
                matches!(
                    relation.predicate.as_str(),
                    "registry_manifest:offersEvidenceType"
                        | "registry_manifest:acceptedEvidenceType"
                        | "registry_manifest:evidenceType"
                        | "cccev:specifiesEvidenceType"
                )
            }) {
                let Some(offering) = self.graph.asset_for_endpoint(&relation.subject) else {
                    continue;
                };
                if offering.kind != SemanticAssetKind::EvidenceOffering {
                    continue;
                }
                for provider in self.graph.outgoing_assets(
                    &offering.id,
                    &[
                        "registry_manifest:evidenceProvider",
                        "registry_manifest:providedBy",
                        "registry_manifest:issuingAuthority",
                    ],
                    &[
                        SemanticAssetKind::EvidenceProvider,
                        SemanticAssetKind::PublicOrganisation,
                    ],
                ) {
                    if seen.insert(provider.asset.id.clone()) {
                        let mut edge_path = self.edge_path.clone();
                        edge_path.push(*relation);
                        edge_path.extend(provider.edge_path);
                        providers.push(EvidenceProviderView {
                            graph: self.graph,
                            asset: provider.asset,
                            edge_path,
                        });
                    }
                }
            }
        }
        providers
    }

    pub fn evidence_offerings(&self) -> Vec<EvidenceOfferingView<'a>> {
        let mut offerings = Vec::new();
        let mut seen = BTreeSet::new();
        if let Some(incoming) = self.graph.incoming.get(self.asset.id.as_str()) {
            for relation in incoming.iter().filter(|relation| {
                matches!(
                    relation.predicate.as_str(),
                    "registry_manifest:offersEvidenceType"
                        | "registry_manifest:acceptedEvidenceType"
                        | "registry_manifest:evidenceType"
                )
            }) {
                let Some(offering) = self.graph.asset_for_endpoint(&relation.subject) else {
                    continue;
                };
                if offering.kind != SemanticAssetKind::EvidenceOffering {
                    continue;
                }
                if seen.insert(offering.id.clone()) {
                    let mut edge_path = self.edge_path.clone();
                    edge_path.push(*relation);
                    offerings.push(EvidenceOfferingView {
                        graph: self.graph,
                        asset: offering,
                        edge_path,
                    });
                }
            }
        }
        offerings
    }

    pub fn has_access_route(&self) -> bool {
        self.evidence_offerings()
            .into_iter()
            .any(|offering| !offering.access_services().is_empty())
    }

    pub fn public_services(&self) -> Vec<PublicServiceView<'a>> {
        let mut services = Vec::new();
        let mut seen = BTreeSet::new();
        if let Some(incoming) = self.graph.incoming.get(self.asset.id.as_str()) {
            for relation in incoming {
                let requirement = match relation.predicate.as_str() {
                    "registry_manifest:acceptedEvidenceType" => {
                        self.graph.asset_for_endpoint(&relation.subject)
                    }
                    "cccev:specifiesEvidenceType" => self
                        .graph
                        .asset_for_endpoint(&relation.subject)
                        .and_then(|list| {
                            self.graph
                                .incoming_assets(
                                    &list.id,
                                    &["cccev:hasEvidenceTypeList"],
                                    &[SemanticAssetKind::Requirement],
                                )
                                .first()
                                .map(|path| path.asset)
                        }),
                    _ => None,
                };
                let Some(requirement) = requirement else {
                    continue;
                };
                for service in self.graph.incoming_assets(
                    &requirement.id,
                    &["cv:holdsRequirement", "cccev:hasRequirement"],
                    &[SemanticAssetKind::PublicService],
                ) {
                    if seen.insert(service.asset.id.clone()) {
                        services.push(PublicServiceView {
                            graph: self.graph,
                            asset: service.asset,
                        });
                    }
                }
            }
        }
        services
    }
}

pub struct EvidenceOfferingView<'a> {
    graph: &'a ServiceGraph<'a>,
    pub asset: &'a SemanticAsset,
    edge_path: Vec<&'a SemanticRelation>,
}

impl<'a> EvidenceOfferingView<'a> {
    pub fn relations(&self) -> &[&'a SemanticRelation] {
        &self.edge_path
    }

    pub fn claims(&self) -> Vec<&'a RelationClaim> {
        claims_for_edges(self.graph, &self.edge_path)
    }

    pub fn evidence(&self) -> Vec<&'a DiscoveryEvidence> {
        evidence_for_edges(self.graph, &self.edge_path)
    }

    pub fn providers(&self) -> Vec<EvidenceProviderView<'a>> {
        self.graph
            .outgoing_assets(
                &self.asset.id,
                &[
                    "registry_manifest:evidenceProvider",
                    "registry_manifest:providedBy",
                    "registry_manifest:issuingAuthority",
                ],
                &[
                    SemanticAssetKind::EvidenceProvider,
                    SemanticAssetKind::PublicOrganisation,
                ],
            )
            .into_iter()
            .map(|provider| {
                let mut edge_path = self.edge_path.clone();
                edge_path.extend(provider.edge_path);
                EvidenceProviderView {
                    graph: self.graph,
                    asset: provider.asset,
                    edge_path,
                }
            })
            .collect()
    }

    pub fn access_services(&self) -> Vec<DataServiceView<'a>> {
        self.graph
            .outgoing_assets(
                &self.asset.id,
                &["registry_manifest:evidenceService", "dcat:accessService"],
                &[SemanticAssetKind::DataService],
            )
            .into_iter()
            .map(|service| {
                let mut edge_path = self.edge_path.clone();
                edge_path.extend(service.edge_path);
                DataServiceView {
                    graph: self.graph,
                    asset: service.asset,
                    edge_path,
                }
            })
            .collect()
    }
}

pub struct DataServiceView<'a> {
    graph: &'a ServiceGraph<'a>,
    pub asset: &'a SemanticAsset,
    edge_path: Vec<&'a SemanticRelation>,
}

impl<'a> DataServiceView<'a> {
    pub fn relations(&self) -> &[&'a SemanticRelation] {
        &self.edge_path
    }

    pub fn claims(&self) -> Vec<&'a RelationClaim> {
        claims_for_edges(self.graph, &self.edge_path)
    }

    pub fn evidence(&self) -> Vec<&'a DiscoveryEvidence> {
        evidence_for_edges(self.graph, &self.edge_path)
    }
}

pub struct EvidenceProviderView<'a> {
    graph: &'a ServiceGraph<'a>,
    pub asset: &'a SemanticAsset,
    edge_path: Vec<&'a SemanticRelation>,
}

impl<'a> EvidenceProviderView<'a> {
    pub fn relations(&self) -> &[&'a SemanticRelation] {
        &self.edge_path
    }

    pub fn claims(&self) -> Vec<&'a RelationClaim> {
        claims_for_edges(self.graph, &self.edge_path)
    }

    pub fn evidence(&self) -> Vec<&'a DiscoveryEvidence> {
        evidence_for_edges(self.graph, &self.edge_path)
    }
}

pub struct FormDefinitionView<'a> {
    graph: &'a ServiceGraph<'a>,
    pub asset: &'a SemanticAsset,
    edge_path: Vec<&'a SemanticRelation>,
}

impl<'a> FormDefinitionView<'a> {
    pub fn relations(&self) -> &[&'a SemanticRelation] {
        &self.edge_path
    }

    pub fn claims(&self) -> Vec<&'a RelationClaim> {
        claims_for_edges(self.graph, &self.edge_path)
    }

    pub fn evidence(&self) -> Vec<&'a DiscoveryEvidence> {
        evidence_for_edges(self.graph, &self.edge_path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceRouteKind {
    EvidenceType,
    EvidenceProvider,
    SupportingDataService,
    EvidenceAccessService,
    Form,
}

pub struct ServiceRouteView<'a> {
    graph: &'a ServiceGraph<'a>,
    pub service: &'a SemanticAsset,
    pub target: &'a SemanticAsset,
    pub route_kind: ServiceRouteKind,
    edges: Vec<&'a SemanticRelation>,
}

impl<'a> ServiceRouteView<'a> {
    pub fn relations(&self) -> &[&'a SemanticRelation] {
        &self.edges
    }

    pub fn claims(&self) -> Vec<&'a RelationClaim> {
        self.edges
            .iter()
            .flat_map(|relation| self.graph.relation_claims(relation.id.as_str()))
            .collect()
    }

    pub fn evidence(&self) -> Vec<&'a DiscoveryEvidence> {
        self.claims()
            .into_iter()
            .map(|claim| &claim.evidence)
            .collect()
    }
}

fn endpoint_asset_id<'a>(
    endpoint: &'a RelationEndpoint,
    assets_by_id: &HashMap<&'a str, &'a SemanticAsset>,
    assets_by_uri: &HashMap<&'a str, &'a SemanticAsset>,
) -> Option<&'a str> {
    match endpoint {
        RelationEndpoint::Asset { asset_id, uri } => assets_by_id
            .get(asset_id.as_str())
            .map(|asset| asset.id.as_str())
            .or_else(|| {
                uri.as_deref()
                    .and_then(|uri| assets_by_uri.get(uri).map(|asset| asset.id.as_str()))
            }),
        RelationEndpoint::External { uri } => assets_by_uri
            .get(uri.as_str())
            .map(|asset| asset.id.as_str()),
        RelationEndpoint::BlankNode { .. } => None,
    }
}

fn claims_for_edges<'a>(
    graph: &'a ServiceGraph<'a>,
    edges: &[&'a SemanticRelation],
) -> Vec<&'a RelationClaim> {
    edges
        .iter()
        .flat_map(|relation| graph.relation_claims(relation.id.as_str()))
        .collect()
}

fn evidence_for_edges<'a>(
    graph: &'a ServiceGraph<'a>,
    edges: &[&'a SemanticRelation],
) -> Vec<&'a DiscoveryEvidence> {
    claims_for_edges(graph, edges)
        .into_iter()
        .map(|claim| &claim.evidence)
        .collect()
}
