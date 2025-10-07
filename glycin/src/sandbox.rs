// Copyright (c) 2024 GNOME Foundation Inc.

use std::ffi::{c_int, c_void};
use std::fs::{canonicalize, DirEntry, File};
use std::io::{self, BufRead, BufReader, Seek};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use gio::glib;
use libseccomp::error::SeccompError;
use libseccomp::{ScmpAction, ScmpFilterContext, ScmpSyscall};
use memfd::{Memfd, MemfdOptions};
use nix::libc::siginfo_t;
use nix::sys::resource;

use crate::config::{ConfigEntry, ImageLoaderConfig};
use crate::util::{self, new_async_mutex, spawn_blocking, AsyncMutex};
use crate::{Error, SandboxMechanism};

type SystemSetupStore = Arc<Result<SystemSetup, Arc<io::Error>>>;

static SYSTEM_SETUP: AsyncMutex<Option<SystemSetupStore>> = new_async_mutex(None);

/// List of allowed syscalls
///
/// All syscalls are blocked by default via seccomp. Only the following syscalls
/// are allowed. The feature is only available for sandboxes using bubblewrap.
const ALLOWED_SYSCALLS: &[&str] = &[
    "access",
    "arch_prctl",
    "arm_fadvise64_64",
    "brk",
    "capget",
    "capset",
    "chdir",
    "clock_getres",
    "clock_gettime",
    "clock_gettime64",
    "clone",
    "clone3",
    "close",
    "connect",
    "creat",
    "dup",
    "epoll_create",
    "epoll_create1",
    "epoll_ctl",
    "epoll_pwait",
    "epoll_wait",
    "eventfd",
    "eventfd2",
    "execve",
    "exit",
    "exit_group",
    "faccessat",
    "fadvise64",
    "fadvise64_64",
    "fchdir",
    "fcntl",
    "fcntl",
    "fcntl64",
    "fstat",
    "fstatfs",
    "fstatfs64",
    "ftruncate",
    "ftruncate64",
    "futex",
    "futex_time64",
    "get_mempolicy",
    "getcwd",
    "getdents64",
    "getegid",
    "getegid32",
    "geteuid",
    "geteuid32",
    "getgid",
    "getgid32",
    "getpid",
    "getppid",
    "getpriority",
    "getrandom",
    "gettid",
    "gettimeofday",
    "getuid",
    "getuid32",
    "ioctl",
    "madvise",
    "membarrier",
    "memfd_create",
    "mmap",
    "mmap2",
    "mprotect",
    "mremap",
    "munmap",
    "newfstatat",
    "open",
    "openat",
    "pipe",
    "pipe2",
    "pivot_root",
    "poll",
    "ppoll",
    "ppoll_time64",
    "prctl",
    "pread64",
    "prlimit64",
    "read",
    "readlink",
    "readlinkat",
    "recv",
    "recvfrom",
    "recvmsg",
    "restart_syscall",
    "rseq",
    "rt_sigaction",
    "rt_sigprocmask",
    "rt_sigreturn",
    "sched_getaffinity",
    "sched_yield",
    "sendmsg",
    "sendto",
    "set_mempolicy",
    "set_mempolicy",
    "set_robust_list",
    "set_thread_area",
    "set_tid_address",
    "set_tls",
    "setpriority",
    "sigaltstack",
    "signalfd4",
    "socket",
    "socketcall",
    "stat",
    "statfs",
    "statfs64",
    "statx",
    "sysinfo",
    "tgkill",
    "timerfd_create",
    "timerfd_settime",
    "timerfd_settime64",
    "ugetrlimit",
    "uname",
    "unshare",
    "wait4",
    "write",
    "writev",
];

/// Extra syscalls only allowed with fontconfig
///
/// We are only allowing them for fontconfig since generally we don't want to
/// allow such filesystem operations. But seccomp needs them for cache
/// operations.
const ALLOWED_SYSCALLS_FONTCONFIG: &[&str] = &[
    "chmod",
    "link",
    "linkat",
    "rename",
    "renameat",
    "renameat2",
    "unlink",
    "unlinkat",
];

const INHERITED_ENVIRONMENT_VARIABLES: &[&str] = &["RUST_BACKTRACE", "RUST_LOG", "XDG_RUNTIME_DIR"];

pub struct Sandbox {
    sandbox_mechanism: SandboxMechanism,
    config_entry: ConfigEntry,
    dbus_socket: UnixStream,
    ro_bind_extra: Vec<PathBuf>,
}

static_assertions::assert_impl_all!(Sandbox: Send, Sync);

pub struct SpawnedSandbox {
    pub command: Command,
    // Keep seccomp fd alive until process exits
    pub _seccomp_fd: Option<Memfd>,
    pub _dbus_socket: UnixStream,
}

static_assertions::assert_impl_all!(SpawnedSandbox: Send, Sync);

impl Sandbox {
    pub fn new(
        sandbox_mechanism: SandboxMechanism,
        config_entry: ConfigEntry,
        dbus_socket: UnixStream,
    ) -> Self {
        Self {
            sandbox_mechanism,
            config_entry,
            dbus_socket,
            ro_bind_extra: Vec::new(),
        }
    }

    fn exec(&self) -> &Path {
        self.config_entry.exec()
    }

    pub fn add_ro_bind(&mut self, path: PathBuf) {
        self.ro_bind_extra.push(path);
    }

    pub async fn spawn(self) -> Result<SpawnedSandbox, Error> {
        let dbus_fd = self.dbus_socket.as_raw_fd();

        let mut shared_fds = Vec::new();

        let (mut command, seccomp_fd) = match self.sandbox_mechanism {
            SandboxMechanism::Bwrap => {
                let seccomp_memfd = Self::seccomp_export_bpf(&self.seccomp_filter()?)?;
                let command = self.bwrap_command(&seccomp_memfd).await?;

                shared_fds.push(seccomp_memfd.as_raw_fd());

                (command, Some(seccomp_memfd))
            }
            SandboxMechanism::FlatpakSpawn => {
                let command = self.flatpak_spawn_command();

                (command, None)
            }
            SandboxMechanism::NotSandboxed => {
                eprintln!("WARNING: Glycin running without sandbox.");
                let command = self.no_sandbox_command();

                (command, None)
            }
        };

        command.arg("--dbus-fd");
        command.arg(dbus_fd.to_string());

        command.stdin(Stdio::piped());
        command.stderr(Stdio::piped());
        command.stdout(Stdio::piped());

        shared_fds.push(self.dbus_socket.as_raw_fd());

        unsafe {
            command.pre_exec(move || {
                libc::close_range(3, libc::c_uint::MAX, libc::CLOSE_RANGE_CLOEXEC as i32);

                // Allow FDs to be passed to child process
                for raw_fd in &shared_fds {
                    let fd = BorrowedFd::borrow_raw(*raw_fd);
                    if let Ok(flags) = nix::fcntl::fcntl(&fd, nix::fcntl::FcntlArg::F_GETFD) {
                        let mut flags = nix::fcntl::FdFlag::from_bits_truncate(flags);
                        flags.remove(nix::fcntl::FdFlag::FD_CLOEXEC);
                        let _ = nix::fcntl::fcntl(&fd, nix::fcntl::FcntlArg::F_SETFD(flags));
                    }
                }

                Ok(())
            });
        }

        Ok(SpawnedSandbox {
            command,
            _seccomp_fd: seccomp_fd,
            _dbus_socket: self.dbus_socket,
        })
    }

    async fn bwrap_command(&self, seccomp_memfd: &Memfd) -> Result<Command, Error> {
        let mut command = Command::new("bwrap");

        command.args([
            "--unshare-all",
            "--die-with-parent",
            // change working directory to something that exists
            "--chdir",
            "/",
            // Make /usr available as read only
            "--ro-bind",
            "/usr",
            "/usr",
            // Make tmpfs dev available
            "--dev",
            "/dev",
            // Additional linker configuration via /etc/ld.so.conf if available
            "--ro-bind-try",
            "/etc/ld.so.cache",
            "/etc/ld.so.cache",
            // Add /nix/store on systems with Nix
            "--ro-bind-try",
            "/nix/store",
            "/nix/store",
            // Create a fake HOME for glib to not throw warnings
            "--tmpfs",
            "/tmp-home",
            // Create a fake runtime dir for glib to not throw warnings
            "--tmpfs",
            "/tmp-run",
            // setup clean environment
            "--clearenv",
            "--setenv",
            "HOME",
            "/tmp-home",
            "--setenv",
            "XDG_RUNTIME_DIR",
            "/tmp-run",
        ]);

        // Inherit some environment variables
        for key in INHERITED_ENVIRONMENT_VARIABLES {
            if let Some(val) = std::env::var_os(key) {
                command.arg("--setenv");
                command.arg(key);
                command.arg(val);
            }
        }

        let system_setup_arc = SystemSetup::cached().await;

        let system = match system_setup_arc.as_ref().as_ref() {
            Err(err) => {
                return Err(err.clone().into());
            }
            Ok(system) => system,
        };

        // Symlink paths like /usr/lib64 to /lib64
        for (dest, src) in &system.lib_symlinks {
            command.arg("--symlink");
            command.arg(&src);
            command.arg(&dest);
        }

        let mut mounted_paths = Vec::<PathBuf>::new();
        let mut mount = |command: &mut Command, way: &str, path: &Path| {
            if path.is_symlink() {
                if !mounted_paths.iter().any(|x| path.starts_with(x)) {
                    match canonicalize(&path) {
                        Ok(target) => {
                            command.arg("--symlink");
                            command.arg(&target);
                            command.arg(&path);
                            tracing::trace!("Symlink {path:?} -> {target:?}");
                        }
                        Err(err) => tracing::debug!("Couldn't canonicalize path {path:?}: {err}"),
                    }
                } else {
                    tracing::trace!("Parent of symlink {path:?} already mounted. Skipping.");
                }
            }

            match canonicalize(&path) {
                Ok(path) => {
                    if !mounted_paths.iter().any(|x| path.starts_with(x)) {
                        command.arg(way);
                        command.arg(&path);
                        command.arg(&path);
                        tracing::trace!("Mounting {path:?}");
                        mounted_paths.push(path);
                    } else {
                        tracing::trace!("Parent of mount path {path:?} already mounted. Skipping.");
                    }
                }
                Err(err) => tracing::debug!("Couldn't canonicalize path {path:?}: {err}"),
            }
        };

        let caps = get_caps();
        let mut caps_reset_guard = None;

        match caps {
            Ok(caps) => {
                const CAP_DAC_OVERRIDE_POSITION: u32 = 1_u32 % 32;
                const CAP_DAC_READ_SEARCH_POSTION: u32 = 2_u32 % 32;

                if caps[0].effective & (1 << CAP_DAC_OVERRIDE_POSITION) != 0
                    || caps[0].effective & (1 << CAP_DAC_READ_SEARCH_POSTION) != 0
                {
                    let mut new_caps = caps.clone();
                    new_caps[0].effective &= !(1 << CAP_DAC_OVERRIDE_POSITION);
                    new_caps[0].effective &= !(1 << CAP_DAC_READ_SEARCH_POSTION);

                    if let Err(err) = set_caps(new_caps) {
                        tracing::error!("Failed to set caps: {err}");
                    } else {
                        caps_reset_guard = Some(CapsGuard(caps));
                    }
                } else {
                    tracing::trace!("CAP_DAC_OVERRIDE not set. Not touching CAPs");
                }
            }
            Err(ref err) => tracing::error!("Couldn't get Linux caps: {err}"),
        }

        // Mount paths like /lib64 if they exist
        for dir in &system.lib_dirs {
            mount(&mut command, "--ro-bind", dir);
        }

        // Make extra dirs available
        for dir in &self.ro_bind_extra {
            mount(&mut command, "--ro-bind", dir);
        }

        // Make loader binary available if not in /usr. This is useful for testing and
        // adding loaders in user (/home) configurations.
        if !self.exec().starts_with("/usr") {
            mount(&mut command, "--ro-bind", self.exec());
        }

        // Fontconfig
        if !self.config_entry.fontconfig() {
            tracing::trace!("Fontconfig not enabled for loader/editor");
        } else if let Some(fc_paths) = crate::fontconfig::cached_paths() {
            // Expose paths to fonts, configs, and caches
            for path in fc_paths {
                mount(&mut command, "--ro-bind-try", path);
            }

            // Fontconfig needs a writeable cache if the cache is outdated
            let cache_dir = PathBuf::from_iter([
                glib::user_cache_dir(),
                "glycin".into(),
                self.exec().iter().skip(1).collect(),
            ]);

            let fc_cache_dir = PathBuf::from_iter([cache_dir.clone(), "fontconfig".into()]);

            // Create cache dir
            match util::spawn_blocking(move || std::fs::create_dir_all(fc_cache_dir)).await {
                Err(err) => tracing::warn!("Failed to create fontconfig cache dir: {err:?}"),
                Ok(()) => {
                    command.arg("--bind-try");
                    command.arg(&cache_dir);
                    command.arg(&cache_dir);

                    command.arg("--setenv");
                    command.arg("XDG_CACHE_HOME");
                    command.arg(&cache_dir);
                }
            }
        } else {
            tracing::warn!("Failed to load fonftconfig environment");
        }

        // Reset to original caps
        drop(caps_reset_guard);

        // Configure seccomp
        command.arg("--seccomp");
        command.arg(seccomp_memfd.as_raw_fd().to_string());

        // Loader binary
        command.arg(self.exec());

        // Set sandbox memory limit
        unsafe {
            command.pre_exec(|| {
                Self::set_memory_limit();
                Ok(())
            });
        }

        Ok(command)
    }

    fn flatpak_spawn_command(&self) -> Command {
        let mut command = Command::new("flatpak-spawn");

        let memory_limit = Self::memory_limit();
        let dbus_fd = self.dbus_socket.as_raw_fd();

        tracing::debug!("Setting prlimit to {memory_limit} bytes");

        command.args([
            "--sandbox",
            // die with parent
            "--watch-bus",
            // change working directory to something that exists
            "--directory=/",
        ]);

        // Start from a clean environment
        //
        // It's not really cleared due to this issue but nothing we can do about this:
        // <https://github.com/flatpak/flatpak/issues/5271>
        command.env_clear();

        // Inherit some environment variables
        for key in INHERITED_ENVIRONMENT_VARIABLES {
            if let Some(val) = std::env::var_os(key) {
                command.env(key, val);
            }
        }

        // Forward dbus connection
        command.arg(format!("--forward-fd={dbus_fd}"));

        // Start loader with memory limit
        command.arg("prlimit");
        command.arg(format!("--as={memory_limit}"));

        // Loader binary
        command.arg(self.exec());

        // Let flatpak-spawn die if the thread calling it exits
        unsafe {
            command.pre_exec(|| {
                nix::sys::prctl::set_pdeathsig(nix::sys::signal::SIGKILL).map_err(Into::into)
            });
        }

        command
    }

    fn no_sandbox_command(&self) -> Command {
        let mut command = Command::new(self.exec());

        command.env_clear();

        // Inherit some environment variables
        for key in INHERITED_ENVIRONMENT_VARIABLES {
            if let Some(val) = std::env::var_os(key) {
                command.env(key, val);
            }
        }

        // Set sandbox memory limit
        unsafe {
            command.pre_exec(|| {
                nix::sys::prctl::set_pdeathsig(nix::sys::signal::SIGKILL).map_err(Into::into)
            });
        }

        command
    }

    /// Memory limit in bytes that should be applied to sandboxes
    fn memory_limit() -> resource::rlim_t {
        // Lookup free memory
        if let Some(mem_available) = Self::mem_available() {
            Self::calculate_memory_limit(mem_available)
        } else {
            tracing::warn!("glycin: Unable to determine available memory via /proc/meminfo");

            // Default to 1 GB memory limit
            const { (1024 as resource::rlim_t).pow(3) }
        }
    }

    /// Try to determine how much memory is available on the system
    fn mem_available() -> Option<resource::rlim_t> {
        if let Ok(file) = File::open("/proc/meminfo") {
            let meminfo = BufReader::new(file);
            let mut total_avail_kb: Option<resource::rlim_t> = None;

            for line in meminfo.lines().map_while(Result::ok) {
                if line.starts_with("MemAvailable:") || line.starts_with("SwapFree:") {
                    tracing::trace!("Using /proc/meminfo: {line}");
                    if let Some(mem_avail_kb) = line
                        .split(' ')
                        .filter(|x| !x.is_empty())
                        .nth(1)
                        .and_then(|x| x.parse::<resource::rlim_t>().ok())
                    {
                        total_avail_kb =
                            Some(total_avail_kb.unwrap_or(0).saturating_add(mem_avail_kb));
                    }
                }
            }

            if let Some(total_avail_kb) = total_avail_kb {
                let mem_available = total_avail_kb.saturating_mul(1024);

                return Some(mem_available);
            }
        }

        None
    }

    /// Calculate memory that the sandbox will be allowed to use
    fn calculate_memory_limit(mem_available: resource::rlim_t) -> resource::rlim_t {
        // Consider max of 20 GB free RAM for use
        let mem_considered = resource::rlim_t::min(
            mem_available,
            const { (1024 as resource::rlim_t).pow(3).saturating_mul(20) },
        )
        // Keep at least 200 MB free
        .saturating_sub(1024 * 1024 * 200);

        // Allow usage of 80% of considered memory
        (mem_considered as f64 * 0.8) as resource::rlim_t
    }

    /// Set memory limit for the current process
    fn set_memory_limit() {
        let limit = Self::memory_limit();

        let msg = b"Setting process memory limit\n";
        unsafe {
            let _ = libc::write(libc::STDERR_FILENO, msg.as_ptr() as *const _, msg.len());
        }

        if resource::setrlimit(resource::Resource::RLIMIT_AS, limit, limit).is_err() {
            let msg = b"Error setrlimit(RLIMIT_AS)\n";
            unsafe {
                let _ = libc::write(libc::STDERR_FILENO, msg.as_ptr() as *const _, msg.len());
            }
        }
    }

    fn seccomp_filter(&self) -> Result<ScmpFilterContext, SeccompError> {
        // Using `KillProcess` allows rejected syscalls to be logged by auditd. But it
        // doesn't work with tools like valgrind. That's why it's not used by default.
        let mut filter = if std::env::var("GLYCIN_SECCOMP_DEFAULT_ACTION")
            .ok()
            .as_deref()
            == Some("KILL_PROCESS")
        {
            ScmpFilterContext::new(ScmpAction::KillProcess)?
        } else {
            ScmpFilterContext::new(ScmpAction::Trap)?
        };

        let mut syscalls = vec![ALLOWED_SYSCALLS];
        if self.config_entry.fontconfig() {
            // Enable some write operations for fontconfig to update its cache
            syscalls.push(ALLOWED_SYSCALLS_FONTCONFIG);
        }

        for syscall_name in syscalls.into_iter().flatten() {
            let syscall = ScmpSyscall::from_name(syscall_name)?;
            filter.add_rule(ScmpAction::Allow, syscall)?;
        }

        Ok(filter)
    }

    /// Make seccomp filters available under FD
    ///
    /// Bubblewrap supports taking an fd to seccomp filters in the BPF format.
    fn seccomp_export_bpf(filter: &ScmpFilterContext) -> Result<Memfd, Error> {
        let memfd = MemfdOptions::default()
            .close_on_exec(false)
            .create("seccomp-bpf-filter")?;
        let mut file = memfd.as_file();

        filter.export_bpf(&mut file)?;

        file.rewind()?;

        Ok(memfd)
    }

    /// Returns `true` if bwrap syscalls are blocked
    pub async fn check_bwrap_syscalls_blocked() -> bool {
        match Self::check_bwrap_syscalls_blocked_internal().await {
            Err(err) => {
                tracing::info!("Can't determine if bwrap syscalls are blocked: {err} ({err:?})");
                // For error states we assume that bwrap failed for other reasons than sandbox
                // creation being blocked
                false
            }
            Ok(blocked) => {
                tracing::debug!("bwrap sandboxing available: {}", !blocked);
                blocked
            }
        }
    }

    async fn check_bwrap_syscalls_blocked_internal() -> Result<bool, Error> {
        let config_entry = ConfigEntry::Loader(ImageLoaderConfig {
            // The binary is not really relevant, since sandbox is also assumed to work, if the
            // binary does not exist.
            exec: PathBuf::from("/usr/bin/true"),
            expose_base_dir: false,
            fontconfig: false,
        });

        let (dbus_socket, _) = UnixStream::pair()?;
        let sandbox = Self::new(SandboxMechanism::Bwrap, config_entry, dbus_socket);

        let seccomp_memfd = Self::seccomp_export_bpf(&sandbox.seccomp_filter()?)?;
        let mut command = sandbox.bwrap_command(&seccomp_memfd).await?;

        unsafe {
            command.pre_exec(|| {
                setup_sigsys_handler();
                Ok(())
            })
        };

        tracing::debug!("Testing bwrap availability with: {command:?}");

        let output = spawn_blocking(move || command.output()).await?;

        tracing::debug!(
            "bwrap availability test returned: {output:?} (Signal: {signal:?}, Code: {code:?})",
            signal = output.status.signal(),
            code = output.status.code(),
        );

        if output.status.success() {
            Ok(false)
        } else {
            if matches!(output.status.signal(), Some(libc::SIGSYS))
                || output.status.code() == Some(128 + libc::SIGSYS)
            {
                tracing::debug!("bwrap syscalls not available: Terminated with SIGSYS");
                Ok(true)
            } else if std::str::from_utf8(&output.stderr).map_or(false, |x| {
                [
                    "Creating new namespace failed",
                    "No permissions to create a new namespace",
                    // Wrong grammar in older bwrap versions
                    "No permissions to creating new namespace",
                    // Wording of an old Debian patch
                    "No permissions to create new namespace",
                ]
                .iter()
                .any(|y| x.contains(y))
            }) {
                tracing::debug!("bwrap syscalls not available: STDERR contains known string");
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

#[derive(Debug, Default)]
struct SystemSetup {
    // Dirs that need to be symlinked (UsrMerge)
    lib_symlinks: Vec<(PathBuf, PathBuf)>,
    // Dirs that need mounting (not UsrMerged)
    lib_dirs: Vec<PathBuf>,
}

impl SystemSetup {
    async fn cached() -> SystemSetupStore {
        let mut system_setup = SYSTEM_SETUP.lock().await;

        if let Some(arc) = &*system_setup {
            arc.clone()
        } else {
            let arc = Arc::new(Self::new().await.map_err(Arc::new));

            *system_setup = Some(arc.clone());

            arc
        }
    }

    async fn new() -> io::Result<SystemSetup> {
        let mut system = SystemSetup::default();

        system.load_lib_dirs().await?;

        Ok(system)
    }

    async fn load_lib_dirs(&mut self) -> io::Result<()> {
        let dir_content = std::fs::read_dir("/");

        match dir_content {
            Ok(dir_content) => {
                for entry in dir_content {
                    if let Err(err) = self.add_dir(entry).await {
                        tracing::warn!("Unable to access entry in root directory (/): {err}");
                    }
                }
            }
            Err(err) => {
                tracing::error!("Unable to list root directory (/) entries: {err}");
            }
        }

        Ok(())
    }

    async fn add_dir(&mut self, entry: io::Result<DirEntry>) -> io::Result<()> {
        let entry = entry?;
        let path = entry.path();

        if let Some(last_segment) = path.file_name() {
            if last_segment.as_encoded_bytes().starts_with(b"lib") {
                let metadata = entry.metadata()?;
                if metadata.is_dir() {
                    // Lib dirs like /lib
                    self.lib_dirs.push(entry.path());
                } else if metadata.is_symlink() {
                    // Symlinks like /lib -> /usr/lib
                    let target = canonicalize(&path)?;
                    // Only use symlinks that link somewhere into /usr/
                    if target.starts_with("/usr/") {
                        self.lib_symlinks.push((path, target));
                    }
                }
            }
        };

        Ok(())
    }
}

#[allow(non_camel_case_types)]
extern "C" fn sigsys_handler(_: c_int, _info: *mut siginfo_t, _: *mut c_void) {
    libc_eprint("glycin sandbox availability test: Blocked syscall used\n");

    unsafe {
        libc::exit(128 + libc::SIGSYS);
    }
}

fn setup_sigsys_handler() {
    let mut mask = nix::sys::signal::SigSet::empty();
    mask.add(nix::sys::signal::Signal::SIGSYS);

    let sigaction = nix::sys::signal::SigAction::new(
        nix::sys::signal::SigHandler::SigAction(sigsys_handler),
        nix::sys::signal::SaFlags::SA_SIGINFO,
        mask,
    );

    unsafe {
        if nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGSYS, &sigaction).is_err() {
            libc_eprint(
                "glycin sandbox availability test: Failed to init syscall failure signal handler",
            );
        }
    };
}

fn libc_eprint(s: &str) {
    unsafe {
        libc::write(
            libc::STDERR_FILENO,
            s.as_ptr() as *const libc::c_void,
            s.len(),
        );
    }
}

#[repr(C)]
#[derive(Debug)]
struct CapHeader {
    version: u32,
    pid: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CapData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

const _LINUX_CAPABILITY_VERSION_3: u32 = 0x20080522;

fn capget(header: &mut CapHeader, data: &mut [CapData; 2]) -> std::io::Result<()> {
    if unsafe {
        libc::syscall(
            libc::SYS_capget,
            header as *mut CapHeader,
            data as *mut CapData,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    } else {
        Ok(())
    }
}

fn capset(header: &mut CapHeader, data: &mut [CapData; 2]) -> std::io::Result<()> {
    if unsafe {
        libc::syscall(
            libc::SYS_capset,
            header as *mut CapHeader,
            data as *mut CapData,
        ) as i32
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    } else {
        Ok(())
    }
}

fn get_caps() -> std::io::Result<[CapData; 2]> {
    let mut hdr = CapHeader {
        version: _LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };

    let mut data: [CapData; 2] = unsafe { std::mem::zeroed() };

    capget(&mut hdr, &mut data)?;

    Ok(data)
}

fn set_caps(mut caps: [CapData; 2]) -> std::io::Result<()> {
    let mut hdr = CapHeader {
        version: _LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };

    capset(&mut hdr, &mut caps)
}

struct CapsGuard([CapData; 2]);

impl Drop for CapsGuard {
    fn drop(&mut self) {
        if let Err(err) = set_caps(self.0) {
            tracing::error!("Failed to reset linux caps to original state: {err}")
        }
    }
}
