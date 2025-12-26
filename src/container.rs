use nix::fcntl::{open, OFlag};
use nix::mount::{mount, MsFlags};
use nix::sched::{clone, CloneFlags};
use nix::sys::signal::{kill, Signal};
use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{chdir, chroot, close, dup2, execvp, getcwd, read, sethostname, write, Pid};

use std::ffi::CString;
use std::fs;
use std::os::unix::io::RawFd;
use std::path::Path;

use crate::cgroup_manager::CGroupManager;
use crate::config::Config;

pub struct ChildArgs {
    pub _config: Config,
    pub detached: bool,
    pub ctrl_socket: [i32; 2],
}

impl ChildArgs {
    pub fn new(config: Config, detached: bool, ctrl_socket: [i32; 2]) -> Self {
        ChildArgs {
            _config: config,
            detached,
            ctrl_socket,
        }
    }
}

pub struct Container {
    _config: Config,
    stack_memory: Vec<u8>,
    _cgroup_manager: CGroupManager,
}

impl Container {
    pub fn new(config: Config, stack_memory: Vec<u8>) -> Self {
        let cgroup_manager = CGroupManager::new(config.clone());
        Container {
            _config: config,
            stack_memory,
            _cgroup_manager: cgroup_manager,
        }
    }

    pub fn start(&self) -> Pid {
        // Start the container
        if let Err(e) = self._cgroup_manager.setup() {
            // Here you "caught" the String error message into the variable 'e'
            eprintln!("[MUN-AR ERROR] Cgroup setup failed: {}", e);
        }
        // created a child process
        let child_t: Pid = self.create_container_process(true);

        if Pid == -1 {
            self._cgroup_manager.teardown();
            return -1;
        }

        if let Err(e) = self._cgroup_manager.apply(child_t) {
            eprintln!("[MUN-AR ERROR] Cgroup apply failed: {}", e);
        }

        println!(
            "[MUN-AR] Container started with PID {}  name {}",
            child_t, self._config.name
        );
        child_t;
    }

    pub fn run(&self) -> i32 {
        // Run the container
        if Err(e) = self._cgroup_manager.setup() {
            eprintln!("[MUN-AR ERROR] Cgroup setup failed: {}", e);
            return -1;
        }

        let child_pid: Pid = self.create_container_process(false);

        if child_pid == -1 {
            self._cgroup_manager.teardown();
            return -1;
        }

        // TODO:add networking support here

        self._cgroup_manager.apply(child_pid);
        // wait for chil reaping
        let exit = match waitpid(child_pid, 0) {
            Ok(WaitStatus::Exited(_pid, status)) => {
                if (status != 0) {
                    eprintln!("[MUN-AR ERROR] Container exited with status {}", status);
                }
                status
            }
            Ok(WaitStatus::Signaled(_pid, sig, _coredumped)) => {
                eprintln!("[MUN-AR ERROR] Container exited with signal {}", sig);
                -1
            }
            _ => {
                eprintln!("[MUN-AR ERROR] Container exited with unknown status");
                -1
            }
        };
        self._cgroup_manager.teardown();
        exit as i32
    }

    fn create_container_process(&self, detached: bool) -> Pid {
        //add scok pair
        //
        // 1. Create Socketpair (Replaces socketpair(AF_UNIX, SOCK_STREAM, 0, ctrl_socks))
        // nix returns a tuple: (parent_fd, child_fd)
        let (parent_sock, child_sock) = socketpair(
            AddressFamily::Unix,
            SockType::Stream,
            None,
            SockFlag::empty(),
        )
        .map_err(|e| format!("Socketpair failed: {}", e))?;

        let flags = CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWUSER;

        let child_args = ChildArgs::new(self._config.clone(), detached, [parent_sock, child_sock]);
        let cb = Box::new(move || {
            // Child logic: close the parent's end of the socket immediately
            let _ = close(child_args.ctrl_socket[0]);
            // Execute the child initialization
            self.child_function(child_args)
        });

        let child_pid: Pid = clone(
            cb,
            &mut self.stack_memory,
            flags,
            Some(signal::SIGCHLD as i32),
        )
        .map_err(|e| {
            let _ = close(parent_sock);
            let _ = close(child_sock);
            format!("clone failed: {}", e)
        })?;
        println!("[Main] Signaling child to proceed...");
        write(parent_sock, b"1").map_err(|e| format!("Failed to signal child: {}", e))?;

        let _ = close(parent_sock);

        Ok(child_pid)
    }

    fn child_function(&self, child_args: ChildArgs) -> i32 {
        //  let child_config=ChildArgs.clone();
        let mut buf = [0u8; 1];
        //wait for parent signal

        if let Err(e) = close(child_args.ctrl_socket[0]) {
            eprintln!("[MUN-AR ERROR] Failed to close control socket: {}", e);
            return -1;
        }
        if let Err(e) = read(child_args.ctrl_socket[1], &mut buf) {
            eprintln!("[MUN-AR ERROR] Failed to read control socket: {}", e);
            return -1;
        }
        if let Err(e) = close(child_args.ctrl_socket[1]) {
            eprintln!("[MUN-AR ERROR] Failed to close control socket: {}", e);
            return -1;
        }

        println!("[CHILD] Hostname set to :{}", self._config.hostname);
        if let Err(e) = sethostname(self._config.hostname) {
            eprintln!("[MUN-AR ERROR] Failed to set hostname: {}", e);
            return -1;
        }
        // apply security in future here
        // applting chroot
        if let Err(e) = self.setup_simple_chroot(&self._config.rootfs_path) {
            eprintln!("[MUN-AR ERROR] Failed to chroot: {}", e);
            return -1;
        }

        // Detached Mode
        if (child_args.detached) {
            match open("/dev/null", OFlag::O_RDWR, nix::sys::stat::Mode::empty()) {
                Ok(dev_null) => {
                    let _ = dup2(dev_null, 0); // STDIN
                    let _ = dup2(dev_null, 1); // STDOUT
                    let _ = dup2(dev_null, 2); // STDERR
                    let _ = close(dev_null);
                }
                Err(e) => eprintln!("[CHILD] Warning: Could not open /dev/null: {}", e),
            }
        }

        // Execute commands
        println!("CHILD COMMAND Waitining to be executed");

        // conveting to c - compatibe string
        let cmd = CString::new(self._config.command.clone()).unwrap();

        let mut args: Vec<CString> = self
            ._config
            .args
            .iter()
            .map(|arg| CString::new(arg.clone()).expect("Failed to create CString"))
            .collect();

        args.insert(0, cmd.clone());

        match execvp(&cmd, &args) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("[Child] execvp failed for {}: {}", self._config.command, e);
                1
            }
        }
    }

    fn setup_simple_chroot(&self, rootfs_path: &str) -> Result<(), String> {
        // Enter Chroot
        chroot(rootfs_path).map_err(|e| format!("chroot failed: {}", e))?;
        // Change directory to rootfs
        chdir("/").map_err(|e| format!("chdir failed: {}", e))?;

        //make diretor

        let _ = fs::create_dir_all("proc").map_err(|e| format!("create_dir_all failed: {}", e))?;

        mount(
            Some("proc"),
            "proc",
            Some("Proc"),
            MsFlags::empty(),
            None::<&str>,
        )
        .map_err(|e| format!("Mount proc failed: {}", e))?;

        let _ = fs::create_dir_all("dev").map_err(|e| format!("create_dir_all failed: {}", e))?;
        let dev_flags = MsFlags::NOSUID | MsFlags::STRICTATIME;
        mount(
            Some("tmpfs"),
            "dev",
            Some("tempfs"),
            dev_flags,
            Some("mode=755"),
        )
        .map_err(|e| format!("Mount dev failed: {}", e))?;

        let _ =
            fs::create_dir_all("dev/pts").map_err(|e| format!("create_dir_all failed: {}", e))?;
        let pty_flags = MsFlags::NOSUID | MsFlags::STRICTATIME;
        mount(
            Some("devpts"),
            "dev/pts",
            Some("devpts"),
            MsFlags::empty(),
            None::<&str>,
        )
        .map_err(|e| format!("Mount dev/pty failed: {}", e))?;

        Ok(())
    }
}
