//! Backend-independent terminal operations. No CLI syntax crosses this boundary.
use std::{ffi::OsString, path::PathBuf, time::Duration};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionId(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneId(pub String);

#[derive(Clone, Debug)]
pub struct TerminalPane {
    pub id: PaneId,
    pub tab_id: Option<u32>,
    pub tab_name: String,
    pub title: String,
    pub cwd: Option<String>,
}

pub struct NewPane {
    pub tab_id: Option<u32>,
    pub cwd: PathBuf,
    pub title: String,
    pub floating: bool,
    pub close_on_exit: bool,
    pub no_focus: bool,
    pub program: OsString,
    pub args: Vec<OsString>,
}

pub trait TerminalHost {
    /// Optional live-session inventory, excluding exited sessions awaiting resurrection.
    fn active_sessions(&self) -> Result<Option<std::collections::BTreeSet<String>>, String> {
        Ok(None)
    }
    fn list_panes(
        &self,
        session: &SessionId,
        all_tabs: bool,
        timeout: Duration,
    ) -> Result<Vec<TerminalPane>, String>;
    fn screen(&self, session: &SessionId, pane: &PaneId) -> Result<String, String>;
    /// Styled viewport for composer detection; plain backends remain supported.
    fn styled_screen(&self, session: &SessionId, pane: &PaneId) -> Result<String, String> {
        self.screen(session, pane)
    }
    fn write_text(&self, session: &SessionId, pane: &PaneId, text: &str) -> Result<(), String>;
    fn write_bytes(&self, session: &SessionId, pane: &PaneId, bytes: &[u8]) -> Result<(), String>;
    fn close_pane(&self, session: &SessionId, pane: &PaneId) -> Result<(), String>;
    fn new_pane(&self, session: &SessionId, options: &NewPane) -> Result<PaneId, String>;
    /// Create a project tab, starting a detached session if requested.
    fn create_project_tab(
        &self,
        _session: &SessionId,
        _cwd: &std::path::Path,
        _new_session: bool,
    ) -> Result<u32, String> {
        Err("project tab creation is unsupported by this terminal backend".into())
    }
    fn notify_changed(&self, session: &SessionId, event_id: &str) -> Result<(), String>;
}
