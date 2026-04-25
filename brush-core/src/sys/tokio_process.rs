//! Process management utilities

pub(crate) type ProcessId = i32;
pub(crate) use tokio::process::Child;

pub(crate) fn spawn(mut command: std::process::Command) -> std::io::Result<Child> {
    #[cfg(windows)]
    {
        // Suppress the console window Windows would otherwise allocate for a
        // console-subsystem child when brush has no attached console (e.g.,
        // when invoked by a non-console host such as an editor terminal or
        // an automation harness). stdio handles still inherit via
        // STARTUPINFO, so pipelines and redirections are unaffected.
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut command = tokio::process::Command::from(command);
    command.spawn()
}
