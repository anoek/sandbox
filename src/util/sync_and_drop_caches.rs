use anyhow::Result;
use std::fs::File;
use std::io::Write;

/*
 * OverlayFS does not officially support modifying files in either the lower or upper layer while
 * the filesystem is mounted. However by syncing and dropping caches before and after making
 * changes we seem to be able to get away with it in a safe-enough-for-our-purposes kind of way.
 *
 * This function should be called before and after any changes to the upper file system.
 */

pub fn sync_and_drop_caches() -> Result<()> {
    nix::unistd::sync();
    File::create("/proc/sys/vm/drop_caches")?.write_all(b"2")?;
    Ok(())
}
