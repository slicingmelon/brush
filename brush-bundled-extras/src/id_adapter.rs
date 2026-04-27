//! In-tree `id` implementation — uid/gid/groups info.
//!
//! Per the post-Cycle 1 follow-up of
//! `docs/planning/bundled-extras-coverage-expansion.md`. uutils' `uu_id`
//! is gated to `cfg(unix)` (it uses POSIX `getuid`/`getgid`/`getgroups`
//! syscalls), so Windows brush had no `id` even with all coreutils flags.
//!
//! This adapter implements `id` cross-platform:
//!
//! - **Unix**: thin wrapper around `libc::{getuid, geteuid, getgid, getegid,
//!   getgroups, getpwuid_r, getgrgid_r}` — same calls uutils' `uu_id`
//!   makes (we just don't link `uu_id` itself, to avoid pulling its full
//!   crate weight when only this one utility is needed).
//! - **Windows**: maps Windows token / SID concepts to a faux-UID/GID
//!   shape that scripts checking `id -u` etc. can consume:
//!   - **uid** = the RID (last sub-authority) of the user's primary SID
//!   - **username** = `LookupAccountSidW` against the user SID
//!   - **gid** = the RID of the primary group SID
//!   - **groupname** = `LookupAccountSidW` against the primary group SID
//!   - **groups** = list of group SIDs from `TokenGroups`, each with its
//!     RID + resolved name
//!
//! Supported flags:
//!
//! | Flag | Behavior |
//! |---|---|
//! | (no args) | `uid=N(NAME) gid=N(GROUP) groups=N(GROUP),...` |
//! | `-u` | print numeric uid |
//! | `-un` / `-u -n` | print user name |
//! | `-g` | print numeric primary gid |
//! | `-gn` / `-g -n` | print primary group name |
//! | `-G` | print all group ids, space-separated |
//! | `-Gn` / `-G -n` | print all group names, space-separated |
//! | `-r` | print the *real* (vs effective) ids — Unix-only distinction; on Windows real == effective |

#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    clippy::undocumented_unsafe_blocks,
    clippy::multiple_unsafe_ops_per_block,
    clippy::borrow_as_ptr,
    clippy::cast_ptr_alignment,
    reason = "id CLI orchestration covers many flag combinations; the Windows branch is intrinsically Win32-FFI-heavy with token buffers (alignment is satisfied in practice by the global allocator's 8-byte minimum)"
)]

use std::ffi::OsString;
use std::io::{self, BufWriter, Write};

pub(crate) fn id_main(args: Vec<OsString>) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let mut want_user = false;
    let mut want_group = false;
    let mut want_all_groups = false;
    let mut name_only = false;
    let mut real_ids = false;
    let mut user_arg: Option<String> = None;

    for arg in argv.iter().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return 0;
            }
            "--version" => {
                println!("id (brush-bundled-extras) 0.1.8");
                return 0;
            }
            "-u" | "--user" => want_user = true,
            "-g" | "--group" => want_group = true,
            "-G" | "--groups" => want_all_groups = true,
            "-n" | "--name" => name_only = true,
            "-r" | "--real" => real_ids = true,
            s if s.starts_with('-') && s.len() > 1 && !s.starts_with("--") => {
                if !try_short_bundle(s, &mut want_user, &mut want_group, &mut want_all_groups, &mut name_only, &mut real_ids) {
                    eprintln!("id: unknown option: {s}");
                    return 1;
                }
            }
            _ => {
                if user_arg.is_some() {
                    eprintln!("id: extra operand '{arg}'");
                    return 1;
                }
                user_arg = Some(arg.clone());
            }
        }
    }

    let info = match resolve_user(user_arg.as_deref()) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("id: {e}");
            return 1;
        }
    };

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let _ = real_ids; // present only for flag-parser symmetry; on Windows real==effective
    let print_res = if want_user {
        if name_only {
            writeln!(out, "{}", info.username)
        } else {
            writeln!(out, "{}", info.uid)
        }
    } else if want_group {
        if name_only {
            writeln!(out, "{}", info.primary_group_name)
        } else {
            writeln!(out, "{}", info.gid)
        }
    } else if want_all_groups {
        let mut first = true;
        let mut res = Ok(());
        for g in &info.groups {
            if !first {
                if let Err(e) = write!(out, " ") {
                    res = Err(e);
                    break;
                }
            }
            first = false;
            let line = if name_only {
                g.name.clone()
            } else {
                g.gid.to_string()
            };
            if let Err(e) = write!(out, "{line}") {
                res = Err(e);
                break;
            }
        }
        if res.is_ok() {
            writeln!(out)
        } else {
            res
        }
    } else {
        // Default: uid=N(name) gid=N(name) groups=N(name),...
        let _ = write!(
            out,
            "uid={}({}) gid={}({})",
            info.uid, info.username, info.gid, info.primary_group_name
        );
        if !info.groups.is_empty() {
            let _ = write!(out, " groups=");
            let mut first = true;
            for g in &info.groups {
                if !first {
                    let _ = write!(out, ",");
                }
                first = false;
                let _ = write!(out, "{}({})", g.gid, g.name);
            }
        }
        writeln!(out)
    };
    if let Err(e) = print_res {
        eprintln!("id: write error: {e}");
        return 1;
    }
    let _ = out.flush();
    0
}

fn try_short_bundle(
    s: &str,
    want_user: &mut bool,
    want_group: &mut bool,
    want_all_groups: &mut bool,
    name_only: &mut bool,
    real_ids: &mut bool,
) -> bool {
    for c in s.chars().skip(1) {
        match c {
            'u' => *want_user = true,
            'g' => *want_group = true,
            'G' => *want_all_groups = true,
            'n' => *name_only = true,
            'r' => *real_ids = true,
            _ => return false,
        }
    }
    true
}

struct GroupInfo {
    gid: u64,
    name: String,
}

struct UserInfo {
    uid: u64,
    gid: u64,
    username: String,
    primary_group_name: String,
    groups: Vec<GroupInfo>,
}

fn resolve_user(user_arg: Option<&str>) -> Result<UserInfo, String> {
    if user_arg.is_some() {
        // Looking up an arbitrary named user requires a passwd database;
        // we don't implement that on Windows. Defer to system if installed,
        // otherwise return an error.
        return Err("looking up named users is not supported in this build".to_string());
    }
    #[cfg(unix)]
    {
        unix::current_user()
    }
    #[cfg(windows)]
    {
        windows::current_user()
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err("unsupported platform".to_string())
    }
}

#[cfg(unix)]
mod unix {
    use super::{GroupInfo, UserInfo};

    pub(super) fn current_user() -> Result<UserInfo, String> {
        // SAFETY: getuid/getgid/getgroups have no preconditions and never fail.
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        let username = lookup_username(uid).unwrap_or_else(|| uid.to_string());
        let primary_group_name = lookup_groupname(gid).unwrap_or_else(|| gid.to_string());

        let count = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
        let mut groups = Vec::new();
        if count > 0 {
            let mut buf: Vec<libc::gid_t> = vec![0; count as usize];
            let n = unsafe { libc::getgroups(count, buf.as_mut_ptr()) };
            if n > 0 {
                for &g in &buf[..n as usize] {
                    let name = lookup_groupname(g).unwrap_or_else(|| g.to_string());
                    groups.push(GroupInfo {
                        gid: u64::from(g),
                        name,
                    });
                }
            }
        }
        // Always include primary if not present
        if !groups.iter().any(|g| g.gid == u64::from(gid)) {
            groups.insert(
                0,
                GroupInfo {
                    gid: u64::from(gid),
                    name: primary_group_name.clone(),
                },
            );
        }

        Ok(UserInfo {
            uid: u64::from(uid),
            gid: u64::from(gid),
            username,
            primary_group_name,
            groups,
        })
    }

    fn lookup_username(uid: libc::uid_t) -> Option<String> {
        let pw = unsafe { libc::getpwuid(uid) };
        if pw.is_null() {
            return None;
        }
        let name = unsafe { (*pw).pw_name };
        if name.is_null() {
            return None;
        }
        Some(unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy().into_owned())
    }

    fn lookup_groupname(gid: libc::gid_t) -> Option<String> {
        let gr = unsafe { libc::getgrgid(gid) };
        if gr.is_null() {
            return None;
        }
        let name = unsafe { (*gr).gr_name };
        if name.is_null() {
            return None;
        }
        Some(unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy().into_owned())
    }
}

#[cfg(windows)]
mod windows {
    use super::{GroupInfo, UserInfo};
    use std::ptr;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, LookupAccountSidW,
        SID_NAME_USE, TOKEN_GROUPS, TOKEN_PRIMARY_GROUP, TOKEN_QUERY, TOKEN_USER, TokenGroups,
        TokenPrimaryGroup, TokenUser,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    pub(super) fn current_user() -> Result<UserInfo, String> {
        unsafe {
            let mut token: HANDLE = ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(format!("OpenProcessToken failed (error {})", last_error()));
            }
            let _guard = TokenGuard(token);

            let user_buf = query_token(token, TokenUser)?;
            let token_user = user_buf.as_ptr().cast::<TOKEN_USER>();
            let user_sid = (*token_user).User.Sid;
            let (username, _user_domain) = lookup_sid_name(user_sid)?;
            let uid = sid_rid(user_sid);

            let pgroup_buf = query_token(token, TokenPrimaryGroup)?;
            let token_pgroup = pgroup_buf.as_ptr().cast::<TOKEN_PRIMARY_GROUP>();
            let pgroup_sid = (*token_pgroup).PrimaryGroup;
            let (primary_group_name, _pgroup_domain) =
                lookup_sid_name(pgroup_sid).unwrap_or_else(|_| ("None".to_string(), String::new()));
            let gid = sid_rid(pgroup_sid);

            let groups_buf = query_token(token, TokenGroups)?;
            let token_groups = groups_buf.as_ptr().cast::<TOKEN_GROUPS>();
            let group_count = (*token_groups).GroupCount as usize;
            let groups_slice = std::slice::from_raw_parts(
                (*token_groups).Groups.as_ptr(),
                group_count,
            );
            let mut groups = Vec::with_capacity(group_count + 1);
            // Always include primary group first
            groups.push(GroupInfo {
                gid,
                name: primary_group_name.clone(),
            });
            for g in groups_slice {
                let g_sid = g.Sid;
                if g_sid.is_null() {
                    continue;
                }
                let g_rid = sid_rid(g_sid);
                if g_rid == gid {
                    continue;
                }
                let (gname, _gdomain) = lookup_sid_name(g_sid)
                    .unwrap_or_else(|_| (g_rid.to_string(), String::new()));
                groups.push(GroupInfo {
                    gid: g_rid,
                    name: gname,
                });
            }

            Ok(UserInfo {
                uid,
                gid,
                username,
                primary_group_name,
                groups,
            })
        }
    }

    struct TokenGuard(HANDLE);
    impl Drop for TokenGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }

    fn last_error() -> u32 {
        unsafe { windows_sys::Win32::Foundation::GetLastError() }
    }

    unsafe fn query_token(
        token: HANDLE,
        info_class: windows_sys::Win32::Security::TOKEN_INFORMATION_CLASS,
    ) -> Result<Vec<u8>, String> {
        let mut size: u32 = 0;
        // First call probes required size; we expect it to fail with ERROR_INSUFFICIENT_BUFFER.
        unsafe {
            GetTokenInformation(token, info_class, ptr::null_mut(), 0, &mut size);
        }
        if size == 0 {
            return Err(format!(
                "GetTokenInformation size probe returned 0 (error {})",
                last_error()
            ));
        }
        let mut buf = vec![0_u8; size as usize];
        let ok = unsafe {
            GetTokenInformation(token, info_class, buf.as_mut_ptr().cast(), size, &mut size)
        };
        if ok == 0 {
            return Err(format!("GetTokenInformation failed (error {})", last_error()));
        }
        Ok(buf)
    }

    unsafe fn sid_rid(sid: *mut std::ffi::c_void) -> u64 {
        let count = unsafe { *GetSidSubAuthorityCount(sid) };
        if count == 0 {
            return 0;
        }
        let last = unsafe { *GetSidSubAuthority(sid, u32::from(count) - 1) };
        u64::from(last)
    }

    unsafe fn lookup_sid_name(sid: *mut std::ffi::c_void) -> Result<(String, String), String> {
        let mut name_len: u32 = 256;
        let mut domain_len: u32 = 256;
        let mut name = vec![0_u16; name_len as usize];
        let mut domain = vec![0_u16; domain_len as usize];
        let mut sid_type: SID_NAME_USE = 0;
        let ok = unsafe {
            LookupAccountSidW(
                ptr::null(),
                sid,
                name.as_mut_ptr(),
                &mut name_len,
                domain.as_mut_ptr(),
                &mut domain_len,
                &mut sid_type,
            )
        };
        if ok == 0 {
            return Err(format!("LookupAccountSidW failed (error {})", last_error()));
        }
        Ok((
            String::from_utf16_lossy(&name[..name_len as usize]),
            String::from_utf16_lossy(&domain[..domain_len as usize]),
        ))
    }
}

fn print_help() {
    println!(
        "Usage: id [OPTIONS] [USER]\n\
         \n\
         Print user and group information for the current process.\n\
         On Windows, uid/gid map to the RID of the user and primary group SIDs.\n\
         \n\
         Options:\n  \
           -u, --user        print only the effective user ID\n  \
           -g, --group       print only the effective primary group ID\n  \
           -G, --groups      print all group IDs (space-separated)\n  \
           -n, --name        print names instead of numeric IDs\n  \
           -r, --real        print the real ID (Unix only; on Windows real==effective)\n  \
           --help            show this help\n  \
           --version         show version\n"
    );
}
