use crate::lanes::gate::GateResult;
use serde::{Deserialize, Serialize};

/// Outcome of a single lane execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaneStatus {
    Success,
    Skipped,
    Failed,
}

impl LaneStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success | Self::Skipped)
    }
}

/// Summary of durable writes performed by a lane.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneWritesSummary {
    pub tables_read: Vec<String>,
    pub tables_written: Vec<String>,
    pub notes: Vec<String>,
}

impl LaneWritesSummary {
    pub fn wrote(mut self, table: impl Into<String>) -> Self {
        self.tables_written.push(table.into());
        self
    }

    pub fn read(mut self, table: impl Into<String>) -> Self {
        self.tables_read.push(table.into());
        self
    }

    pub fn note(mut self, message: impl Into<String>) -> Self {
        self.notes.push(message.into());
        self
    }
}

/// Result returned by [`crate::lanes::Lane::run`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneResult {
    pub lane_name: String,
    pub status: LaneStatus,
    pub writes: LaneWritesSummary,
    pub gate_results: Vec<GateResult>,
    pub error_message: Option<String>,
}

impl LaneResult {
    pub fn success(lane_name: impl Into<String>, writes: LaneWritesSummary) -> Self {
        Self {
            lane_name: lane_name.into(),
            status: LaneStatus::Success,
            writes,
            gate_results: Vec::new(),
            error_message: None,
        }
    }

    pub fn skipped(lane_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            lane_name: lane_name.into(),
            status: LaneStatus::Skipped,
            writes: LaneWritesSummary::default().note(reason),
            gate_results: Vec::new(),
            error_message: None,
        }
    }

    pub fn failed(lane_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            lane_name: lane_name.into(),
            status: LaneStatus::Failed,
            writes: LaneWritesSummary::default(),
            gate_results: Vec::new(),
            error_message: Some(message.into()),
        }
    }

    pub fn with_gate_results(mut self, gate_results: Vec<GateResult>) -> Self {
        self.gate_results = gate_results;
        self
    }

    pub fn has_blocking_gate_failure(&self) -> bool {
        self.gate_results.iter().any(GateResult::is_blocking)
    }
}

/// Aggregate report from a [`crate::lanes::LinearRunner`] execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinearRunReport {
    pub lane_results: Vec<LaneResult>,
    pub stopped_early: bool,
    pub stop_reason: Option<String>,
}

impl LinearRunReport {
    pub fn completed_all_lanes(&self) -> bool {
        !self.stopped_early
    }

    pub fn last_lane_result(&self) -> Option<&LaneResult> {
        self.lane_results.last()
    }
}
