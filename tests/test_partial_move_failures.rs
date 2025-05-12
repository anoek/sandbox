mod fixtures;

use std::{fs::create_dir, path::Path};

use anyhow::Result;
use fixtures::*;
use rstest::*;

/* Trying to accept a change that depends on another move */
#[rstest]
fn test_partial_move_failures(mut sandbox: SandboxManager) -> Result<()> {
    let dir = sandbox.test_filename("dir");
    let dir2 = sandbox.test_filename("dir2");
    let dir2_path = Path::new(&dir2);
    let file = dir2_path.join("file");

    create_dir(&dir)?;

    sandbox.run(&["mv", dir.as_str(), dir2.as_str()])?;
    sandbox.run(&["touch", file.to_str().unwrap()])?;

    sandbox.run(&["status", file.to_str().unwrap()])?;
    assert!(sandbox.xfail(&["accept", file.to_str().unwrap()]));
    //assert!(sandbox.last_stdout.contains("Cowardly refusing"));

    Ok(())
}
