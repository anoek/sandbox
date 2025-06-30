use anyhow::Result;
use log::error;
use nix::errno::Errno;
use nix::fcntl::AtFlags;
use nix::sys::stat::{FileStat, fstatat};
use std::ffi::CString;
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileDetails {
    pub path: PathBuf,
    pub stat: FileStat,
}

impl FileDetails {
    pub fn from_path(path: &PathBuf) -> Result<Option<FileDetails>> {
        let stat = match fstatat(None, path, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(e) => {
                if e == Errno::ENOENT {
                    return Ok(None);
                } else {
                    error!("Error getting stat for {}: {}", path.display(), e);
                    return Err(anyhow::anyhow!(
                        "Error getting stat for {}: {}",
                        path.display(),
                        e
                    ));
                }
            }
        };

        Ok(Some(FileDetails {
            path: path.clone(),
            stat,
        }))
    }

    pub fn is_symlink(&self) -> bool {
        self.stat.st_mode & libc::S_IFMT == libc::S_IFLNK
    }

    pub fn is_file(&self) -> bool {
        self.stat.st_mode & libc::S_IFMT == libc::S_IFREG
    }

    pub fn is_dir(&self) -> bool {
        self.stat.st_mode & libc::S_IFMT == libc::S_IFDIR
    }

    pub fn is_char_device(&self) -> bool {
        self.stat.st_mode & libc::S_IFMT == libc::S_IFCHR
    }

    /* An opaque directory means that we have created the directory
     * within the sandbox, possibly removing a previous directory by
     * the same name along with all its contents. */
    pub fn is_opaque(&self) -> bool {
        let mut buffer = vec![0; 1];
        let path_cstr = path_to_cstring(&self.path)
            .expect("Failed to create CString from path");
        let result = unsafe {
            libc::lgetxattr(
                path_cstr.as_ptr(),
                c"trusted.overlay.opaque".as_ptr(),
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
            )
        };

        result != -1
    }

    pub fn is_removed(&self) -> bool {
        let mut buffer = vec![0; 1];
        let path_cstr = path_to_cstring(&self.path)
            .expect("Failed to create CString from path");

        if self.is_char_device() {
            let major = (self.stat.st_rdev >> 8) & 0xff;
            let minor = self.stat.st_rdev & 0xff;
            return major == 0 && minor == 0;
        }

        let result = unsafe {
            libc::lgetxattr(
                path_cstr.as_ptr(),
                c"trusted.overlay.whiteout".as_ptr(),
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
            )
        };

        result != -1
    }

    /* Returns the path as specified by the overlay redirect xattr if the file is renamed. */
    pub fn is_renamed(&self) -> Result<Option<PathBuf>> {
        self.is_renamed_with_buffer_size::<256>()
    }
    fn is_renamed_with_buffer_size<const BUFFER_SIZE: usize>(
        &self,
    ) -> Result<Option<PathBuf>> {
        // The BUFFER_SIZE is an optimization for large change sets. By trying 256 (which works in
        // a lot of real world cases) before falling back to 64KiB (the max for filesystems that
        // handle the largest xattrs, XFS, BTRFS and ZFS [EXT 2/3/4 is 4k]) we shave of tens of
        // percent of wall clock runtime. I think it's just helping us not blow our CPU cache as
        // callgrind/cachegrind doesn't really change with this optimization, but the wall clock
        // time does (at least on this computer.)
        let mut buffer = [MaybeUninit::<u8>::uninit(); BUFFER_SIZE];
        let path_cstr = path_to_cstring(&self.path)?;
        let xattr_name = "trusted.overlay.redirect";
        let xattr_name_cstr = CString::new(xattr_name)
            .expect("Failed to create CString for xattr name");

        let result = unsafe {
            libc::lgetxattr(
                path_cstr.as_ptr(),
                xattr_name_cstr.as_ptr(),
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
            )
        };

        if result == -1 {
            let errno = nix::errno::Errno::last_raw();
            if errno == libc::ENODATA {
                return Ok(None);
            } else if errno == libc::ERANGE && BUFFER_SIZE < 65536 {
                // Try again with a larger buffer.
                return self.is_renamed_with_buffer_size::<65536>();
            } else {
                return Err(anyhow::anyhow!(
                    "Error getting xattr data for {}: {}",
                    self.path.display(),
                    errno
                ));
            }
        }

        let rename_data = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr() as *const u8,
                result as usize,
            )
        };
        let rename_data_str = String::from_utf8_lossy(rename_data);
        Ok(Some(PathBuf::from(rename_data_str.to_string())))
    }

    pub fn display_type(&self) -> String {
        match self.stat.st_mode & libc::S_IFMT {
            libc::S_IFREG => "file".to_string(),
            libc::S_IFDIR => "directory".to_string(),
            libc::S_IFLNK => "symlink".to_string(),
            libc::S_IFCHR => "character device".to_string(),
            libc::S_IFBLK => "block device".to_string(),
            libc::S_IFIFO => "FIFO".to_string(),
            libc::S_IFSOCK => "socket".to_string(),
            _ => "unknown".to_string(),
        }
    }
}

fn path_to_cstring(path: &Path) -> Result<CString> {
    CString::new(path.to_string_lossy().as_bytes()).map_err(|e| {
        anyhow::anyhow!("Failed to create CString from path: {}", e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // lazy coverage fill out for things that are hard to reach for integration tests
    #[test]
    fn path_to_cstring_failure() {
        let path = PathBuf::from("/have/\0a/null/byte");
        let result = path_to_cstring(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_inaccessible_path() {
        let path = PathBuf::from("/etc/passwd/is/not/a/directory");
        let result = FileDetails::from_path(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_renamed() {
        let path = PathBuf::from("/");
        let mut tmp = FileDetails::from_path(&path).unwrap().unwrap();
        tmp.path = PathBuf::from("/root/test");
        let result = tmp.is_renamed();
        assert!(result.is_err());
    }

    #[test]
    fn test_display_type() {
        let path = PathBuf::from("/");
        let mut tmp = FileDetails::from_path(&path).unwrap().unwrap();
        tmp.path = PathBuf::from("/root/test");
        let result = tmp.display_type();
        assert_eq!(result, "directory");

        tmp.stat.st_mode = libc::S_IFREG;
        let result = tmp.display_type();
        assert_eq!(result, "file");

        tmp.stat.st_mode = libc::S_IFLNK;
        let result = tmp.display_type();
        assert_eq!(result, "symlink");

        tmp.stat.st_mode = libc::S_IFCHR;
        let result = tmp.display_type();
        assert_eq!(result, "character device");

        tmp.stat.st_mode = libc::S_IFBLK;
        let result = tmp.display_type();
        assert_eq!(result, "block device");

        tmp.stat.st_mode = libc::S_IFIFO;
        let result = tmp.display_type();
        assert_eq!(result, "FIFO");

        tmp.stat.st_mode = libc::S_IFSOCK;
        let result = tmp.display_type();
        assert_eq!(result, "socket");

        tmp.stat.st_mode = libc::S_IFMT;
        let result = tmp.display_type();
        assert_eq!(result, "unknown");
    }

    #[test]
    fn test_file_details_from_empty_pathbuf_is_none() {
        let result = FileDetails::from_path(&PathBuf::new());
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
