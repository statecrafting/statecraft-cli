//! Emulate chmod on a pinned writable object. Never continue a child syscall.
//!
//! Landlock does not mediate chmod. Seccomp notifications copy arguments and
//! resolve an owned O_PATH handle before checking its kernel-reported location.
//! Only directory grants allow metadata changes; exact data-only grants do not.

use super::*;
use std::os::unix::net::UnixDatagram;

#[repr(C)]
#[derive(Default)]
struct Data {
    number: i32,
    arch: u32,
    ip: u64,
    args: [u64; 6],
}
#[repr(C)]
#[derive(Default)]
struct Notification {
    id: u64,
    pid: u32,
    flags: u32,
    data: Data,
}
#[repr(C)]
struct Response {
    id: u64,
    value: i64,
    error: i32,
    flags: u32,
}

pub(super) fn validate() -> Result<(), Refused> {
    let mut sizes = [0_u16; 3];
    // SAFETY: the kernel writes the three UAPI size fields.
    if unsafe { libc::syscall(libc::SYS_seccomp, 3, 0, sizes.as_mut_ptr()) } != 0
        || sizes != [80, 24, 64]
    {
        return Err(Refused::at(
            "seccomp-notification-layout",
            format!("{sizes:?}"),
        ));
    }
    Ok(())
}

pub(super) fn prepare(roots: Vec<PathBuf>) -> io::Result<UnixDatagram> {
    let (parent, child) = UnixDatagram::pair()?;
    parent.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
    std::thread::Builder::new()
        .name("confined-metadata".into())
        .spawn(move || {
            if let Ok(listener) = receive(&parent) {
                serve(listener, &roots);
            }
        })?;
    Ok(child)
}

/// Post-fork, syscall-only transfer of the new notification descriptor.
pub(super) unsafe fn send(socket: i32, listener: i32) -> io::Result<()> {
    let mut byte = 0_u8;
    let mut vector = libc::iovec {
        iov_base: (&mut byte as *mut u8).cast(),
        iov_len: 1,
    };
    let mut control = [0_usize; 3];
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut vector;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = std::mem::size_of_val(&control);
    let header = unsafe { libc::CMSG_FIRSTHDR(&message) };
    unsafe {
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(4) as usize;
        std::ptr::write_unaligned(libc::CMSG_DATA(header).cast::<i32>(), listener);
    }
    if unsafe { libc::sendmsg(socket, &message, libc::MSG_NOSIGNAL) } != 1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn receive(socket: &UnixDatagram) -> io::Result<File> {
    let mut byte = 0_u8;
    let mut vector = libc::iovec {
        iov_base: (&mut byte as *mut u8).cast(),
        iov_len: 1,
    };
    let mut control = [0_usize; 3];
    // SAFETY: owned buffers have the exact bounded ancillary-data layout.
    unsafe {
        let mut message: libc::msghdr = std::mem::zeroed();
        message.msg_iov = &mut vector;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = std::mem::size_of_val(&control);
        if libc::recvmsg(socket.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC) != 1 {
            return Err(io::Error::last_os_error());
        }
        let header = libc::CMSG_FIRSTHDR(&message);
        if header.is_null()
            || message.msg_flags & libc::MSG_CTRUNC != 0
            || (*header).cmsg_level != libc::SOL_SOCKET
            || (*header).cmsg_type != libc::SCM_RIGHTS
            || (*header).cmsg_len != libc::CMSG_LEN(4) as usize
        {
            return Err(io::Error::other("invalid notification descriptor"));
        }
        Ok(File::from_raw_fd(std::ptr::read_unaligned(
            libc::CMSG_DATA(header).cast::<i32>(),
        )))
    }
}

fn valid(listener: &File, id: u64) -> bool {
    // SAFETY: ID_VALID reads this request identity, with no child memory access.
    unsafe { libc::ioctl(listener.as_raw_fd(), 0x40082102_u64, &id) == 0 }
}

fn serve(listener: File, roots: &[PathBuf]) {
    loop {
        let mut poll = libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one initialized poll descriptor, no pointer retention.
        if unsafe { libc::poll(&mut poll, 1, -1) } < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return;
        }
        if poll.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
            return;
        }
        let mut request = Notification::default();
        // SAFETY: validated UAPI layout, zeroed before each receive.
        if unsafe { libc::ioctl(listener.as_raw_fd(), 0xc0502100_u64, &mut request) } != 0 {
            continue;
        }
        let result = emulate(&listener, &request, roots);
        let response = Response {
            id: request.id,
            value: 0,
            error: result
                .err()
                .map_or(0, |e| -e.raw_os_error().unwrap_or(libc::EPERM)),
            // Zero flags are essential: no SECCOMP_USER_NOTIF_FLAG_CONTINUE.
            flags: 0,
        };
        // SAFETY: response contains no pointers and emulates the result only.
        unsafe {
            libc::ioctl(listener.as_raw_fd(), 0xc0182101_u64, &response);
        }
    }
}

fn denied() -> io::Error {
    io::Error::from_raw_os_error(libc::EPERM)
}

fn emulate(listener: &File, request: &Notification, roots: &[PathBuf]) -> io::Result<()> {
    if request.flags != 0 || !valid(listener, request.id) {
        return Err(denied());
    }
    let number = request.data.number as libc::c_long;
    let args = request.data.args;
    let (object, mode) = if number == libc::SYS_fchmod {
        (
            proc_handle(request.pid, &format!("fd/{}", args[0] as i32))?,
            args[1],
        )
    } else {
        let (directory, address, mode, flags) = if number == libc::SYS_fchmodat || number == 452 {
            (
                args[0] as i32,
                args[1],
                args[2],
                if number == 452 { args[3] } else { 0 },
            )
        } else {
            #[cfg(target_arch = "x86_64")]
            if number != libc::SYS_chmod {
                return Err(denied());
            }
            #[cfg(not(target_arch = "x86_64"))]
            return Err(denied());
            #[cfg(target_arch = "x86_64")]
            {
                (libc::AT_FDCWD, args[0], args[1], 0)
            }
        };
        if flags & !(libc::AT_EMPTY_PATH as u64) != 0 {
            return Err(denied());
        }
        let path = copy_path(request.pid, address)?;
        if path.as_bytes().is_empty() && flags == libc::AT_EMPTY_PATH as u64 {
            (proc_handle(request.pid, &format!("fd/{directory}"))?, mode)
        } else {
            let base = if path.as_bytes().starts_with(b"/") {
                proc_handle(request.pid, "root")?
            } else if directory == libc::AT_FDCWD {
                proc_handle(request.pid, "cwd")?
            } else {
                proc_handle(request.pid, &format!("fd/{directory}"))?
            };
            let relative = CString::new(
                path.as_bytes()
                    .strip_prefix(b"/")
                    .unwrap_or(path.as_bytes()),
            )
            .map_err(|_| denied())?;
            #[repr(C)]
            struct How {
                flags: u64,
                mode: u64,
                resolve: u64,
            }
            let how = How {
                flags: (libc::O_PATH | libc::O_CLOEXEC | libc::O_NOFOLLOW) as u64,
                mode: 0,
                resolve: 2 | 4,
            };
            // SAFETY: copied argument, anchored resolution, no child pointer.
            let fd = unsafe {
                libc::syscall(
                    libc::SYS_openat2,
                    base.as_raw_fd(),
                    relative.as_ptr(),
                    &how,
                    std::mem::size_of::<How>(),
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            (unsafe { File::from_raw_fd(fd as i32) }, mode)
        }
    };
    if mode & !0o777 != 0 {
        return Err(denied());
    }
    let metadata = object.metadata()?;
    if !(metadata.is_file() || metadata.is_dir()) {
        return Err(denied());
    }
    let location = std::fs::read_link(format!("/proc/self/fd/{}", object.as_raw_fd()))?;
    if !location.is_absolute()
        || location.as_os_str().as_bytes().ends_with(b" (deleted)")
        || !roots.iter().any(|root| location.starts_with(root))
        || !valid(listener, request.id)
    {
        return Err(denied());
    }
    // The owned descriptor pins the checked inode. Child changes to arguments,
    // symlinks and descriptors cannot redirect this operation to another inode.
    let empty = c"";
    if unsafe {
        libc::syscall(
            452,
            object.as_raw_fd(),
            empty.as_ptr(),
            mode,
            libc::AT_EMPTY_PATH,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn proc_handle(pid: u32, suffix: &str) -> io::Result<File> {
    let path = CString::new(format!("/proc/{pid}/{suffix}")).map_err(|_| denied())?;
    // Kernel-owned proc links deliberately yield a pinned descriptor.
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn copy_path(pid: u32, address: u64) -> io::Result<CString> {
    let mut bytes = [0_u8; 4096];
    let local = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let remote = libc::iovec {
        iov_base: address as usize as *mut libc::c_void,
        iov_len: bytes.len(),
    };
    let count = unsafe { libc::process_vm_readv(pid as i32, &local, 1, &remote, 1, 0) };
    if count <= 0 {
        return Err(io::Error::last_os_error());
    }
    let end = bytes[..count as usize]
        .iter()
        .position(|b| *b == 0)
        .ok_or_else(denied)?;
    CString::new(&bytes[..end]).map_err(|_| denied())
}
