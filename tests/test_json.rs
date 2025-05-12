mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_json(mut sandbox: SandboxManager) -> Result<()> {
    let filename = sandbox.test_filename("file");
    sandbox.run(&["touch", &filename])?;
    sandbox.run(&["--json", "status", &filename])?;

    let json_output = sandbox.last_stdout.clone();
    let json: serde_json::Value = serde_json::from_str(&json_output)?;
    println!("json: {}", json);
    println!("changes: {}", json["changes"].as_array().unwrap().len());
    println!("changes[0]: {}", json["changes"][0]);

    assert_eq!(json["changes"].as_array().unwrap().len(), 1);
    assert!(
        json["changes"][0]["destination"]
            .to_string()
            .contains(&filename)
    );

    Ok(())
}
