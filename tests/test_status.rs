mod fixtures;

use std::{path::PathBuf, process::Command};

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_status(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("file");
    let removed = sandbox.test_filename("removed");
    let moved = sandbox.test_filename("moved");
    let moved_2 = sandbox.test_filename("moved_2");

    std::fs::write(&removed, "test")?;
    std::fs::create_dir(&moved)?;

    sandbox.run(&["touch", &filename])?;
    sandbox.run(&["mv", &moved, &moved_2])?;
    sandbox.run(&["rm", &removed])?;

    sandbox.run(&["status", "/"])?;
    assert!(sandbox.last_stdout.contains("Matching changes"));
    assert!(sandbox.last_stdout.contains(&filename));
    assert!(!sandbox.last_stdout.contains("No changes in"));
    println!("{}", sandbox.last_stdout);
    assert!(
        !sandbox
            .last_stdout
            .contains("external or non-matching changes")
    );
    assert!(sandbox.last_stdout.contains(&format!("> {}", &moved)));
    assert!(sandbox.last_stdout.contains(&format!("- {}", &removed)));

    sandbox.run(&["status", "/", &filename])?;
    assert!(sandbox.last_stdout.contains("Matching changes"));
    assert!(sandbox.last_stdout.contains(&filename));
    assert!(!sandbox.last_stdout.contains("No changes in"));
    assert!(
        !sandbox
            .last_stdout
            .contains("external or non-matching changes")
    );

    sandbox.run(&["status", "/", "foobar"])?;
    assert!(sandbox.last_stdout.contains("Matching changes"));
    assert!(sandbox.last_stdout.contains(&filename));
    assert!(
        !sandbox
            .last_stdout
            .contains("external or non-matching changes")
    );

    Ok(())
}

#[rstest]
fn test_status_outside(mut sandbox: SandboxManager) -> Result<()> {
    let filename = "/tmp/test";
    sandbox.run(&["touch", filename])?;
    sandbox.run(&["status"])?;
    println!("{}", sandbox.last_stdout);
    assert!(
        sandbox
            .last_stdout
            .contains("external or non-matching changes")
    );
    sandbox.run(&["status", "/", filename])?;
    println!("{}", sandbox.last_stdout);
    assert!(sandbox.last_stdout.contains("Matching changes"));
    assert!(sandbox.last_stdout.contains(filename));
    Ok(())
}

#[rstest]
fn test_status_no_matching_changes(mut sandbox: SandboxManager) -> Result<()> {
    let filename = "/tmp/test";
    sandbox.run(&["touch", filename])?;
    sandbox.run(&["status", "not-the-file-name"])?;
    assert!(sandbox.last_stdout.contains("No matching changes"));
    Ok(())
}

#[rstest]
fn test_status_changed_attributes(mut sandbox: SandboxManager) -> Result<()> {
    let mode_file = sandbox.test_filename("mode");
    let uid_file = sandbox.test_filename("uid");
    let gid_file = sandbox.test_filename("gid");
    std::fs::write(&mode_file, "test")?;
    std::fs::write(&uid_file, "test")?;
    std::fs::write(&gid_file, "test")?;

    sandbox.run(&["touch", mode_file.as_str()])?;
    sandbox.run(&["touch", uid_file.as_str()])?;
    sandbox.run(&["touch", gid_file.as_str()])?;

    assert!(sandbox.pass(&["status"]));
    assert!(!sandbox.last_stderr.contains(&mode_file));
    assert!(!sandbox.last_stderr.contains(&uid_file));
    assert!(!sandbox.last_stderr.contains(&gid_file));

    sandbox.run(&["chmod", "777", mode_file.as_str()])?;
    assert!(sandbox.pass(&["status"]));
    assert!(sandbox.last_stderr.contains(&mode_file));
    assert!(!sandbox.last_stderr.contains(&uid_file));
    assert!(!sandbox.last_stderr.contains(&gid_file));

    Command::new("sudo")
        .args(["chown", "2", uid_file.as_str()])
        .output()?;
    Command::new("sudo")
        .args(["chgrp", "2", gid_file.as_str()])
        .output()?;

    sandbox.run(&["chmod", "777", uid_file.as_str()])?;
    assert!(sandbox.pass(&["status"]));
    assert!(sandbox.last_stderr.contains(&mode_file));
    assert!(sandbox.last_stderr.contains(&uid_file));
    assert!(sandbox.last_stderr.contains(&gid_file));

    Ok(())
}

#[rstest]
fn test_external_changes(mut sandbox: SandboxManager) -> Result<()> {
    let filename = "/tmp/test-external-changes-pre";
    std::fs::write(filename, "test")?;
    sandbox.run(&["touch", "/tmp/test-external-changes-post"])?;
    sandbox.run(&[
        "bash",
        "-c",
        "echo 'test2' > /tmp/test-external-changes-pre",
    ])?;
    sandbox.run(&["status", "/"])?;
    assert!(sandbox.last_stdout.contains("Matching changes"));
    assert!(
        sandbox
            .last_stdout
            .contains("/tmp/test-external-changes-pre")
    );
    assert!(
        sandbox
            .last_stdout
            .contains("/tmp/test-external-changes-post")
    );

    std::fs::remove_file(filename)?;

    Ok(())
}

#[rstest]
fn test_status_gitignore(mut sandbox: SandboxManager) -> Result<()> {
    let base = PathBuf::from(&sandbox.test_filename("test"));
    std::fs::create_dir_all(base.to_str().unwrap())?;
    let gitignore_path = base.join(".gitignore");
    std::fs::write(
        gitignore_path.to_str().unwrap(),
        "
# Allow everything that was previously blocked by the project's gitignore
!*

# Things we want to test
empty
dir1
suffix/
two
/anchored
space-at-end\\ 

!**/present*
",
    )?;

    sandbox.run(&["mkdir", base.join("empty").to_str().unwrap()])?;
    sandbox.run(&["mkdir", base.join("dir1").to_str().unwrap()])?;
    sandbox.run(&["mkdir", base.join("suffix").to_str().unwrap()])?;
    sandbox.run(&["mkdir", "-p", base.join("two/.part").to_str().unwrap()])?;
    sandbox.run(&["mkdir", base.join("anchored").to_str().unwrap()])?;
    sandbox.run(&["touch", base.join("space-at-end").to_str().unwrap()])?;
    sandbox.run(&["touch", base.join("space-at-end ").to_str().unwrap()])?;

    sandbox.run(&["touch", base.join("dir1/file1").to_str().unwrap()])?;
    sandbox.run(&["touch", base.join("suffix/file2").to_str().unwrap()])?;
    sandbox.run(&["touch", base.join("two/.part/file3").to_str().unwrap()])?;
    sandbox.run(&["touch", base.join("anchored/file4").to_str().unwrap()])?;

    sandbox.run(&["touch", base.join("dir1/present1").to_str().unwrap()])?;
    sandbox.run(&["touch", base.join("suffix/present2").to_str().unwrap()])?;
    sandbox
        .run(&["touch", base.join("two/.part/present3").to_str().unwrap()])?;
    sandbox
        .run(&["touch", base.join("anchored/present4").to_str().unwrap()])?;

    sandbox.set_ignored(false);
    sandbox.run(&["status", base.to_str().unwrap()])?;

    println!("{}", sandbox.last_stderr);
    println!("{}", sandbox.last_stdout);

    assert!(!sandbox.last_stdout.contains("space-at-end "));
    assert!(!sandbox.last_stdout.contains("empty"));
    assert!(!sandbox.last_stdout.contains("file1"));
    assert!(!sandbox.last_stdout.contains("file2"));
    assert!(!sandbox.last_stdout.contains("file3"));
    assert!(!sandbox.last_stdout.contains("file4"));

    assert!(sandbox.last_stdout.contains("space-at-end"));
    assert!(sandbox.last_stdout.contains("present1"));
    assert!(sandbox.last_stdout.contains("present2"));
    assert!(sandbox.last_stdout.contains("present3"));
    assert!(sandbox.last_stdout.contains("present4"));

    Ok(())
}
