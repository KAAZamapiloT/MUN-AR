use std::arch::x86_64::_CMP_LE_OS;
use std::intrinsics::atomic_load_seqcst;
use std::os::raw::c_void;

use crate::cgroup_manager::CGroupManager;
use crate::config::Config;

use nix::sys::signal::{kill, Signal};
use nix::user::Pid;
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
        let flags=CLONE_PID | CLONE_NEWNS | CLONE_NEWUTS |
            CLONE_NEWIPC | CLONE_NEWUSER | CLONE_NEWNET | CLONE_NEWCGROUP;
        let mut ctrl_sock:[i32;2];
        // add security
        let child_args=ChildArgs::new(&self._config,detached,ctrl_sock);

        c_void stack_top=child_args.stack_top;

        let child_pid:Pid= clone(flags, stack_top, child_args);

        write(ctrl_sock[1], child_pid.as_raw());
        Close(ctrl_sock[1]);
        child_pid
    }

    fn child_function(&self) -> i32 {
        12
    }
}
