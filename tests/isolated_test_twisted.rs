mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{
    fs::{create_dir_all, write},
    path::{Path, PathBuf},
};

#[rstest]
fn test_twisted(mut sandbox: SandboxManager) -> Result<()> {
    let path = PathBuf::from(&sandbox.test_filename_no_rid("source"));

    create_dir_all(format!("{}/a/b/c", path.display()))?;
    write(format!("{}/a/b/c/file_c", path.display()), "file_c")?;
    write(format!("{}/a/b/file_b", path.display()), "file_b")?;

    sandbox.run(&[
        "mkdir",
        "-p",
        format!("{}/a/b/c/d", path.display()).as_str(),
    ])?;
    sandbox.run(&[
        "mv",
        format!("{}/a/b/c/d", path.display()).as_str(),
        format!("{}/d", path.display()).as_str(),
    ])?;
    sandbox.run(&[
        "mv",
        format!("{}/a", path.display()).as_str(),
        format!("{}/d/a", path.display()).as_str(),
    ])?;
    sandbox.run(&[
        "cp",
        format!("{}/d/a/b/c/file_c", path.display()).as_str(),
        format!("{}/d/file_c_v2", path.display()).as_str(),
    ])?;
    sandbox.run(&[
        "cp",
        format!("{}/d/a/b/file_b", path.display()).as_str(),
        format!("{}/d/a/b/c/file_c", path.display()).as_str(),
    ])?;
    sandbox
        .run(&["rm", format!("{}/d/a/b/file_b", path.display()).as_str()])?;

    sandbox.run(&["accept"])?;

    assert!(
        std::fs::read_to_string(format!("{}/d/file_c_v2", path.display()))?
            == "file_c"
    );
    assert!(
        !Path::new(format!("{}/d/a/b/file_b", path.display()).as_str())
            .exists()
    );
    assert!(
        std::fs::read_to_string(format!("{}/d/a/b/c/file_c", path.display()))?
            == "file_b"
    );

    Ok(())
}

#[rstest]
fn test_twisted_fail_rmdir_on_separate_device(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let path = PathBuf::from(&sandbox.test_filename_no_rid("source"));

    create_dir_all(format!("{}/a/b/c", path.display()))?;
    write(format!("{}/a/b/c/file_c", path.display()), "file_c")?;
    write(format!("{}/a/b/file_b", path.display()), "file_b")?;

    sandbox.run(&[
        "mkdir",
        "-p",
        format!("{}/a/b/c/d", path.display()).as_str(),
    ])?;
    sandbox.run(&[
        "mv",
        format!("{}/a/b/c/d", path.display()).as_str(),
        format!("{}/d", path.display()).as_str(),
    ])?;
    sandbox.run(&[
        "mv",
        format!("{}/a", path.display()).as_str(),
        format!("{}/d/a", path.display()).as_str(),
    ])?;
    sandbox.run(&[
        "cp",
        format!("{}/d/a/b/c/file_c", path.display()).as_str(),
        format!("{}/d/file_c_v2", path.display()).as_str(),
    ])?;
    sandbox.run(&[
        "cp",
        format!("{}/d/a/b/file_b", path.display()).as_str(),
        format!("{}/d/a/b/c/file_c", path.display()).as_str(),
    ])?;
    sandbox
        .run(&["rm", format!("{}/d/a/b/file_b", path.display()).as_str()])?;

    assert!(sandbox.exfail(
        &["accept"],
        "TEST_ACCEPT_FAIL_RMDIR_ON_DIFFERENT_DEVICE",
        "1"
    ));

    Ok(())
}
