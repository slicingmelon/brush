//! Bundled commands: utilities that ship inside the brush binary.
//!
//! Utilities are shipped busybox-style (one binary, many names) but execute
//! as a subprocess of brush so that shell redirections, pipes, and
//! process-group state are honored by code that reads/writes the host
//! process's standard fds (e.g., uutils crates).
//!
//! ## Protocol
//!
//! The brush binary recognizes a hidden first-position argument
//! [`DISPATCH_FLAG`] followed by `<NAME> [ARGS...]`. When present, brush
//! dispatches early in `main()` to the registered function for `NAME`, before
//! any shell state is built, and exits with the function's return code. The
//! dispatched function has the same signature as `uutils`' `uumain`:
//! `fn(Vec<OsString>) -> i32`, with the bundled name as `argv[0]`.
//!
//! ## Shell integration
//!
//! For every entry in the registry, [`register_shims`] installs a brush
//! builtin (using `register_builtin_if_unset`, so brush's own builtins always
//! win on conflict). The builtin's execution path uses brush-core's existing
//! external-command machinery to spawn `current_exe() <DISPATCH_FLAG> <name>
//! <args...>`, inheriting the shell's redirection state for free.
//!
//! The mechanism is generic — the registry is just `name → fn pointer`. The
//! `experimental-bundled-coreutils` feature populates it with uutils, but
//! anything matching the signature can be registered.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use brush_core::ExecutionExitCode;
use brush_core::builtins::{BoxFuture, ContentOptions, ContentType, Registration};
use brush_core::commands::{self, CommandArg, ExecutionContext};
use brush_core::extensions::ShellExtensions;

/// The leading flag that signals a bundled-command dispatch.
///
/// Deliberately obscure so that it's unlikely to collide with future
/// first-class shell flags or with scripts that happen to contain the
/// literal token.
pub const DISPATCH_FLAG: &str = "--invoke-bundled";

/// Signature of a bundled command's entry point — matches `uu_*::uumain`.
pub type BundledFn = fn(args: Vec<OsString>) -> i32;

/// Process-wide registry. Set once at startup, read on each shim invocation
/// (and during bundled-dispatch fast path).
static REGISTRY: OnceLock<HashMap<String, BundledFn>> = OnceLock::new();

/// Cached path to the running brush executable. Populated lazily on first
/// shim invocation; left as `Err`-equivalent if `current_exe()` fails.
static SELF_EXE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Installs the bundled-command registry. Idempotent: only the first call
/// takes effect.
#[allow(
    clippy::implicit_hasher,
    reason = "registry uses the default hasher; callers build with HashMap::new()"
)]
pub fn install(commands: HashMap<String, BundledFn>) {
    let _ = REGISTRY.set(commands);
}

/// Installs the registry from all compiled-in providers.
///
/// Providers are controlled by Cargo features. Binaries should call this
/// once, before [`maybe_dispatch`], so both the dispatch fast path and the
/// shell's shim builtins see a populated registry.
///
/// Multi-provider merge order: coreutils first, then non-coreutils
/// extras (findutils/diffutils/procps via `brush-bundled-extras`).
/// `HashMap::extend` semantics mean a later insertion under the same name
/// would overwrite an earlier one — currently no name overlap exists
/// between `brush-coreutils-builtins` and `brush-bundled-extras`, so
/// merge order has no observable effect today. If a future provider
/// introduces a name collision, the resolution policy needs explicit
/// documentation here.
pub fn install_default_providers() {
    #[allow(unused_mut)]
    let mut commands: HashMap<String, BundledFn> = HashMap::new();

    #[cfg(feature = "experimental-bundled-coreutils")]
    commands.extend(brush_coreutils_builtins::bundled_commands());

    #[cfg(any(
        feature = "experimental-bundled-extras",
        feature = "experimental-bundled-extras-findutils",
        feature = "experimental-bundled-extras-uutils-sed",
        feature = "experimental-bundled-extras-awk-rs",
        feature = "experimental-bundled-extras-fastgrep",
        feature = "experimental-bundled-extras-utils",
        feature = "experimental-bundled-extras-compression"
    ))]
    {
        // `brush-bundled-extras` declares its own `BundledFn` type alias
        // matching this crate's. They are nominally distinct types but
        // identical structurally (`fn(Vec<OsString>) -> i32`); the
        // explicit cast at insertion is to suppress Rust's nominal-type
        // mismatch.
        commands.extend(
            brush_bundled_extras::bundled_commands()
                .into_iter()
                .map(|(k, f)| (k, f as BundledFn)),
        );
    }

    install(commands);
}

/// Returns the registered bundled commands, if [`install`] was called.
#[must_use]
pub fn registry() -> Option<&'static HashMap<String, BundledFn>> {
    REGISTRY.get()
}

/// Runs the bundled-command fast path if the process was invoked for it.
///
/// If the process was invoked as `brush <DISPATCH_FLAG> <NAME> [ARGS...]`
/// (with `<DISPATCH_FLAG>` as the very first argument after `argv[0]`), runs
/// the registered function and returns its exit code as `Some(code)`. The
/// caller is responsible for exiting the process with that code —
/// centralizing the exit call in the binary's `main()` keeps destructors /
/// panic hooks / tracing guards in the loop.
///
/// Returns `None` when the process was not invoked as a bundled dispatch, so
/// normal shell startup can proceed.
///
/// The dispatch flag is only recognized in the leading position so that
/// ordinary scripts and command lines containing the literal token elsewhere
/// are not affected.
#[must_use]
pub fn maybe_dispatch() -> Option<i32> {
    let mut raw = std::env::args_os();
    let _argv0 = raw.next();
    let first = raw.next()?;
    if first != DISPATCH_FLAG {
        return None;
    }

    // Everything after `DISPATCH_FLAG` belongs to the bundled command. The
    // first such argument is the command name; subsequent arguments form its
    // argv (with the name itself supplied as argv[0] to match the convention
    // `uutils` and most CLI tools expect).
    let rest: Vec<OsString> = raw.collect();
    let Some((name, args)) = rest.split_first() else {
        eprintln!("brush: {DISPATCH_FLAG} requires a command name");
        return Some(exit_code(ExecutionExitCode::InvalidUsage));
    };

    // The registry is keyed by UTF-8 `String`, so a non-UTF-8 name can never
    // match. Reject up front rather than allocating a lossy-substituted
    // lookup key that could accidentally collide with a real registration.
    let Some(name_str) = name.to_str() else {
        eprintln!("brush: unknown bundled command: {}", name.to_string_lossy());
        return Some(exit_code(ExecutionExitCode::NotFound));
    };

    let Some(func) = REGISTRY.get().and_then(|r| r.get(name_str)) else {
        eprintln!("brush: unknown bundled command: {name_str}");
        return Some(exit_code(ExecutionExitCode::NotFound));
    };

    let mut argv: Vec<OsString> = Vec::with_capacity(1 + args.len());
    argv.push(name.clone());
    argv.extend(args.iter().cloned());

    Some(func(argv))
}

fn exit_code(code: ExecutionExitCode) -> i32 {
    u8::from(code).into()
}

/// Returns the path to the running brush executable (cached).
fn self_exe() -> Option<&'static PathBuf> {
    SELF_EXE
        .get_or_init(|| std::env::current_exe().ok())
        .as_ref()
}

/// Help/usage content provider for the shim builtin. brush calls this for
/// `help <name>`, `type <name>`, etc.
#[allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "signature dictated by brush_core::builtins::CommandContentFunc"
)]
fn shim_content(
    name: &str,
    content_type: ContentType,
    _options: &ContentOptions,
) -> Result<String, brush_core::Error> {
    match content_type {
        ContentType::ShortDescription => Ok(format!("{name} - bundled command")),
        ContentType::DetailedHelp => Ok(format!(
            "{name} - bundled command (executes via `brush {DISPATCH_FLAG} {name}`)\n"
        )),
        // A bundled command never contributes its own short-usage or man page
        // through this path; detailed help comes from the bundled utility
        // itself (`brush <DISPATCH_FLAG> <name> --help` or equivalent).
        ContentType::ShortUsage | ContentType::ManPage => Ok(String::new()),
    }
}

/// Defensive fallback execute function. **Unreachable on the normal call
/// path** since [`shim_registration`] attaches a
/// [`brush_core::builtins::BundledDispatch`] that routes execution
/// through brush-core's external-spawn machinery before ever reaching
/// this function. Kept for two reasons: (1) `Registration::execute_func`
/// is not optional, so we must supply something; (2) if a future
/// refactor of brush-core forgets to honor `bundled_dispatch`, this
/// function preserves the pre-Path-A behavior (spawn child, await
/// completion) so bundled commands continue to work — just without
/// pipeline parallelism.
//
// Historical TODOs (Path A resolves both):
//
//   * Process-group propagation — solved by Cycle 1 plumbing
//     (`ExecutionContext::process_group_id`) plus Path A routing through
//     `execute_via_external` which honors the field via
//     `commands.rs::execute_external_command`.
//
//   * Pipeline serialization — solved by Path A. brush-core now sees
//     `Registration::bundled_dispatch.is_some()` and short-circuits to
//     `execute_via_bundled`, which returns
//     `ExecutionSpawnResult::StartedProcess` immediately. The pipeline
//     spawn loop in `interp::spawn_pipeline_processes` proceeds to the
//     next stage without waiting.
fn shim_execute<SE: ShellExtensions>(
    context: ExecutionContext<'_, SE>,
    args: Vec<CommandArg>,
) -> BoxFuture<'_, Result<brush_core::ExecutionResult, brush_core::Error>> {
    Box::pin(async move {
        let exe_path = if let Some(p) = self_exe() {
            p.to_string_lossy().into_owned()
        } else {
            let _ = writeln!(
                context.stderr(),
                "brush: cannot determine path to running executable"
            );
            return Ok(ExecutionExitCode::CannotExecute.into());
        };

        // Build the argv for the spawned brush. `SimpleCommand::args[0]` is
        // dropped by the external-execution path (argv[0] of the spawned
        // process comes from `cmd.argv0` below), so a placeholder suffices;
        // args[1..] become the spawned process's argv[1..]. The caller's
        // `args[0]` is the bundled name by builtin-dispatch convention — we
        // replace it with an explicit `<name>` after `DISPATCH_FLAG` so the
        // child's dispatcher sees it in a fixed slot.
        let bundled_name = context.command_name.clone();
        let pgid = context.process_group_id;
        let mut child_args: Vec<CommandArg> = Vec::with_capacity(args.len() + 2);
        child_args.push(CommandArg::String(String::new())); // args[0], dropped
        child_args.push(CommandArg::String(DISPATCH_FLAG.into()));
        child_args.push(CommandArg::String(bundled_name.clone()));
        child_args.extend(args.into_iter().skip(1));

        let mut cmd = commands::SimpleCommand::new(
            commands::ShellForCommand::ParentShell(context.shell),
            context.params,
            exe_path,
            child_args,
        );
        cmd.use_functions = false;
        // Override the spawned process's argv[0] so tools that report errors
        // via their own argv[0] (uutils' `uucore::util_name()` reads
        // `std::env::args_os()[0]` into a LazyLock at first use) render as
        // `<name>:` rather than `brush:`. Without this the child sees the
        // brush exe path as argv[0] and misattributes errors.
        cmd.argv0 = Some(bundled_name);
        // Inherit the enclosing pipeline's pgid so this child joins the same
        // process group as its neighbours. No-op on Windows, where
        // `process_group` is a stub.
        cmd.process_group_id = pgid;

        let spawn_result = cmd.execute().await?;
        let wait_result = spawn_result.wait().await?;
        Ok(wait_result.into())
    })
}

/// Constructs a [`Registration`] for the bundled-shim builtin. The same
/// registration value can be reused for every bundled name; per-name
/// dispatch happens via `context.command_name` at execution time.
///
/// **Path A wiring (proto/bundled-path-a)**: this registration carries
/// a [`brush_core::builtins::BundledDispatch`] that tells brush-core to
/// route the call through external-spawn machinery instead of calling
/// `shim_execute` inline. The inline execute path (which awaits a child
/// to completion) is preserved as a defensive fallback for builds where
/// the dispatch field is somehow ignored, but on the normal call path
/// `shim_execute` is unreachable.
///
/// Returns `None` if the brush executable's path cannot be determined
/// at registration time (extremely rare — implies `current_exe()`
/// failed). In that case bundled commands are not registerable; this
/// matches the prior behavior of the inline path which would have
/// failed at execute time anyway.
fn shim_registration<SE: ShellExtensions>() -> Option<Registration<SE>> {
    let exe_path = self_exe()?.clone();
    Some(
        brush_core::builtins::raw_builtin::<SE>(shim_execute::<SE>, shim_content)
            .with_bundled_dispatch(brush_core::builtins::BundledDispatch {
                exe_path,
                dispatch_flag: DISPATCH_FLAG.to_string(),
            }),
    )
}

/// Registers a shim builtin for every name in the installed bundled-command
/// registry.
///
/// Uses `register_builtin_if_unset` so brush's own builtins (echo, printf,
/// true, false, etc.) win on conflict.
pub fn register_shims<SE: ShellExtensions>(shell: &mut brush_core::Shell<SE>) {
    let Some(registry) = REGISTRY.get() else {
        return;
    };
    // Build the registration once; reused for every bundled name. If
    // `current_exe()` failed at startup we silently skip — matches the
    // prior behavior where `shim_execute` would have failed at the
    // first invocation anyway.
    let Some(registration) = shim_registration::<SE>() else {
        return;
    };
    for name in registry.keys() {
        shell.register_builtin_if_unset(name.clone(), registration.clone());
    }
}
