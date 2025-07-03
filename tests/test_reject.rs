mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{fs::create_dir, fs::write, path::Path};

#[rstest]
fn test_reject_file(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("file");
    let path = Path::new(&filename);
    assert!(!path.exists());
    sandbox.run(&["touch", &filename])?;
    assert!(!path.exists());
    sandbox.run(&["reject", &filename])?;
    assert!(!path.exists());
    sandbox.run(&["accept", &filename])?;
    assert!(!path.exists());
    Ok(())
}

#[rstest]
fn test_reject_remove_file(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("file");
    let path = Path::new(&filename);
    write(path, "test")?;
    assert!(path.exists());
    sandbox.run(&["rm", &filename])?;
    assert!(path.exists());
    sandbox.run(&["reject", &filename])?;
    assert!(path.exists());
    sandbox.run(&["accept", &filename])?;
    assert!(path.exists());
    Ok(())
}

#[rstest]
fn test_reject_remove_directory(mut sandbox: SandboxManager) -> Result<()> {
    let dirname = sandbox.test_filename("dir");
    let path = Path::new(&dirname);
    create_dir(path)?;
    assert!(path.exists());
    sandbox.run(&["rm", "-r", &dirname])?;
    assert!(path.exists());
    sandbox.run(&["reject", &dirname])?;
    assert!(path.exists());
    sandbox.run(&["accept", &dirname])?;
    assert!(path.exists());
    Ok(())
}

#[rstest]
fn test_reject_remove_filled_directory(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dirname = sandbox.test_filename("dir");
    let path = Path::new(&dirname);
    create_dir(path)?;
    write(path.join("file"), "test")?;
    assert!(path.exists());
    sandbox.run(&["rm", "-r", &dirname])?;
    assert!(path.exists());
    sandbox.run(&["reject", &dirname])?;
    assert!(path.exists());
    sandbox.run(&["accept", &dirname])?;
    assert!(path.exists());
    Ok(())
}

#[rstest]
fn test_reject_remove_filled_directory_with_opaque_directory_not_empty(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dirname = sandbox.test_filename("dir");
    let path = Path::new(&dirname);
    create_dir(path)?;
    write(path.join("file"), "test")?;
    assert!(path.exists());
    sandbox.run(&["rm", "-r", &dirname])?;
    assert!(path.exists());
    sandbox.run(&["mkdir", &dirname])?;
    sandbox.run(&["touch", path.join("file2").to_str().unwrap()])?;
    assert!(sandbox.pass(&["reject", &dirname]));
    assert!(path.exists());
    assert!(path.join("file").exists());
    assert!(!path.join("file2").exists());
    Ok(())
}

#[rstest]
fn test_reject_remove_filled_directory_with_opaque_directory(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dirname = sandbox.test_filename("dir");
    let path = Path::new(&dirname);
    create_dir(path)?;
    write(path.join("file"), "test")?;
    assert!(path.exists());
    sandbox.run(&["rm", "-r", &dirname])?;
    assert!(path.exists());
    sandbox.run(&["mkdir", &dirname])?;
    sandbox.run(&["touch", path.join("file2").to_str().unwrap()])?;
    sandbox.run(&["reject"])?;
    assert!(path.exists());
    sandbox.run(&["accept"])?;
    assert!(path.exists());
    assert!(path.join("file").exists());
    assert!(!path.join("file2").exists());
    std::fs::remove_file(path.join("file"))?;
    std::fs::remove_dir(&dirname)?;
    Ok(())
}
