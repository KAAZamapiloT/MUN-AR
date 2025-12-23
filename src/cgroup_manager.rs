use crate::config::Config;
use nix::unistd::Pid;
use std::fs;
use std::fs::DirBuilder;
use std::io::ErrorKind;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;

pub struct CGroupManager {
    _config: Config,
    cgroup_path: String,
    container_name: String,
}

impl CGroupManager {
    pub fn new(config: Config) -> Self {
        // 1. Extract values first so we don't "move" config yet
        let container_name = config.hostname.clone();
        let cgroup_path = format!("/sys/fs/cgroup/{}", container_name);

        // 2. Now build the struct
        Self {
            container_name,
            cgroup_path,
            config, // 'config' is moved here last
        }
    }
    pub fn setup(&self) -> Result<(), String> {
        //1-> create a cgroup directory
        let mut builder = DirBuilder::new();
        builder.mode(0755);
        builder
            .create(&self.cgroup_path)
            .map_err(|e| format!("Failed to create cgroup directory: {}", e))?;

        //2 set memory limit
        if config.memory_limit > 0 {
            let memory_path = Path::new(&self.cgroup_path).join("memory.max");

            let memory_in_bytes = config.memory_limit * 1024 * 1024;
            fs::write(
                &memory_path,
                memory_in_bytes
                    .to_string()
                    .map_err(|e| format!("Error:Could not write to {}: {} ", memory_path, e)),
            )?;
        }
        // 3 setting process limit
        if config.process_limit > 0 {
            let process_path = Path::new(&self.cgroup_path).join("pids.max");
            fs::write(
                &process_path,
                config
                    .process_limit
                    .to_string()
                    .map_err(|e| format!("Error:Could not write to {}: {} ", process_path, e)),
            )?;
        }
        println!("Setup of cgroup completed {}", self.container_name);
        Ok(())
    }

    pub fn apply_config(&self, pid: Pid) -> Result<(), String> {
        let procs_file = Path::new(&self.cgroup_path).join("cgroup.procs");
        fs::write(
            &procs_file,
            pid.as_raw()
                .to_string()
                .map_err(|e| format!("Error:Could not write to {}: {} ", procs_file, e)),
        )?;
        Ok(())
    }

    pub fn teardown(&self) {
        let mut builder = Builder::new();
        builder.delete(&self.cgroup_path);
    }
}
