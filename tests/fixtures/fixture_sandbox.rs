use anyhow::Result;
use log::{error, warn};
use rand::Rng;
use rstest::*;
use std::path::Path;
use std::process::Child;
use std::{path::PathBuf, process::Command};
//use uuid::Uuid;

const COVERAGE_TEST_DATA_DIR: &str = "generated-test-data";

pub fn rid() -> String {
    let mut rng = rand::rng();
    let rid: String = (0..10)
        .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
        .collect();
    rid
}

pub fn get_sandbox_bin() -> String {
    // Get the original working directory when the tests start
    // This is needed because tests may change directories
    let cargo_manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| {
            // Fallback: try to find the project root by looking for Cargo.toml
            let mut current = std::env::current_dir().unwrap();
            loop {
                if current.join("Cargo.toml").exists() {
                    break current.to_string_lossy().to_string();
                }
                if let Some(parent) = current.parent() {
                    current = parent.to_path_buf();
                } else {
                    panic!("Could not find project root");
                }
            }
        });

    let project_root = Path::new(&cargo_manifest_dir);

    let bin = if let Ok(current_exe) = std::env::current_exe() {
        if current_exe.to_string_lossy().contains("/coverage/") {
            "target/coverage/sandbox"
        } else {
            "target/debug/sandbox"
        }
    } else {
        // Fallback if we can't determine the path
        "target/debug/sandbox"
    };

    let path = project_root.join(bin);
    // Convert to absolute path
    let absolute_path = path.canonicalize().unwrap_or_else(|_| path.clone());
    println!("Path: {}", absolute_path.to_string_lossy());
    absolute_path.to_string_lossy().to_string()
}

pub struct SandboxManager {
    pub name: String,
    pub last_stdout: String,
    pub last_stderr: String,
    pub all_stdout: String,
    pub all_stderr: String,
    pub debug_mode: bool,
    pub no_sudo: bool,
    /* we store the sandbox binary path so we can chdir as necessary */
    pub sandbox_bin: String,
    pub no_default_options: bool,
    pub ignored: bool, // Controls whether --ignored is automatically added
}

impl SandboxManager {
    pub fn new() -> Self {
        let name = format!("sandbox-coverage-test-{}", rid());

        #[allow(clippy::panic)]
        match std::fs::create_dir_all(
            Path::new(COVERAGE_TEST_DATA_DIR).join(&name),
        ) {
            Ok(_) => (),
            Err(e) => {
                panic!(
                    "Failed to create {} dir: {}",
                    COVERAGE_TEST_DATA_DIR, e
                );
            }
        }

        Self {
            name,
            last_stdout: String::new(),
            last_stderr: String::new(),
            all_stdout: String::new(),
            all_stderr: String::new(),
            debug_mode: false,
            no_sudo: false,
            sandbox_bin: get_sandbox_bin(),
            no_default_options: false,
            ignored: true, // Default to true to maintain existing behavior
        }
    }

    /* When debug mode is on, the sandbox will not be cleaned up when the fixture is dropped */
    #[allow(dead_code)]
    pub fn set_debug_mode(&mut self, debug_mode: bool) {
        self.debug_mode = debug_mode;
    }

    /* Controls whether --ignored is automatically added to commands */
    #[allow(dead_code)]
    pub fn set_ignored(&mut self, ignored: bool) {
        self.ignored = ignored;
    }

    #[allow(dead_code)]
    pub fn dir(&self) -> Result<PathBuf> {
        let mut cmd = Command::new("sudo");
        cmd.args(["-E", &self.sandbox_bin]);

        if self.no_sudo {
            cmd = Command::new(&self.sandbox_bin);
        }

        cmd.args([format!("--name={}", &self.name)]);
        cmd.args(["config", "sandbox_dir"]);
        let output = cmd.output()?;
        let base = String::from_utf8_lossy(&output.stdout).to_string();
        let base = base.trim();
        Ok(PathBuf::from(base))
    }

    #[allow(dead_code)]
    pub fn test_filename(&self, prefix: &str) -> String {
        format!(
            "{}/{}/{}-{}",
            COVERAGE_TEST_DATA_DIR,
            self.name,
            prefix,
            rid()
        )
    }

    #[allow(dead_code)]
    pub fn test_filename_no_rid(&self, prefix: &str) -> String {
        format!("{}/{}/{}", COVERAGE_TEST_DATA_DIR, self.name, prefix,)
    }

    pub fn run(&mut self, args: &[&str]) -> Result<std::process::Output> {
        self.run_with_env(args, "", "")
    }

    #[allow(dead_code)]
    pub fn run_with_stdin(
        &mut self,
        args: &[&str],
        stdin_input: &str,
    ) -> Result<std::process::Output> {
        use std::io::Write;
        use std::process::Stdio;

        let mut cmd = Command::new("sudo");
        cmd.args(["-E", &self.sandbox_bin]);

        if self.no_sudo {
            cmd = Command::new(&self.sandbox_bin);
        }

        if !self.no_default_options
            && !args.iter().any(|arg| {
                arg.starts_with("--log_level") || arg.starts_with("-v")
            })
        {
            cmd.args(["-v"]);
        }

        if self.ignored {
            cmd.args(["--ignored"]);
        }

        if !self.no_default_options
            && !args.iter().any(|arg| arg.starts_with("--name"))
        {
            cmd.args([format!("--name={}", &self.name)]);
        }
        cmd.args(args);

        // Set up stdin
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        println!(
            "Running command with stdin: {} {}",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        );

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn command: {:?}", e))?;

        // Write to stdin
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(stdin_input.as_bytes()).map_err(|e| {
                anyhow::anyhow!("Failed to write to stdin: {:?}", e)
            })?;
        }

        // Wait for the command to complete
        let output = child.wait_with_output().map_err(|e| {
            anyhow::anyhow!("Failed to wait for command: {:?}", e)
        })?;

        self.last_stdout = String::from_utf8_lossy(&output.stdout).to_string();
        self.last_stderr = String::from_utf8_lossy(&output.stderr).to_string();
        self.all_stdout += &self.last_stdout;
        self.all_stderr += &self.last_stderr;

        if let Some(code) = output.status.code() {
            if code != 0 {
                return Err(anyhow::anyhow!(
                    "Command returned non-zero exit code: {}\nstdout: {}\nstderr: {}",
                    code,
                    self.last_stdout,
                    self.last_stderr
                ));
            }
        } else {
            return Err(anyhow::anyhow!(
                "Command did not return a valid exit code\nstdout: {}\nstderr: {}",
                self.last_stdout,
                self.last_stderr
            ));
        }

        Ok(output)
    }

    pub fn run_with_env(
        &mut self,
        args: &[&str],
        env_key: &str,
        env_value: &str,
    ) -> Result<std::process::Output> {
        let mut cmd = Command::new("sudo");
        if !env_key.is_empty() {
            println!("Setting env var: {}={}", env_key, env_value);
            cmd.env(env_key, env_value);
        }
        cmd.args(["-E", &self.sandbox_bin]);

        if self.no_sudo {
            cmd = Command::new(&self.sandbox_bin);
        }

        if !self.no_default_options
            && !args.iter().any(|arg| {
                arg.starts_with("--log_level") || arg.starts_with("-v")
            })
        {
            cmd.args(["-v"]);
        }

        if self.ignored {
            cmd.args(["--ignored"]);
        }

        if !self.no_default_options
            && !args.iter().any(|arg| arg.starts_with("--name"))
        {
            cmd.args([format!("--name={}", &self.name)]);
        }
        cmd.args(args);
        println!(
            "Running command: {} {}",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        );
        match cmd.output() {
            Ok(output) => {
                self.last_stdout =
                    String::from_utf8_lossy(&output.stdout).to_string();
                self.last_stderr =
                    String::from_utf8_lossy(&output.stderr).to_string();
                self.all_stdout += &self.last_stdout;
                self.all_stderr += &self.last_stderr;

                if let Some(code) = output.status.code() {
                    if code != 0 {
                        return Err(anyhow::anyhow!(
                            "Command returned non-zero exit code: {}\nstdout: {}\nstderr: {}",
                            code,
                            self.last_stdout,
                            self.last_stderr
                        ));
                    }
                } else {
                    return Err(anyhow::anyhow!(
                        "Command did not return a valid exit code\nstdout: {}\nstderr: {}",
                        self.last_stdout,
                        self.last_stderr
                    ));
                }

                Ok(output)
            }
            Err(e) => Err(anyhow::anyhow!("Command failed: {:?}", e)),
        }
    }

    #[allow(dead_code)]
    pub fn run_in_background(&mut self, args: &[&str]) -> Result<Child> {
        let mut cmd = Command::new("sudo");
        cmd.args(["-E", &self.sandbox_bin]);

        if self.no_sudo {
            cmd = Command::new(&self.sandbox_bin);
        }

        if !self.no_default_options {
            cmd.args([format!("--name={}", &self.name)]);
        }
        cmd.args(args);

        println!(
            "Running command in background: {} {}",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        );

        Ok(cmd.spawn()?)
    }

    #[allow(dead_code)]
    pub fn exfail(
        &mut self,
        args: &[&str],
        env_key: &str,
        env_value: &str,
    ) -> bool {
        if self.run_with_env(args, env_key, env_value).is_err() {
            return true;
        }
        println!("last_stderr: {}", self.last_stderr);
        println!("last_stdout: {}", self.last_stdout);
        false
    }

    #[allow(dead_code)]
    pub fn epass(
        &mut self,
        args: &[&str],
        env_key: &str,
        env_value: &str,
    ) -> bool {
        if let Ok(output) = self.run_with_env(args, env_key, env_value) {
            return output.status.code().unwrap() == 0;
        }
        println!("last_stderr: {}", self.last_stderr);
        println!("last_stdout: {}", self.last_stdout);
        false
    }

    #[allow(dead_code)]
    pub fn pass(&mut self, args: &[&str]) -> bool {
        if let Ok(output) = self.run(args) {
            return output.status.code().unwrap() == 0;
        }
        println!("last_stderr: {}", self.last_stderr);
        println!("last_stdout: {}", self.last_stdout);
        false
    }

    #[allow(dead_code)]
    pub fn xfail(&mut self, args: &[&str]) -> bool {
        if let Ok(output) = self.run(args) {
            return output.status.code().unwrap() != 0;
        }
        println!("last_stderr: {}", self.last_stderr);
        println!("last_stdout: {}", self.last_stdout);
        true
    }
}

impl Drop for SandboxManager {
    fn drop(&mut self) {
        // Reset flags to default state for cleanup commands
        self.no_default_options = false;
        self.ignored = true;

        if let Err(e) = self.run(&["stop"]) {
            error!("Failed to kill sandbox: {}", e);
            error!("last_stderr: {}", self.last_stderr);
            error!("last_stdout: {}", self.last_stdout);
            return;
        }
        if let Err(e) = self.run(&["accept", "**/*.profraw"]) {
            error!("Failed to accept profraw files: {}", e);
            error!("last_stderr: {}", self.last_stderr);
            error!("last_stdout: {}", self.last_stdout);
            return;
        }
        let dirname = Path::new(COVERAGE_TEST_DATA_DIR).join(&self.name);
        if !self.debug_mode {
            if let Err(e) = std::fs::remove_dir_all(&dirname) {
                // Only warn if the error is not "file not found" - the directory
                // may have already been cleaned up by the delete functionality
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!("Failed to remove {} dir: {}", dirname.display(), e);
                }
            }
            let sandbox_name = self.name.clone();
            match self.run(&["delete", "-y", sandbox_name.as_str()]) {
                Ok(_) => (),
                Err(e) => {
                    warn!("Failed to delete sandbox: {}", e);
                    warn!("last_stderr: {}", self.last_stderr);
                    warn!("last_stdout: {}", self.last_stdout);
                }
            }
        } else {
            warn!("Debug mode is on, *NOT* cleaning up {}", dirname.display());
        }
    }
}

#[fixture]
pub fn sandbox() -> SandboxManager {
    SandboxManager::new()
}
