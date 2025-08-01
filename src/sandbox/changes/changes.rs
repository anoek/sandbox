use crate::config::Config;
use crate::sandbox::Sandbox;
use crate::sandbox::changes::*;
use crate::util::find_mount_point;
use anyhow::Context;
use anyhow::Result;
use log::error;
use log::info;
use log::trace;
use nix::NixPath;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;

struct UpperEntry {
    lower_path: PathBuf,
    upper_path: PathBuf,
    upper_details: FileDetails,
    source_path: Option<PathBuf>,
    source_details: Option<FileDetails>,
}

struct IgnorePattern {
    negate: bool,
    pattern: String,
}

pub struct CountResult {
    pub not_ignored: usize,
    pub ignored: usize,
}

const BUILT_IN_IGNORE_PATTERNS: &[&str] = &[
    "/tmp/**",
    "/home/*/.*/**",
    "/home/*/.*",
    "**/.git/**",
    "**/.git",
];

impl Sandbox {
    /**
     * This function will walk the upper directory and create a list of changes that have been
     * made. It will return a vector of ChangeEntry structs that can be used to apply the changes.
     *
     * For more information on the algorithm see the accept action documentation.
     */
    pub fn changes(&self, config: &Config) -> Result<ChangeEntries> {
        let mut change_entries: Vec<ChangeEntry> = Vec::new();

        /* Files that are moved (and not re-created) will have a corresponding deleted indicator in
         * the upper file system and the file it was moved to will have a corresponding renamed
         * indicator. When accepting changes, we need to *not* remove the old file but rather
         * rename it, so we will set a is_moved flag so we can handle this case. */
        let mut renamed_paths: HashSet<PathBuf> = HashSet::new();

        /* Get a list of all of the paths in the upper directory along with their corresponding
         * source and destination paths, and file details. */
        let upper_entries = self.upper_entries(config.ignored)?;

        /* First pass to find all renamed paths and build a hash of where they are coming from
         * so we can avoid removing them when we see the whiteout/opaque. */
        for entry in &upper_entries {
            if entry.upper_details.is_renamed()?.is_some() {
                if let Some(source_path) = &entry.source_path {
                    renamed_paths.insert(source_path.clone());
                }
            }
        }

        /* Second pass to build the list of changes */
        for entry in &upper_entries {
            let upper_file_details = FileDetails::from_path(&entry.upper_path)?
                .context("Failed to get file details for upper path")?;

            let source_is_dir = match &entry.source_path {
                Some(source_path) => source_path.is_dir(),
                None => false,
            };

            /* Deal with removes */
            if entry.upper_details.is_opaque()
                || entry.upper_details.is_removed()
                || (!entry.upper_details.is_dir() && source_is_dir)
            {
                if let Some(source_path) = &entry.source_path {
                    /* Unless we are dealing with a rename */
                    if !renamed_paths.contains(source_path) {
                        /* If the source path is a directory, we need to remove all of the files in it. */
                        if source_path.is_dir() {
                            WalkDir::new(source_path)
                                .into_iter()
                                .flatten()
                                .try_for_each(|dir_entry| -> Result<()> {
                                    let path = dir_entry.path().to_path_buf();
                                    let details = FileDetails::from_path(&path)
                                        .context(format!(
                                            "Failed to get file details for {}",
                                            path.display()
                                        ))?
                                        .context("File details should always exist for existing paths")?;

                                    change_entries.push(ChangeEntry {
                                        destination: path.clone(),
                                        operation: EntryOperation::Remove,
                                        source: Some(details),
                                        staged: if dir_entry.path()
                                            == source_path
                                        {
                                            Some(entry.upper_details.clone())
                                        } else {
                                            None
                                        },
                                        tmp_path: None,
                                    });
                                    Ok(())
                                })?;
                        } else {
                            change_entries.push(ChangeEntry {
                                destination: source_path.clone(),
                                operation: EntryOperation::Remove,
                                source: entry.source_details.clone(),
                                staged: Some(entry.upper_details.clone()),
                                tmp_path: None,
                            });
                        }
                    }
                } else {
                    /* Newly created directories will not have a source path but will still
                     * be flagged as opaque. This is fine, there's nothing to do here. */
                }
            }

            if !entry.upper_details.is_removed() {
                if upper_file_details.is_renamed()?.is_some() {
                    if entry.source_details.is_none() {
                        info!(
                            "entry: {} {:?}",
                            entry.lower_path.display(),
                            entry.source_details
                        );
                        change_entries.push(ChangeEntry {
                            destination: entry.lower_path.clone(),
                            operation: EntryOperation::Error(
                                ChangeError::RedirectPathNotFound,
                            ),
                            source: entry.source_details.clone(),
                            staged: Some(entry.upper_details.clone()),
                            tmp_path: None,
                        });
                        continue;
                    }

                    change_entries.push(ChangeEntry {
                        destination: entry.lower_path.clone(),
                        operation: EntryOperation::Rename,
                        source: entry.source_details.clone(),
                        staged: Some(entry.upper_details.clone()),
                        tmp_path: None,
                    });
                } else {
                    // If we haven't removed the file and haven't renamed it, then it's been added or changed.

                    /* Character devices, block devices, FIFOs, and sockets are not supported */
                    let mode = entry.upper_details.stat.st_mode & libc::S_IFMT;
                    if mode != libc::S_IFREG
                        && mode != libc::S_IFDIR
                        && mode != libc::S_IFLNK
                    {
                        change_entries.push(ChangeEntry {
                            destination: entry.lower_path.clone(),
                            operation: EntryOperation::Error(
                                ChangeError::UnsupportedFileType,
                            ),
                            source: entry.source_details.clone(),
                            staged: Some(upper_file_details.clone()),
                            tmp_path: None,
                        });
                    } else {
                        /* Symlinks, directories, and normal files */
                        change_entries.push(ChangeEntry {
                            destination: entry.lower_path.clone(),
                            operation: EntryOperation::Set(
                                if entry.source_details.is_some()
                                    && !has_opaque_ancestor(&entry.upper_path)
                                {
                                    SetType::Modify
                                } else {
                                    SetType::Create
                                },
                            ),
                            source: entry.source_details.clone(),
                            staged: Some(upper_file_details.clone()),
                            tmp_path: None,
                        });
                    }
                }
            }
        }

        Ok(ChangeEntries(change_entries))
    }

    pub fn count_upper_entries(&self, config: &Config) -> Result<CountResult> {
        let mut count = 0;
        let mut ignored = 0;
        let mut resolved_ignores: HashMap<PathBuf, Vec<IgnorePattern>> =
            HashMap::new();

        for walkdir_entry in WalkDir::new(&self.upper_base)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = walkdir_entry.path().strip_prefix(&self.upper_base)?;

            let base = match path.components().next() {
                Some(base) => base,
                None => {
                    continue;
                }
            };

            let base_decoded = match data_encoding::BASE32_NOPAD_NOCASE
                .decode(base.as_os_str().as_bytes())
            {
                Ok(decoded) => match String::from_utf8(decoded) {
                    Ok(s) => s,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            let sub = path.components().skip(1).collect::<Vec<_>>();

            if sub.is_empty() {
                continue;
            }

            let mut lower_path = PathBuf::from(base_decoded.clone());
            for component in &sub {
                lower_path.push(component);
            }

            if walkdir_entry.path().is_dir() {
                // only count files, not directories
                continue;
            }

            /* Check if we should ignore this based on ignore rules and files */
            count += 1;
            if !config.ignored
                && self.is_ignored(
                    &base_decoded,
                    &lower_path,
                    path,
                    &mut resolved_ignores,
                )
            {
                ignored += 1;
                continue;
            }
        }
        Ok(CountResult {
            not_ignored: count - ignored,
            ignored,
        })
    }

    /**
     * Walks the upper directory and creates a list of paths that have been changed in some way.
     * This primarily exists to deal with decoding the base32 encoded paths and making
     * it easier to reason about and reduce the clutter of the `changes` function.
     */
    fn upper_entries(&self, include_ignored: bool) -> Result<Vec<UpperEntry>> {
        let mut resolved_ignores: HashMap<PathBuf, Vec<IgnorePattern>> =
            HashMap::new();
        let mut ret = Vec::new();

        for walkdir_entry in WalkDir::new(&self.upper_base)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = walkdir_entry.path().strip_prefix(&self.upper_base)?;

            let base = match path.components().next() {
                Some(base) => base,
                None => {
                    continue;
                }
            };

            let base_decoded = match data_encoding::BASE32_NOPAD_NOCASE
                .decode(base.as_os_str().as_bytes())
            {
                Ok(decoded) => match String::from_utf8(decoded) {
                    Ok(s) => s,
                    Err(_) => {
                        error!(
                            "Skipping invalid base32 encoded base path: {}",
                            base.as_os_str().to_string_lossy()
                        );
                        continue;
                    }
                },
                Err(_) => {
                    error!(
                        "Skipping invalid base32 encoded base path: {}",
                        base.as_os_str().to_string_lossy()
                    );
                    continue;
                }
            };

            let sub = path.components().skip(1).collect::<Vec<_>>();

            if sub.is_empty() {
                continue;
            }

            let mut lower_path = PathBuf::from(base_decoded.clone());
            for component in &sub {
                lower_path.push(component);
            }

            /* Check if we should ignore this based on ignore rules and files */
            if !include_ignored
                && self.is_ignored(
                    &base_decoded,
                    &lower_path,
                    path,
                    &mut resolved_ignores,
                )
            {
                continue;
            }

            let mut upper_path = self.upper_base.clone();
            upper_path.push(base);
            let upper_root = upper_path.clone();
            for component in &sub {
                upper_path.push(component);
            }

            /* Note, get_source_lower_path_for_upper_path deals with following redirects */
            let source_path = get_source_lower_path_for_upper_path(
                &upper_path,
                &upper_root,
                &lower_path,
                &PathBuf::from(base_decoded.clone()),
            )?;

            let source_details = match &source_path {
                Some(source_path) => FileDetails::from_path(source_path)?,
                None => None,
            };
            let upper_details = FileDetails::from_path(&upper_path)?
                .context("Failed to get file details for upper path (something is very wrong)")?;

            ret.push(UpperEntry {
                lower_path,
                upper_path,
                upper_details,
                source_path,
                source_details,
            });
        }

        Ok(ret)
    }

    /* This function will check if a path is ignored based on the ignore rules and files.
     *
     * We're trying to follow the spirit of gitignore rules in which we look for ignore files in
     * parent directories and apply all the rules in them first, all the way up to our current
     * directory, last matching rule wins.
     * */
    fn is_ignored(
        &self,
        base_decoded: &str,
        lower_path: &Path,
        upper_relative_path: &Path,
        resolved_ignores: &mut HashMap<PathBuf, Vec<IgnorePattern>>,
    ) -> bool {
        let mut ignored = false;

        let lower_path_str = lower_path
            .to_str()
            .expect("lower_path should have a string representation");

        // Check built-in patterns first
        for pattern in BUILT_IN_IGNORE_PATTERNS {
            if fast_glob::glob_match(pattern, lower_path_str) {
                return true;
            }
        }

        // Get the base32 encoded root component from upper_relative_path
        let mut upper_components = upper_relative_path.components();
        let upper_base32_root =
            upper_components.next().map(|c| c.as_os_str().to_owned());

        // Build list of parent directories to check for ignore files
        let mut current_lower_dir = PathBuf::from(base_decoded);
        let mut current_upper_dir = self.upper_base.clone();
        if let Some(ref base32) = upper_base32_root {
            current_upper_dir.push(base32);
        }

        // Walk through each component of the path
        let lower_components: Vec<_> = lower_path
            .strip_prefix(&current_lower_dir)
            .expect("lower_path should be a prefix of current_lower_dir")
            .components()
            .collect();

        for (i, component) in lower_components.iter().enumerate() {
            current_lower_dir.push(component);
            current_upper_dir.push(component);

            // Build the relative path from this directory to our target
            let relative_path: PathBuf =
                lower_components[i + 1..].iter().collect();
            let relative_str = relative_path.to_str().unwrap_or("");

            // Check if we have cached patterns for this directory
            let patterns = resolved_ignores
                .entry(current_lower_dir.clone())
                .or_insert_with(|| {
                    resolve_ignores(&current_upper_dir, &current_lower_dir)
                });

            // Check if any pattern matches
            for pattern in patterns {
                if fast_glob::glob_match(&pattern.pattern, relative_str) {
                    ignored = !pattern.negate;
                }
            }
        }

        ignored
    }
}

/* Resolve ignore patterns from the upper and lower directories, with files in
 * upper_dir taking precedence over files in lower_dir. */
fn resolve_ignores(upper_dir: &Path, lower_dir: &Path) -> Vec<IgnorePattern> {
    let mut patterns: Vec<IgnorePattern> = Vec::new();

    for ignore_file in &[".gitignore", ".ignore"] {
        for dir in &[upper_dir, lower_dir] {
            let file_path = dir.join(ignore_file);
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                trace!("Found ignore file: {}", file_path.display());
                parse_ignore_content(&content, &mut patterns);
                break;
            }
        }
    }

    patterns
}

/* Parse the content of an ignore file and add patterns to the provided vector */
fn parse_ignore_content(content: &str, patterns: &mut Vec<IgnorePattern>) {
    for line in content.lines() {
        let mut trimmed = line.trim().to_string();
        if line.ends_with("\\ ") {
            trimmed.pop(); // pop the \
            trimmed += " ";
        }
        let original_trimmed = trimmed.clone();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut negate = false;

        // Handle negation and escaped exclamation marks
        if trimmed.starts_with("\\!") {
            // Escaped exclamation - remove the backslash
            trimmed = trimmed[1..].to_string();
        } else if trimmed.starts_with("!") {
            // Negation - remove the ! and set negate flag
            trimmed = trimmed[1..].to_string();
            negate = true;
        }

        // Normalize multiple consecutive slashes to single slash
        while trimmed.contains("//") {
            trimmed = trimmed.replace("//", "/");
        }

        /* Per https://git-scm.com/docs/gitignore
         * """
         *  If there is a separator at the beginning or middle (or both) of the pattern,
         *  then the pattern is relative to the directory level of the particular .gitignore
         *  file itself. Otherwise the pattern may also match at any level below the
         * .gitignore level.
         * """
         * */
        if trimmed.starts_with("/") {
            // Anchored pattern - remove the leading slash
            trimmed = trimmed
                .strip_prefix("/")
                .expect("should be present")
                .to_string();
        } else if !trimmed.contains("/") && !trimmed.starts_with("**") {
            // No slash means it can match at any level - add **/ prefix
            trimmed = format!("**/{trimmed}");
        } else {
            // If pattern contains a slash but doesn't start with /, it's relative to the
            // .gitignore directory
        }

        let only_match_directories = trimmed.ends_with("/");

        // Handle directory patterns - fast_glob requires /** suffix for directories
        if only_match_directories && !trimmed.ends_with("/**") {
            trimmed = trimmed.trim_end_matches('/').to_string();
            trimmed.push_str("/**");
        }

        // Special case: patterns without trailing slash that don't contain wildcards
        // should match both the item itself and everything under it (if it's a directory)
        // For example, "/target" should match "target" and "target/foo/bar"
        if !only_match_directories
            && !original_trimmed.contains('*')
            && !original_trimmed.contains('?')
        {
            // Add a second pattern that matches everything under this path
            patterns.push(IgnorePattern {
                negate,
                pattern: format!("{trimmed}/**"),
            });
        }

        patterns.push(IgnorePattern {
            negate,
            pattern: trimmed,
        });
    }
}

pub fn by_reverse_source(a: &ChangeEntry, b: &ChangeEntry) -> Ordering {
    match (&a.source, &b.source) {
        (Some(a_source), Some(b_source)) => b_source.path.cmp(&a_source.path),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub fn by_destination(a: &ChangeEntry, b: &ChangeEntry) -> Ordering {
    a.destination.cmp(&b.destination)
}

pub fn by_staged_descending(a: &ChangeEntry, b: &ChangeEntry) -> Ordering {
    match (&a.staged, &b.staged) {
        (Some(a_staged), Some(b_staged)) => b_staged.path.cmp(&a_staged.path),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => b.destination.cmp(&a.destination),
    }
}
/* Resolves the source path for a given upper path. This deals with following redirects.
 * for the given path, or an ancestor path. */
fn get_source_lower_path_for_upper_path(
    upper_path: &Path,
    upper_root: &Path,
    lower_path: &Path,
    lower_root: &Path,
) -> Result<Option<PathBuf>> {
    /* We're going to walk backwards from our path looking for any redirects along the way. We'll
     * store the path components's we've popped off the back and once we bottom out at a mount
     * point or a redirect we'll join them back together. If that corresponding path exists on the
     * lower filesystem we'll return it, otherwise we'll return None. */

    let mut components = PathBuf::new();
    let mut cur_upper = upper_path.to_path_buf();
    let mut cur_lower = lower_path.to_path_buf();

    while let Some(cur_details) = FileDetails::from_path(&cur_upper)? {
        if cur_upper == *upper_root {
            // bottomed out at the mount point, no redirect found
            break;
        }

        if let Some(xattr_path) = cur_details.is_renamed()? {
            // Found a redirect. Add the components we've built up to the path
            // found in the xattr for our (potential) source path.
            components = match components.is_empty() {
                true => PathBuf::from(&xattr_path),
                false => PathBuf::from(&xattr_path).join(components),
            };
            let is_relative_to_mount_point = xattr_path.starts_with("/");

            components = if is_relative_to_mount_point {
                find_mount_point(cur_lower.clone())?
                    .join(components.strip_prefix("/").unwrap_or(&components))
            } else {
                let lower_parent = cur_lower.parent().context(format!(
                    "Failed to get parent for {}",
                    cur_lower.display()
                ))?;

                lower_parent.join(components)
            };

            if components.exists() {
                return Ok(Some(components));
            } else {
                return Ok(None);
            }
        }

        // otherwise no redirect found here, try the parent
        let cur_trailing_component = cur_upper.file_name().context(format!(
            "Failed to get trailing component for {}",
            cur_upper.display()
        ))?;

        let upper_parent = cur_upper.parent().context(format!(
            "Failed to get parent for {}",
            cur_upper.display()
        ))?;

        let lower_parent = cur_lower.parent().context(format!(
            "Failed to get parent for {}",
            cur_lower.display()
        ))?;

        components = match components.is_empty() {
            true => PathBuf::from(cur_trailing_component),
            false => PathBuf::from(cur_trailing_component).join(components),
        };

        cur_upper = PathBuf::from(upper_parent);
        cur_lower = PathBuf::from(lower_parent);
    }

    components = lower_root.join(components);

    if components.exists() {
        Ok(Some(components))
    } else {
        Ok(None)
    }
}

fn has_opaque_ancestor(path: &Path) -> bool {
    let mut current = path.to_path_buf();
    while let Some(parent) = current.parent() {
        let parent = PathBuf::from(parent);
        if let Ok(Some(details)) = FileDetails::from_path(&parent) {
            if details.is_opaque() {
                return true;
            }
        } else {
            return false;
        }
        current = parent;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /* Rounds out our coverage tests for this function which in normal use won't ever hit the
     * None, None case, but for rust completeness we need it */
    #[test]
    fn test_by_staged_descending() {
        let mut change_entries = vec![
            ChangeEntry {
                destination: PathBuf::from("/tmp/test"),
                operation: EntryOperation::Set(SetType::Create),
                source: None,
                staged: None,
                tmp_path: None,
            },
            ChangeEntry {
                destination: PathBuf::from("/tmp/test2"),
                operation: EntryOperation::Set(SetType::Create),
                source: None,
                staged: None,
                tmp_path: None,
            },
        ];

        change_entries.sort_by(by_staged_descending);

        assert_eq!(change_entries[0].destination, PathBuf::from("/tmp/test2"));
        assert_eq!(change_entries[1].destination, PathBuf::from("/tmp/test"));
    }

    /* Rounds out our coverage tests for this function wouldn't otherwise hit this case because we
     * are walking accessible real directories */
    #[test]
    fn test_has_opaque_ancestor() {
        let path = PathBuf::from("/tmp/test-non-existent-path/foo");
        assert!(!has_opaque_ancestor(&path));
    }
}
