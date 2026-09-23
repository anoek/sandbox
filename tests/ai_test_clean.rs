mod fixtures;

use anyhow::Result;
use chrono::{Local, TimeDelta};
use fixtures::*;
use rstest::*;
use std::path::PathBuf;

/* These tests use a private storage directory so that `clean` never touches
 * real sandboxes, or test sandboxes belonging to other tests. Only stopped
 * sandboxes are used: faking a running one would mean pointing its pid file
 * at a host process, and stopping it would kill everything in the host pid
 * namespace. */

struct CleanEnv {
    sandbox: SandboxManager,
    storage_dir: PathBuf,
}

impl CleanEnv {
    fn new() -> Result<Self> {
        let sandbox = sandbox();
        let storage_dir = std::env::current_dir()?
            .join(sandbox.test_filename_no_rid("storage"));
        std::fs::create_dir_all(&storage_dir)?;
        Ok(Self {
            sandbox,
            storage_dir,
        })
    }

    fn storage_arg(&self) -> String {
        format!("--storage-dir={}", self.storage_dir.display())
    }

    fn run(&mut self, args: &[&str]) -> Result<()> {
        let storage_arg = self.storage_arg();
        let mut full_args = vec![storage_arg.as_str()];
        full_args.extend_from_slice(args);
        self.sandbox.run(&full_args)?;
        Ok(())
    }

    /// Lays out the on-disk structure of a stopped sandbox. Real sandboxes
    /// can't be created in a non-default storage dir from within a sandbox
    /// (overlayfs stacking limits), and `clean` only needs the layout.
    fn create_stopped(&self, name: &str) -> Result<()> {
        for sub in ["upper", "work", "overlay"] {
            std::fs::create_dir_all(self.storage_dir.join(name).join(sub))?;
        }
        std::fs::File::create(self.storage_dir.join(format!("{}.lock", name)))?;
        Ok(())
    }

    fn exists(&self, name: &str) -> bool {
        self.storage_dir.join(format!("{}.lock", name)).exists()
    }
}

impl Drop for CleanEnv {
    fn drop(&mut self) {
        let storage_arg = self.storage_arg();
        let _ = self.sandbox.run(&[&storage_arg, "delete", "-y", "*"]);
        let _ = std::fs::remove_dir_all(&self.storage_dir);
    }
}

fn ephemeral_name(age: TimeDelta) -> String {
    let created = Local::now() - age;
    format!(
        "ephemeral_{}.{}",
        created.format("%Y-%m-%d_%H-%M-%S"),
        rid()
    )
}

#[rstest]
fn test_clean_removes_old_ephemeral_and_test_sandboxes() -> Result<()> {
    let mut env = CleanEnv::new()?;
    let id = rid();

    let old = ephemeral_name(TimeDelta::days(8));
    let recent = ephemeral_name(TimeDelta::days(6));
    let bad_date = format!("ephemeral_not-a-date-{}", id);
    let coverage = format!("sandbox-coverage-test-clean-{}", id);
    let bind_empty = format!("sandbox-bind-empty-sudo-{}", id);
    let bind_empty_dest =
        format!("sandbox-bind-empty-dest-sandbox-coverage-test-{}", id);
    let regular = format!("clean-keep-{}", id);

    for name in [
        &old,
        &recent,
        &bad_date,
        &coverage,
        &bind_empty,
        &bind_empty_dest,
        &regular,
    ] {
        env.create_stopped(name)?;
    }

    env.run(&["clean", "-y"])?;
    assert!(env.sandbox.last_stdout.contains("4 sandboxes deleted"));

    for name in [&old, &coverage, &bind_empty, &bind_empty_dest] {
        assert!(!env.exists(name), "{} should have been cleaned", name);
    }
    for name in [&recent, &bad_date, &regular] {
        assert!(env.exists(name), "{} should have been kept", name);
    }

    Ok(())
}

#[rstest]
fn test_clean_prompts_for_confirmation() -> Result<()> {
    let mut env = CleanEnv::new()?;
    let old_stopped = ephemeral_name(TimeDelta::days(30));
    env.create_stopped(&old_stopped)?;

    let storage_arg = env.storage_arg();
    env.sandbox
        .run_with_stdin(&[&storage_arg, "clean"], "n\n")?;
    assert!(
        env.sandbox
            .last_stdout
            .contains("The following sandboxes will be deleted")
    );
    assert!(env.sandbox.last_stdout.contains(&old_stopped));
    assert!(
        env.sandbox
            .last_stdout
            .contains("Delete operation cancelled")
    );
    assert!(env.exists(&old_stopped));

    Ok(())
}

#[rstest]
fn test_clean_nothing_to_clean() -> Result<()> {
    let mut env = CleanEnv::new()?;
    env.create_stopped(&ephemeral_name(TimeDelta::zero()))?;

    env.run(&["clean", "-y"])?;
    assert!(env.sandbox.last_stdout.contains("No sandboxes to clean."));

    Ok(())
}
