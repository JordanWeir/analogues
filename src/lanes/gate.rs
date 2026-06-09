use crate::lanes::{context::LaneContext, result::LaneResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const GATE_STATUS_PASS: &str = "pass";
pub const GATE_STATUS_WARN: &str = "warn";
pub const GATE_STATUS_REJECT: &str = "reject";
pub const GATE_STATUS_QUARANTINE: &str = "quarantine";

/// Quality gate outcome for a lane checkpoint.
///
/// Aligns with the blackboard quality gate model in `01-pipeline-plan.md`:
/// pass, warn, reject, or quarantine before downstream lanes trust output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateStatus {
    Pass,
    Warn,
    Reject,
    Quarantine,
}

impl GateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => GATE_STATUS_PASS,
            Self::Warn => GATE_STATUS_WARN,
            Self::Reject => GATE_STATUS_REJECT,
            Self::Quarantine => GATE_STATUS_QUARANTINE,
        }
    }

    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::Reject | Self::Quarantine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_name: String,
    pub status: GateStatus,
    pub message: Option<String>,
}

impl GateResult {
    pub fn pass(gate_name: impl Into<String>) -> Self {
        Self {
            gate_name: gate_name.into(),
            status: GateStatus::Pass,
            message: None,
        }
    }

    pub fn warn(gate_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            gate_name: gate_name.into(),
            status: GateStatus::Warn,
            message: Some(message.into()),
        }
    }

    pub fn reject(gate_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            gate_name: gate_name.into(),
            status: GateStatus::Reject,
            message: Some(message.into()),
        }
    }

    pub fn quarantine(gate_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            gate_name: gate_name.into(),
            status: GateStatus::Quarantine,
            message: Some(message.into()),
        }
    }

    pub fn is_blocking(&self) -> bool {
        self.status.is_blocking()
    }
}

/// Post-lane quality check evaluated against workspace state and lane output.
#[async_trait]
pub trait Gate: Send + Sync {
    fn name(&self) -> &'static str;

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult;
}
