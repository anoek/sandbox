/*
 * This file contains the heart of the logic that deals with extracting changes from the overlayfs
 * filesystem. See the overlayfs documentation for more details.
 *
 * https://docs.kernel.org/filesystems/overlayfs.html
 *
 * */

pub mod change_entries;
pub mod changes;
pub mod file_details;

pub use change_entries::*;
pub use file_details::*;
