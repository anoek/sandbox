mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{
    fs::{create_dir_all, write},
    path::{Path, PathBuf},
};

// todo: move file no modification, verify contents
// todo: move file then modify contents, verify contents

fn entry(base: &Path, name: &str) -> Result<PathBuf> {
    let mut path = base.to_path_buf();
    path.push(name);
    create_dir_all(&path)?;
    write(path.join("file"), name)?;
    Ok(path)
}

/* This test creates a deep directory structure on the host system,
 * then sorts the directory structure in the sandbox */
#[rstest]
fn test_accept_deep_reshuffle(mut sandbox: SandboxManager) -> Result<()> {
    let mut path = PathBuf::from(&sandbox.test_filename_no_rid("source"));
    let base = path.clone();
    let disorder = ["3", "1", "5", "2", "8", "4", "9", "7", "6", "0"];

    for name in disorder {
        path = entry(&path, name)?;
    }

    /* Sorts the directory structure in the sandbox so we end up with */
    /* 0/1/2/3/4/5/6/7/8/9 */
    let mut to_path = base.clone();
    for i in 0..10 {
        sandbox.run(&[
            "bash",
            "-c",
            format!("find {} | grep '{}$'", base.display(), i).as_str(),
        ])?;
        let from_path = sandbox.last_stdout.trim().to_string();
        to_path = to_path.join(format!("{}", i));
        let to_path_str = to_path.display().to_string();
        sandbox.run(&["mv", &from_path, &to_path_str])?;
    }

    //sandbox.run(&["status"])?;
    sandbox.run(&["accept"])?;

    //let output = sandbox.run(&["ls", "-R", base.display().as_str()])?;
    //println!("{}", output.stdout);

    /* Verify the contents of the directory structure and files */
    let mut to_path = base.clone();
    for i in 0..10 {
        to_path = to_path.join(format!("{}", i));
        let file_path = to_path.join("file");
        println!("validating {}", file_path.display());
        let file_contents = std::fs::read_to_string(file_path)?;
        assert_eq!(file_contents, format!("{}", i));
    }

    Ok(())
}
