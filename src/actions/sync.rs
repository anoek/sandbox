use crate::{
    outln,
    util::{set_json_output, sync_and_drop_caches},
};
use anyhow::{Context, Result};
use serde_json::json;

pub fn sync() -> Result<()> {
    sync_and_drop_caches().context("Failed to synchronize host changes")?;
    set_json_output("sync", &json!("success"));
    outln!("Synchronized host changes");
    Ok(())
}
