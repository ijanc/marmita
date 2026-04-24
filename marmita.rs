// vim: set tw=79 cc=80 ts=4 sw=4 sts=4 et :
//
// Copyright (c) 2026 Murilo Ijanc' <murilo@ijanc.org>
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//

use std::{
    env,
    ffi::{CStr, CString},
    fs,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process, ptr, slice,
    time::{SystemTime, UNIX_EPOCH},
};

const VENDOR_DIR: &str = "vendor";
const VENDOR_FILE: &str = "vendor/VENDOR";
const TRAILER: &str =
    "Managed by marmita(1).  Use 'marmita update' to refresh.\n";

//////////////////////////////////////////////////////////////////////////////
// FFI (libgit2)
//////////////////////////////////////////////////////////////////////////////

#[allow(non_camel_case_types)]
mod ffi {
    use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

    pub const GIT_OID_RAWSZ: usize = 20;
    pub const GIT_OBJECT_BLOB: c_int = 3;

    pub const GIT_CLONE_OPTIONS_VERSION: c_uint = 1;

    pub const GIT_PASSTHROUGH: c_int = -30;
    pub const GIT_CREDENTIAL_SSH_KEY: c_uint = 1 << 1;
    pub const GIT_CREDENTIAL_USERNAME: c_uint = 1 << 5;

    #[repr(C)]
    pub struct git_oid {
        pub id: [c_uchar; GIT_OID_RAWSZ],
    }

    pub enum git_repository {}
    pub enum git_object {}
    pub enum git_blob {}
    pub enum git_credential {}

    #[repr(C)]
    pub struct git_error {
        pub message: *const c_char,
        pub klass: c_int,
    }

    #[repr(C)]
    pub struct git_strarray {
        pub strings: *mut *mut c_char,
        pub count: usize,
    }

    // Layout taken from libgit2 1.9 git2/checkout.h.  All callback fields
    // are typed as raw pointers since marmita never sets them.
    #[repr(C)]
    pub struct git_checkout_options {
        pub version: c_uint,
        pub checkout_strategy: c_uint,
        pub disable_filters: c_int,
        pub dir_mode: c_uint,
        pub file_mode: c_uint,
        pub file_open_flags: c_int,
        pub notify_flags: c_uint,
        pub notify_cb: *mut c_void,
        pub notify_payload: *mut c_void,
        pub progress_cb: *mut c_void,
        pub progress_payload: *mut c_void,
        pub paths: git_strarray,
        pub baseline: *mut c_void,
        pub baseline_index: *mut c_void,
        pub target_directory: *const c_char,
        pub ancestor_label: *const c_char,
        pub our_label: *const c_char,
        pub their_label: *const c_char,
        pub perfdata_cb: *mut c_void,
        pub perfdata_payload: *mut c_void,
    }

    pub type git_credential_acquire_cb = unsafe extern "C" fn(
        out: *mut *mut git_credential,
        url: *const c_char,
        username_from_url: *const c_char,
        allowed_types: c_uint,
        payload: *mut c_void,
    ) -> c_int;

    // Layout taken from libgit2 1.9 git2/remote.h.  The deprecated
    // update_tips and resolve_url fields are present because the system
    // libgit2 is not built with GIT_DEPRECATE_HARD.
    #[repr(C)]
    pub struct git_remote_callbacks {
        pub version: c_uint,
        pub sideband_progress: *mut c_void,
        pub completion: *mut c_void,
        pub credentials: Option<git_credential_acquire_cb>,
        pub certificate_check: *mut c_void,
        pub transfer_progress: *mut c_void,
        pub update_tips: *mut c_void,
        pub pack_progress: *mut c_void,
        pub push_transfer_progress: *mut c_void,
        pub push_update_reference: *mut c_void,
        pub push_negotiation: *mut c_void,
        pub transport: *mut c_void,
        pub remote_ready: *mut c_void,
        pub payload: *mut c_void,
        pub resolve_url: *mut c_void,
        pub update_refs: *mut c_void,
    }

    #[repr(C)]
    pub struct git_proxy_options {
        pub version: c_uint,
        pub typ: c_int,
        pub url: *const c_char,
        pub credentials: *mut c_void,
        pub certificate_check: *mut c_void,
        pub payload: *mut c_void,
    }

    #[repr(C)]
    pub struct git_fetch_options {
        pub version: c_int,
        pub callbacks: git_remote_callbacks,
        pub prune: c_int,
        pub update_fetchhead: c_uint,
        pub download_tags: c_int,
        pub proxy_opts: git_proxy_options,
        pub depth: c_int,
        pub follow_redirects: c_int,
        pub custom_headers: git_strarray,
    }

    #[repr(C)]
    pub struct git_clone_options {
        pub version: c_uint,
        pub checkout_opts: git_checkout_options,
        pub fetch_opts: git_fetch_options,
        pub bare: c_int,
        pub local: c_int,
        pub checkout_branch: *const c_char,
        pub repository_cb: *mut c_void,
        pub repository_cb_payload: *mut c_void,
        pub remote_cb: *mut c_void,
        pub remote_cb_payload: *mut c_void,
    }

    #[link(name = "git2")]
    unsafe extern "C" {
        pub fn git_libgit2_init() -> c_int;
        pub fn git_libgit2_shutdown() -> c_int;

        pub fn git_clone(
            out: *mut *mut git_repository,
            url: *const c_char,
            path: *const c_char,
            opts: *const git_clone_options,
        ) -> c_int;
        pub fn git_clone_options_init(
            opts: *mut git_clone_options,
            version: c_uint,
        ) -> c_int;
        pub fn git_repository_free(repo: *mut git_repository);

        pub fn git_revparse_single(
            out: *mut *mut git_object,
            repo: *mut git_repository,
            spec: *const c_char,
        ) -> c_int;
        pub fn git_object_id(obj: *const git_object) -> *const git_oid;
        pub fn git_object_type(obj: *const git_object) -> c_int;
        pub fn git_object_free(obj: *mut git_object);

        pub fn git_oid_tostr(
            out: *mut c_char,
            n: usize,
            oid: *const git_oid,
        ) -> *const c_char;

        pub fn git_blob_rawcontent(blob: *const git_blob) -> *const c_void;
        pub fn git_blob_rawsize(blob: *const git_blob) -> i64;

        pub fn git_credential_ssh_key_from_agent(
            out: *mut *mut git_credential,
            username: *const c_char,
        ) -> c_int;
        pub fn git_credential_ssh_key_new(
            out: *mut *mut git_credential,
            username: *const c_char,
            publickey: *const c_char,
            privatekey: *const c_char,
            passphrase: *const c_char,
        ) -> c_int;
        pub fn git_credential_username_new(
            out: *mut *mut git_credential,
            username: *const c_char,
        ) -> c_int;

        pub fn git_error_last() -> *const git_error;
    }
}

//////////////////////////////////////////////////////////////////////////////
// libgit2 wrapper
//////////////////////////////////////////////////////////////////////////////

struct Repo(*mut ffi::git_repository);

impl Drop for Repo {
    fn drop(&mut self) {
        unsafe { ffi::git_repository_free(self.0) };
    }
}

fn git_init() -> Result<(), String> {
    let rc = unsafe { ffi::git_libgit2_init() };
    if rc < 0 {
        return Err(format!("libgit2 init: {}", last_err()));
    }
    Ok(())
}

fn git_shutdown() {
    unsafe { ffi::git_libgit2_shutdown() };
}

fn last_err() -> String {
    unsafe {
        let e = ffi::git_error_last();
        if e.is_null() || (*e).message.is_null() {
            return "unknown libgit2 error".to_string();
        }
        CStr::from_ptr((*e).message).to_string_lossy().into_owned()
    }
}

fn cstring(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|e| e.to_string())
}

fn cstring_path(p: &Path) -> Result<CString, String> {
    CString::new(p.as_os_str().as_bytes()).map_err(|e| e.to_string())
}

struct CredState {
    attempt: i32,
    keys: Vec<CString>,
}

unsafe extern "C" fn cred_cb(
    out: *mut *mut ffi::git_credential,
    _url: *const std::os::raw::c_char,
    username_from_url: *const std::os::raw::c_char,
    allowed_types: std::os::raw::c_uint,
    payload: *mut std::os::raw::c_void,
) -> std::os::raw::c_int {
    if allowed_types & ffi::GIT_CREDENTIAL_USERNAME != 0 {
        return unsafe {
            ffi::git_credential_username_new(out, username_from_url)
        };
    }
    if allowed_types & ffi::GIT_CREDENTIAL_SSH_KEY == 0 {
        return ffi::GIT_PASSTHROUGH;
    }
    let state = unsafe { &mut *(payload as *mut CredState) };
    let attempt = state.attempt;
    state.attempt += 1;
    if attempt == 0 {
        return unsafe {
            ffi::git_credential_ssh_key_from_agent(out, username_from_url)
        };
    }
    let idx = (attempt - 1) as usize;
    if idx >= state.keys.len() {
        return -1;
    }
    unsafe {
        ffi::git_credential_ssh_key_new(
            out,
            username_from_url,
            ptr::null(),
            state.keys[idx].as_ptr(),
            ptr::null(),
        )
    }
}

fn ssh_keyfiles() -> Vec<CString> {
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    ["id_ed25519", "id_rsa", "id_ecdsa"]
        .iter()
        .filter_map(|name| {
            let path = format!("{home}/.ssh/{name}");
            if Path::new(&path).exists() {
                CString::new(path).ok()
            } else {
                None
            }
        })
        .collect()
}

fn clone(url: &str, path: &Path) -> Result<Repo, String> {
    let url_c = cstring(url)?;
    let path_c = cstring_path(path)?;
    let mut opts: ffi::git_clone_options = unsafe { std::mem::zeroed() };
    let rc = unsafe {
        ffi::git_clone_options_init(&mut opts, ffi::GIT_CLONE_OPTIONS_VERSION)
    };
    if rc != 0 {
        return Err(format!("clone init: {}", last_err()));
    }
    let mut state = CredState {
        attempt: 0,
        keys: ssh_keyfiles(),
    };
    opts.fetch_opts.callbacks.payload =
        &mut state as *mut _ as *mut std::os::raw::c_void;
    opts.fetch_opts.callbacks.credentials = Some(cred_cb);
    let mut repo: *mut ffi::git_repository = ptr::null_mut();
    let rc = unsafe {
        ffi::git_clone(&mut repo, url_c.as_ptr(), path_c.as_ptr(), &opts)
    };
    if rc != 0 {
        return Err(format!("clone {url}: {}", last_err()));
    }
    Ok(Repo(repo))
}

fn resolve_commit(repo: &Repo, spec: &str) -> Result<String, String> {
    let peeled = format!("{spec}^{{commit}}");
    let s_c = cstring(&peeled)?;
    let mut obj: *mut ffi::git_object = ptr::null_mut();
    let rc =
        unsafe { ffi::git_revparse_single(&mut obj, repo.0, s_c.as_ptr()) };
    if rc != 0 {
        return Err(format!("resolve {spec}: {}", last_err()));
    }
    let oid = unsafe { ffi::git_object_id(obj) };
    let mut buf = [0u8; 41];
    unsafe {
        ffi::git_oid_tostr(buf.as_mut_ptr() as *mut i8, buf.len(), oid);
        ffi::git_object_free(obj);
    }
    let nul = buf.iter().position(|&b| b == 0).unwrap_or(40);
    std::str::from_utf8(&buf[..nul])
        .map(|s| s.to_string())
        .map_err(|e| e.to_string())
}

fn read_blob(repo: &Repo, commit: &str, file: &str) -> Result<Vec<u8>, String> {
    let spec = format!("{commit}:{file}");
    let s_c = cstring(&spec)?;
    let mut obj: *mut ffi::git_object = ptr::null_mut();
    let rc =
        unsafe { ffi::git_revparse_single(&mut obj, repo.0, s_c.as_ptr()) };
    if rc != 0 {
        return Err(format!("lookup {file}: {}", last_err()));
    }
    let ty = unsafe { ffi::git_object_type(obj) };
    if ty != ffi::GIT_OBJECT_BLOB {
        unsafe { ffi::git_object_free(obj) };
        return Err(format!("{file} is not a regular file at {commit}"));
    }
    let blob = obj as *mut ffi::git_blob;
    let size = unsafe { ffi::git_blob_rawsize(blob) } as usize;
    let data = unsafe { ffi::git_blob_rawcontent(blob) } as *const u8;
    let bytes = unsafe { slice::from_raw_parts(data, size) }.to_vec();
    unsafe { ffi::git_object_free(obj) };
    Ok(bytes)
}

//////////////////////////////////////////////////////////////////////////////
// VENDOR manifest
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone)]
struct Entry {
    file: String,
    origin: String,
    reference: Option<String>,
    commit: String,
    date: String,
}

fn parse_vendor(text: &str) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    let mut cur: Option<Entry> = None;
    for (n, raw) in text.lines().enumerate() {
        let lineno = n + 1;
        if raw.trim().is_empty() {
            if let Some(e) = cur.take() {
                push_entry(&mut out, e, lineno)?;
            }
            continue;
        }
        if !raw.starts_with(|c: char| c.is_whitespace()) {
            // Header line.  Anything that does not look like an entry
            // header (e.g. trailing prose) ends the manifest body.
            if let Some(e) = cur.take() {
                push_entry(&mut out, e, lineno)?;
            }
            let name = raw.trim();
            if name.contains(char::is_whitespace) {
                // Free-form line; stop parsing entries here.
                break;
            }
            cur = Some(Entry {
                file: name.to_string(),
                origin: String::new(),
                reference: None,
                commit: String::new(),
                date: String::new(),
            });
            continue;
        }
        let line = raw.trim_start();
        let (key, val) = match line.split_once(':') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => {
                return Err(format!(
                    "{VENDOR_FILE}:{lineno}: expected 'key: value'"
                ));
            }
        };
        let e = cur.as_mut().ok_or_else(|| {
            format!("{VENDOR_FILE}:{lineno}: attribute outside entry")
        })?;
        match key {
            "origin" => e.origin = val.to_string(),
            "ref" => e.reference = Some(val.to_string()),
            "commit" => e.commit = val.to_string(),
            "date" => e.date = val.to_string(),
            _ => {
                return Err(format!(
                    "{VENDOR_FILE}:{lineno}: unknown attribute '{key}'"
                ));
            }
        }
    }
    if let Some(e) = cur.take() {
        push_entry(&mut out, e, 0)?;
    }
    Ok(out)
}

fn push_entry(
    out: &mut Vec<Entry>,
    e: Entry,
    lineno: usize,
) -> Result<(), String> {
    if e.origin.is_empty() || e.commit.is_empty() || e.date.is_empty() {
        return Err(format!(
            "{}:{}: entry '{}' is missing origin/commit/date",
            VENDOR_FILE, lineno, e.file
        ));
    }
    out.push(e);
    Ok(())
}

fn format_vendor(entries: &[Entry]) -> String {
    let mut s = String::new();
    for e in entries {
        s.push_str(&e.file);
        s.push('\n');
        s.push_str(&format!("\torigin:\t{}\n", e.origin));
        if let Some(r) = &e.reference {
            s.push_str(&format!("\tref:\t{r}\n"));
        }
        s.push_str(&format!("\tcommit:\t{}\n", e.commit));
        s.push_str(&format!("\tdate:\t{}\n", e.date));
        s.push('\n');
    }
    s.push_str(TRAILER);
    s
}

fn read_manifest() -> Result<Vec<Entry>, String> {
    match fs::read_to_string(VENDOR_FILE) {
        Ok(t) => parse_vendor(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("read {VENDOR_FILE}: {e}")),
    }
}

fn write_manifest(entries: &mut [Entry]) -> Result<(), String> {
    entries.sort_by(|a, b| a.file.cmp(&b.file));
    fs::create_dir_all(VENDOR_DIR)
        .map_err(|e| format!("mkdir {VENDOR_DIR}: {e}"))?;
    fs::write(VENDOR_FILE, format_vendor(entries))
        .map_err(|e| format!("write {VENDOR_FILE}: {e}"))
}

//////////////////////////////////////////////////////////////////////////////
// Helpers
//////////////////////////////////////////////////////////////////////////////

fn deduce_filename(url: &str) -> String {
    let last = url.rsplit(['/', ':']).next().unwrap_or(url);
    let stem = last.strip_suffix(".git").unwrap_or(last);
    format!("{stem}.rs")
}

fn looks_like_oid(s: &str) -> bool {
    s.len() >= 7 && s.len() <= 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn today_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

// Howard Hinnant's days_from_civil inverse.  See
// https://howardhinnant.github.io/date_algorithms.html#civil_from_days
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn make_tempdir() -> Result<PathBuf, String> {
    let pid = process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let p = env::temp_dir().join(format!("marmita.{pid}.{nanos}"));
    if p.exists() {
        fs::remove_dir_all(&p)
            .map_err(|e| format!("clean tempdir {p:?}: {e}"))?;
    }
    Ok(p)
}

fn fetch_file(
    url: &str,
    spec: &str,
    file: &str,
) -> Result<(String, Vec<u8>), String> {
    let tmp = make_tempdir()?;
    let result = (|| {
        let repo = clone(url, &tmp)?;
        let commit = resolve_commit(&repo, spec)?;
        let bytes = read_blob(&repo, &commit, file)?;
        Ok((commit, bytes))
    })();
    let _ = fs::remove_dir_all(&tmp);
    result
}

//////////////////////////////////////////////////////////////////////////////
// Commands
//////////////////////////////////////////////////////////////////////////////

fn cmd_add(args: &[String]) -> Result<(), String> {
    let mut url: Option<String> = None;
    let mut reference: Option<String> = None;
    let mut file_arg: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-r" => {
                i += 1;
                if i >= args.len() {
                    return Err("-r requires an argument".to_string());
                }
                reference = Some(args[i].clone());
            }
            other if url.is_none() => url = Some(other.to_string()),
            other if file_arg.is_none() => file_arg = Some(other.to_string()),
            _ => return Err(usage_str().to_string()),
        }
        i += 1;
    }
    let url = url.ok_or_else(|| usage_str().to_string())?;
    let file = file_arg.unwrap_or_else(|| deduce_filename(&url));
    let spec = reference.as_deref().unwrap_or("HEAD");

    let (commit, bytes) = fetch_file(&url, spec, &file)?;

    fs::create_dir_all(VENDOR_DIR)
        .map_err(|e| format!("mkdir {VENDOR_DIR}: {e}"))?;
    let dest = Path::new(VENDOR_DIR).join(&file);
    fs::write(&dest, &bytes)
        .map_err(|e| format!("write {}: {e}", dest.display()))?;

    let mut entries: Vec<Entry> = read_manifest()?
        .into_iter()
        .filter(|e| e.file != file)
        .collect();
    let stored_ref = match &reference {
        Some(r) if !looks_like_oid(r) => Some(r.clone()),
        _ => None,
    };
    entries.push(Entry {
        file: file.clone(),
        origin: url,
        reference: stored_ref,
        commit: commit.clone(),
        date: today_utc(),
    });
    write_manifest(&mut entries)?;

    println!("added {file} ({})", short(&commit));
    Ok(())
}

fn cmd_update(args: &[String]) -> Result<(), String> {
    if args.len() > 1 {
        return Err(usage_str().to_string());
    }
    let mut entries = read_manifest()?;
    if entries.is_empty() {
        return Err(format!("{VENDOR_FILE}: no entries"));
    }
    let target = args.first().cloned();
    let mut touched = 0usize;
    for e in entries.iter_mut() {
        if let Some(t) = &target
            && &e.file != t
        {
            continue;
        }
        let spec = match &e.reference {
            Some(r) => r.clone(),
            None => {
                if target.is_some() {
                    return Err(format!(
                        "{}: pinned to commit, no ref to follow",
                        e.file
                    ));
                }
                eprintln!(
                    "marmita: skip {} (pinned to commit, no ref)",
                    e.file
                );
                continue;
            }
        };
        let (commit, bytes) = fetch_file(&e.origin, &spec, &e.file)?;
        let dest = Path::new(VENDOR_DIR).join(&e.file);
        fs::write(&dest, &bytes)
            .map_err(|err| format!("write {}: {err}", dest.display()))?;
        let changed = e.commit != commit;
        e.commit = commit.clone();
        e.date = today_utc();
        touched += 1;
        if changed {
            println!("updated {} -> {}", e.file, short(&commit));
        } else {
            println!("up to date {} ({})", e.file, short(&commit));
        }
    }
    if let Some(t) = target
        && touched == 0
    {
        return Err(format!("{t}: no such entry"));
    }
    write_manifest(&mut entries)?;
    Ok(())
}

fn cmd_rm(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err(usage_str().to_string());
    }
    let file = &args[0];
    let mut entries = read_manifest()?;
    let before = entries.len();
    entries.retain(|e| &e.file != file);
    if entries.len() == before {
        return Err(format!("{file}: no such entry"));
    }
    let path = Path::new(VENDOR_DIR).join(file);
    match fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("remove {}: {e}", path.display())),
    }
    write_manifest(&mut entries)?;
    println!("removed {file}");
    Ok(())
}

fn cmd_list(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err(usage_str().to_string());
    }
    let entries = read_manifest()?;
    for e in &entries {
        let r = e.reference.as_deref().unwrap_or("-");
        println!(
            "{}\t{}\t{}\t{}\t{}",
            e.file,
            short(&e.commit),
            r,
            e.date,
            e.origin,
        );
    }
    Ok(())
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(12)]
}

//////////////////////////////////////////////////////////////////////////////
// Entry point
//////////////////////////////////////////////////////////////////////////////

fn usage_str() -> &'static str {
    "usage: marmita add [-r ref] <url> [file]\n       marmita update [file]\n       marmita rm <file>\n       marmita list\n       marmita -V"
}

fn main() {
    process::exit(run());
}

fn run() -> i32 {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("{}", usage_str());
        return 1;
    }
    if args[1] == "-V" {
        println!("marmita {}", env!("MARMITA_VERSION"));
        return 0;
    }
    if let Err(e) = git_init() {
        eprintln!("marmita: {e}");
        return 2;
    }
    let rest: Vec<String> = args[2..].to_vec();
    let result = match args[1].as_str() {
        "add" => cmd_add(&rest),
        "update" => cmd_update(&rest),
        "rm" => cmd_rm(&rest),
        "list" => cmd_list(&rest),
        other => Err(format!("unknown command '{other}'\n{}", usage_str())),
    };
    git_shutdown();
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("marmita: {e}");
            1
        }
    }
}
