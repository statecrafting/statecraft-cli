//! Small native surface. Handles are owned; post-fork work is syscall-only.

use super::{Policy, Probe, Refused};
use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Component, Path};
use std::process::Command;

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod linux;

pub(super) struct Prepared {
    #[cfg(target_os = "macos")]
    profile: String,
    #[cfg(target_os = "linux")]
    linux: linux::Rules,
}

impl Prepared {
    pub(super) fn new(policy: &Policy) -> Result<Self, Refused> {
        #[cfg(target_os = "macos")]
        {
            if !Path::new("/usr/bin/sandbox-exec").is_file() {
                return Err(Refused::at(
                    "mechanism",
                    "/usr/bin/sandbox-exec unavailable",
                ));
            }
            let mut profile = String::from(
                "(version 1)\n(allow default)\n(deny file-write*)\n(deny signal)\n(allow signal (target same-sandbox))\n(deny network-outbound)\n(allow network-outbound (remote tcp \"*:443\"))\n(allow network-outbound (literal \"/private/var/run/mDNSResponder\"))\n(deny network-outbound (remote ip \"localhost:*\"))\n(deny appleevent-send)\n(deny job-creation)\n(deny mach-lookup (global-name \"com.apple.coreservices.launchservicesd\") (global-name \"com.apple.coreservices.appleevents\") (global-name \"com.apple.xpc.launchd\"))\n",
            );
            for root in &policy.inaccessible {
                profile.push_str(&format!("(deny file-read* (subpath {}))\n", quote(root)?));
            }
            for root in &policy.readable {
                profile.push_str(&format!("(allow file-read* (subpath {}))\n", quote(root)?));
                // Resolving the readable exchange requires metadata on its
                // ancestors, but grants neither their bytes nor their entries.
                for ancestor in root.ancestors().skip(1) {
                    profile.push_str(&format!(
                        "(allow file-read-metadata (literal {}))\n",
                        quote(ancestor)?
                    ));
                }
            }
            for grant in &policy.writable {
                let filter = if grant.directory {
                    "subpath"
                } else {
                    "literal"
                };
                // File grants permit data writes, not directory-entry mutations.
                let operation = if grant.directory {
                    "file-write*"
                } else {
                    "file-write-data"
                };
                profile.push_str(&format!(
                    "(allow {operation} ({filter} {}))\n",
                    quote(&grant.path)?
                ));
            }
            for prefix in &policy.adjacent_prefixes {
                profile.push_str(&format!(
                    "(allow file-write* (literal {}) (regex #{}))\n",
                    quote(prefix)?,
                    serde_json::to_string(&format!("^{}\\.[^/]*$", regex_literal(prefix)?))
                        .map_err(|e| Refused::at("profile", e))?
                ));
            }
            profile.push_str("(allow file-write-data file-ioctl (literal \"/dev/null\") (literal \"/dev/tty\") (literal \"/dev/zero\"))\n");
            Ok(Self { profile })
        }
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                linux: linux::Rules::new(policy)?,
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = policy;
            Err(Refused::at("mechanism", "unsupported platform"))
        }
    }

    pub(super) fn identity(&self) -> Vec<u8> {
        #[cfg(target_os = "macos")]
        {
            self.profile.as_bytes().to_vec()
        }
        #[cfg(target_os = "linux")]
        {
            self.linux.identity.clone()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Vec::new()
        }
    }

    pub(super) fn mechanism(&self) -> &'static str {
        #[cfg(target_os = "macos")]
        {
            "seatbelt"
        }
        #[cfg(target_os = "linux")]
        {
            "landlock-and-seccomp"
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            "unavailable"
        }
    }

    pub(super) fn command(&self, program: &Path, args: &[&str]) -> Command {
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut command = Command::new("/usr/bin/sandbox-exec");
            command.args(["-p", &self.profile]).arg(program).args(args);
            command
        };
        #[cfg(not(target_os = "macos"))]
        let mut command = {
            let mut command = Command::new(program);
            command.args(args);
            command
        };
        #[cfg(target_os = "linux")]
        self.linux.apply(&mut command);
        #[cfg(target_os = "macos")]
        // SAFETY: the child performs only close syscalls before exec. No Rust
        // state or locks are touched after fork.
        {
            let mut descriptors = vec![
                libc::proc_fdinfo {
                    proc_fd: 0,
                    proc_fdtype: 0
                };
                65536
            ];
            unsafe {
                command.pre_exec(move || {
                    let size = std::mem::size_of_val(descriptors.as_slice()) as i32;
                    let bytes = libc::proc_pidinfo(
                        libc::getpid(),
                        libc::PROC_PIDLISTFDS,
                        0,
                        descriptors.as_mut_ptr().cast(),
                        size,
                    );
                    if bytes <= 0 || bytes >= size {
                        return Err(io::Error::other(
                            "cannot enumerate every inherited descriptor",
                        ));
                    }
                    for descriptor in
                        &descriptors[..bytes as usize / std::mem::size_of::<libc::proc_fdinfo>()]
                    {
                        if descriptor.proc_fd > 2 {
                            libc::close(descriptor.proc_fd);
                        }
                    }
                    Ok(())
                });
            }
        }
        command
    }
}

#[cfg(target_os = "macos")]
fn quote(path: &Path) -> Result<String, Refused> {
    let value = path
        .to_str()
        .ok_or_else(|| Refused::at("profile", "non-UTF-8 root"))?;
    serde_json::to_string(value).map_err(|e| Refused::at("profile", e))
}

#[cfg(target_os = "macos")]
fn regex_literal(path: &Path) -> Result<String, Refused> {
    let value = path
        .to_str()
        .ok_or_else(|| Refused::at("profile", "non-UTF-8 prefix"))?;
    let mut escaped = String::new();
    for c in value.chars() {
        if ".+*?()[]{}^$|\\".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    Ok(escaped)
}

pub(super) fn open_directory(path: &Path) -> io::Result<File> {
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::Error::from_raw_os_error(libc::ELOOP));
    }
    // Normalize platform aliases before anchoring. Every component in the
    // resulting physical path is opened without following another link.
    let physical = std::fs::canonicalize(path)?;
    let mut handle = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/")?;
    for component in physical.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
        let fd = unsafe {
            libc::openat(
                handle.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        handle = unsafe { File::from_raw_fd(fd) };
    }
    Ok(handle)
}

pub(super) fn directory_relative(root: &File, path: &Path) -> io::Result<File> {
    let mut directory = root.try_clone()?;
    for component in path.components() {
        let Component::Normal(name) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "strictly relative directory required",
            ));
        };
        let name = CString::new(name.as_bytes()).map_err(io::Error::other)?;
        // SAFETY: the held directory and terminated name are live. Each new
        // descriptor is immediately owned; no component can follow a link.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        directory = unsafe { File::from_raw_fd(fd) };
    }
    Ok(directory)
}

pub(super) fn unlink_relative(root: &File, path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("relative parent absent"))?;
    let directory = directory_relative(root, parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("relative leaf absent"))?;
    let name = CString::new(name.as_bytes()).map_err(io::Error::other)?;
    // SAFETY: this removes only the named leaf in the held parent directory.
    // unlinkat does not dereference a leaf link or delete a directory.
    if unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn read_relative(root: &File, path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let (bytes, truncated) = read_prefix(root, path, limit)?;
    if truncated {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file exceeds bound",
        ));
    }
    Ok(bytes)
}

pub(super) fn read_prefix(root: &File, path: &Path, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut dir = root.try_clone()?;
    let components: Vec<_> = path.components().collect();
    if components.is_empty()
        || components
            .iter()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "data path must be strictly relative",
        ));
    }
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            unreachable!()
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"))?;
        let last = index + 1 == components.len();
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if last { 0 } else { libc::O_DIRECTORY };
        // SAFETY: dir is owned and live; name is terminated; openat returns a
        // new descriptor, immediately placed in an owning File.
        let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fd is a newly opened owned descriptor.
        let file = unsafe { File::from_raw_fd(fd) };
        if last {
            let metadata = file.metadata()?;
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "not a bounded ordinary file",
                ));
            }
            let mut bytes = Vec::new();
            file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
            let truncated = bytes.len() > limit;
            bytes.truncate(limit);
            return Ok((bytes, truncated));
        }
        dir = file;
    }
    unreachable!()
}

pub(super) fn probe(probe: &Probe) -> BTreeMap<String, bool> {
    let mut result = BTreeMap::new();
    let denied =
        |error: &io::Error| matches!(error.raw_os_error(), Some(libc::EACCES | libc::EPERM));
    for (label, path) in [
        ("home", &probe.home),
        ("records", &probe.records),
        ("target", &probe.target),
    ] {
        let read = std::fs::read(path);
        result.insert(
            if label == "target" {
                format!("{label}Read")
            } else {
                format!("{label}ReadDenied")
            },
            if label == "target" {
                read.is_ok()
            } else {
                read.as_ref().is_err_and(denied)
            },
        );
        let write = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path);
        result.insert(
            format!("{label}WriteDenied"),
            write.as_ref().is_err_and(denied),
        );
    }
    result.insert(
        "workspaceWrite".into(),
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&probe.workspace)
            .and_then(|mut f| f.write_all(b"boundary probe\n"))
            .is_ok(),
    );
    result.insert(
        "workspaceMetadata".into(),
        std::fs::set_permissions(&probe.workspace, std::fs::Permissions::from_mode(0o600)).is_ok(),
    );
    result.insert(
        "targetMetadataDenied".into(),
        std::fs::set_permissions(&probe.target, std::fs::Permissions::from_mode(0o700))
            .as_ref()
            .is_err_and(denied),
    );
    result.insert(
        "workspaceCleanup".into(),
        std::fs::remove_file(&probe.workspace).is_ok(),
    );
    result.insert(
        "gateWrite".into(),
        OpenOptions::new()
            .append(true)
            .open(&probe.gate)
            .and_then(|mut f| f.write_all(b"boundary probe\n"))
            .is_ok(),
    );
    let unix = std::os::unix::net::UnixStream::connect(&probe.unix_socket);
    result.insert("unixDenied".into(), unix.as_ref().is_err_and(denied));
    let loopback = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], probe.loopback_port)),
        std::time::Duration::from_secs(1),
    );
    result.insert(
        "loopbackDenied".into(),
        loopback.as_ref().is_err_and(denied),
    );
    // SAFETY: signal 0 checks access without delivering a signal.
    result.insert(
        "supervisorSignalDenied".into(),
        unsafe { libc::kill(probe.supervisor as i32, 0) } == -1
            && denied(&io::Error::last_os_error()),
    );
    #[cfg(target_os = "linux")]
    linux::probe_extra(probe, &mut result);
    #[cfg(target_os = "macos")]
    {
        let mut descriptors = vec![
            libc::proc_fdinfo {
                proc_fd: 0,
                proc_fdtype: 0
            };
            65536
        ];
        // SAFETY: the allocated buffer is correctly sized for proc_fdinfo.
        let bytes = unsafe {
            libc::proc_pidinfo(
                libc::getpid(),
                libc::PROC_PIDLISTFDS,
                0,
                descriptors.as_mut_ptr().cast(),
                std::mem::size_of_val(descriptors.as_slice()) as i32,
            )
        };
        result.insert(
            "noInheritedDescriptors".into(),
            bytes > 0
                && (bytes as usize) < std::mem::size_of_val(descriptors.as_slice())
                && descriptors[..bytes as usize / std::mem::size_of::<libc::proc_fdinfo>()]
                    .iter()
                    .all(|fd| fd.proc_fd <= 2),
        );
    }
    #[cfg(target_os = "linux")]
    result.insert(
        "noInheritedDescriptors".into(),
        std::fs::read_dir("/proc/self/fd").is_ok_and(|entries| {
            entries.into_iter().all(|entry| {
                entry
                    .ok()
                    .and_then(|e| e.file_name().to_str().and_then(|s| s.parse::<i32>().ok()))
                    .is_some_and(|fd| fd <= 3)
            })
        }),
    );
    result
}

pub(super) fn snapshot(root: &File) -> io::Result<BTreeMap<std::path::PathBuf, (u32, Vec<u8>)>> {
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::MetadataExt;
    fn walk(
        root: &File,
        prefix: &Path,
        out: &mut BTreeMap<std::path::PathBuf, (u32, Vec<u8>)>,
        total: &mut usize,
        entries: &mut usize,
    ) -> io::Result<()> {
        // SAFETY: openat creates a fresh directory description for fdopendir.
        let fd = unsafe {
            libc::openat(
                root.as_raw_fd(),
                c".".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fd is owned; fdopendir assumes ownership on success.
        let directory = unsafe { libc::fdopendir(fd) };
        if directory.is_null() {
            unsafe {
                libc::close(fd);
            }
            return Err(io::Error::last_os_error());
        }
        struct Directory(*mut libc::DIR);
        impl Drop for Directory {
            fn drop(&mut self) {
                unsafe {
                    libc::closedir(self.0);
                }
            }
        }
        let directory = Directory(directory);
        loop {
            // SAFETY: directory remains live. Copy the name before readdir again.
            #[cfg(target_os = "macos")]
            unsafe {
                *libc::__error() = 0;
            }
            #[cfg(target_os = "linux")]
            unsafe {
                *libc::__errno_location() = 0;
            }
            let entry = unsafe { libc::readdir(directory.0) };
            if entry.is_null() {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(0) {
                    return Err(error);
                }
                break;
            }
            let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_bytes()
                .to_vec();
            if name == b"."
                || name == b".."
                || (prefix.as_os_str().is_empty() && name.eq_ignore_ascii_case(b".git"))
            {
                continue;
            }
            if prefix.components().count() >= 128 {
                return Err(io::Error::other("workspace exceeds depth bound"));
            }
            let path = prefix.join(std::ffi::OsString::from_vec(name.clone()));
            let c_name = CString::new(name).map_err(io::Error::other)?;
            let mut stat: libc::stat = unsafe { std::mem::zeroed() };
            // SAFETY: root is live and stat is correctly sized. Links are never followed.
            if unsafe {
                libc::fstatat(
                    root.as_raw_fd(),
                    c_name.as_ptr(),
                    &mut stat,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } < 0
            {
                return Err(io::Error::last_os_error());
            }
            *entries += 1;
            if *entries > 100_000 {
                return Err(io::Error::other("workspace has too many entries"));
            }
            let kind = stat.st_mode & libc::S_IFMT;
            if kind == libc::S_IFDIR {
                let fd = unsafe {
                    libc::openat(
                        root.as_raw_fd(),
                        c_name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                let child = unsafe { File::from_raw_fd(fd) };
                walk(&child, &path, out, total, entries)?;
            } else if kind == libc::S_IFLNK {
                let mut text = vec![0; 4097];
                let length = unsafe {
                    libc::readlinkat(
                        root.as_raw_fd(),
                        c_name.as_ptr(),
                        text.as_mut_ptr().cast(),
                        text.len(),
                    )
                };
                if length < 0 {
                    return Err(io::Error::last_os_error());
                }
                if length as usize >= text.len() {
                    return Err(io::Error::other("link text exceeds bound"));
                }
                text.truncate(length as usize);
                out.insert(path, (0o120000, text));
            } else if kind == libc::S_IFREG {
                let fd = unsafe {
                    libc::openat(
                        root.as_raw_fd(),
                        c_name.as_ptr(),
                        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                let file = unsafe { File::from_raw_fd(fd) };
                let metadata = file.metadata()?;
                if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
                    return Err(io::Error::other("not a bounded ordinary file"));
                }
                let mode = if metadata.mode() & 0o111 != 0 {
                    0o100755
                } else {
                    0o100644
                };
                let mut bytes = Vec::new();
                file.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                *total += bytes.len();
                if bytes.len() > 64 * 1024 * 1024 || *total > 1024 * 1024 * 1024 {
                    return Err(io::Error::other("workspace exceeds byte bound"));
                }
                out.insert(path, (mode, bytes));
            } else {
                return Err(io::Error::other("workspace contains a nonregular entry"));
            }
        }
        Ok(())
    }
    let mut entries = BTreeMap::new();
    walk(root, Path::new(""), &mut entries, &mut 0, &mut 0)?;
    Ok(entries)
}
