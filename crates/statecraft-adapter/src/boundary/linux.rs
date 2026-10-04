//! Landlock and seccomp. Unknown ABIs refuse rather than omit unknown rights.

use super::*;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Arc;

#[path = "metadata.rs"]
mod metadata;

const CREATE: libc::c_long = 444;
const ADD: libc::c_long = 445;
const RESTRICT: libc::c_long = 446;
const READ: u64 = 1 | 4 | 8;
const FILE: u64 = 1 | 2 | 4 | (1 << 14) | (1 << 15);

#[repr(C)]
struct Attributes {
    fs: u64,
    net: u64,
    scoped: u64,
}
#[repr(C, packed)]
struct Beneath {
    access: u64,
    parent: i32,
}
#[repr(C)]
struct Port {
    access: u64,
    port: u64,
}

pub(super) struct Rules {
    fd: Arc<File>,
    filter: Arc<Vec<libc::sock_filter>>,
    pub(super) identity: Vec<u8>,
    metadata_roots: Vec<PathBuf>,
}

impl Rules {
    pub(super) fn new(policy: &Policy) -> Result<Self, Refused> {
        // SAFETY: version query has no pointer arguments.
        let abi = unsafe { libc::syscall(CREATE, std::ptr::null::<u8>(), 0, 1) };
        if !(6..=9).contains(&abi) {
            return Err(Refused::at(
                "landlock-ABI",
                format!("ABI {abi}; supported measured layouts are 6 through 9"),
            ));
        }
        listeners()?;
        mount_aliases(policy)?;
        metadata::validate()?;
        // Protect every supervisor descriptor from same-user /proc access.
        // It intentionally remains disabled through process exit; restoring it
        // while another boundary still owns a handle would create a race.
        if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) } != 0 {
            return Err(Refused::at(
                "supervisor-not-dumpable",
                io::Error::last_os_error(),
            ));
        }
        let full = if abi >= 9 { 0x1ffff } else { 0xffff };
        let attrs = Attributes {
            fs: full,
            net: 3,
            scoped: 3,
        };
        // SAFETY: attributes have the kernel UAPI layout and lifetime of call.
        let fd = unsafe { libc::syscall(CREATE, &attrs, std::mem::size_of::<Attributes>(), 0) };
        if fd < 0 {
            return Err(Refused::at("landlock-create", io::Error::last_os_error()));
        }
        // SAFETY: creation returned a new owned descriptor.
        let file = unsafe { File::from_raw_fd(fd as i32) };
        let mut entries = Vec::<(std::path::PathBuf, u64)>::new();
        read_siblings(Path::new("/"), &policy.inaccessible, &mut entries)?;
        for path in &policy.readable {
            entries.push((path.clone(), READ));
        }
        for grant in &policy.writable {
            entries.push((
                grant.path.clone(),
                if grant.directory { full } else { FILE },
            ));
        }
        if !policy.adjacent_prefixes.is_empty() {
            return Err(Refused::at(
                "write-root",
                "Linux cannot grant adjacent temporary configuration files",
            ));
        }
        for device in ["/dev/null", "/dev/tty", "/dev/zero"] {
            if Path::new(device).exists() {
                entries.push((device.into(), FILE));
            }
        }
        for (path, access) in &entries {
            let name = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| Refused::at("landlock-root", "NUL"))?;
            // SAFETY: name is terminated; O_PATH does not read device contents.
            let root = unsafe {
                libc::open(
                    name.as_ptr(),
                    libc::O_PATH | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                )
            };
            if root < 0 {
                return Err(Refused::at(
                    "landlock-open-root",
                    format!("{}: {}", path.display(), io::Error::last_os_error()),
                ));
            }
            let handle = unsafe { File::from_raw_fd(root) };
            let access = if handle
                .metadata()
                .map_err(|e| Refused::at("landlock-root", e))?
                .is_dir()
            {
                *access
            } else {
                access & FILE
            };
            let rule = Beneath {
                access,
                parent: handle.as_raw_fd(),
            };
            if unsafe { libc::syscall(ADD, file.as_raw_fd(), 1, &rule, 0) } != 0 {
                return Err(Refused::at(
                    "landlock-add-root",
                    format!("{}: {}", path.display(), io::Error::last_os_error()),
                ));
            }
        }
        let port = Port {
            access: 2,
            port: 443,
        };
        if unsafe { libc::syscall(ADD, file.as_raw_fd(), 2, &port, 0) } != 0 {
            return Err(Refused::at("landlock-port", io::Error::last_os_error()));
        }
        let filter = filter()?;
        let mut identity =
            serde_json::to_vec(&("pinned-chmod-emulation-v1", abi, full, entries, policy))
                .map_err(|e| Refused::at("ruleset-identity", e))?;
        for instruction in &filter {
            identity.extend_from_slice(&instruction.code.to_le_bytes());
            identity.extend_from_slice(&[instruction.jt, instruction.jf]);
            identity.extend_from_slice(&instruction.k.to_le_bytes());
        }
        Ok(Self {
            fd: Arc::new(file),
            filter: Arc::new(filter),
            identity,
            metadata_roots: policy
                .writable
                .iter()
                .filter(|g| g.directory)
                .map(|g| g.path.clone())
                .collect(),
        })
    }

    pub(super) fn apply(&self, command: &mut Command) {
        let fd = Arc::clone(&self.fd);
        let filter = Arc::clone(&self.filter);
        let channel = match metadata::prepare(self.metadata_roots.clone()) {
            Ok(channel) => channel,
            Err(error) => {
                let errno = error.raw_os_error().unwrap_or(libc::EIO);
                unsafe {
                    command.pre_exec(move || Err(io::Error::from_raw_os_error(errno)));
                }
                return;
            }
        };
        // SAFETY: everything is prepared before fork. The closure uses only
        // prctl/syscalls and no allocation, locks, environment or filesystem
        // traversal. Both captured allocations remain live through exec.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                    || libc::syscall(RESTRICT, fd.as_raw_fd(), 0) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                let program = libc::sock_fprog {
                    len: filter.len() as u16,
                    filter: filter.as_ptr() as *mut _,
                };
                let listener = libc::syscall(libc::SYS_seccomp, 1, 8, &program);
                if listener < 0 {
                    return Err(io::Error::last_os_error());
                }
                metadata::send(channel.as_raw_fd(), listener as i32)?;
                // Close every nonstandard descriptor explicitly, including the
                // ruleset and the inherited protected handles, before exec.
                if libc::syscall(libc::SYS_close_range, 3_u32, u32::MAX, 0) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}

fn read_siblings(
    path: &Path,
    forbidden: &[std::path::PathBuf],
    out: &mut Vec<(std::path::PathBuf, u64)>,
) -> Result<(), Refused> {
    if forbidden.iter().any(|p| path.starts_with(p)) {
        return Ok(());
    }
    if !forbidden.iter().any(|p| p.starts_with(path)) {
        out.push((path.to_path_buf(), READ));
        return Ok(());
    }
    for entry in std::fs::read_dir(path).map_err(|e| Refused::at("landlock-read-siblings", e))? {
        let entry = entry.map_err(|e| Refused::at("landlock-read-siblings", e))?;
        // Symlink aliases gain no path rule. Their destinations are judged by
        // their own canonical rules, including any inaccessible ancestor.
        if !entry
            .file_type()
            .map_err(|e| Refused::at("landlock-read-siblings", e))?
            .is_symlink()
        {
            read_siblings(&entry.path(), forbidden, out)?;
        }
    }
    Ok(())
}

fn listeners() -> Result<(), Refused> {
    for table in ["tcp", "tcp6", "udp", "udp6"] {
        let data = std::fs::read_to_string(format!("/proc/net/{table}"))
            .map_err(|e| Refused::at("listener-check", e))?;
        for line in data.lines().skip(1) {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 8 {
                return Err(Refused::at(
                    "listener-check",
                    "unknown proc socket table format",
                ));
            }
            let port = fields[1]
                .rsplit(':')
                .next()
                .and_then(|p| u16::from_str_radix(p, 16).ok())
                .ok_or_else(|| Refused::at("listener-check", "invalid socket port"))?;
            let uid: u32 = fields[7]
                .parse()
                .map_err(|_| Refused::at("listener-check", "invalid socket owner"))?;
            if (table.starts_with("tcp") && port == 443 && fields[3] == "0A")
                || (table.starts_with("udp") && uid == unsafe { libc::geteuid() })
            {
                return Err(Refused::at(
                    "listener-check",
                    format!("host {table} listener on port {port}, UID {uid}"),
                ));
            }
        }
    }
    Ok(())
}

fn instruction(code: u16, k: u32, jt: u8, jf: u8) -> libc::sock_filter {
    libc::sock_filter { code, k, jt, jf }
}
fn filter() -> Result<Vec<libc::sock_filter>, Refused> {
    #[cfg(target_arch = "x86_64")]
    let arch = 0xc000003e;
    #[cfg(target_arch = "aarch64")]
    let arch = 0xc00000b7;
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    return Err(Refused::at(
        "seccomp-architecture",
        "unsupported syscall layout",
    ));
    let deny = 0x00050000 | libc::EPERM as u32;
    let allow = 0x7fff0000;
    let mut out = vec![
        instruction(0x20, 4, 0, 0),
        instruction(0x15, arch, 1, 0),
        instruction(0x06, 0x80000000, 0, 0),
        instruction(0x20, 0, 0, 0),
    ];
    // Reject x32 and all other alternate syscall-number encodings.
    out.extend([
        instruction(0x35, 0x40000000, 0, 1),
        instruction(0x06, deny, 0, 0),
    ]);
    let mut denied = vec![
        libc::SYS_io_uring_setup,
        libc::SYS_open_by_handle_at,
        libc::SYS_setns,
        libc::SYS_fchown,
        libc::SYS_fchownat,
        libc::SYS_setxattr,
        libc::SYS_lsetxattr,
        libc::SYS_fsetxattr,
        libc::SYS_removexattr,
        libc::SYS_lremovexattr,
        libc::SYS_fremovexattr,
        libc::SYS_utimensat,
        libc::SYS_mount,
    ];
    #[cfg(target_arch = "x86_64")]
    denied.extend([
        libc::SYS_chown,
        libc::SYS_lchown,
        libc::SYS_utime,
        libc::SYS_utimes,
        libc::SYS_futimesat,
    ]);
    // New fchmodat2 and mount API syscalls must not bypass metadata denials.
    denied.extend([428, 429, 430, 431, 432, 433, 442]);
    #[allow(unused_mut)]
    let mut mediated = vec![libc::SYS_fchmod, libc::SYS_fchmodat, 452];
    #[cfg(target_arch = "x86_64")]
    mediated.push(libc::SYS_chmod);
    for number in mediated {
        out.extend([
            instruction(0x15, number as u32, 0, 1),
            instruction(0x06, 0x7fc00000, 0, 0),
        ]);
    }
    for number in denied {
        out.extend([
            instruction(0x15, number as u32, 0, 1),
            instruction(0x06, deny, 0, 0),
        ]);
    }
    out.extend([
        instruction(0x15, libc::SYS_clone3 as u32, 0, 1),
        instruction(0x06, 0x00050000 | libc::ENOSYS as u32, 0, 0),
    ]);
    for (number, families) in [
        (libc::SYS_socket, vec![libc::AF_INET, libc::AF_INET6]),
        (libc::SYS_socketpair, vec![libc::AF_UNIX]),
    ] {
        let length = families.len() + 3;
        out.push(instruction(0x15, number as u32, 0, length as u8));
        out.push(instruction(0x20, 16, 0, 0));
        for (i, family) in families.iter().enumerate() {
            out.push(instruction(
                0x15,
                *family as u32,
                (families.len() - i) as u8,
                0,
            ));
        }
        out.push(instruction(0x06, deny, 0, 0));
        out.push(instruction(0x06, allow, 0, 0));
    }
    for number in [libc::SYS_clone, libc::SYS_unshare] {
        out.extend([
            instruction(0x15, number as u32, 0, 4),
            instruction(0x20, 16, 0, 0),
            instruction(0x45, libc::CLONE_NEWUSER as u32, 0, 1),
            instruction(0x06, deny, 0, 0),
            instruction(0x06, allow, 0, 0),
        ]);
    }
    out.push(instruction(0x06, allow, 0, 0));
    Ok(out)
}

struct Mount {
    device: String,
    root: PathBuf,
    point: PathBuf,
}

fn mount_path(field: &str) -> Result<PathBuf, Refused> {
    let bytes = field.as_bytes();
    let mut decoded = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            let escape = bytes
                .get(index + 1..index + 4)
                .ok_or_else(|| Refused::at("mount-alias", "truncated mountinfo escape"))?;
            if !escape.iter().all(|b| (b'0'..=b'7').contains(b)) {
                return Err(Refused::at("mount-alias", "invalid mountinfo escape"));
            }
            let value = (escape[0] - b'0') as u16 * 64
                + (escape[1] - b'0') as u16 * 8
                + (escape[2] - b'0') as u16;
            if value == 0 || value > 255 {
                return Err(Refused::at("mount-alias", "invalid mountinfo byte"));
            }
            decoded.push(value as u8);
            index += 4;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(decoded));
    if !path.is_absolute() {
        return Err(Refused::at("mount-alias", "relative mountinfo path"));
    }
    Ok(path)
}

fn mounts(text: &str) -> Result<Vec<Mount>, Refused> {
    text.lines()
        .map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 10 || !fields[6..].contains(&"-") {
                return Err(Refused::at("mount-alias", "malformed mountinfo"));
            }
            Ok(Mount {
                device: fields[2].into(),
                root: mount_path(fields[3])?,
                point: mount_path(fields[4])?,
            })
        })
        .collect()
}

fn aliases(root: &Path, mounts: &[Mount]) -> Result<Vec<(PathBuf, PathBuf)>, Refused> {
    let source = mounts
        .iter()
        .filter(|mount| root.starts_with(&mount.point))
        .max_by_key(|mount| mount.point.components().count())
        .ok_or_else(|| Refused::at("mount-alias", "protected root has no mount"))?;
    let physical = source.root.join(root.strip_prefix(&source.point).unwrap());
    Ok(mounts
        .iter()
        .filter(|mount| mount.device == source.device)
        .filter_map(|mount| {
            if let Ok(relative) = physical.strip_prefix(&mount.root) {
                Some((root.to_path_buf(), mount.point.join(relative)))
            } else if let Ok(relative) = mount.root.strip_prefix(&physical) {
                // A bind of only a protected descendant exposes that subtree too.
                Some((root.join(relative), mount.point.clone()))
            } else {
                None
            }
        })
        .filter(|(original, alias)| original != alias)
        .collect())
}

fn mount_aliases(policy: &Policy) -> Result<(), Refused> {
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;
    let mut text = String::new();
    File::open("/proc/self/mountinfo")
        .map_err(|e| Refused::at("mount-alias", e))?
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| Refused::at("mount-alias", e))?;
    if text.len() > 4 * 1024 * 1024 {
        return Err(Refused::at("mount-alias", "mount table exceeds bound"));
    }
    let mounts = mounts(&text)?;
    for root in policy.inaccessible.iter().chain(&policy.readonly) {
        for (original, alias) in aliases(root, &mounts)? {
            let original_metadata =
                fs::metadata(&original).map_err(|e| Refused::at("mount-alias", e))?;
            match fs::metadata(&alias) {
                Ok(metadata)
                    if metadata.dev() == original_metadata.dev()
                        && metadata.ino() == original_metadata.ino() =>
                {
                    return Err(Refused::at(
                        "mount-alias",
                        format!("{} also appears at {}", original.display(), alias.display()),
                    ));
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(Refused::at("mount-alias", e)),
            }
        }
    }
    Ok(())
}

pub(super) fn probe_extra(probe: &Probe, out: &mut BTreeMap<String, bool>) {
    // SAFETY: probes use invalid or empty arguments and close any unexpected
    // successful descriptor immediately. They never mutate protected bytes.
    unsafe {
        let fd = libc::socket(libc::AF_NETLINK, libc::SOCK_RAW, 0);
        out.insert(
            "otherSocketFamilyDenied".into(),
            fd == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::EPERM),
        );
        if fd >= 0 {
            libc::close(fd);
        }
        let fd = libc::syscall(libc::SYS_io_uring_setup, 0, std::ptr::null::<u8>());
        out.insert(
            "ioUringDenied".into(),
            fd == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::EPERM),
        );
        if fd >= 0 {
            libc::close(fd as i32);
        }
        let fd = libc::syscall(libc::SYS_open_by_handle_at, -1, std::ptr::null::<u8>(), 0);
        out.insert(
            "openByHandleDenied".into(),
            fd == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::EPERM),
        );
        if fd >= 0 {
            libc::close(fd as i32);
        }
    }
    let access = std::fs::read(format!(
        "/proc/{}/fd/{}",
        probe.supervisor, probe.protected_fd
    ));
    out.insert(
        "supervisorFdDenied".into(),
        access.is_err_and(|e| matches!(e.raw_os_error(), Some(libc::EPERM | libc::EACCES))),
    );
}

#[cfg(test)]
mod mount_tests {
    use super::*;

    #[test]
    #[ignore = "requires CAP_SYS_ADMIN in an isolated mount namespace"]
    fn an_actual_bind_mount_of_a_protected_subtree_refuses_admission() {
        let fixture = tempfile::tempdir().unwrap();
        let protected = fixture.path().join("protected");
        let subtree = protected.join("nested");
        let exposed = fixture.path().join("exposed");
        fs::create_dir_all(&subtree).unwrap();
        fs::create_dir(&exposed).unwrap();
        fs::write(subtree.join("record"), b"authority").unwrap();
        assert!(
            std::process::Command::new("/usr/bin/mount")
                .arg("--bind")
                .arg(&subtree)
                .arg(&exposed)
                .status()
                .unwrap()
                .success()
        );
        struct Mounted(PathBuf);
        impl Drop for Mounted {
            fn drop(&mut self) {
                let _ = std::process::Command::new("/usr/bin/umount")
                    .arg(&self.0)
                    .status();
            }
        }
        let _mount = Mounted(exposed.clone());
        assert_eq!(fs::read(exposed.join("record")).unwrap(), b"authority");
        let refusal = crate::boundary::Prepared::prepare(Policy {
            inaccessible: vec![protected],
            readonly: vec![],
            readable: vec![],
            writable: vec![],
            adjacent_prefixes: vec![],
        })
        .err()
        .unwrap();
        assert_eq!(refusal.step, "mount-alias");
    }

    #[test]
    fn bind_mounts_of_ancestors_and_protected_roots_are_aliases() {
        let table = mounts("1 0 8:1 / / rw - ext4 /dev/x rw\n2 1 8:1 /home /readable rw - ext4 /dev/x rw\n3 1 8:1 /home/a/records /exposed rw - ext4 /dev/x rw\n4 1 8:2 /home /unrelated rw - ext4 /dev/y rw\n").unwrap();
        assert_eq!(
            aliases(Path::new("/home/a/records"), &table).unwrap(),
            vec![
                (
                    PathBuf::from("/home/a/records"),
                    PathBuf::from("/readable/a/records")
                ),
                (PathBuf::from("/home/a/records"), PathBuf::from("/exposed"))
            ]
        );
        let descendants = mounts("1 0 8:1 / / rw - ext4 /dev/x rw\n2 1 8:1 /home/a/records/nested /exposed rw - ext4 /dev/x rw\n").unwrap();
        assert_eq!(
            aliases(Path::new("/home/a/records"), &descendants).unwrap(),
            vec![(
                PathBuf::from("/home/a/records/nested"),
                PathBuf::from("/exposed")
            )]
        );
        assert_eq!(
            mount_path("/space\\040name").unwrap(),
            PathBuf::from("/space name")
        );
        assert!(mount_path("/bad\\777").is_err());
    }
}
