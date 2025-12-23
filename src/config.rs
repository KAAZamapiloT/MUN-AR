pub struct Config {
    pub hostname: String,
    pub rootfs_path: String,
    pub command: String,
    pub args: Vec<String>,
    pub memory_limit: u64,
    pub cpu_limit: u64,
}

impl Config {
    pub fn new(name: &str, path: &str) -> Self {
        Config {
            hostname: name.to_string(),
            rootfs_path: path.to_string(),
            command: String::new(),
            args: Vec::new(),
            memory_limit: 0,
            cpu_limit: 0,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hostname: "default".to_string(),
            rootfs_path: "rootfs".to_string(),
            command: "bin/sh".to_string(),
            args: Vec::new(),
            memory_limit: 512,
            cpu_limit: 10,
        }
    }
}
