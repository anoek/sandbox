mod fixtures;

use std::fs;
use std::path::Path;

use anyhow::Result;
use fixtures::*;
use rstest::*;

// Find all .profraw files in the sub-sandboxes upper and copy them over to coverage/profraw
fn copy_sub_sandbox_profraw(sub_dir: &Path) -> Result<()> {
    println!("sub_dir: {}", sub_dir.display());
    let sub_upper = sub_dir.join("upper");

    let coverage_dir = Path::new("coverage/profraw");

    fn copy_profraw_files(src_dir: &Path, dest_dir: &Path) -> Result<()> {
        if src_dir.is_dir() {
            for entry in fs::read_dir(src_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    copy_profraw_files(&path, dest_dir)?;
                } else if path.extension().and_then(|s| s.to_str())
                    == Some("profraw")
                {
                    let file_name = path.file_name().unwrap();
                    let dest_path = dest_dir.join(file_name);
                    println!("Copying {:?} to {:?}", path, dest_path);
                    fs::copy(&path, &dest_path)?;
                }
            }
        }
        Ok(())
    }

    copy_profraw_files(Path::new(&sub_upper), coverage_dir)?;
    Ok(())
}

#[rstest]
fn test_overlayfs_stack_depth_message(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_debug_mode(true);
    let sandbox_bin = get_sandbox_bin() + "-setuid";

    assert!(sandbox.xfail(&[&sandbox_bin, &sandbox_bin, &sandbox_bin]));
    assert!(
        sandbox
            .all_stderr
            .contains("Maximum overlayfs stacking depth exceeded")
    );

    let sub_dir = sandbox.run(&[&sandbox_bin, "config", "sandbox_dir"])?;
    let sub_dir = String::from_utf8_lossy(&sub_dir.stdout).to_string();
    let sub_dir = sub_dir.trim().to_string();

    copy_sub_sandbox_profraw(Path::new(&sub_dir))?;
    Ok(())
}
