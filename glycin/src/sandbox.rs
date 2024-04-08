// Copyright (c) 2024 GNOME Foundation Inc.

use std::fs::{canonicalize, DirEntry, File};
use std::io::{self, BufRead, BufReader, Seek};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;

use libseccomp::error::SeccompError;
use libseccomp::{ScmpAction, ScmpFilterContext, ScmpSyscall};
use memfd::{Memfd, MemfdOptions};
use nix::sys::resource;

use crate::{Error, SandboxMechanism};

static SYSTEM_SETUP: async_lock::Mutex<Option<Arc<io::Result<SystemSetup>>>> =
    async_lock::Mutex::new(None);

const ALLOWED_SYSCALLS: &[&str] = &[
    "access",
    "arch_prctl",
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
    "recvfrom",
    "recvmsg",
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
    "sigaltstack",
    "signalfd4",
    "socket",
    "socketcall",
    "stat",
    "statfs",
    "statfs64",
    "statx",
    "sysinfo",
    "timerfd_create",
    "timerfd_settime",
    "timerfd_settime64",
    "ugetrlimit",
    "unshare",
    "wait4",
    "write",
];

pub struct Sandbox {
    sandbox_mechanism: SandboxMechanism,
    command: PathBuf,
    stdin: UnixStream,
    ro_bind_extra: Vec<PathBuf>,
}

pub struct SpawnedSandbox {
    pub child: Child,
    pub info: SandboxInfo,
}

pub struct SandboxInfo {
    pub command_dbg: String,
    pub seccomp_fd: Option<Memfd>,
}

impl Sandbox {
    pub fn new(sandbox_mechanism: SandboxMechanism, command: PathBuf, stdin: UnixStream) -> Self {
        Self {
            sandbox_mechanism,
            command,
            stdin,
            ro_bind_extra: Vec::new(),
        }
    }

    pub fn add_ro_bind(&mut self, path: PathBuf) {
        self.ro_bind_extra.push(path);
    }

    pub async fn spawn(self) -> crate::Result<SpawnedSandbox> {
        // Determine command line args
        let (bin, args, seccomp_fd) = match self.sandbox_mechanism {
            SandboxMechanism::Bwrap => {
                let mut args = self.bwrap_args().await?;

                let seccomp_memfd = Self::seccomp_export_bpf(&Self::seccomp_filter()?)?;
                args.push("--seccomp".into());
                args.push(seccomp_memfd.as_raw_fd().to_string().into());

                args.push(self.command);

                ("bwrap".into(), args, Some(seccomp_memfd))
            }
            SandboxMechanism::FlatpakSpawn => {
                let memory_limit = Self::memory_limit();

                let args = vec![
                    "--sandbox".into(),
                    // die with parent
                    "--watch-bus".into(),
                    // change working directory to something that exists
                    "--directory=/".into(),
                    // Start loader with memory limit
                    "prlimit".into(),
                    format!("--as={memory_limit}").into(),
                    // Loader binary
                    self.command,
                ];

                ("flatpak-spawn".into(), args, None)
            }
            SandboxMechanism::NotSandboxed => {
                eprintln!("WARNING: Glycin running without sandbox.");
                (self.command, vec![], None)
            }
        };

        let mut command = Command::new(bin);
        command.stdin(OwnedFd::from(self.stdin));
        command.args(args);

        // Set memory limit for sandbox
        if matches!(self.sandbox_mechanism, SandboxMechanism::Bwrap) {
            unsafe {
                command.pre_exec(|| {
                    Self::set_memory_limit();
                    Ok(())
                });
            }
        }

        let command_dbg = format!("{:?}", command);
        let child = command.spawn().map_err(|err| Error::SpawnError {
            cmd: command_dbg.clone(),
            err: Arc::new(err),
        })?;

        Ok(SpawnedSandbox {
            child,
            info: SandboxInfo {
                command_dbg,
                seccomp_fd,
            },
        })
    }

    async fn bwrap_args(&self) -> crate::Result<Vec<PathBuf>> {
        let mut args: Vec<PathBuf> = Vec::new();

        args.extend(
            [
                "--unshare-all",
                "--clearenv",
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
                // Create a fake HOME for glib to not throw warnings
                "--tmpfs",
                "/tmp-home",
                "--setenv",
                "HOME",
                "/tmp-home",
                // Create a fake runtime dir for glib to not throw warnings
                "--tmpfs",
                "/tmp-run",
                "--setenv",
                "XDG_RUNTIME_DIR",
                "/tmp-run",
                // Fontconfig
                "--ro-bind-try",
                "/etc/fonts",
                "/etc/fonts",
                "--ro-bind-try",
                "/var/cache/fontconfig",
                "/var/cache/fontconfig",
            ]
            .iter()
            .map(|x| (*x).into())
            .collect::<Vec<_>>(),
        );

        let system_setup_arc = SystemSetup::cached().await;
        let system = system_setup_arc.as_ref().as_ref().unwrap();

        // Symlink paths like /usr/lib64 to /lib64
        for (dest, src) in &system.lib_symlinks {
            args.push("--symlink".into());
            args.push(src.clone());
            args.push(dest.clone());
        }

        // Mount paths like /lib64 if they exist
        for dir in &system.lib_dirs {
            args.push("--ro-bind".into());
            args.push(dir.clone());
            args.push(dir.clone());
        }

        // Make extra dirs available
        for dir in &self.ro_bind_extra {
            args.push("--ro-bind".into());
            args.push(dir.clone());
            args.push(dir.clone());
        }

        // Make loader binary available if not in /usr. This is useful for testing and
        // adding loaders in user (/home) configurations.
        if !self.command.starts_with("/usr") {
            args.push("--ro-bind".into());
            args.push(self.command.clone());
            args.push(self.command.clone());
        }

        Ok(args)
    }

    /// Memory limit in bytes that should be applied to sandboxes
    fn memory_limit() -> resource::rlim_t {
        // Lookup free memory
        if let Some(mem_available) = Self::mem_available() {
            Self::calculate_memory_limit(mem_available)
        } else {
            eprintln!("glycin: Unable to determine available memory via /proc/meminfo");

            // Default to 1 GB memory limit
            1024 * 1024 * 1024
        }
    }

    /// Try to determine how much memory is available on the system
    fn mem_available() -> Option<resource::rlim_t> {
        if let Ok(file) = File::open("/proc/meminfo") {
            let meminfo = BufReader::new(file);

            for line in meminfo.lines().flatten() {
                if line.starts_with("MemAvailable:") {
                    if let Some(mem_avail_kb) = line
                        .split(' ')
                        .filter(|x| !x.is_empty())
                        .nth(1)
                        .and_then(|x| x.parse::<resource::rlim_t>().ok())
                    {
                        let mem_available = mem_avail_kb.saturating_mul(1024);

                        return Some(Self::calculate_memory_limit(mem_available));
                    }
                }
            }
        }

        None
    }

    /// Calculate memory that the sandbox will be allowed to use
    fn calculate_memory_limit(mem_available: resource::rlim_t) -> resource::rlim_t {
        // Consider max of 10 GB free RAM for use
        let mem_considered = resource::rlim_t::min(
            mem_available,
            (1024 as resource::rlim_t * 1024 * 1024).saturating_mul(10),
        )
        // Keep at least 200 MB free
        .saturating_sub(1024 * 1024 * 200);

        // Allow usage of 80% of considered memory
        let limit = (mem_considered as f64 * 0.8) as resource::rlim_t;

        limit
    }

    /// Set memory limit for the current process
    fn set_memory_limit() {
        let limit = Self::memory_limit();

        if let Err(err) = resource::setrlimit(resource::Resource::RLIMIT_AS, limit, limit) {
            eprintln!("Error setrlimit(RLIMIT_AS, {limit}): {err}");
        }
    }

    fn seccomp_filter() -> Result<ScmpFilterContext, SeccompError> {
        let mut filter = ScmpFilterContext::new_filter(ScmpAction::Trap)?;

        for syscall_name in ALLOWED_SYSCALLS {
            let syscall = ScmpSyscall::from_name(syscall_name)?;
            filter.add_rule(ScmpAction::Allow, syscall).unwrap();
        }

        Ok(filter)
    }

    fn seccomp_export_bpf(filter: &ScmpFilterContext) -> crate::Result<Memfd> {
        let mut memfd = MemfdOptions::default()
            .close_on_exec(false)
            .create("seccomp-bpf-filter")?;

        filter.export_bpf(&mut memfd)?;

        let mut file = memfd.as_file();
        file.rewind().unwrap();

        Ok(memfd)
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
    async fn cached() -> Arc<io::Result<SystemSetup>> {
        let mut system_setup = SYSTEM_SETUP.lock().await;

        if let Some(arc) = &*system_setup {
            arc.clone()
        } else {
            let arc = Arc::new(Self::new().await);

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
        let dir_content: Result<std::fs::ReadDir, std::io::Error> = std::fs::read_dir("/");

        match dir_content {
            Ok(dir_content) => {
                for entry in dir_content {
                    if let Err(err) = self.add_dir(entry).await {
                        eprintln!("Unable to access entry in root directory (/): {err}");
                    }
                }
            }
            Err(err) => {
                eprintln!("Unable to list root directory (/) entries: {err}");
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
                    // Only use symlinks that link somehwere into /usr/
                    if target.starts_with("/usr/") {
                        self.lib_symlinks.push((path, target));
                    }
                }
            }
        };

        Ok(())
    }
}
