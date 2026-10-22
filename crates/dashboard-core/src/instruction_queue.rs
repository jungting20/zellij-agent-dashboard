use crate::{Agent, Identity, Liveness, PanePresence, Status, StatusSource, Store};
use serde::{Deserialize, Serialize};

pub const MAX_QUEUED_INSTRUCTIONS: usize = 32;
pub const MAX_QUEUED_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedInstruction {
    pub id: String,
    pub text: String,
    pub at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    Dispatching,
    Sent,
    Uncertain,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueueDelivery {
    pub instruction: QueuedInstruction,
    pub claimed_at_ms: u64,
    pub state: DeliveryState,
    pub started_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InstructionQueue {
    pub pending: Vec<QueuedInstruction>,
    pub delivery: Option<QueueDelivery>,
}

impl InstructionQueue {
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.delivery.is_none()
    }

    // Screen readiness is an observation, not a synthetic TurnFinished event.
    pub(crate) fn observe(&mut self, working: bool, ready: bool, failed: bool, at_ms: u64) {
        let Some(delivery) = self.delivery.as_mut() else {
            return;
        };
        if delivery.state == DeliveryState::Uncertain || at_ms <= delivery.claimed_at_ms {
            return;
        }
        if failed {
            delivery.state = DeliveryState::Uncertain;
            delivery.message = "작업 실패 감지 · 확인 후 예약 재개 필요".into();
        } else if working {
            delivery.started_at_ms.get_or_insert(at_ms);
            if delivery.state == DeliveryState::Sent {
                delivery.message = "작업 완료 확인 대기".into();
            }
        } else if ready && delivery.started_at_ms.is_some_and(|at| at_ms > at) {
            delivery.completed_at_ms = Some(at_ms);
            if delivery.state == DeliveryState::Sent {
                self.delivery = None;
            }
        }
    }
}

pub fn validate_instruction(text: &str) -> Result<(), String> {
    if text.trim().is_empty()
        || text.len() > MAX_QUEUED_BYTES
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err("입력은 64 KiB 이하의 텍스트여야 합니다".into());
    }
    Ok(())
}

impl Agent {
    fn ready_for_queued_input(&self, now_ms: u64) -> bool {
        self.liveness == Liveness::Live
            && !self.ended
            && self.pane.presence == PanePresence::Present
            && matches!(self.status, Status::Idle | Status::Done)
            && match self.status_source {
                StatusSource::Hook => self.last_hook_report_ms.is_some(),
                StatusSource::Screen => self.last_report_ms.is_some_and(|at| {
                    Some(at) == self.last_screen_report_ms && now_ms.saturating_sub(at) <= 10_000
                }),
                StatusSource::Unknown => false,
            }
    }
}

impl Store {
    /// Input while busy, or behind an existing queue, always joins the FIFO.
    /// None means the caller may use the ordinary immediate-input path.
    pub fn queue_input(
        &mut self,
        target: &Identity,
        id: &str,
        text: &str,
        at_ms: u64,
    ) -> Result<Option<usize>, String> {
        validate_instruction(text)?;
        self.validate_target(target)?;
        let agent = self.data.agents.get_mut(&target.agent_id).unwrap();
        let queue = &mut agent.instruction_queue;
        if let Some(index) = queue.pending.iter().position(|item| item.id == id) {
            if queue.pending[index].text != text {
                return Err("예약 ID가 다른 내용에 재사용되었습니다".into());
            }
            return Ok(Some(index + 1));
        }
        if let Some(delivery) = &queue.delivery {
            if delivery.instruction.id == id {
                if delivery.instruction.text != text {
                    return Err("예약 ID가 다른 내용에 재사용되었습니다".into());
                }
                return Ok(Some(0));
            }
        }
        if queue.is_empty() && !matches!(agent.status, Status::Working | Status::Compact) {
            return Ok(None);
        }
        let bytes = queue.pending.iter().map(|i| i.text.len()).sum::<usize>()
            + queue
                .delivery
                .as_ref()
                .map_or(0, |d| d.instruction.text.len());
        if id.is_empty()
            || queue.pending.len() + usize::from(queue.delivery.is_some())
                >= MAX_QUEUED_INSTRUCTIONS
            || bytes + text.len() > MAX_QUEUED_BYTES
        {
            return Err("예약 한도 초과 · 에이전트당 32개, 총 64 KiB".into());
        }
        queue.pending.push(QueuedInstruction {
            id: id.into(),
            text: text.into(),
            at_ms,
        });
        let position = queue.pending.len();
        self.touch_queue(&target.agent_id);
        Ok(Some(position))
    }

    fn touch_queue(&mut self, id: &str) {
        self.changes.agents.insert(id.into());
        self.changes.metadata = true;
        self.data.revision += 1;
    }

    pub fn expire_queue_deliveries(&mut self, now_ms: u64) {
        let mut changed = Vec::new();
        for agent in self.data.agents.values_mut() {
            if let Some(delivery) = agent.instruction_queue.delivery.as_mut() {
                let expired = match delivery.state {
                    DeliveryState::Dispatching => {
                        now_ms.saturating_sub(delivery.claimed_at_ms) > 15_000
                    }
                    DeliveryState::Sent => {
                        delivery.started_at_ms.is_none()
                            && now_ms.saturating_sub(delivery.claimed_at_ms) > 60_000
                    }
                    DeliveryState::Uncertain => false,
                };
                if expired {
                    delivery.state = DeliveryState::Uncertain;
                    delivery.message =
                        "전송 또는 작업 시작 확인 시간 초과 · 자동 재전송 없음".into();
                    changed.push(agent.identity.agent_id.clone());
                }
            }
        }
        for id in changed {
            self.touch_queue(&id);
        }
    }

    /// Persist this claim before any terminal operation; replicas cannot claim it twice.
    pub fn claim_queued_input(&mut self, now_ms: u64) -> Option<(Identity, QueuedInstruction)> {
        let id = self
            .data
            .agents
            .values()
            .filter(|a| {
                a.ready_for_queued_input(now_ms)
                    && a.instruction_queue.delivery.is_none()
                    && !a.instruction_queue.pending.is_empty()
            })
            .min_by_key(|a| {
                (
                    a.instruction_queue.pending[0].at_ms,
                    a.identity.agent_id.clone(),
                )
            })?
            .identity
            .agent_id
            .clone();
        let agent = self.data.agents.get_mut(&id).unwrap();
        let instruction = agent.instruction_queue.pending.remove(0);
        agent.instruction_queue.delivery = Some(QueueDelivery {
            instruction: instruction.clone(),
            claimed_at_ms: now_ms,
            state: DeliveryState::Dispatching,
            started_at_ms: None,
            completed_at_ms: None,
            message: "전송 중".into(),
        });
        let identity = agent.identity.clone();
        self.touch_queue(&id);
        Some((identity, instruction))
    }

    pub fn validate_queued_delivery(
        &self,
        target: &Identity,
        id: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        self.validate_target(target)?;
        let agent = &self.data.agents[&target.agent_id];
        if !agent.ready_for_queued_input(now_ms)
            || !agent.instruction_queue.delivery.as_ref().is_some_and(|d| {
                d.instruction.id == id
                    && d.state == DeliveryState::Dispatching
                    && d.started_at_ms.is_none()
                    && now_ms.saturating_sub(d.claimed_at_ms) <= 15_000
            })
        {
            return Err("예약 전송 전에 대상 상태가 변경되었습니다".into());
        }
        Ok(())
    }

    pub fn finish_queued_delivery(
        &mut self,
        target: &Identity,
        id: &str,
        error: Option<String>,
    ) -> Result<(), String> {
        let agent = self
            .data
            .agents
            .get_mut(&target.agent_id)
            .ok_or("예약 대상 실행이 없습니다")?;
        if agent.identity != *target {
            return Err("예약 대상 실행이 변경되었습니다".into());
        }
        let Some(delivery) = agent
            .instruction_queue
            .delivery
            .as_mut()
            .filter(|d| d.instruction.id == id)
        else {
            return Err("예약 전송 기록이 변경되었습니다".into());
        };
        if let Some(error) = error {
            delivery.state = DeliveryState::Uncertain;
            delivery.message = format!("전송 확인 필요 · 자동 재전송 없음: {error}");
        } else if delivery.state != DeliveryState::Uncertain {
            delivery.state = DeliveryState::Sent;
            delivery.message = if delivery.started_at_ms.is_some() {
                "작업 완료 확인 대기"
            } else {
                "작업 시작 확인 대기"
            }
            .into();
            if delivery.completed_at_ms.is_some() {
                agent.instruction_queue.delivery = None;
            }
        }
        self.touch_queue(&target.agent_id);
        Ok(())
    }

    pub fn cancel_queued_input(&mut self, target: &Identity, id: &str) -> Result<(), String> {
        self.validate_target(target)?;
        let queue = &mut self
            .data
            .agents
            .get_mut(&target.agent_id)
            .unwrap()
            .instruction_queue;
        let index = queue
            .pending
            .iter()
            .position(|item| item.id == id)
            .ok_or("대기 중인 예약만 취소할 수 있습니다")?;
        queue.pending.remove(index);
        self.touch_queue(&target.agent_id);
        Ok(())
    }

    /// Explicit acknowledgement discards the uncertain item; never retry it.
    pub fn resume_instruction_queue(&mut self, target: &Identity) -> Result<(), String> {
        self.validate_target(target)?;
        let queue = &mut self
            .data
            .agents
            .get_mut(&target.agent_id)
            .unwrap()
            .instruction_queue;
        if !queue
            .delivery
            .as_ref()
            .is_some_and(|d| d.state == DeliveryState::Uncertain)
        {
            return Err("확인이 필요한 예약이 없습니다".into());
        }
        queue.delivery = None;
        self.touch_queue(&target.agent_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentEvent, EventKind, FoundProcess, PaneInfo, StateSignal, StatusObservation};

    fn found(id: &str) -> FoundProcess {
        FoundProcess {
            identity: Identity {
                agent_id: id.into(),
                session_name: "dev".into(),
                session_epoch: "epoch".into(),
                pane_id: 7,
                incarnation_id: id.into(),
                pid: 20,
                process_started: id.into(),
            },
            tool: "codex".into(),
            cwd: "/tmp".into(),
        }
    }
    fn screen(store: &mut Store, at: u64, status: Option<Status>) {
        store
            .apply_signal(&StateSignal::Screen(StatusObservation {
                identity: found("run").identity,
                tool: "codex".into(),
                observation_id: format!("screen-{at}"),
                observed_at_ms: at,
                status,
                visible_idle: true,
                rule_id: "test".into(),
            }))
            .unwrap();
    }
    fn seeded() -> (Store, Identity) {
        let mut store = Store::default();
        let target = found("run").identity;
        store.reconcile(&[found("run")], 1000);
        store.observe_pane(
            &target,
            PaneInfo {
                presence: PanePresence::Present,
                observed_at_ms: 4000,
                ..PaneInfo::default()
            },
        );
        screen(&mut store, 5000, Some(Status::Working));
        (store, target)
    }
    fn enqueue(store: &mut Store, target: &Identity, id: &str) {
        assert!(store.queue_input(target, id, id, 5100).unwrap().is_some());
    }
    #[test]
    fn fifo_needs_start_and_completion_before_next_delivery() {
        let (mut store, target) = seeded();
        enqueue(&mut store, &target, "one");
        enqueue(&mut store, &target, "two");
        assert!(store.claim_queued_input(5200).is_none());
        screen(&mut store, 6000, Some(Status::Idle));
        let (_, item) = store.claim_queued_input(6001).unwrap();
        assert_eq!(item.id, "one");
        assert!(store.claim_queued_input(6002).is_none());
        store.finish_queued_delivery(&target, "one", None).unwrap();
        for at in [6500, 7000] {
            screen(&mut store, at, Some(Status::Idle));
            assert!(store.claim_queued_input(at + 1).is_none());
        }
        screen(&mut store, 8000, Some(Status::Working));
        screen(&mut store, 9000, Some(Status::Idle));
        assert_eq!(store.claim_queued_input(9001).unwrap().1.id, "two");
    }
    #[test]
    fn completion_during_dispatch_waits_for_send_ack_and_hooks_own_completion() {
        let (mut store, target) = seeded();
        enqueue(&mut store, &target, "one");
        enqueue(&mut store, &target, "two");
        screen(&mut store, 6000, Some(Status::Idle));
        store.claim_queued_input(6001).unwrap();
        let hook = |sequence, kind, at| AgentEvent {
            schema_version: 1,
            event_id: format!("hook-{sequence}"),
            identity: target.clone(),
            tool: "codex".into(),
            kind,
            sequence,
            observed_at_ms: at,
            cwd: String::new(),
            summary: String::new(),
            detail: String::new(),
        };
        store.apply(&hook(1, EventKind::TurnStarted, 7000)).unwrap();
        screen(&mut store, 8000, Some(Status::Idle));
        assert!(store.claim_queued_input(8001).is_none());
        store
            .apply(&hook(2, EventKind::TurnFinished, 9000))
            .unwrap();
        assert!(store.claim_queued_input(9001).is_none());
        store.finish_queued_delivery(&target, "one", None).unwrap();
        assert_eq!(store.claim_queued_input(9002).unwrap().1.id, "two");
    }
    #[test]
    fn uncertainty_reload_expiry_and_manual_ack_never_retry_the_item() {
        let (mut store, target) = seeded();
        enqueue(&mut store, &target, "one");
        enqueue(&mut store, &target, "two");
        screen(&mut store, 6000, Some(Status::Idle));
        store.claim_queued_input(6001).unwrap();
        let mut store: Store =
            serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        store.expire_queue_deliveries(22000);
        assert_eq!(
            store.agents["run"]
                .instruction_queue
                .delivery
                .as_ref()
                .unwrap()
                .state,
            DeliveryState::Uncertain
        );
        screen(&mut store, 23000, Some(Status::Working));
        screen(&mut store, 24000, Some(Status::Idle));
        assert!(store.claim_queued_input(24001).is_none());
        assert!(store.cancel_queued_input(&target, "one").is_err());
        store.resume_instruction_queue(&target).unwrap();
        assert_eq!(store.claim_queued_input(24002).unwrap().1.id, "two");
        store
            .finish_queued_delivery(&target, "two", Some("write failed".into()))
            .unwrap();
        assert!(store.claim_queued_input(24003).is_none());
    }
    #[test]
    fn limits_duplicates_cancel_generation_and_legacy_defaults() {
        let (mut store, target) = seeded();
        for index in 0..MAX_QUEUED_INSTRUCTIONS {
            enqueue(&mut store, &target, &format!("{index}"));
        }
        assert!(store
            .queue_input(&target, "overflow", "text", 5100)
            .is_err());
        assert_eq!(store.queue_input(&target, "0", "0", 5101).unwrap(), Some(1));
        assert!(store.queue_input(&target, "0", "different", 5101).is_err());
        store.cancel_queued_input(&target, "0").unwrap();
        assert_eq!(store.agents["run"].instruction_queue.pending[0].id, "1");
        store.reconcile(&[found("new")], 6000);
        assert!(store.agents["new"].instruction_queue.is_empty());
        assert!(store.claim_queued_input(6001).is_none());
        let mut data = store.into_data();
        data.schema_version = 3;
        let mut json = serde_json::to_value(data).unwrap();
        for agent in json["agents"].as_object_mut().unwrap().values_mut() {
            agent.as_object_mut().unwrap().remove("instruction_queue");
        }
        let restored = Store::loaded(serde_json::from_value(json).unwrap(), None).unwrap();
        assert_eq!(restored.schema_version, crate::SCHEMA_VERSION);
        assert!(restored
            .agents
            .values()
            .all(|a| a.instruction_queue.is_empty()));
    }
    #[test]
    fn byte_limit_invalid_input_and_failed_turn_stop_progress() {
        let (mut store, target) = seeded();
        let large = "x".repeat(MAX_QUEUED_BYTES);
        store.queue_input(&target, "large", &large, 5100).unwrap();
        assert!(store.queue_input(&target, "extra", "x", 5101).is_err());
        for text in ["", "  ", "bad\x1btext"] {
            assert!(store.queue_input(&target, "invalid", text, 5101).is_err());
        }
        store.cancel_queued_input(&target, "large").unwrap();
        enqueue(&mut store, &target, "one");
        screen(&mut store, 6000, Some(Status::Idle));
        store.claim_queued_input(6001).unwrap();
        screen(&mut store, 7000, Some(Status::Failed));
        store.finish_queued_delivery(&target, "one", None).unwrap();
        assert_eq!(
            store.agents["run"]
                .instruction_queue
                .delivery
                .as_ref()
                .unwrap()
                .state,
            DeliveryState::Uncertain
        );
        assert!(store.claim_queued_input(7001).is_none());
    }

    #[test]
    fn overlays_and_stale_screen_readiness_do_not_release_the_queue() {
        let (mut store, target) = seeded();
        enqueue(&mut store, &target, "one");
        screen(&mut store, 6000, Some(Status::Idle));
        screen(&mut store, 7000, None);
        assert!(store.claim_queued_input(7001).is_none());
        screen(&mut store, 8000, Some(Status::Idle));
        assert!(store.claim_queued_input(18001).is_none());
        screen(&mut store, 19000, Some(Status::Idle));
        store.claim_queued_input(19001).unwrap();
        store.finish_queued_delivery(&target, "one", None).unwrap();
        store.expire_queue_deliveries(80000);
        assert_eq!(
            store.agents["run"]
                .instruction_queue
                .delivery
                .as_ref()
                .unwrap()
                .state,
            DeliveryState::Uncertain
        );
    }
}
