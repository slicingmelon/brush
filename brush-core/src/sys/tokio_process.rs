//! Process management utilities

pub(crate) type ProcessId = i32;
pub(crate) use tokio::process::Child;

#[cfg(windows)]
fn host_has_attached_console() -> bool {
    use std::sync::OnceLock;
    static HAS_CONSOLE: OnceLock<bool> = OnceLock::new();
    *HAS_CONSOLE.get_or_init(|| {
        // `GetConsoleWindow` returns NULL when the calling process has no
        // attached console. We only need this once per process — the
        // attached-console state is established at startup and the spawn
        // path is hot.
        unsafe extern "system" {
            fn GetConsoleWindow() -> *mut core::ffi::c_void;
        }
        // SAFETY: `GetConsoleWindow` is a Win32 API with no preconditions —
        // it always returns a HWND or NULL, never invokes UB.
        !unsafe { GetConsoleWindow() }.is_null()
    })
}

pub(crate) fn spawn(mut command: std::process::Command) -> std::io::Result<Child> {
    #[cfg(windows)]
    {
        // Suppress the console window Windows would otherwise allocate for a
        // console-subsystem child when brush itself has NO attached console
        // (e.g., invoked by a non-console host such as Claude Code's Bash
        // tool, an editor terminal, or an automation harness). In that
        // scenario the child inherits stdio handles that are pipes/files,
        // so suppressing the new console is purely cosmetic and correct.
        //
        // CRITICAL: when brush DOES have an attached console (interactive
        // cmd / pwsh / Windows Terminal use), CREATE_NO_WINDOW detaches the
        // child from that console. Inherited stdio handles are then console
        // handles the child can no longer write to, and bundled coreutils
        // (`ls`, `cat`, `wc`, ...) which re-exec brush as a shim child
        // produce no visible output. So gate the flag on "host has no
        // console of its own".
        if !host_has_attached_console() {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
    }
    let mut command = tokio::process::Command::from(command);
    command.spawn()
}
