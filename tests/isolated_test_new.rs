mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_new_last_name_with_ms() -> Result<()> {
    let mut sandboxes: Vec<SandboxManager> = Vec::new();

    for _ in 0..6 {
        let mut sandbox: SandboxManager = SandboxManager::new();
        sandbox.no_default_options = true;
        sandboxes.push(sandbox);
    }

    for sandbox in &mut sandboxes {
        // We spin here to test to ensure we've exercised the code that
        // dedupes names at the ms level. 6 times in case this test case
        // is crossing a second-boundary (the clock second is rolling over that is)

        assert!(sandbox.epass(
            &["--new", "-v", "true"],
            "TEST_NAME_WITH_MS",
            "sandbox-coverage-new-with-ms"
        ));
        sandbox.run(&["--last", "config", "name"])?;
        sandbox.name = sandbox.last_stdout.trim().to_string();

        println!("sandbox.name: {}", sandbox.name);
    }

    Ok(())
}
