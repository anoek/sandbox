// Integration tests for SandboxSettings
// These tests verify that settings serialization, validation, and file operations work correctly

mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use serde_json;
use std::{fs, path::PathBuf};

use ::sandbox::config::{BindMount, BindMountOptions, Network};
use ::sandbox::sandbox::{SandboxSettings, mount_overlays::MountHash};
use std::collections::HashMap;

// Helper to create a minimal Config for testing
fn create_test_config() -> ::sandbox::config::Config {
    ::sandbox::config::Config {
        log_level: log::LevelFilter::Info,
        name: "test_sandbox".to_string(),
        storage_dir: PathBuf::from("/tmp/test_storage"),
        sandbox_dir: PathBuf::from("/tmp/test_sandbox"),
        upper_cwd: PathBuf::from("/tmp/test_upper"),
        overlay_cwd: PathBuf::from("/tmp/test_overlay"),
        net: Network::None,
        sources: HashMap::new(),
        ignored: false,
        bind_mounts: Vec::new(),
        no_default_binds: false,
        config_files: Vec::new(),
    }
}

#[rstest]
fn test_from_config() -> Result<()> {
    let mut config = create_test_config();
    config.net = Network::Host;
    
    let bind_mount = BindMount {
        source: PathBuf::from("/source"),
        target: PathBuf::from("/target"),
        options: BindMountOptions::ReadOnly,
        argument: "test_bind".to_string(),
    };
    config.bind_mounts = vec![bind_mount.clone()];

    let mounts = vec![MountHash {
        hash: "test_hash".to_string(),
        dir: "/test/dir".to_string(),
    }];

    let settings = SandboxSettings::from_config(&config, &mounts);

    assert_eq!(settings.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(settings.mounts, mounts);
    assert_eq!(settings.network, Network::Host);
    assert_eq!(settings.bind_mounts, vec![bind_mount]);

    Ok(())
}

#[rstest]
fn test_user_bind_mounts_filtering_through_validation() -> Result<()> {
    let mut config = create_test_config();
    
    let user_bind = BindMount {
        source: PathBuf::from("/user/source"),
        target: PathBuf::from("/user/target"),
        options: BindMountOptions::ReadWrite,
        argument: "user_bind".to_string(),
    };
    
    let data_bind = BindMount {
        source: PathBuf::from("/data/source"),
        target: PathBuf::from("/data/target"),
        options: BindMountOptions::ReadOnly,
        argument: "data".to_string(),
    };
    
    config.bind_mounts = vec![user_bind.clone(), data_bind.clone()];
    let settings = SandboxSettings::from_config(&config, &[]);
    
    // Test that "data" bind mount is filtered out by trying validation with only data bind
    let mut config_with_data_only = create_test_config();
    config_with_data_only.bind_mounts = vec![data_bind];
    
    let result = settings.validate_against_config(&config_with_data_only, &[]);
    
    // Should fail because user_bind was removed, proving "data" bind is ignored
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Bind mount configuration has changed"));
    assert!(error_msg.contains("user_bind"));
    
    Ok(())
}

#[rstest]
fn test_save_and_load_from_file() -> Result<()> {
    let sandbox = SandboxManager::new();
    let test_dir = format!("generated-test-data/settings-{}", sandbox.name);
    fs::create_dir_all(&test_dir)?;
    let settings_path = PathBuf::from(format!("{}/settings.json", test_dir));

    let mut config = create_test_config();
    config.net = Network::None;
    
    let bind_mount = BindMount {
        source: PathBuf::from("/test/source"),
        target: PathBuf::from("/test/target"),
        options: BindMountOptions::Mask,
        argument: "test_mount".to_string(),
    };
    config.bind_mounts = vec![bind_mount.clone()];

    let mounts = vec![
        MountHash {
            hash: "hash1".to_string(),
            dir: "/dir1".to_string(),
        },
        MountHash {
            hash: "hash2".to_string(),
            dir: "/dir2".to_string(),
        },
    ];

    let original_settings = SandboxSettings::from_config(&config, &mounts);

    // Test save_to_file
    original_settings.save_to_file(&settings_path)?;
    
    // Verify file was created and contains valid JSON
    assert!(settings_path.exists());
    let file_content = fs::read_to_string(&settings_path)?;
    let _: serde_json::Value = serde_json::from_str(&file_content)?; // Verify it's valid JSON

    // Test load_from_file
    let loaded_settings = SandboxSettings::load_from_file(&settings_path)?;
    
    assert_eq!(loaded_settings, original_settings);
    assert_eq!(loaded_settings.network, Network::None);
    assert_eq!(loaded_settings.mounts.len(), 2);
    assert_eq!(loaded_settings.bind_mounts.len(), 1);
    assert_eq!(loaded_settings.bind_mounts[0].argument, "test_mount");

    Ok(())
}

#[rstest]
fn test_load_from_nonexistent_file() -> Result<()> {
    let nonexistent_path = PathBuf::from("/nonexistent/path/settings.json");
    
    let result = SandboxSettings::load_from_file(&nonexistent_path);
    
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Settings file does not exist"));
    
    Ok(())
}

#[rstest]
fn test_load_from_invalid_json_file() -> Result<()> {
    let sandbox = SandboxManager::new();
    let test_dir = format!("generated-test-data/invalid-json-{}", sandbox.name);
    fs::create_dir_all(&test_dir)?;
    let settings_path = PathBuf::from(format!("{}/invalid.json", test_dir));

    // Write invalid JSON
    fs::write(&settings_path, "{ invalid json }")?;
    
    let result = SandboxSettings::load_from_file(&settings_path);
    
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Failed to parse settings.json"));
    
    Ok(())
}

#[rstest]
fn test_save_to_invalid_path() -> Result<()> {
    let config = create_test_config();
    let settings = SandboxSettings::from_config(&config, &[]);
    
    let invalid_path = PathBuf::from("/nonexistent/directory/settings.json");
    
    let result = settings.save_to_file(&invalid_path);
    
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Failed to write settings.json"));
    
    Ok(())
}

#[rstest]
fn test_validate_against_config_identical() -> Result<()> {
    let mut config = create_test_config();
    config.net = Network::Host;
    
    let bind_mount = BindMount {
        source: PathBuf::from("/source"),
        target: PathBuf::from("/target"),
        options: BindMountOptions::ReadOnly,
        argument: "test_bind".to_string(),
    };
    config.bind_mounts = vec![bind_mount];

    let mounts = vec![MountHash {
        hash: "test_hash".to_string(),
        dir: "/test/dir".to_string(),
    }];

    let settings = SandboxSettings::from_config(&config, &mounts);
    
    // Validation against the same config should succeed
    let result = settings.validate_against_config(&config, &mounts);
    assert!(result.is_ok());

    Ok(())
}

#[rstest]
fn test_validate_against_config_network_change() -> Result<()> {
    let mut original_config = create_test_config();
    original_config.net = Network::Host;
    
    let mounts = vec![MountHash {
        hash: "test_hash".to_string(),
        dir: "/test/dir".to_string(),
    }];

    let settings = SandboxSettings::from_config(&original_config, &mounts);
    
    // Change network configuration
    let mut new_config = original_config.clone();
    new_config.net = Network::None;
    
    let result = settings.validate_against_config(&new_config, &mounts);
    
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Network configuration has changed"));
    assert!(error_msg.contains("Host"));
    assert!(error_msg.contains("None"));
    assert!(error_msg.contains("Please stop existing sandbox to change network settings"));

    Ok(())
}

#[rstest]
fn test_validate_against_config_mounts_removed() -> Result<()> {
    let config = create_test_config();
    
    let original_mounts = vec![
        MountHash {
            hash: "hash1".to_string(),
            dir: "/dir1".to_string(),
        },
        MountHash {
            hash: "hash2".to_string(),
            dir: "/dir2".to_string(),
        },
    ];

    let settings = SandboxSettings::from_config(&config, &original_mounts);
    
    // Remove one mount
    let new_mounts = vec![MountHash {
        hash: "hash1".to_string(),
        dir: "/dir1".to_string(),
    }];
    
    let result = settings.validate_against_config(&config, &new_mounts);
    
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Mount configuration has changed"));
    assert!(error_msg.contains("Removed mounts"));
    assert!(error_msg.contains("/dir2"));
    assert!(error_msg.contains("Please stop the existing sandbox to change mount settings"));

    Ok(())
}

#[rstest]
fn test_validate_against_config_mounts_added() -> Result<()> {
    let config = create_test_config();
    
    let original_mounts = vec![MountHash {
        hash: "hash1".to_string(),
        dir: "/dir1".to_string(),
    }];

    let settings = SandboxSettings::from_config(&config, &original_mounts);
    
    // Add another mount
    let new_mounts = vec![
        MountHash {
            hash: "hash1".to_string(),
            dir: "/dir1".to_string(),
        },
        MountHash {
            hash: "hash2".to_string(),
            dir: "/dir2".to_string(),
        },
    ];
    
    let result = settings.validate_against_config(&config, &new_mounts);
    
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Mount configuration has changed"));
    assert!(error_msg.contains("Added mounts"));
    assert!(error_msg.contains("/dir2"));

    Ok(())
}

#[rstest]
fn test_validate_against_config_bind_mounts_removed() -> Result<()> {
    let mut original_config = create_test_config();
    
    let bind1 = BindMount {
        source: PathBuf::from("/source1"),
        target: PathBuf::from("/target1"),
        options: BindMountOptions::ReadOnly,
        argument: "bind1".to_string(),
    };
    
    let bind2 = BindMount {
        source: PathBuf::from("/source2"),
        target: PathBuf::from("/target2"),
        options: BindMountOptions::ReadWrite,
        argument: "bind2".to_string(),
    };
    
    original_config.bind_mounts = vec![bind1.clone(), bind2];
    
    let settings = SandboxSettings::from_config(&original_config, &[]);
    
    // Remove one bind mount
    let mut new_config = original_config.clone();
    new_config.bind_mounts = vec![bind1];
    
    let result = settings.validate_against_config(&new_config, &[]);
    
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Bind mount configuration has changed"));
    assert!(error_msg.contains("Removed bind mounts"));
    assert!(error_msg.contains("bind2"));
    assert!(error_msg.contains("Please stop the existing sandbox to change bind mount settings"));

    Ok(())
}

#[rstest]
fn test_validate_against_config_bind_mounts_added() -> Result<()> {
    let mut original_config = create_test_config();
    
    let bind1 = BindMount {
        source: PathBuf::from("/source1"),
        target: PathBuf::from("/target1"),
        options: BindMountOptions::ReadOnly,
        argument: "bind1".to_string(),
    };
    
    original_config.bind_mounts = vec![bind1.clone()];
    
    let settings = SandboxSettings::from_config(&original_config, &[]);
    
    // Add another bind mount
    let bind2 = BindMount {
        source: PathBuf::from("/source2"),
        target: PathBuf::from("/target2"),
        options: BindMountOptions::Mask,
        argument: "bind2".to_string(),
    };
    
    let mut new_config = original_config.clone();
    new_config.bind_mounts = vec![bind1, bind2];
    
    let result = settings.validate_against_config(&new_config, &[]);
    
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Bind mount configuration has changed"));
    assert!(error_msg.contains("Added bind mounts"));
    assert!(error_msg.contains("bind2"));

    Ok(())
}

#[rstest]
fn test_validate_against_config_data_bind_mount_ignored() -> Result<()> {
    let mut original_config = create_test_config();
    
    let user_bind = BindMount {
        source: PathBuf::from("/user/source"),
        target: PathBuf::from("/user/target"),
        options: BindMountOptions::ReadOnly,
        argument: "user_bind".to_string(),
    };
    
    let data_bind = BindMount {
        source: PathBuf::from("/data/source"),
        target: PathBuf::from("/data/target"),
        options: BindMountOptions::ReadWrite,
        argument: "data".to_string(),
    };
    
    original_config.bind_mounts = vec![user_bind.clone(), data_bind.clone()];
    
    let settings = SandboxSettings::from_config(&original_config, &[]);
    
    // New config has only the data bind mount (user bind removed)
    let mut new_config = original_config.clone();
    new_config.bind_mounts = vec![data_bind];
    
    let result = settings.validate_against_config(&new_config, &[]);
    
    // Should fail because user_bind was removed, even though data bind is ignored
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Bind mount configuration has changed"));
    assert!(error_msg.contains("Removed bind mounts"));
    assert!(error_msg.contains("user_bind"));

    Ok(())
}

#[rstest]
fn test_validate_against_config_multiple_changes() -> Result<()> {
    let mut original_config = create_test_config();
    
    let mount1 = MountHash {
        hash: "hash1".to_string(),
        dir: "/dir1".to_string(),
    };
    
    let mount2 = MountHash {
        hash: "hash2".to_string(),
        dir: "/dir2".to_string(),
    };
    
    let bind1 = BindMount {
        source: PathBuf::from("/bind1"),
        target: PathBuf::from("/target1"),
        options: BindMountOptions::ReadOnly,
        argument: "bind1".to_string(),
    };
    
    original_config.bind_mounts = vec![bind1.clone()];
    original_config.net = Network::Host;
    
    let original_mounts = vec![mount1, mount2];
    let settings = SandboxSettings::from_config(&original_config, &original_mounts);
    
    // Change everything
    let mut new_config = original_config.clone();
    new_config.net = Network::None; // This should be the first error
    new_config.bind_mounts = vec![]; // Remove bind mount
    let new_mounts = vec![]; // Remove all mounts
    
    let result = settings.validate_against_config(&new_config, &new_mounts);
    
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    // Should fail on network change first
    assert!(error_msg.contains("Network configuration has changed"));

    Ok(())
}