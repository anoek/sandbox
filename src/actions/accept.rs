/*
= Sandbox Acceptance Algorithm

==  Notes on OverlayFS

-   Attributes on files in the upper filesystem can be manually inspected using
    `getfattr`, such as `getfattr -h -d -m 'trusted.*' <filenames>`

-   When a file or directory is deleted either a special character device with
    major `0` and minor `0` is created in place of the file in the upper fs,
    denoting the file has been removed, or alternatively a zero length file with
    the attribute `trusted.overlay.whiteout` set. These both mean the same
    thing, that the path has been removed, and is called a `whiteout`.

-   When a directory is moved the `trusted.overlay.redirect` attribute is set to
    the path of the original file or directory, and a `whiteout` file is created
    at the location of the original source path.

-   If a new directory is created in place of one that has been removed or
    renamed, the `trusted.overlay.opaque` attribute will be set, indicating that
    the entire contents of that directory is "new" and the source should have
    either been renamed or needs to be deleted.

-   If the path contents has not been changed by attributes have been
    `trusted.overlayfs.metacopy` will be set. This is only true if we've mounted
    with `metacopy=on`. Note, at the time of writing we always mount with `metacopy=off`
    for two reasons, 1) there's a edge case potential security issue with mounting
    with it on (see https://docs.kernel.org/filesystems/overlayfs.html ), and 2)
    it keeps our implementation simpler as we don't have to deal with it, instead
    overlayfs will make a full copy whenever anything changes.

-   The `upper` file system for OverlayFS is a directory that contains all of the
    changes made. This is the copy on write part of OverlayFS, from this we'll
    derive our change set. The `lower` file system is our host file system where
    any unchanged files are derived from.

-   We have separate OverlayFS mounts that correspond to each normal mount on the
    system. For example, if we have three partitions, one for `/`, one for `/home`,
    and one for `/var`, we have three OverlayFS mounts, one for each. Wherever
    we store our sandbox changes we will have three directories, `upper`, `work`,
    and `overlay`, each will have a corresponding base32 encoded directory of the
    mount point name, so `F4` for `/`, `F5UG63LF` for `/home`, and so forth.

== Calculating changes and accepting them

Our objective is two fold: 1) Provide the user with an itemized list of changes
for review, and 2) safely apply those changes when instructed to do so.

The primary complexity we deal with in constructing the change sets is dealing
with moving directories. With moves, parents can become children, can replace
other directories by the same name of other directories on the lower file
system, the sources can be replaced by other directories, and in general get
fairly twisted around.

The basic gist of how we approach a merge is to remove all of the files that
need removing, then move all of the directories and files that are going to move out
of the way into temporary locations in a flat structure, then finally walk through
the list of changes creating files directories and moving the previously moved
paths to their final destination.

For more details read on.

=== Calculating the changes

For any given change entry we have three paths and a temporary path we consider:

-   The staged path, the path that's in the upper file system.
-   The destination path, the path that the file or directory will end up at after any moves.
-   The source path, the path that the file or directory is currently at before any moves.
-   Finally a temporary path, which we use during the move process described below.

To construct our change set we scan through all of the changes in the upper file
system and find all of the redirects/renames, constructing a set of all of the
source files they came from, call this `renamed_paths`.

We then make a second pass of all files in the upper file system to make change entries:

-   If a path is a whiteout path, the path is an opaque directory, or the path has changed from a
    directory to non directory
    -   If the source path is in `renamed_paths` do nothing (as we are moving the file, not
        removing it)
    -   If the source path is not in `renamed_paths`, create a `Remove` entry for the path,
        and if it is a directory, remove entries for all sub-paths (not following symlinks).
-   Else if a path has been renamed, generate a `Renamed` entry for it
-   Otherwise we are either creating a new file or modifying an existing one, judged based on
    whether we have a corresponding source path for the staged path or not.

=== Applying the changes

Once we have a change set we can apply it.

1. In descending path order (most specific paths first) we process the `Remove` entries in the
   list.
2. In descending path order we then move `Renamed` entries to their temporary locations.
3. In ascending path order (least specific paths first) process our changes and renames again.
    - If we have a `Set` we copy the file from the upper to a temporary path in the lower
      filesystem, then rename it to its final destination. (The two step process is just so we have
      atomic moves).
    - If we have a `Renamed` entry we move the file or directory from the temporary path in step #2
      to its final destination.
*/

use crate::outln;
use crate::sandbox::changes::changes::{by_destination, by_reverse_source};
use crate::sandbox::changes::{EntryOperation, FileDetails};
use crate::util::{find_mount_point, sync_and_drop_caches};
use crate::{config::Config, sandbox::Sandbox};
use anyhow::Result;
use colored::*;
use log::{debug, error, info, trace};
use nix::fcntl::AtFlags;
use nix::sys::stat::{FchmodatFlags, Mode, fchmodat};
use nix::unistd::{Gid, Uid, fchownat};
use std::fs;
use std::path::{Path, PathBuf};

/* The accept function will accept the changes from the sandbox into the real filesystem. */
pub fn accept(
    config: &Config,
    sandbox: &Sandbox,
    patterns: &[String],
) -> Result<()> {
    trace!("Accepting changes from sandbox {}", sandbox.name);

    let cwd = std::env::current_dir()?;
    let all_changes = sandbox.changes(config)?;

    let mut changes = all_changes.matching(&cwd, patterns);
    let non_matching_count = all_changes.len() - changes.len();

    if !changes.is_empty() {
        let mut accepted_count = 0;
        for pretend in [true, false] {
            let mut deferred_stage_removals = Vec::new();

            /* Process removes */
            changes.sort_by(by_reverse_source);
            for change in changes.iter() {
                if let EntryOperation::Remove = &change.operation {
                    let source = change.source.as_ref().ok_or_else(||
                        anyhow::anyhow!("Remove for file {} didn't match an existing path to remove", change.destination.display())
                    )?;

                    if source.is_file() || source.is_symlink() {
                        if !pretend {
                            rm(&source.path, false)?;
                            accepted_count += 1;
                        }
                    } else if source.is_dir() {
                        if !pretend {
                            rmdir(&source.path)?;
                            accepted_count += 1;
                        }
                    } else {
                        return Err(anyhow::anyhow!(
                            "Error removing {}: cowardly refusing to remove special file",
                            source.path.display()
                        ));
                    }
                    if !pretend {
                        if let Some(staged) = &change.staged {
                            deferred_stage_removals.push(staged.path.clone());
                        }
                    }
                }
            }

            /* Prepare any pending moves by pre-moving them to a flat file structure */
            changes.sort_by(by_reverse_source);
            let pending_renames = changes
                .iter_mut()
                .filter_map(|entry| {
                    if entry.operation == EntryOperation::Rename {
                        let new_name =
                            entry.destination.file_name().unwrap_or_default();
                        entry.source.as_ref().map(|source| {
                            let old_name =
                                source.path.file_name().unwrap_or_default();
                            let tmp_path = cwd.join(format!(
                                ".rename-{}-to-{}-{}",
                                old_name.to_str().unwrap_or(""),
                                new_name.to_str().unwrap_or(""),
                                uuid::Uuid::new_v4()
                            ));
                            entry.tmp_path = Some(tmp_path.clone());

                            struct Rename {
                                old_path: PathBuf,
                                tmp_path: PathBuf,
                                new_path: PathBuf,
                            }

                            Rename {
                                old_path: source.path.clone(),
                                tmp_path,
                                new_path: entry.destination.clone(),
                            }
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            if pretend {
                info!(
                    "Pre-moving {} files to a temporary flat file structure",
                    pending_renames.len()
                );
            }

            //pending_renames.sort_by(|a, b| b.old_path.cmp(&a.old_path));

            for change in pending_renames {
                if pretend {
                    trace!("Preparing to rename {}", change.old_path.display());
                    trace!("   -> {}", change.tmp_path.display());
                    trace!("   -> {}", change.new_path.display());

                    let mp1 = find_mount_point(change.old_path.clone())?;
                    let mp2 = find_mount_point(change.tmp_path.clone())?;
                    let mp3 = find_mount_point(change.new_path.clone())?;

                    /* This really shouldn't happen as the cross mount handling is
                     * handled by removing old files and adding the new ones, so
                     * we should never get here. I feel weird not having this sanity
                     * check though, so here it stays and we'll stub in a case for
                     * coverage testing. */
                    #[cfg(feature = "coverage")]
                    let mp1 = if std::env::var_os(
                        "TEST_RENAME_ACROSS_MOUNT_POINTS_FAILURE",
                    )
                    .is_some()
                    {
                        trace!(
                            "TEST_RENAME_ACROSS_MOUNT_POINTS_FAILURE is set, returning non-existent path"
                        );
                        PathBuf::from("/non-existent-path")
                    } else {
                        mp1
                    };

                    if mp1 != mp2 || mp2 != mp3 {
                        error!(
                            "Cannot accept rename {} because it crosses a mount point",
                            change.old_path.display()
                        );
                        return Err(anyhow::anyhow!(
                            "Cannot accept rename {} because it crosses a mount point",
                            change.old_path.display()
                        ));
                    }
                } else {
                    trace!(
                        "Renaming {} to {}",
                        change.old_path.display(),
                        change.tmp_path.display()
                    );
                    fs::rename(&change.old_path, &change.tmp_path)?;
                }
            }

            /* Proceed with the normal acceptance logic */
            changes.sort_by(by_destination);
            let mut json_output = Vec::new();
            for change in changes.iter_mut() {
                if !pretend {
                    json_output.push(change.to_json());
                }
                let destination = &change.destination;
                let staged = &change.staged;

                #[cfg(feature = "coverage")]
                let staged: &Option<FileDetails> =
                    if std::env::var_os("TEST_NO_STAGED_FILE").is_some() {
                        &None
                    } else {
                        staged
                    };

                #[cfg(feature = "coverage")]
                let staged =
                    if std::env::var_os("TEST_BAD_STAGED_FILE").is_some() {
                        let mut ret = staged.clone().unwrap();
                        ret.stat.st_mode = 0o0;
                        &Some(ret)
                    } else {
                        staged
                    };

                match &change.operation {
                    EntryOperation::Set(_) => {
                        if let Some(staged) = staged {
                            if staged.is_file() {
                                let extension =
                                    uuid::Uuid::new_v4().to_string();
                                let tmp_path =
                                    destination.with_extension(extension);
                                if !pretend {
                                    cp(&staged.path, &tmp_path)?;
                                    mv(&tmp_path, destination)?;
                                    set_permissions(destination, staged)?;
                                    deferred_stage_removals
                                        .push(staged.path.clone());
                                    accepted_count += 1;
                                }
                            } else if staged.is_symlink() {
                                if !pretend {
                                    info!(
                                        "Accepting symlink {} as file",
                                        staged.path.display()
                                    );
                                    let target = fs::read_link(&staged.path)?;
                                    if FileDetails::from_path(destination)?
                                        .is_some()
                                    {
                                        /* At this point we can count on it being a file like thing
                                         * as our changes logic generates removes for directories. */
                                        debug!("rm {}", destination.display());
                                        fs::remove_file(destination)?;
                                    }
                                    ln_s(&target, destination)?;
                                    set_permissions(destination, staged)?;
                                    deferred_stage_removals
                                        .push(staged.path.clone());
                                    accepted_count += 1;
                                }
                            } else if staged.is_dir() {
                                if !pretend {
                                    let is_already_dir = destination.exists()
                                        && destination.is_dir();
                                    if !is_already_dir {
                                        mkdir(destination)?;
                                    }
                                    set_permissions(destination, staged)?;
                                    deferred_stage_removals
                                        .push(staged.path.clone());
                                    accepted_count += 1;
                                }
                            } else {
                                // this should be unreachable as changes should be generating
                                // Error's not Set's for these, but just in case
                                return Err(anyhow::anyhow!(
                                    "{} - Error accepting {} file, we cannot move these. Please resolve this manually first.",
                                    destination.display(),
                                    staged.display_type()
                                ));
                            }
                        } else {
                            // this should be unreachable, but just in case
                            return Err(anyhow::anyhow!(
                                "{} - invalid staged file",
                                destination.display()
                            ));
                        }
                    }
                    EntryOperation::Remove => {
                        /* We've already done the removes in the first pass, nothing more to do here */
                    }
                    EntryOperation::Rename => {
                        if !pretend {
                            let tmp_path = &change.tmp_path
                                .as_ref()
                                .ok_or_else(|| anyhow::anyhow!("Error completing rename: tmp_path is None"))?;
                            info!(
                                "Completing rename {} to {}",
                                tmp_path.display(),
                                destination.display()
                            );
                            mv(tmp_path, destination)?;
                            if let Some(staged) = &change.staged {
                                deferred_stage_removals
                                    .push(staged.path.clone());
                            }
                            accepted_count += 1;
                        }
                    }
                    EntryOperation::Error(error) => {
                        return Err(anyhow::anyhow!(
                            "{} - {}",
                            destination.display(),
                            error
                        ));
                    }
                }
            }

            if !pretend && !deferred_stage_removals.is_empty() {
                deferred_stage_removals.sort_by(|a, b| b.cmp(a));
                deferred_stage_removals.dedup();
                for path in deferred_stage_removals.iter() {
                    debug!("preparing to remove {}", path.display());
                }

                info!("Cleaning up deferred directory removals");
                for path in deferred_stage_removals {
                    if !path.is_symlink() && path.is_dir() {
                        debug!("rmdir_recursive {}", path.display());
                        rmdir_recursive(&path)?;
                    } else {
                        rm(&path, true)?;
                    }
                }
            }
        }

        if accepted_count > 0 {
            outln!("\n{} changes accepted\n", accepted_count);
        }
    } else {
        outln!("\nNo changes in this directory to accept\n");
    }

    if non_matching_count > 0 {
        outln!(
            "\n{} external or non-matching not accepted\n",
            non_matching_count
        );
    }

    outln!("\n");

    sync_and_drop_caches()?;

    Ok(())
}

fn set_permissions(path: &Path, staged: &FileDetails) -> Result<()> {
    chown(path, staged.stat.st_uid, staged.stat.st_gid)?;

    if (staged.stat.st_mode & libc::S_IFMT) != libc::S_IFLNK {
        chmod(path, staged.stat.st_mode)?;
    }

    Ok(())
}

fn rm(path: &Path, staged: bool) -> Result<()> {
    if staged {
        debug!("{}", format!("rm {}", path.display()).bright_black());
    } else {
        debug!("rm {}", path.display());
    }
    fs::remove_file(path)?;
    Ok(())
}

fn mkdir(path: &Path) -> Result<()> {
    debug!("mkdir {}", path.display());
    fs::create_dir(path)?;
    Ok(())
}

fn ln_s(target: &Path, path: &Path) -> Result<()> {
    debug!("ln -s {} {}", target.display(), path.display());
    std::os::unix::fs::symlink(target, path)?;
    Ok(())
}

fn cp(old_path: &Path, new_path: &Path) -> Result<()> {
    debug!("cp {} {}", old_path.display(), new_path.display());
    fs::copy(old_path, new_path)?;
    Ok(())
}

fn mv(old_path: &Path, new_path: &Path) -> Result<()> {
    debug!("mv {} {}", old_path.display(), new_path.display());
    fs::rename(old_path, new_path)?;
    Ok(())
}

fn chown(path: &Path, uid: u32, gid: u32) -> Result<()> {
    debug!("chown {}:{} {}", uid, gid, path.display());
    fchownat(
        None,
        path,
        Some(Uid::from_raw(uid)),
        Some(Gid::from_raw(gid)),
        AtFlags::AT_SYMLINK_NOFOLLOW,
    )?;
    Ok(())
}

fn chmod(path: &Path, mode: u32) -> Result<()> {
    debug!("chmod {:o} {}", mode, path.display());
    fchmodat(
        None,
        path,
        Mode::from_bits_truncate(mode),
        FchmodatFlags::NoFollowSymlink,
    )?;
    Ok(())
}

fn rmdir(path: &Path) -> Result<()> {
    debug!("rmdir {}", path.display());
    fs::remove_dir(path)?;
    Ok(())
}

fn rmdir_recursive(path: &Path) -> Result<()> {
    // Get the device number of the root directory
    let root_device = nix::sys::stat::stat(path)?.st_dev;

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        // Check if the directory is on the same device
        let entry_device = nix::sys::stat::stat(&path)?.st_dev;

        #[cfg(feature = "coverage")]
        let entry_device =
            if std::env::var_os("TEST_ACCEPT_FAIL_RMDIR_ON_DIFFERENT_DEVICE")
                .is_some()
            {
                0
            } else {
                entry_device
            };
        debug!(
            "entry_device: {}   root_device: {}",
            entry_device, root_device
        );

        if entry_device != root_device {
            return Err(anyhow::anyhow!(
                "Cannot remove {}: directory is on a different device",
                path.display()
            ));
        }

        if path.is_dir() {
            rmdir_recursive(&path)?;
        } else {
            debug!("{}", format!("rm {}", path.display()).bright_black());
            fs::remove_file(path)?;
            return Ok(());
        }
    }

    debug!("{}", format!("rmdir {}", path.display()).bright_black());
    fs::remove_dir(path)?;
    Ok(())
}
