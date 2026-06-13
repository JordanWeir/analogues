use super::{
    build_catalog::{BuildCatalogLane, CatalogResolutionStrategy},
    build_narrative_map::BuildNarrativeMapLane,
    init_workspace::InitWorkspaceLane, lane::Lane,
};
use crate::{
    lanes::build_narrative_map::strategy::NarrativeMapStrategy, workspace::InitWorkspaceRequest,
};
use std::sync::Arc;

/// Build the linear init pipeline lanes for a workspace request.
pub fn lanes_for_request(request: &InitWorkspaceRequest) -> Vec<Arc<dyn Lane>> {
    let mut lanes: Vec<Arc<dyn Lane>> = vec![Arc::new(InitWorkspaceLane::new(request))];
    if request.fetch_financials {
        if let Some(strategy) = request.mapping_strategy {
            lanes.push(Arc::new(BuildCatalogLane::new(
                CatalogResolutionStrategy::from_mapping_strategy(strategy),
            )));
            if request.build_narrative_map {
                lanes.push(Arc::new(BuildNarrativeMapLane::new(
                    NarrativeMapStrategy::agent_defaults(std::path::PathBuf::new()),
                )));
            }
        }
    }
    lanes
}
