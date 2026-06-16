use super::{
    build_catalog::{BuildCatalogLane, CatalogResolutionStrategy},
    build_narrative_map::BuildNarrativeMapLane,
    init_workspace::InitWorkspaceLane, lane::Lane, research_pipeline::financial_analysis_lanes,
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
            if request.runs_narrative_map() {
                lanes.push(Arc::new(BuildNarrativeMapLane::new(
                    NarrativeMapStrategy::agent_defaults(std::path::PathBuf::new()),
                )));
            }
            if request.build_financial_analysis {
                lanes.extend(financial_analysis_lanes());
            }
        }
    }
    lanes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn base_request() -> InitWorkspaceRequest {
        InitWorkspaceRequest {
            ticker: "ORCL".to_string(),
            date: "2026-06-13".to_string(),
            base_dir: PathBuf::from("reports"),
            ..InitWorkspaceRequest::default()
        }
    }

    fn lane_names(request: &InitWorkspaceRequest) -> Vec<&'static str> {
        lanes_for_request(request)
            .iter()
            .map(|lane| lane.name())
            .collect()
    }

    #[test]
    fn default_request_runs_full_research_pipeline() {
        let names = lane_names(&base_request());
        assert_eq!(
            names,
            vec![
                "init_workspace",
                "build_catalog",
                "build_narrative_map",
                "financial_fan_out",
            ]
        );
    }

    #[test]
    fn narrative_map_can_be_disabled_without_financial_analysis() {
        let mut request = base_request();
        request.build_narrative_map = false;
        request.build_financial_analysis = false;
        let names = lane_names(&request);
        assert_eq!(names, vec!["init_workspace", "build_catalog"]);
    }

    #[test]
    fn financial_analysis_can_be_disabled_while_keeping_narrative_map() {
        let mut request = base_request();
        request.build_financial_analysis = false;
        let names = lane_names(&request);
        assert_eq!(
            names,
            vec!["init_workspace", "build_catalog", "build_narrative_map"]
        );
    }
}
