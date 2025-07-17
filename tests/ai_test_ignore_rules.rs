mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;
use std::path::Path;
use std::process::Command;

// cspell:ignore файл filea fileb filex

#[rstest]
fn test_gitignore_functionality(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.set_ignored(false); // Don't automatically add --ignored for gitignore tests

    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("gitignore-test-dir");
    std::fs::create_dir_all(&test_dir)?;

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    let gitignore_contents = r#"
# ignore single file
ignored-file

# ignore entire directory
ignored-dir/

# ignore all *.tmp in dir but keep one
dir/*.tmp
!dir/keep.tmp

# anchored ignore relative to this gitignore directory
/anchored-file

# ignore a directory but allow a child
negate-me
!negate-me/keep
"#;
    // Create .gitignore file within the sandbox
    let gitignore_path = Path::new(&test_dir).join(".gitignore");
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "cat > {} << 'EOF'\n{}EOF",
            gitignore_path.to_str().unwrap(),
            gitignore_contents
        ),
    ])?;

    // Files that SHOULD be ignored
    let ignored_file = Path::new(&test_dir).join("ignored-file");
    let ignored_dir_file = Path::new(&test_dir).join("ignored-dir/a-file");
    let ignored_tmp = Path::new(&test_dir).join("dir/temp.tmp");
    let ignored_negate_dir = Path::new(&test_dir).join("negate-me/file.txt");
    let anchored_file = Path::new(&test_dir).join("anchored-file");

    // Files that should NOT be ignored by /anchored-file rule
    let non_anchored_1 = Path::new(&test_dir).join("nested/anchored-file");
    let non_anchored_2 =
        Path::new(&test_dir).join("nested/deeper/anchored-file");

    // Files that should NOT be ignored (explicit negations)
    let included_file = Path::new(&test_dir).join("included-file");
    let included_keep_tmp = Path::new(&test_dir).join("dir/keep.tmp");
    let included_negate_keep = Path::new(&test_dir).join("negate-me/keep");

    // Create directories
    sandbox.run(&[
        "mkdir",
        "-p",
        Path::new(&test_dir).join("ignored-dir").to_str().unwrap(),
        Path::new(&test_dir).join("dir").to_str().unwrap(),
        Path::new(&test_dir).join("negate-me").to_str().unwrap(),
        Path::new(&test_dir).join("nested/deeper").to_str().unwrap(),
    ])?;

    // Create files
    for path in [
        &ignored_file,
        &ignored_dir_file,
        &ignored_tmp,
        &ignored_negate_dir,
        &anchored_file,
        &non_anchored_1,
        &non_anchored_2,
        &included_file,
        &included_keep_tmp,
        &included_negate_keep,
    ] {
        sandbox.run(&["touch", path.to_str().unwrap()])?;
    }

    // 1) Run `status` WITHOUT the --ignored flag
    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();
    // Files that should be present
    for inc in [
        &included_file,
        &included_keep_tmp,
        &included_negate_keep,
        &non_anchored_1,
        &non_anchored_2,
    ] {
        assert!(stdout.contains(inc.to_str().unwrap()));
    }
    // The primary ignored file and anchored file should *not* be present
    for ign in [&ignored_file, &anchored_file] {
        assert!(!stdout.contains(ign.to_str().unwrap()));
    }

    // 2) Run `status` WITH the --ignored flag via SandboxManager
    sandbox.set_ignored(true);
    sandbox.run(&["status", &test_dir])?;
    let stdout_flag = sandbox.last_stdout.clone();
    sandbox.set_ignored(false);
    // Now every file should appear because we are passing --ignored implicitly
    for path in [
        &ignored_file,
        &ignored_dir_file,
        &ignored_tmp,
        &ignored_negate_dir,
        &anchored_file,
        &non_anchored_1,
        &non_anchored_2,
        &included_file,
        &included_keep_tmp,
        &included_negate_keep,
    ] {
        assert!(stdout_flag.contains(path.to_str().unwrap()));
    }

    Ok(())
}

#[rstest]
fn test_gitignore_double_asterisk_patterns(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false); // Don't automatically add --ignored for gitignore tests

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("double-asterisk-test");
    std::fs::create_dir_all(&test_dir)?;

    let gitignore_contents = r#"
# Match foo directories anywhere
**/foo/
# Match all .log files anywhere
**/*.log
# Match bar files in any test directory
**/test/**/bar
# Match any file under docs
docs/**
"#;
    // Create .gitignore file within the sandbox
    let gitignore_path = Path::new(&test_dir).join(".gitignore");
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "cat > {} << 'EOF'\n{}EOF",
            gitignore_path.to_str().unwrap(),
            gitignore_contents
        ),
    ])?;

    // Files that SHOULD be ignored
    let ignored_files = vec![
        Path::new(&test_dir).join("foo/file.txt"),
        Path::new(&test_dir).join("sub/foo/file.txt"),
        Path::new(&test_dir).join("deep/nested/foo/file.txt"),
        Path::new(&test_dir).join("error.log"),
        Path::new(&test_dir).join("sub/dir/debug.log"),
        Path::new(&test_dir).join("test/bar"),
        Path::new(&test_dir).join("sub/test/bar"),
        Path::new(&test_dir).join("sub/test/nested/bar"),
        Path::new(&test_dir).join("docs/readme.md"),
        Path::new(&test_dir).join("docs/api/guide.md"),
    ];

    // Files that should NOT be ignored
    let included_files = vec![
        Path::new(&test_dir).join("foo"), // file, not directory
        Path::new(&test_dir).join("foobar/file.txt"),
        Path::new(&test_dir).join("test.txt"),
        Path::new(&test_dir).join("sub/bar"), // not under test directory
        Path::new(&test_dir).join("readme.md"), // not under docs
    ];

    // Create all directories needed
    sandbox.run(&[
        "mkdir",
        "-p",
        Path::new(&test_dir).join("foo").to_str().unwrap(),
        Path::new(&test_dir).join("sub/foo").to_str().unwrap(),
        Path::new(&test_dir)
            .join("deep/nested/foo")
            .to_str()
            .unwrap(),
        Path::new(&test_dir).join("sub/dir").to_str().unwrap(),
        Path::new(&test_dir).join("test").to_str().unwrap(),
        Path::new(&test_dir)
            .join("sub/test/nested")
            .to_str()
            .unwrap(),
        Path::new(&test_dir).join("docs/api").to_str().unwrap(),
        Path::new(&test_dir).join("foobar").to_str().unwrap(),
        Path::new(&test_dir).join("sub").to_str().unwrap(),
    ])?;

    // Create all files
    for file in ignored_files.iter().chain(included_files.iter()) {
        sandbox.run(&["touch", file.to_str().unwrap()])?;
    }

    // Test without --ignored flag
    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    // Verify included files are shown
    for file in &included_files {
        assert!(
            stdout.contains(file.to_str().unwrap()),
            "Expected {} to be included",
            file.display()
        );
    }

    // Verify ignored files are not shown
    for file in &ignored_files {
        assert!(
            !stdout.contains(file.to_str().unwrap()),
            "Expected {} to be ignored",
            file.display()
        );
    }

    Ok(())
}

#[rstest]
fn test_gitignore_hierarchy(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.set_ignored(false); // Don't automatically add --ignored for gitignore tests

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("hierarchy-test");
    std::fs::create_dir_all(Path::new(&test_dir).join("subdir/deep"))?;

    // Root .gitignore
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '*.root' > {}/.gitignore", test_dir),
    ])?;

    // Subdirectory with its own .gitignore
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "echo '*.sub\n!important.sub' > {}/subdir/.gitignore",
            test_dir
        ),
    ])?;

    // Deeper subdirectory
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '*.deep' > {}/subdir/deep/.gitignore", test_dir),
    ])?;

    // Create test files
    let test_files = vec![
        (Path::new(&test_dir).join("file.root"), true), // ignored by root
        (Path::new(&test_dir).join("file.txt"), false), // not ignored
        (Path::new(&test_dir).join("subdir/file.root"), true), // ignored by parent
        (Path::new(&test_dir).join("subdir/file.sub"), true), // ignored by subdir
        (Path::new(&test_dir).join("subdir/important.sub"), false), // negated
        (Path::new(&test_dir).join("subdir/file.txt"), false), // not ignored
        (Path::new(&test_dir).join("subdir/deep/file.root"), true), // ignored by ancestor
        (Path::new(&test_dir).join("subdir/deep/file.sub"), true), // ignored by parent
        (Path::new(&test_dir).join("subdir/deep/file.deep"), true), // ignored by deep
        (Path::new(&test_dir).join("subdir/deep/file.txt"), false), // not ignored
    ];

    for (file, _) in &test_files {
        sandbox.run(&["touch", file.to_str().unwrap()])?;
    }

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    for (file, should_be_ignored) in &test_files {
        if *should_be_ignored {
            assert!(
                !stdout.contains(file.to_str().unwrap()),
                "Expected {} to be ignored",
                file.display()
            );
        } else {
            assert!(
                stdout.contains(file.to_str().unwrap()),
                "Expected {} to be included",
                file.display()
            );
        }
    }

    Ok(())
}

#[rstest]
fn test_gitignore_special_patterns(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.set_ignored(false); // Don't automatically add --ignored for gitignore tests

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("special-patterns-test");
    std::fs::create_dir_all(&test_dir)?;

    let gitignore_contents = r#"
# Character classes
*.[oa]
log[0-9].txt
file[!x].txt

# Escaped characters
\!important.txt
file\ with\ spaces.txt
\#hashtag.txt

# Question mark
config.?ml

# Trailing spaces (should be trimmed)
trailing.txt   

# Patterns with spaces (must be escaped)
path\ with\ spaces/
"#;
    let gitignore_path = Path::new(&test_dir).join(".gitignore");
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "cat > {} << 'EOF'\n{}\nEOF",
            gitignore_path.to_str().unwrap(),
            gitignore_contents
        ),
    ])?;

    // Files that SHOULD be ignored
    let ignored_files = vec![
        Path::new(&test_dir).join("lib.a"),
        Path::new(&test_dir).join("lib.o"),
        Path::new(&test_dir).join("log1.txt"),
        Path::new(&test_dir).join("log9.txt"),
        Path::new(&test_dir).join("filea.txt"),
        Path::new(&test_dir).join("fileb.txt"),
        Path::new(&test_dir).join("!important.txt"),
        Path::new(&test_dir).join("file with spaces.txt"),
        Path::new(&test_dir).join("#hashtag.txt"),
        Path::new(&test_dir).join("config.xml"),
        Path::new(&test_dir).join("config.yml"),
        Path::new(&test_dir).join("trailing.txt"),
        Path::new(&test_dir).join("path with spaces/file.txt"),
    ];

    // Files that should NOT be ignored
    let included_files = vec![
        Path::new(&test_dir).join("lib.c"),
        Path::new(&test_dir).join("logA.txt"),
        Path::new(&test_dir).join("filex.txt"),
        Path::new(&test_dir).join("important.txt"),
        Path::new(&test_dir).join("config.toml"),
        Path::new(&test_dir).join("path/with/spaces/file.txt"),
    ];

    // Create directories
    sandbox.run(&[
        "mkdir",
        "-p",
        Path::new(&test_dir)
            .join("path with spaces")
            .to_str()
            .unwrap(),
    ])?;
    sandbox.run(&[
        "mkdir",
        "-p",
        Path::new(&test_dir)
            .join("path/with/spaces")
            .to_str()
            .unwrap(),
    ])?;

    // Create all files
    for file in ignored_files.iter().chain(included_files.iter()) {
        sandbox.run(&["touch", file.to_str().unwrap()])?;
    }

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    for file in &included_files {
        assert!(
            stdout.contains(file.to_str().unwrap()),
            "Expected {} to be included",
            file.display()
        );
    }

    for file in &ignored_files {
        assert!(
            !stdout.contains(file.to_str().unwrap()),
            "Expected {} to be ignored",
            file.display()
        );
    }

    Ok(())
}

#[rstest]
fn test_gitignore_directory_patterns(
    mut sandbox: SandboxManager,
) -> Result<()> {
    sandbox.set_ignored(false); // Don't automatically add --ignored for gitignore tests

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("directory-patterns-test");
    std::fs::create_dir_all(&test_dir)?;

    let gitignore_contents = r#"
# Directory only (trailing slash)
build/
# File or directory
temp
# Explicitly not a directory
!temp/
# Directory anywhere
**/node_modules/
"#;
    let gitignore_path = Path::new(&test_dir).join(".gitignore");
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "echo '{}' > {}",
            gitignore_contents,
            gitignore_path.to_str().unwrap()
        ),
    ])?;

    // Create directory structure
    sandbox.run(&[
        "mkdir",
        "-p",
        Path::new(&test_dir).join("build").to_str().unwrap(),
        Path::new(&test_dir).join("src/build").to_str().unwrap(),
        Path::new(&test_dir).join("temp").to_str().unwrap(),
        Path::new(&test_dir).join("node_modules").to_str().unwrap(),
        Path::new(&test_dir)
            .join("src/node_modules")
            .to_str()
            .unwrap(),
    ])?;

    let test_files = vec![
        // build/ pattern - only directories named build at the same level as .gitignore
        (Path::new(&test_dir).join("build/file.txt"), true),
        (Path::new(&test_dir).join("src/build/file.txt"), false), // not at root level, so not ignored
        // Note: Can't test file named "build" as directory "build" already exists

        // temp pattern - both files and directories
        // Note: Our implementation treats !temp/ as un-ignoring contents, which differs from Git
        (Path::new(&test_dir).join("temp/file.txt"), false), // Git would ignore this, we don't
        // Note: Can't test file named "temp" as directory "temp" already exists

        // node_modules anywhere
        (Path::new(&test_dir).join("node_modules/package.json"), true),
        (
            Path::new(&test_dir).join("src/node_modules/package.json"),
            true,
        ),
        // Not ignored
        (Path::new(&test_dir).join("src/file.txt"), false),
        (Path::new(&test_dir).join("README.md"), false),
    ];

    // Create files
    for (file, _) in &test_files {
        sandbox.run(&["touch", file.to_str().unwrap()])?;
    }

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    for (file, should_be_ignored) in &test_files {
        if *should_be_ignored {
            assert!(
                !stdout.contains(file.to_str().unwrap()),
                "Expected {} to be ignored",
                file.display()
            );
        } else {
            assert!(
                stdout.contains(file.to_str().unwrap()),
                "Expected {} to be included",
                file.display()
            );
        }
    }

    Ok(())
}

#[rstest]
fn test_gitignore_complex_negation(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.set_ignored(false); // Don't automatically add --ignored for gitignore tests

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("complex-negation-test");
    std::fs::create_dir_all(&test_dir)?;

    let gitignore_contents = r#"
# Ignore everything in logs
logs/*
# But not error logs
!logs/*error*
# But do ignore archived error logs
logs/*error*.gz

# Ignore all .data files
*.data
# Except in the config directory
!config/*.data
# But do ignore backup data files even in config
config/*.data.bak
"#;
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "echo '{}' > {}",
            gitignore_contents,
            Path::new(&test_dir).join(".gitignore").to_str().unwrap()
        ),
    ])?;

    sandbox.run(&[
        "mkdir",
        "-p",
        Path::new(&test_dir).join("logs").to_str().unwrap(),
        Path::new(&test_dir).join("config").to_str().unwrap(),
        Path::new(&test_dir).join("src").to_str().unwrap(),
    ])?;

    let test_files = vec![
        // Logs directory
        (Path::new(&test_dir).join("logs/app.log"), true),
        (Path::new(&test_dir).join("logs/error.log"), false), // negated
        (Path::new(&test_dir).join("logs/error-2023.log"), false), // negated
        (Path::new(&test_dir).join("logs/error.log.gz"), true), // re-ignored
        (Path::new(&test_dir).join("logs/app-error.log.gz"), true), // re-ignored
        // Data files
        (Path::new(&test_dir).join("file.data"), true),
        (Path::new(&test_dir).join("config/settings.data"), false), // negated
        (Path::new(&test_dir).join("config/settings.data.bak"), true), // re-ignored
        (Path::new(&test_dir).join("src/file.data"), true),
    ];

    for (file, _) in &test_files {
        sandbox.run(&["touch", file.to_str().unwrap()])?;
    }

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    for (file, should_be_ignored) in &test_files {
        if *should_be_ignored {
            assert!(
                !stdout.contains(file.to_str().unwrap()),
                "Expected {} to be ignored",
                file.display()
            );
        } else {
            assert!(
                stdout.contains(file.to_str().unwrap()),
                "Expected {} to be included",
                file.display()
            );
        }
    }

    Ok(())
}

/// Verify that the SANDBOX_IGNORED environment variable enables inclusion of
/// ignored files even when the --ignored CLI flag is not provided.
#[rstest]
fn test_env_flag_includes_ignored(mut sandbox: SandboxManager) -> Result<()> {
    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("env-ignore-test");
    std::fs::create_dir_all(&test_dir)?;

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo 'ignored-file' > {}/.gitignore", test_dir),
    ])?;

    let ignored_file = Path::new(&test_dir).join("ignored-file");
    let included_file = Path::new(&test_dir).join("included");

    // Create the files (using --ignored via helper, fine for creation phase)
    sandbox.run(&["touch", ignored_file.to_str().unwrap()])?;
    sandbox.run(&["touch", included_file.to_str().unwrap()])?;

    // Call `status` WITHOUT any env var or flag - should not show ignored files
    sandbox.set_ignored(false); // Temporarily disable automatic --ignored
    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();
    sandbox.set_ignored(true); // Reset back
    assert!(!stdout.contains(ignored_file.to_str().unwrap()));
    assert!(stdout.contains(included_file.to_str().unwrap()));

    // Call `status` WITHOUT the flag but WITH env var set to true
    sandbox.set_ignored(false); // Disable automatic --ignored
    sandbox.run_with_env(&["status", &test_dir], "SANDBOX_IGNORED", "true")?;
    let stdout = sandbox.last_stdout.clone();
    sandbox.set_ignored(true); // Reset back

    // Both files should be reported now
    assert!(stdout.contains(ignored_file.to_str().unwrap()));
    assert!(stdout.contains(included_file.to_str().unwrap()));

    // Invalid value for SANDBOX_IGNORED
    sandbox.set_ignored(false); // Disable automatic --ignored
    let result =
        sandbox.run_with_env(&["status", &test_dir], "SANDBOX_IGNORED", "cow");
    sandbox.set_ignored(true); // Reset back
    assert!(
        result.is_err(),
        "Expected error for invalid SANDBOX_IGNORED value"
    );

    // False value for SANDBOX_IGNORED
    sandbox.set_ignored(false); // Disable automatic --ignored
    sandbox.run_with_env(&["status", &test_dir], "SANDBOX_IGNORED", "false")?;
    let stdout = sandbox.last_stdout.clone();
    sandbox.set_ignored(true); // Reset back
    assert!(!stdout.contains(ignored_file.to_str().unwrap()));
    assert!(stdout.contains(included_file.to_str().unwrap()));

    // Override values for ignored - CLI flag should take precedence over env var
    // This one needs to stay as Command::new because we need to pass --ignored directly
    // and test that it overrides the env var
    sandbox.set_ignored(false); // Disable automatic --ignored
    let output = Command::new("sudo")
        .env("SANDBOX_IGNORED", "false")
        .args(["-E", &sandbox.sandbox_bin])
        .args([
            "-v",
            "--ignored",
            &format!("--name={}", &sandbox.name),
            "status",
            &test_dir,
        ])
        .output()?;
    assert!(output.status.success());
    // Both files should be reported because cli takes precedence over env var
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(ignored_file.to_str().unwrap()));
    assert!(stdout.contains(included_file.to_str().unwrap()));
    sandbox.set_ignored(true); // Reset back

    Ok(())
}

#[rstest]
fn test_builtin_tmp_ignore(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.set_ignored(false); // Don't automatically add --ignored for gitignore tests

    let test_dir = sandbox.test_filename("builtin_ignore");
    fs::create_dir_all(&test_dir)?;
    let ignore_id = rid();
    let ignored_file = format!("{}/builtin-ignore-{}", test_dir, ignore_id);

    sandbox.run(&["touch", &ignored_file])?;

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    assert!(!stdout.contains(&format!("builtin-ignore-{}", ignore_id)));

    // with --ignored
    sandbox.set_ignored(true);
    sandbox.run(&["status", &test_dir])?;
    let stdout_flag = sandbox.last_stdout.clone();
    sandbox.set_ignored(false);

    assert!(stdout_flag.contains(&format!("builtin-ignore-{}", ignore_id)));

    Ok(())
}

#[rstest]
fn test_gitignore_edge_cases(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.set_ignored(false); // Don't automatically add --ignored for gitignore tests

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("edge-cases-test");
    std::fs::create_dir_all(&test_dir)?;

    let gitignore_contents = r#"
# Empty lines and comments should be ignored


    # Indented comment
    
# Patterns with leading/trailing whitespace (should be trimmed)
    spaced.txt    
	tabbed.txt	

# Literal backslash at end
literal\\

# Multiple consecutive slashes
path//to///file

# Dots in patterns
.hidden
..double
...triple

# Very long pattern (test buffer limits)
very/long/path/that/goes/on/and/on/and/on/and/on/and/on/and/on/file.txt

# Unicode in patterns (if supported)
файл.txt
文件.txt

# Patterns that look like negations but aren't
\!not-a-negation

# block the file, but then allow
*allow-file-that-starts-with-a-bang
!!allow-file-that-starts-with-a-bang
"#;
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "cat > {} << 'EOF'\n{}\nEOF",
            Path::new(&test_dir).join(".gitignore").to_str().unwrap(),
            gitignore_contents
        ),
    ])?;

    // Create nested directories for path normalization test
    sandbox.run(&["mkdir", "-p",
        Path::new(&test_dir).join("path/to").to_str().unwrap(),
        Path::new(&test_dir).join("very/long/path/that/goes/on/and/on/and/on/and/on/and/on/and/on").to_str().unwrap()
    ])?;

    let test_files = vec![
        // Should be ignored
        (Path::new(&test_dir).join("spaced.txt"), true),
        (Path::new(&test_dir).join("tabbed.txt"), true),
        (Path::new(&test_dir).join("literal\\"), true),
        (Path::new(&test_dir).join("path/to/file"), true), // matches normalized path
        (Path::new(&test_dir).join(".hidden"), true),
        (Path::new(&test_dir).join("..double"), true),
        (Path::new(&test_dir).join("...triple"), true),
        (
            Path::new(&test_dir).join("very/long/path/that/goes/on/and/on/and/on/and/on/and/on/and/on/file.txt"),
            true,
        ),
        (Path::new(&test_dir).join("файл.txt"), true),
        (Path::new(&test_dir).join("文件.txt"), true),
        (Path::new(&test_dir).join("!not-a-negation"), true),
        // Should NOT be ignored
        (Path::new(&test_dir).join("!allow-file-that-starts-with-a-bang"), false),
        (Path::new(&test_dir).join("normal.txt"), false),
        (Path::new(&test_dir).join("path/file"), false),
        (Path::new(&test_dir).join("visible"), false),
    ];

    for (file, _) in &test_files {
        sandbox.run(&["touch", file.to_str().unwrap()])?;
    }

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    for (file, should_be_ignored) in &test_files {
        if *should_be_ignored {
            assert!(
                !stdout.contains(file.to_str().unwrap()),
                "Expected {} to be ignored",
                file.display()
            );
        } else {
            assert!(
                stdout.contains(file.to_str().unwrap()),
                "Expected {} to be included",
                file.display()
            );
        }
    }

    Ok(())
}

#[rstest]
fn test_gitignore_case_sensitivity(mut sandbox: SandboxManager) -> Result<()> {
    sandbox.set_ignored(false); // Don't automatically add --ignored for gitignore tests

    // Create parent .gitignore to un-ignore test directory
    let parent_gitignore_path =
        format!("generated-test-data/{}/.gitignore", &sandbox.name);
    sandbox.run(&[
        "sh",
        "-c",
        &format!("echo '!*/' > {}", parent_gitignore_path),
    ])?;

    // Create test directory using test_filename
    let test_dir = sandbox.test_filename("case-sensitivity-test");
    std::fs::create_dir_all(&test_dir)?;

    // Git ignore patterns are case-sensitive on case-sensitive filesystems
    let gitignore_contents = r#"
# Case sensitive patterns
README.md
*.TXT
Build/
"#;
    let gitignore_path = Path::new(&test_dir).join(".gitignore");
    sandbox.run(&[
        "sh",
        "-c",
        &format!(
            "echo '{}' > {}",
            gitignore_contents,
            gitignore_path.to_str().unwrap()
        ),
    ])?;

    sandbox.run(&[
        "mkdir",
        "-p",
        Path::new(&test_dir).join("Build").to_str().unwrap(),
        Path::new(&test_dir).join("build").to_str().unwrap(),
    ])?;

    let test_files = vec![
        // Should be ignored (exact case match)
        (Path::new(&test_dir).join("README.md"), true),
        (Path::new(&test_dir).join("file.TXT"), true),
        (Path::new(&test_dir).join("Build/output"), true),
        // Should NOT be ignored (different case)
        (Path::new(&test_dir).join("readme.md"), false),
        (Path::new(&test_dir).join("Readme.md"), false),
        (Path::new(&test_dir).join("README.MD"), false),
        (Path::new(&test_dir).join("file.txt"), false),
        (Path::new(&test_dir).join("file.Txt"), false),
        (Path::new(&test_dir).join("build/output"), false),
    ];

    for (file, _) in &test_files {
        sandbox.run(&["touch", file.to_str().unwrap()])?;
    }

    sandbox.run(&["status", &test_dir])?;
    let stdout = sandbox.last_stdout.clone();

    for (file, should_be_ignored) in &test_files {
        if *should_be_ignored {
            assert!(
                !stdout.contains(file.to_str().unwrap()),
                "Expected {} to be ignored",
                file.display()
            );
        } else {
            assert!(
                stdout.contains(file.to_str().unwrap()),
                "Expected {} to be included",
                file.display()
            );
        }
    }

    Ok(())
}
