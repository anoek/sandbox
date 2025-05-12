mod fixtures;

use anyhow::Result;
use fixtures::*;
use libc::{getgid, getuid};
use rstest::*;
use std::os::unix::fs::MetadataExt;
use std::{fs::create_dir, path::Path};

#[rstest]
fn test_accept_file(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("file");
    let path = Path::new(&filename);
    sandbox.run(&["touch", &filename])?;
    assert!(!path.exists());
    sandbox.run(&["accept", &filename])?;
    assert!(path.exists());
    std::fs::remove_file(path)?;
    assert!(!path.exists());
    sandbox.run(&["accept", &filename])?;
    assert!(!path.exists());
    Ok(())
}

#[rstest]
fn test_accept_directory(mut sandbox: SandboxManager) -> Result<()> {
    let dirname = sandbox.test_filename("dir");
    let path = Path::new(&dirname);
    sandbox.run(&["mkdir", &dirname])?;
    assert!(!path.exists());
    sandbox.run(&["accept", &dirname])?;
    assert!(path.exists());
    std::fs::remove_dir(path)?;
    assert!(!path.exists());
    Ok(())
}

#[rstest]
fn test_accept_symlink(mut sandbox: SandboxManager) -> Result<()> {
    let dirname = sandbox.test_filename("test-dir");
    let symlink = sandbox.test_filename("test-symlink");
    sandbox.run(&["mkdir", &dirname])?;
    sandbox.run(&[
        "ln",
        "-s",
        Path::new(&dirname).file_name().unwrap().to_str().unwrap(),
        &symlink,
    ])?;
    assert!(!Path::new(&dirname).exists());
    assert!(!Path::new(&symlink).exists());
    sandbox.run(&["accept", "**/test-*"])?;
    assert!(Path::new(&dirname).exists());
    assert!(Path::new(&symlink).exists());
    std::fs::remove_dir(Path::new(&dirname))?;
    std::fs::remove_file(Path::new(&symlink))?;
    Ok(())
}

#[rstest]
fn test_accept_rename_file(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("test-file");
    let new_filename = sandbox.test_filename("test-file-new");
    let path = Path::new(&filename);
    let new_path = Path::new(&new_filename);
    assert!(!path.exists());
    assert!(!new_path.exists());
    std::fs::write(path, "test")?;
    assert!(path.exists());
    assert!(!new_path.exists());
    sandbox.run(&["mv", &filename, &new_filename])?;
    assert!(path.exists());
    assert!(!new_path.exists());
    sandbox.run(&["accept"])?;
    assert!(!path.exists());
    assert!(new_path.exists());
    Ok(())
}

#[rstest]
fn test_accept_check_ownership_and_permissions(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let filename = sandbox.test_filename("test-file");
    sandbox.run(&["touch", &filename])?;
    assert!(!Path::new(&filename).exists());
    sandbox.run(&["chmod", "664", &filename])?;
    sandbox.run(&["accept", &filename])?;
    assert!(Path::new(&filename).exists());
    let metadata = std::fs::metadata(Path::new(&filename))?;
    assert_eq!(metadata.uid(), unsafe { getuid() });
    assert_eq!(metadata.gid(), unsafe { getgid() });
    assert_eq!(metadata.mode() & 0o777, 0o664);
    Ok(())
}

#[rstest]
fn test_accept_rename_upper_directory(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dirname = sandbox.test_filename("test-dir");
    let new_dirname = sandbox.test_filename("test-dir-new");
    let path = Path::new(&dirname);
    let new_path = Path::new(&new_dirname);
    assert!(!path.exists());
    assert!(!new_path.exists());
    sandbox.run(&["mkdir", &dirname])?;
    assert!(!path.exists());
    assert!(!new_path.exists());
    sandbox.run(&["mv", &dirname, &new_dirname])?;
    sandbox.run(&["accept"])?;
    assert!(!path.exists());
    assert!(new_path.exists());
    Ok(())
}

#[rstest]
fn test_accept_rename_lower_directory(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dirname = sandbox.test_filename("test-dir");
    let new_dirname = sandbox.test_filename("test-dir-new");
    let path = Path::new(&dirname);
    let new_path = Path::new(&new_dirname);
    std::fs::create_dir(path)?;
    assert!(path.exists());
    assert!(!new_path.exists());
    sandbox.run(&["mv", &dirname, &new_dirname])?;
    assert!(path.exists());
    assert!(!new_path.exists());
    sandbox.run(&["accept"])?;
    assert!(!path.exists());
    assert!(new_path.exists());
    Ok(())
}

#[rstest]
fn test_accept_rename_lower_directory_with_stuff(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dirname = sandbox.test_filename("test-dir");
    let new_dirname = sandbox.test_filename("test-dir-new");
    let path = Path::new(&dirname);
    let new_path = Path::new(&new_dirname);
    std::fs::create_dir(path)?;
    std::fs::write(path.join("test-file"), "test")?;
    assert!(path.exists());
    assert!(!new_path.exists());
    sandbox.run(&["mv", &dirname, &new_dirname])?;
    assert!(path.exists());
    assert!(!new_path.exists());
    sandbox.run(&["accept"])?;
    assert!(!path.exists());
    assert!(new_path.exists());
    assert!(new_path.join("test-file").exists());
    Ok(())
}

#[rstest]
fn test_accept_rename_symlink(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("test-file");
    let lower_symlink = sandbox.test_filename("test-symlink-lower");
    let new_lower_symlink = sandbox.test_filename("test-symlink-lower-new");
    let upper_symlink = sandbox.test_filename("test-symlink-upper");
    let new_upper_symlink = sandbox.test_filename("test-symlink-upper-new");

    let filename_path = Path::new(&filename);
    let lower_symlink_path = Path::new(&lower_symlink);
    let new_lower_symlink_path = Path::new(&new_lower_symlink);
    let upper_symlink_path = Path::new(&upper_symlink);
    let new_upper_symlink_path = Path::new(&new_upper_symlink);

    std::fs::write(filename_path, "test")?;
    std::os::unix::fs::symlink(
        filename_path.file_name().unwrap(),
        lower_symlink_path,
    )?;
    assert!(filename_path.exists());
    assert!(lower_symlink_path.exists());
    assert!(!new_lower_symlink_path.exists());
    assert!(!upper_symlink_path.exists());
    assert!(!new_upper_symlink_path.exists());

    sandbox.run(&[
        "ln",
        "-s",
        filename_path.file_name().unwrap().to_str().unwrap(),
        &upper_symlink,
    ])?;
    assert!(filename_path.exists());
    assert!(lower_symlink_path.exists());
    assert!(!new_lower_symlink_path.exists());
    assert!(!upper_symlink_path.exists());
    assert!(!new_upper_symlink_path.exists());

    sandbox.run(&["mv", &upper_symlink, &new_upper_symlink])?;
    sandbox.run(&["mv", &lower_symlink, &new_lower_symlink])?;

    sandbox.run(&["accept"])?;
    assert!(filename_path.exists());
    assert!(!lower_symlink_path.exists());
    assert!(new_lower_symlink_path.exists());
    assert!(!upper_symlink_path.exists());
    assert!(new_upper_symlink_path.exists());

    // read contents of symlinks to verify they both point to test
    let lower_symlink_contents = std::fs::read(new_lower_symlink_path)?;
    let upper_symlink_contents = std::fs::read(new_upper_symlink_path)?;
    assert_eq!(lower_symlink_contents, b"test");
    assert_eq!(upper_symlink_contents, b"test");

    Ok(())
}

#[rstest]
fn test_accept_remove_file(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("test-file");
    let path = Path::new(&filename);
    std::fs::write(path, "test")?;
    assert!(path.exists());
    sandbox.run(&["rm", &filename])?;
    assert!(path.exists());
    sandbox.run(&["accept"])?;
    assert!(!path.exists());
    Ok(())
}

#[rstest]
fn test_accept_remove_directory(mut sandbox: SandboxManager) -> Result<()> {
    let dirname = sandbox.test_filename("test-dir");
    let path = Path::new(&dirname);
    std::fs::create_dir(path)?;
    assert!(path.exists());
    sandbox.run(&["rmdir", &dirname])?;
    assert!(path.exists());
    sandbox.run(&["accept"])?;
    assert!(!path.exists());
    Ok(())
}

#[rstest]
fn test_accept_remove_symlink(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("test-file");
    let symlink = sandbox.test_filename("test-symlink");
    let path = Path::new(&symlink);
    std::fs::write(Path::new(&filename), "test")?;
    std::os::unix::fs::symlink(
        Path::new(&filename).file_name().unwrap(),
        path,
    )?;
    assert!(path.exists());
    sandbox.run(&["rm", &symlink])?;
    assert!(path.exists());
    sandbox.run(&["accept"])?;
    assert!(Path::new(&filename).exists());
    assert!(!path.exists());
    Ok(())
}

/* Opaque directories are directories that exist on the lower fs but
 * have been moved/removed in the upper, and then a new directory created
 * in the same place. */
#[rstest]
fn test_accept_opaque_directory(mut sandbox: SandboxManager) -> Result<()> {
    let dirname = sandbox.test_filename("test-dir");
    let dirname_path = Path::new(&dirname);
    let dir_renamed = sandbox.test_filename("renamed-dir");
    let dir_renamed_path = Path::new(&dir_renamed);
    let masked_file = dirname_path.join("somefile");
    let post_moved_masked_file = dir_renamed_path.join("somefile");
    let new_file = dirname_path.join("newfile");

    create_dir(dirname_path)?;
    assert!(dirname_path.exists());
    std::fs::write(&masked_file, "test")?;
    assert!(masked_file.exists());

    sandbox.run(&["mv", &dirname, &dir_renamed])?;
    assert!(dirname_path.exists());
    assert!(!dir_renamed_path.exists());

    sandbox.run(&["mkdir", &dirname])?;
    sandbox.run(&["touch", new_file.to_str().unwrap()])?;

    assert!(dirname_path.exists());
    assert!(!dir_renamed_path.exists());
    assert!(masked_file.exists());
    assert!(!new_file.exists());
    assert!(!post_moved_masked_file.exists());

    sandbox.run(&["accept"])?;
    assert!(dirname_path.exists());
    assert!(dir_renamed_path.exists());
    assert!(!masked_file.exists());
    assert!(new_file.exists());
    assert!(post_moved_masked_file.exists());

    Ok(())
}

/* Opaque directories are directories that exist on the lower fs but
 * have been moved/removed in the upper, and then a new directory created
 * in the same place. */
#[rstest]
fn test_accept_removed_opaque_directory(
    mut sandbox: SandboxManager,
) -> Result<()> {
    let dirname = sandbox.test_filename("test-dir");
    let dirname_path = Path::new(&dirname);
    let old_file = dirname_path.join("oldfile");
    let new_file = dirname_path.join("newfile");

    create_dir(dirname_path)?;
    assert!(dirname_path.exists());
    std::fs::write(&old_file, "test")?;
    assert!(old_file.exists());

    sandbox.run(&["rm", "-Rf", &dirname])?;
    assert!(dirname_path.exists());
    assert!(old_file.exists());

    sandbox.run(&["mkdir", &dirname])?;
    sandbox.run(&["touch", new_file.to_str().unwrap()])?;

    assert!(dirname_path.exists());
    assert!(old_file.exists());
    assert!(!new_file.exists());

    sandbox.run(&["accept"])?;
    assert!(dirname_path.exists());
    assert!(!old_file.exists());
    assert!(new_file.exists());

    Ok(())
}

#[rstest]
fn test_accept_negated_pattern(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("test-file");
    let filename2 = sandbox.test_filename("test-file2");
    sandbox.run(&["touch", &filename])?;
    sandbox.run(&["touch", &filename2])?;
    sandbox.run(&["accept", "!**/test-file2*"])?;
    assert!(Path::new(&filename).exists());
    assert!(!Path::new(&filename2).exists());
    Ok(())
}

#[rstest]
fn test_accept_file_ignore_invalid_upper_dirs(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.run(&["config", "sandbox_dir"])?;
    let base = sandbox.last_stdout.trim();
    let upper_dir = Path::new(base).join("upper");
    let invalid_base32_dir = Path::new(&upper_dir).join("invalid");
    let invalid_utf8_dir = Path::new(&upper_dir).join("777Q");
    std::fs::create_dir_all(&invalid_base32_dir)?;
    std::fs::create_dir_all(&invalid_utf8_dir)?;

    let filename = sandbox.test_filename("file");
    let path = Path::new(&filename);
    sandbox.run(&["touch", &filename])?;
    assert!(!path.exists());
    sandbox.run(&["accept", &filename])?;
    assert!(path.exists());
    std::fs::remove_file(path)?;
    assert!(!path.exists());
    sandbox.run(&["accept", &filename])?;
    assert!(!path.exists());
    Ok(())
}

#[rstest]
fn test_file_overwrite(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("file");
    let path = Path::new(&filename);
    std::fs::write(&filename, "test")?;
    sandbox.run(&["rm", &filename])?;
    assert!(path.exists());
    sandbox.run(&["touch", &filename])?;
    assert!(path.exists());
    sandbox.run(&["accept", &filename])?;
    assert!(path.exists());
    let contents = std::fs::read(path)?;
    assert_eq!(contents, b"");
    Ok(())
}

#[rstest]
fn test_chmod_file(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("file");
    let path = Path::new(&filename);
    std::fs::write(&filename, "test")?;
    let orig_metadata = std::fs::metadata(path)?;
    sandbox.run(&["chmod", "660", &filename])?;
    assert!(path.exists());
    let metadata = std::fs::metadata(path)?;
    assert_eq!(metadata.permissions(), orig_metadata.permissions());
    sandbox.run(&["accept", &filename])?;
    assert!(path.exists());
    let metadata = std::fs::metadata(path)?;
    assert_ne!(metadata.mode() & 0o777, orig_metadata.mode() & 0o777);
    Ok(())
}

#[rstest]
fn test_chmod_directory(mut sandbox: SandboxManager) -> Result<()> {
    let dirname = sandbox.test_filename("dir");
    let path = Path::new(&dirname);
    std::fs::create_dir(path)?;
    let orig_metadata = std::fs::metadata(path)?;
    sandbox.run(&["chmod", "770", &dirname])?;
    assert!(path.exists());
    let metadata = std::fs::metadata(path)?;
    assert_eq!(metadata.permissions(), orig_metadata.permissions());
    sandbox.run(&["accept", &dirname])?;
    assert!(path.exists());
    let metadata = std::fs::metadata(path)?;
    println!();
    println!();
    println!();
    println!();
    println!();
    println!();
    println!("output: {}", sandbox.last_stdout);
    println!("error: {}", sandbox.last_stderr);
    println!();
    println!();
    println!();
    assert_ne!(metadata.mode() & 0o777, orig_metadata.mode() & 0o777);
    Ok(())
}

#[rstest]
fn test_accept_symlink_overwrites(mut sandbox: SandboxManager) -> Result<()> {
    let dir = sandbox.test_filename("test-dir");
    let dir_file = Path::new(&dir).join("subfile");
    let file = sandbox.test_filename("test-file");
    let file2 = sandbox.test_filename("test-file2");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&dir_file, "subfile")?;
    std::fs::write(&file, "test")?;
    std::fs::write(&file2, "file2")?;
    let file2_symlink_name =
        Path::new(&file2).file_name().unwrap().to_str().unwrap();

    sandbox.run(&["rm", dir_file.to_str().unwrap()])?;
    sandbox.run(&["rmdir", &dir])?;
    sandbox.run(&["rm", &file])?;
    sandbox.run(&["ln", "-s", file2_symlink_name, &file])?;
    sandbox.run(&["ln", "-s", file2_symlink_name, &dir])?;

    assert!(sandbox.pass(&["accept"]));

    let file_contents = std::fs::read(&file)?;
    let dir_contents = std::fs::read(&dir)?;
    assert_eq!(file_contents, b"file2");
    assert_eq!(dir_contents, b"file2");

    Ok(())
}

#[rstest]
fn test_accept_directory_overwrite(mut sandbox: SandboxManager) -> Result<()> {
    let p = sandbox.test_filename("test-file");

    std::fs::write(&p, "test")?;
    sandbox.run(&["rm", &p])?;
    sandbox.run(&["mkdir", &p])?;
    assert!(sandbox.pass(&["accept"]));
    let path = Path::new(&p);
    assert!(path.exists());
    assert!(path.is_dir());

    Ok(())
}

/* This test creates a directory A/B and then moves it to B/A,
 * while also making changes to files within those directories
 * both before and after the move, and remove files before and after
 * the move. */

#[allow(non_snake_case)]
#[rstest]
fn test_accept_inside_out(mut sandbox: SandboxManager) -> Result<()> {
    let A_str = sandbox.test_filename_no_rid("A");
    let B_str = sandbox.test_filename_no_rid("A/B");
    let post_A_str = sandbox.test_filename_no_rid("B/A");
    let post_B_str = sandbox.test_filename_no_rid("B");

    let A = Path::new(&A_str);
    let A_file_pre = A.join("a_file_pre");
    let A_file_post = A.join("a_file_post");
    let A_file_del_pre = A.join("a_file_del_pre");
    let A_file_del_post = A.join("a_file_del_post");
    let B = Path::new(&B_str);
    let B_file_pre = B.join("b_file_pre");
    let B_file_post = B.join("b_file_post");
    let B_file_del_pre = B.join("b_file_del_pre");
    let B_file_del_post = B.join("b_file_del_post");

    let post_A = Path::new(&post_A_str);
    let post_B = Path::new(&post_B_str);
    let post_A_file_pre = post_A.join("a_file_pre");
    let post_A_file_post = post_A.join("a_file_post");
    let post_B_file_pre = post_B.join("b_file_pre");
    let post_B_file_post = post_B.join("b_file_post");
    let post_A_file_del_pre = post_A.join("a_file_del_pre");
    let post_A_file_del_post = post_A.join("a_file_del_post");
    let post_B_file_del_pre = post_B.join("b_file_del_pre");
    let post_B_file_del_post = post_B.join("b_file_del_post");

    std::fs::create_dir_all(A)?;
    std::fs::create_dir_all(B)?;
    std::fs::write(&A_file_pre, "A_file_pre")?;
    std::fs::write(&B_file_pre, "B_file_pre")?;
    std::fs::write(&A_file_post, "A_file_post")?;
    std::fs::write(&B_file_post, "B_file_post")?;
    std::fs::write(&A_file_del_pre, "A_file_del_pre")?;
    std::fs::write(&B_file_del_pre, "B_file_del_pre")?;
    std::fs::write(&A_file_del_post, "A_file_del_post")?;
    std::fs::write(&B_file_del_post, "B_file_del_post")?;

    // modify before
    sandbox.run(&[
        "bash",
        "-c",
        format!("echo -n 'modified_A_pre' > {}", A_file_pre.display()).as_str(),
    ])?;
    sandbox.run(&[
        "bash",
        "-c",
        format!("echo -n 'modified_B_pre' > {}", B_file_pre.display()).as_str(),
    ])?;
    // remove before
    sandbox.run(&["rm", A_file_del_pre.to_str().unwrap()])?;
    sandbox.run(&["rm", B_file_del_pre.to_str().unwrap()])?;

    // move A/B -> B/A
    sandbox.run(&["mv", B.to_str().unwrap(), post_B.to_str().unwrap()])?;
    sandbox.run(&["mv", A.to_str().unwrap(), post_A.to_str().unwrap()])?;

    // modify after
    sandbox.run(&[
        "bash",
        "-c",
        format!("echo -n 'modified_A_post' > {}", post_A_file_post.display())
            .as_str(),
    ])?;
    sandbox.run(&[
        "bash",
        "-c",
        format!("echo -n 'modified_B_post' > {}", post_B_file_post.display())
            .as_str(),
    ])?;
    // remove after
    sandbox.run(&["rm", post_A_file_del_post.to_str().unwrap()])?;
    sandbox.run(&["rm", post_B_file_del_post.to_str().unwrap()])?;

    // Accept the changes and verify the contents
    assert!(sandbox.pass(&["accept"]));
    assert!(post_A.exists());
    assert!(post_B.exists());
    assert!(post_A_file_pre.exists());
    assert!(post_A_file_post.exists());
    assert!(post_B_file_pre.exists());
    assert!(post_B_file_post.exists());
    assert!(!post_A_file_del_pre.exists());
    assert!(!post_A_file_del_post.exists());
    assert!(!post_B_file_del_pre.exists());
    assert!(!post_B_file_del_post.exists());

    assert_eq!(std::fs::read_to_string(post_A_file_pre)?, "modified_A_pre");
    assert_eq!(
        std::fs::read_to_string(post_A_file_post)?,
        "modified_A_post"
    );
    assert_eq!(std::fs::read_to_string(post_B_file_pre)?, "modified_B_pre");
    assert_eq!(
        std::fs::read_to_string(post_B_file_post)?,
        "modified_B_post"
    );

    Ok(())
}

/*
 * have A/B
 * move A to C
 * Remove C/B
 * mkdir C/B
 * put some files in C/B
 *
 *
 * C/B should be an opaque directory that has been moved
*/

#[allow(non_snake_case)]
#[rstest]
fn test_accept_opaque(mut sandbox: SandboxManager) -> Result<()> {
    let A_str = sandbox.test_filename_no_rid("A");
    let C_str = sandbox.test_filename_no_rid("C");

    let A = Path::new(&A_str);
    let A_B = A.join("B");
    let C = Path::new(&C_str);
    let C_B = C.join("B");
    let A_B_file = A_B.join("file");
    let A_B_file_removed = A_B.join("file_removed");

    std::fs::create_dir_all(&A_B)?;
    std::fs::write(&A_B_file, "A_B_file")?;
    std::fs::write(&A_B_file_removed, "A_B_file_removed")?;

    sandbox.run(&["mv", A.to_str().unwrap(), C.to_str().unwrap()])?;
    sandbox.run(&["rm", "-Rf", C_B.to_str().unwrap()])?;
    sandbox.run(&["mkdir", C_B.to_str().unwrap()])?;
    sandbox.run(&[
        "bash",
        "-c",
        format!(
            "echo -n 'C_B_file' > {}",
            C_B.join("file").to_str().unwrap()
        )
        .as_str(),
    ])?;
    sandbox.run(&[
        "bash",
        "-c",
        format!(
            "echo -n 'C_B_file2' > {}",
            C_B.join("file2").to_str().unwrap()
        )
        .as_str(),
    ])?;

    assert!(sandbox.pass(&["accept"]));

    assert!(!A_B.exists());
    assert!(C.exists());
    assert!(C_B.exists());
    assert!(C_B.join("file").exists());
    assert!(C_B.join("file2").exists());
    assert!(!C_B.join("file_removed").exists());
    assert_eq!(std::fs::read_to_string(C_B.join("file"))?, "C_B_file");
    assert_eq!(std::fs::read_to_string(C_B.join("file2"))?, "C_B_file2");

    return Ok(());
}
