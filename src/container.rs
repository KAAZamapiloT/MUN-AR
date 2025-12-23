use crate::cgroup_manager::CGroupManager;
use crate::config::Config;

use nix::unistd::Pid;
pub struct ChildArgs {
    _config: Config,
    detached: bool,
    ctrl_socket: [i32; 2],
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
            _cgroup_manager,
        }
    }

    pub fn start(&self) -> Pid {
        // Start the container
    }

    pub fn run(&self) -> i64 {
        // Run the container
    }

    fn create_container_process(&self, detached: bool) -> Pid {}

    fn child_function(&self) -> i32 {}
}
