// Copyright (c) 2024 GNOME Foundation Inc.

use std::ffi::{c_int, c_void};
use std::fs::{DirEntry, File, canonicalize};
use std::io::{self, BufRead, BufReader};
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use gio::glib;
use libseccomp::error::SeccompError;
use libseccomp::{
    ScmpAction, ScmpArch, ScmpArgCompare, ScmpCompareOp, ScmpFilterContext, ScmpSyscall,
};
use nix::libc::siginfo_t;
use nix::sys::{memfd, resource};
use nix::unistd;

use crate::config::{ConfigEntry, ImageLoaderConfig, Processor};
use crate::util::{self, AsyncMutex, new_async_mutex, spawn_blocking};
use crate::{Error, ErrorKind, SandboxMechanism};

type SystemSetupStore = Arc<Result<SystemSetup, Arc<io::Error>>>;

static SYSTEM_SETUP: AsyncMutex<Option<SystemSetupStore>> = new_async_mutex(None);

/**** BEGIN NOTE ON CODE SHARING
 *
 * This code is copied from Flatpak:
 *
 *   https://github.com/flatpak/flatpak/blob/main/common/flatpak-run.c
 *
 * It should be routinely updated to account for changes in Flatpak.
 *
 * We ought to split this code out of Flatpak into a subproject, to make
 * make code sharing easier and reduce the need for manual copy/pasting.
 *
 * * END NOTE ON CODE SHARING
 */
const BLOCKED_SYSCALLS: &[(&str, ScmpAction, &[ScmpArgCompare])] = &[
    /* Block dmesg */
    ("syslog", ScmpAction::Errno(libc::EPERM), &[]),
    /* Useless old syscall */
    ("uselib", ScmpAction::Errno(libc::EPERM), &[]),
    /* Don't allow disabling accounting */
    ("acct", ScmpAction::Errno(libc::EPERM), &[]),
    /* Don't allow reading current quota use */
    ("quotactl", ScmpAction::Errno(libc::EPERM), &[]),
    /* Don't allow access to the kernel keyring */
    ("add_key", ScmpAction::Errno(libc::EPERM), &[]),
    ("keyctl", ScmpAction::Errno(libc::EPERM), &[]),
    ("request_key", ScmpAction::Errno(libc::EPERM), &[]),
    /* Scary VM/NUMA ops */
    ("move_pages", ScmpAction::Errno(libc::EPERM), &[]),
    ("mbind", ScmpAction::Errno(libc::EPERM), &[]),
    ("get_mempolicy", ScmpAction::Errno(libc::EPERM), &[]),
    ("set_mempolicy", ScmpAction::Errno(libc::EPERM), &[]),
    ("migrate_pages", ScmpAction::Errno(libc::EPERM), &[]),
    /* Don't allow subnamespace setups: */
    ("unshare", ScmpAction::Errno(libc::EPERM), &[]),
    ("setns", ScmpAction::Errno(libc::EPERM), &[]),
    ("mount", ScmpAction::Errno(libc::EPERM), &[]),
    ("umount", ScmpAction::Errno(libc::EPERM), &[]),
    ("umount2", ScmpAction::Errno(libc::EPERM), &[]),
    ("pivot_root", ScmpAction::Errno(libc::EPERM), &[]),
    ("chroot", ScmpAction::Errno(libc::EPERM), &[]),
    /* Architectures with CONFIG_CLONE_BACKWARDS2: the child stack
     * and flags arguments are reversed so the flags come second */
    #[cfg(target_arch = "s390x")]
    (
        "clone",
        ScmpAction::Errno(libc::EPERM),
        &[ScmpArgCompare::new(
            1,
            ScmpCompareOp::MaskedEqual(libc::CLONE_NEWUSER as u64),
            libc::CLONE_NEWUSER as u64,
        )],
    ),
    /* Normally the flags come first */
    #[cfg(not(target_arch = "s390x"))]
    (
        "clone",
        ScmpAction::Errno(libc::EPERM),
        &[ScmpArgCompare::new(
            0,
            ScmpCompareOp::MaskedEqual(libc::CLONE_NEWUSER as u64),
            libc::CLONE_NEWUSER as u64,
        )],
    ),
    /* Don't allow faking input to the controlling tty (CVE-2017-5226) */
    (
        "ioctl",
        ScmpAction::Errno(libc::EPERM),
        &[ScmpArgCompare::new(
            1,
            ScmpCompareOp::MaskedEqual(0xFFFFFFFF),
            libc::TIOCSTI,
        )],
    ),
    /* In the unlikely event that the controlling tty is a Linux virtual
     * console (/dev/tty2 or similar), copy/paste operations have an effect
     * similar to TIOCSTI (CVE-2023-28100) */
    (
        "ioctl",
        ScmpAction::Errno(libc::EPERM),
        &[ScmpArgCompare::new(
            1,
            ScmpCompareOp::MaskedEqual(0xFFFFFFFF),
            libc::TIOCLINUX,
        )],
    ),
    /* seccomp can't look into clone3()'s struct clone_args to check whether
     * the flags are OK, so we have no choice but to block clone3().
     * Return ENOSYS so user-space will fall back to clone().
     * (CVE-2021-41133; see also https://github.com/moby/moby/commit/9f6b562d) */
    ("clone3", ScmpAction::Errno(libc::ENOSYS), &[]),
    /* New mount manipulation APIs can also change our VFS. There's no
     * legitimate reason to do these in the sandbox, so block all of them
     * rather than thinking about which ones might be dangerous.
     * (CVE-2021-41133) */
    ("open_tree", ScmpAction::Errno(libc::ENOSYS), &[]),
    ("move_mount", ScmpAction::Errno(libc::ENOSYS), &[]),
    ("fsopen", ScmpAction::Errno(libc::ENOSYS), &[]),
    ("fsconfig", ScmpAction::Errno(libc::ENOSYS), &[]),
    ("fsmount", ScmpAction::Errno(libc::ENOSYS), &[]),
    ("fspick", ScmpAction::Errno(libc::ENOSYS), &[]),
    ("mount_setattr", ScmpAction::Errno(libc::ENOSYS), &[]),
    /* Profiling operations; we expect these to be done by tools from outside
     * the sandbox.  In particular perf has been the source of many CVEs.
     */
    ("perf_event_open", ScmpAction::Errno(libc::EPERM), &[]),
    /* Don't allow you to switch to bsd emulation or whatnot */
    (
        "personality",
        ScmpAction::Errno(libc::EPERM),
        &[ScmpArgCompare::new(0, ScmpCompareOp::NotEqual, 0x0000)],
    ),
];

const INHERITED_ENVIRONMENT_VARIABLES: &[&str] = &["RUST_BACKTRACE", "RUST_LOG", "XDG_RUNTIME_DIR"];

pub struct Sandbox {
    sandbox_mechanism: SandboxMechanism,
    config_entry: ConfigEntry,
    exec: PathBuf,
    dbus_socket: UnixStream,
    ro_bind_extra: Vec<PathBuf>,
}

static_assertions::assert_impl_all!(Sandbox: Send, Sync);

pub struct SpawnedSandbox {
    pub command: Command,
    // Keep seccomp fd alive until process exits
    pub _seccomp_fd: Option<OwnedFd>,
    pub _dbus_socket: UnixStream,
}

static_assertions::assert_impl_all!(SpawnedSandbox: Send, Sync);

impl Sandbox {
    pub fn new(
        sandbox_mechanism: SandboxMechanism,
        config_entry: ConfigEntry,
        dbus_socket: UnixStream,
    ) -> Result<Self, Error> {
        Ok(Self {
            sandbox_mechanism,
            exec: config_entry
                .exec()
                .map(|x| x.to_path_buf())
                .ok_or(ErrorKind::ExpectedBinaryProcessor.err())?,
            config_entry,
            dbus_socket,
            ro_bind_extra: Vec::new(),
        })
    }

    fn exec(&self) -> &Path {
        self.exec.as_path()
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
                #[cfg(not(all(target_os = "linux", target_env = "musl")))]
                {
                    libc::close_range(3, libc::c_uint::MAX, libc::CLOSE_RANGE_CLOEXEC as i32);
                }
                #[cfg(all(target_os = "linux", target_env = "musl"))]
                {
                    libc::syscall(
                        libc::SYS_close_range,
                        3,
                        libc::c_uint::MAX,
                        libc::CLOSE_RANGE_CLOEXEC as libc::c_uint,
                    );
                }
                // Allow FDs to be passed to child process
                for raw_fd in &shared_fds {
                    let fd = BorrowedFd::borrow_raw(*raw_fd);
                    if let Ok(flags) = nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_GETFD) {
                        let mut flags = nix::fcntl::FdFlag::from_bits_truncate(flags);
                        flags.remove(nix::fcntl::FdFlag::FD_CLOEXEC);
                        let _ = nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_SETFD(flags));
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

    async fn bwrap_command(&self, seccomp_memfd: &OwnedFd) -> Result<Command, Error> {
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
            command.arg(src);
            command.arg(dest);
        }

        let mut mounted_paths = Vec::<PathBuf>::new();
        let mut mount = |command: &mut Command, way: &str, path: &Path| {
            if path.is_symlink() {
                if !mounted_paths.iter().any(|x| path.starts_with(x)) {
                    match canonicalize(path) {
                        Ok(target) => {
                            if !mounted_paths.iter().any(|x| path.starts_with(x)) {
                                command.arg("--symlink");
                                command.arg(&target);
                                command.arg(path);
                                tracing::trace!("Symlink {path:?} -> {target:?}");
                                mounted_paths.push(path.to_owned());
                            } else {
                                tracing::trace!(
                                    "Parent of symlink path {path:?} already mounted. Skipping."
                                );
                            }
                        }
                        Err(err) => tracing::debug!("Couldn't canonicalize path {path:?}: {err}"),
                    }
                } else {
                    tracing::trace!("Parent of symlink {path:?} already mounted. Skipping.");
                }
            }

            match canonicalize(path) {
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
                const CAP_DAC_OVERRIDE_POSITION: u32 = 1_u32;
                const CAP_DAC_READ_SEARCH_POSTION: u32 = 2_u32;

                if caps[0].effective & (1 << CAP_DAC_OVERRIDE_POSITION) != 0
                    || caps[0].effective & (1 << CAP_DAC_READ_SEARCH_POSTION) != 0
                {
                    let mut new_caps = caps;
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
            match util::spawn_blocking(move || {
                std::fs::create_dir_all(fc_cache_dir).map_err(|x| x.into())
            })
            .await
            .flatten()
            {
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
        let mut filter = ScmpFilterContext::new(ScmpAction::Allow)?;

        #[cfg(target_arch = "x86")]
        filter.add_arch(ScmpArch::X8664)?;
        #[cfg(target_arch = "x86_64")]
        filter.add_arch(ScmpArch::X86)?;
        #[cfg(target_arch = "arm")]
        filter.add_arch(ScmpArch::Aarch64)?;
        #[cfg(target_arch = "aarch64")]
        filter.add_arch(ScmpArch::Arm)?;

        for (syscall_name, action, conditions) in BLOCKED_SYSCALLS {
            let syscall = ScmpSyscall::from_name(syscall_name)?;
            dbg!(&syscall_name);
            filter.add_rule_conditional(*action, syscall, conditions)?;
        }

        Ok(filter)
    }

    /// Make seccomp filters available under FD
    ///
    /// Bubblewrap supports taking an fd to seccomp filters in the BPF format.
    fn seccomp_export_bpf(filter: &ScmpFilterContext) -> Result<OwnedFd, Error> {
        let memfd = memfd::memfd_create(c"seccomp-bpf-filter", memfd::MFdFlags::empty())?;

        filter.export_bpf(&memfd)?;

        unistd::lseek64(&memfd, 0, unistd::Whence::SeekSet)?;

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
            processor: Processor::Binary(PathBuf::from("/usr/bin/true")),
            expose_base_dir: false,
            fontconfig: false,
            identifiers: Vec::new(),
        });

        let (dbus_socket, _) = UnixStream::pair()?;
        let sandbox = Self::new(SandboxMechanism::Bwrap, config_entry, dbus_socket)?;

        let seccomp_memfd = Self::seccomp_export_bpf(&sandbox.seccomp_filter()?)?;
        let mut command = sandbox.bwrap_command(&seccomp_memfd).await?;

        unsafe {
            command.pre_exec(|| {
                setup_sigsys_handler();
                Ok(())
            })
        };

        tracing::debug!("Testing bwrap availability with: {command:?}");

        let output = spawn_blocking(move || command.output()).await??;

        tracing::debug!(
            "bwrap availability test returned: {output:?} (Signal: {signal:?}, Code: {code:?})",
            signal = output.status.signal(),
            code = output.status.code(),
        );

        if output.status.success() {
            Ok(false)
        } else if matches!(output.status.signal(), Some(libc::SIGSYS))
            || output.status.code() == Some(128 + libc::SIGSYS)
        {
            tracing::debug!("bwrap syscalls not available: Terminated with SIGSYS");
            Ok(true)
        } else if std::str::from_utf8(&output.stderr).is_ok_and(|x| {
            [
                "Creating new namespace failed",
                "No permissions to create a new namespace",
                // Wrong grammar in older bwrap versions
                "No permissions to creating new namespace",
                // Wording of an old Debian patch
                "No permissions to create new namespace",
                "bwrap: setting up uid map: Permission denied",
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

        if let Some(last_segment) = path.file_name()
            && last_segment.as_encoded_bytes().starts_with(b"lib")
        {
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
        Err(std::io::Error::last_os_error())
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
        Err(std::io::Error::last_os_error())
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
