use crate::{
    command::CommandRunner,
    terminal::{PaneId, TerminalHost},
};

pub struct HostDependencies<'a> {
    pub terminal: &'a dyn TerminalHost,
    pub repository: &'a dyn crate::repository::Repository,
    pub runner: &'a dyn CommandRunner,
}

// Compatibility with the existing persisted numeric Zellij pane IDs belongs
// at the host boundary, rather than in TerminalHost's portable contract.
pub fn pane_id(number: u32) -> PaneId {
    PaneId(number.to_string())
}
pub fn pane_number(pane: &PaneId) -> Result<u32, String> {
    pane.0
        .parse()
        .map_err(|_| "invalid terminal pane ID".into())
}
