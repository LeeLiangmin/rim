use super::{ExecStatus, ManagerSubcommands};
use crate::core::try_it;
use anyhow::Result;

/// Execute `try-it` command.
pub(super) fn execute(subcommand: &ManagerSubcommands) -> Result<ExecStatus> {
    let ManagerSubcommands::TryIt { path } = subcommand else {
        return Ok(ExecStatus::default());
    };

    // On Linux CLI mode, don't open editor automatically
    #[cfg(target_os = "linux")]
    let open_editor = false;
    #[cfg(not(target_os = "linux"))]
    let open_editor = true;

    try_it::try_it(path.as_deref(), open_editor)?;
    Ok(ExecStatus::new_executed().no_pause(true))
}
