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
    pub cwd: PathBuf,
    pub title: String,
    pub floating: bool,
    pub close_on_exit: bool,
    pub no_focus: bool,
    pub program: OsString,
    pub args: Vec<OsString>,
}

pub trait TerminalHost {
    fn list_panes(
        &self,
        session: &SessionId,
        all_tabs: bool,
        timeout: Duration,
    ) -> Result<Vec<TerminalPane>, String>;
    fn screen(&self, session: &SessionId, pane: &PaneId) -> Result<String, String>;
    fn write_text(&self, session: &SessionId, pane: &PaneId, text: &str) -> Result<(), String>;
    fn write_bytes(&self, session: &SessionId, pane: &PaneId, bytes: &[u8]) -> Result<(), String>;
    fn close_pane(&self, session: &SessionId, pane: &PaneId) -> Result<(), String>;
    fn new_pane(&self, session: &SessionId, options: &NewPane) -> Result<PaneId, String>;
    fn notify_changed(&self, session: &SessionId, event_id: &str) -> Result<(), String>;
}
