//! Domain decisions for host commands. Adapters collect facts and execute effects.
use crate::{
    ActionResult, Identity, LaunchInfo, Liveness, PaneInfo, RequestState, ScanLease, Status,
    StatusSource, Store,
};
use std::collections::BTreeSet;

impl Store {
    pub fn validate_close(&self, target: &Identity) -> Result<(), String> {
        self.validate_target(target)?;
        let mut id = Some(target.agent_id.as_str());
        let mut seen = BTreeSet::new();
        while let Some(current) = id {
            if !seen.insert(current) {
                break;
            }
            let Some(agent) = self.agents.get(current) else {
                break;
            };
            if agent.pinned {
                return Err("고정된 에이전트는 먼저 고정을 해제해야 종료할 수 있습니다".into());
            }
            id = agent.parent_id.as_deref();
        }
        Ok(())
    }

    pub fn close_confirmed(&mut self, target: &Identity) -> Result<(), String> {
        self.validate_close(target)?;
        let agent = self.data.agents.get_mut(&target.agent_id).unwrap();
        agent.ended = true;
        agent.liveness = Liveness::Gone;
        self.data.revision += 1;
        Ok(())
    }

    pub fn set_alias(&mut self, target: &Identity, alias: &str) -> Result<(), String> {
        if alias.len() > 120 || alias.chars().any(char::is_control) {
            return Err("한 줄 태그를 입력해주세요 (120 bytes 이하)".into());
        }
        self.validate_target(target)?;
        let alias = alias.trim();
        let agent = self.data.agents.get_mut(&target.agent_id).unwrap();
        if agent.alias != alias {
            agent.alias = alias.into();
            self.data.revision += 1;
        }
        Ok(())
    }

    pub fn validate_merge(&self, parent: &Identity, child: &Identity) -> Result<(), String> {
        self.validate_target(parent)?;
        self.validate_target(child)?;
        let pa = &self.agents[&parent.agent_id];
        let ch = &self.agents[&child.agent_id];
        if ch.parent_id.as_ref() != Some(&parent.agent_id) {
            return Err("선택한 에이전트는 직접 자식이 아닙니다".into());
        }
        if !matches!(pa.status, Status::Idle | Status::Done)
            || !matches!(ch.status, Status::Idle | Status::Done)
        {
            return Err("부모와 자식이 idle 또는 done일 때 병합 지시를 보낼 수 있습니다".into());
        }
        Ok(())
    }

    pub fn record_launch(&mut self, id: &str, launch: LaunchInfo) -> Result<(), String> {
        self.pending_request(id)?;
        if self.launches.contains_key(id) {
            return Err("launch already recorded".into());
        }
        self.data.launches.insert(id.into(), launch);
        self.data.revision += 1;
        Ok(())
    }

    pub fn record_launch_pane(&mut self, id: &str, pane: u32) -> Result<(), String> {
        let launch = self
            .data
            .launches
            .get_mut(id)
            .ok_or("launch record disappeared")?;
        if launch.pane_id.is_some_and(|old| old != pane) {
            return Err("launch pane changed".into());
        }
        if launch.pane_id != Some(pane) {
            launch.pane_id = Some(pane);
            self.data.revision += 1;
        }
        Ok(())
    }

    /// The adapter extracts the launch marker; generation and association rules stay here.
    pub fn associate_launch(&mut self, target: &Identity, id: &str) -> bool {
        if self.validate_target(target).is_err() {
            return false;
        }
        let Some(launch) = self.data.launches.get_mut(id) else {
            return false;
        };
        let agent = self.data.agents.get_mut(&target.agent_id).unwrap();
        if launch.session != target.session_name
            || launch.epoch != target.session_epoch
            || launch.tool != agent.tool
            || launch.pane_id.is_some_and(|pane| pane != target.pane_id)
            || launch
                .agent_id
                .as_ref()
                .is_some_and(|old| old != &target.agent_id)
        {
            return false;
        }
        if launch.pane_id == Some(target.pane_id)
            && launch.agent_id.as_ref() == Some(&target.agent_id)
            && agent.parent_id == launch.parent_id
            && agent.cwd == launch.cwd
        {
            return false;
        }
        launch.pane_id = Some(target.pane_id);
        launch.agent_id = Some(target.agent_id.clone());
        agent.parent_id.clone_from(&launch.parent_id);
        agent.cwd.clone_from(&launch.cwd);
        self.data.revision += 1;
        true
    }

    fn pending_request(&self, id: &str) -> Result<&ActionResult, String> {
        let record = self.requests.get(id).ok_or("request record disappeared")?;
        if record.state != RequestState::Pending {
            return Err("request already finished".into());
        }
        Ok(record)
    }

    pub fn record_request_path(&mut self, id: &str, path: String) -> Result<(), String> {
        self.pending_request(id)?;
        self.data.requests.get_mut(id).unwrap().path = path;
        self.data.revision += 1;
        Ok(())
    }

    pub fn finish_request(
        &mut self,
        id: &str,
        state: RequestState,
        message: String,
        pane: Option<u32>,
        path: Option<String>,
    ) -> Result<ActionResult, String> {
        self.pending_request(id)?;
        if state == RequestState::Pending {
            return Err("completion must be terminal".into());
        }
        let record = self.data.requests.get_mut(id).unwrap();
        record.state = state;
        record.message = message;
        record.pane_id = pane;
        if let Some(path) = path {
            record.path = path;
        }
        let response = record.clone();
        self.data.revision += 1;
        Ok(response)
    }

    pub fn claim_scan(&mut self, token: &str, at: u64) -> bool {
        if at.saturating_sub(self.last_scan_ms) < 1800
            || self
                .scan_lease
                .as_ref()
                .is_some_and(|lease| lease.expires_at_ms > at)
        {
            return false;
        }
        self.data.scan_lease = Some(ScanLease {
            token: token.into(),
            expires_at_ms: at.saturating_add(10_000),
        });
        true
    }

    pub fn owns_scan(&self, token: &str, at: u64) -> bool {
        self.scan_lease
            .as_ref()
            .is_some_and(|lease| lease.token == token && lease.expires_at_ms > at)
    }

    pub fn release_scan(&mut self, token: &str) -> bool {
        if self
            .scan_lease
            .as_ref()
            .is_none_or(|lease| lease.token != token)
        {
            return false;
        }
        self.data.scan_lease = None;
        true
    }

    pub fn observe_pane_with_cwd(
        &mut self,
        target: &Identity,
        pane: PaneInfo,
        cwd: Option<&str>,
    ) -> bool {
        if !self.observe_pane(target, pane) {
            return false;
        }
        let agent = self.data.agents.get_mut(&target.agent_id).unwrap();
        if agent.status_source != StatusSource::Hook {
            if let Some(cwd) = cwd.filter(|value| !value.is_empty()) {
                agent.cwd = cwd.into();
            }
        }
        true
    }

    pub fn note_screen_attempt(&mut self, target: &Identity, at: u64) -> bool {
        let Some(agent) = self.data.agents.get_mut(&target.agent_id) else {
            return false;
        };
        if agent.identity != *target
            || agent.liveness != Liveness::Live
            || agent.ended
            || agent.status_source == StatusSource::Hook
            || at < agent.last_screen_attempt_ms
        {
            return false;
        }
        if agent.last_screen_attempt_ms != at {
            agent.last_screen_attempt_ms = at;
            self.data.revision += 1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, ActionRequest, FoundProcess};

    fn fixture() -> (Store, Identity) {
        let identity = Identity {
            agent_id: "agent".into(),
            session_name: "dev".into(),
            session_epoch: "server".into(),
            pane_id: 7,
            incarnation_id: "run".into(),
            pid: 20,
            process_started: "start".into(),
        };
        let mut store = Store::default();
        store.reconcile(
            &[FoundProcess {
                identity: identity.clone(),
                tool: "codex".into(),
                cwd: "/tmp".into(),
            }],
            5000,
        );
        (store, identity)
    }

    #[test]
    fn alias_and_close_share_generation_and_pin_protection() {
        let (mut store, target) = fixture();
        let before = store.revision;
        store.set_alias(&target, " 태그 ").unwrap();
        store.set_alias(&target, "태그").unwrap();
        assert_eq!(store.revision, before + 1);
        assert!(store.set_alias(&target, "bad\nlabel").is_err());
        let mut old = target.clone();
        old.incarnation_id = "old".into();
        assert!(store.set_alias(&old, "wrong").is_err());
        store.set_pinned(&target.agent_id, true).unwrap();
        assert!(store.validate_close(&target).is_err());
        assert!(store.close_confirmed(&target).is_err());
        store.set_pinned(&target.agent_id, false).unwrap();
        store.close_confirmed(&target).unwrap();
        assert!(store.agents[&target.agent_id].ended);
        assert_eq!(store.agents[&target.agent_id].liveness, Liveness::Gone);
    }

    #[test]
    fn launch_link_rejects_other_generations_and_completion_is_terminal() {
        let (mut store, target) = fixture();
        let request = ActionRequest {
            request_id: "launch".into(),
            action: Action::Launch {
                session: "dev".into(),
                epoch: "server".into(),
                cwd: "/worktree".into(),
                tool: "codex".into(),
            },
        };
        store.claim(&request, 5001).unwrap();
        store
            .record_launch(
                "launch",
                LaunchInfo {
                    parent_id: Some("parent".into()),
                    session: "dev".into(),
                    epoch: "server".into(),
                    cwd: "/worktree".into(),
                    tool: "codex".into(),
                    pane_id: Some(7),
                    agent_id: None,
                },
            )
            .unwrap();
        let mut old = target.clone();
        old.session_epoch = "old-server".into();
        assert!(!store.associate_launch(&old, "launch"));
        assert!(store.associate_launch(&target, "launch"));
        let revision = store.revision;
        assert!(!store.associate_launch(&target, "launch"));
        assert_eq!(store.revision, revision);
        assert_eq!(
            store.agents[&target.agent_id].parent_id.as_deref(),
            Some("parent")
        );
        store
            .record_request_path("launch", "/worktree".into())
            .unwrap();
        let result = store
            .finish_request(
                "launch",
                RequestState::Uncertain,
                "stopped".into(),
                None,
                None,
            )
            .unwrap();
        assert_eq!(result.path, "/worktree");
        assert!(store
            .finish_request(
                "launch",
                RequestState::Succeeded,
                "retry".into(),
                None,
                None
            )
            .is_err());
        assert!(!store.claim(&request, 6000).unwrap());
    }

    #[test]
    fn scan_release_cannot_clear_replacement_and_late_attempt_cannot_rewind() {
        let (mut store, target) = fixture();
        assert!(store.claim_scan("first", 7000));
        assert!(!store.claim_scan("second", 7001));
        assert!(store.claim_scan("second", 17001));
        assert!(!store.release_scan("first"));
        assert!(store.owns_scan("second", 17002));
        assert!(store.note_screen_attempt(&target, 17002));
        assert!(!store.note_screen_attempt(&target, 16000));
        store
            .data
            .agents
            .get_mut(&target.agent_id)
            .unwrap()
            .status_source = StatusSource::Hook;
        assert!(!store.note_screen_attempt(&target, 18000));
    }
}
