//! Normalized inputs. Tool formats and terminal rules belong to host adapters.
use crate::{AgentEvent, Identity, Status};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusSource {
    #[default]
    Unknown,
    Hook,
    Screen,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusObservation {
    pub identity: Identity,
    pub tool: String,
    pub observation_id: String,
    pub observed_at_ms: u64,
    /// None means an overlay/no-match that must preserve the previous state.
    pub status: Option<Status>,
    pub visible_idle: bool,
    pub rule_id: String,
}

/// Events and snapshots share a core boundary, without inventing turn events
/// from a screen or treating a dashboard submission as a hook connection.
#[derive(Clone, Debug)]
pub enum StateSignal {
    Hook(AgentEvent),
    Screen(StatusObservation),
    Instruction {
        identity: Identity,
        observed_at_ms: u64,
        text: String,
    },
}

impl From<AgentEvent> for StateSignal {
    fn from(event: AgentEvent) -> Self {
        Self::Hook(event)
    }
}

impl From<StatusObservation> for StateSignal {
    fn from(observation: StatusObservation) -> Self {
        Self::Screen(observation)
    }
}
