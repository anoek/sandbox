use anyhow::Context;
use anyhow::{Result, anyhow};
use nix::mount::MsFlags;
use std::ffi::CStr;
use std::ffi::CString;
use std::path::Path;

pub fn mount<S1, S2, S3, S4>(
    source: Option<S1>,
    target: S2,
    fstype: Option<S3>,
    flags: MsFlags,
    data: Option<S4>,
) -> Result<()>
where
    S1: AsRef<std::ffi::OsStr>,
    S2: AsRef<std::ffi::OsStr>,
    S3: AsRef<std::ffi::OsStr>,
    S4: AsRef<std::ffi::OsStr>,
{
    let source_cstr = match &source {
        Some(source) => {
            CString::new(source.as_ref().to_string_lossy().as_bytes())?
        }
        None => CString::new("")?,
    };
    let target_cstr =
        CString::new(target.as_ref().to_string_lossy().as_bytes())?;
    let fstype_cstr = match &fstype {
        Some(fstype) => {
            CString::new(fstype.as_ref().to_string_lossy().as_bytes())?
        }
        None => CString::new("")?,
    };
    let data_cstr = match &data {
        Some(data) => CString::new(data.as_ref().to_string_lossy().as_bytes())?,
        None => CString::new("")?,
    };

    let source = source.map(|_| source_cstr.as_c_str());
    let target = target_cstr.as_c_str();
    let fstype = fstype.map(|_| fstype_cstr.as_c_str());
    let data = data.map(|_| data_cstr.as_c_str());

    let result = nix::mount::mount::<CStr, CStr, CStr, CStr>(
        source, target, fstype, flags, data,
    );

    if let Err(e) = result {
        let err_context = format!(
            "failed to mount {} {} [type={}, flags={}, data={}]",
            source_cstr.to_string_lossy(),
            target_cstr.to_string_lossy(),
            fstype_cstr.to_string_lossy(),
            flags.bits(),
            data_cstr.to_string_lossy(),
        );

        // Check if this is an EINVAL error when trying to mount overlayfs
        if e == nix::errno::Errno::EINVAL
            && fstype_cstr.to_string_lossy() == "overlay"
            && data
                .map(|d| d.to_string_lossy().contains("lowerdir=/"))
                .unwrap_or(false)
        {
            return Err(anyhow!(
                "Maximum overlayfs stacking depth exceeded. \
                The Linux kernel prevents creating overlay filesystems when the lower directory \
                is already on an overlay filesystem. \
                The kernel only supports up to 2 levels of overlayfs stacking by default."
            )).context(err_context);
        }

        return Err(e).context(err_context);
    }

    Ok(())
}

/**
 * Checks our storage path to ensure it's a valid path for our OverlayFS mount options.
 *
 * This is probably overly restrictive, but should be safe for the mount options, I think.
 *
 * TODO: Dig into this and ensure it's bullet proof. If OverlayFS supports escaping of commas, we
 * should do that.
 */
pub fn check_path_for_mount_option_compatibility(path: &Path) -> Result<()> {
    let components = path.components();

    if components.count() == 0 {
        return Err(anyhow!("Storage path {} is empty", path.display()));
    }

    path.components().try_for_each(|component| {
        let component_str = match component.as_os_str().to_str() {
            Some(s) => s,
            None => {
                return Err(anyhow!(
                    "Storage path {} contains invalid character",
                    path.display(),
                ));
            }
        };

        if !component_str.chars().all(|c| {
            c.is_alphanumeric()
                || c == '_'
                || c == '-'
                || c == '.'
                || c == '/'
                || c == '@'
                || c == '%'
        }) {
            Err(anyhow!(
                "Storage path {} contains invalid character {}",
                path.display(),
                component_str
            ))
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use super::*;

    #[test]
    fn test_check_path_for_mount_option_compatibility() {
        let path = Path::new("/tmp/test");
        assert!(check_path_for_mount_option_compatibility(path).is_ok());
    }

    #[test]
    fn test_check_path_for_mount_option_no_spaces() {
        let path = Path::new("/tmp/test test");
        assert!(check_path_for_mount_option_compatibility(path).is_err());
    }

    #[test]
    fn test_check_path_for_mount_option_compatibility_empty() {
        let path = Path::new("");
        assert!(check_path_for_mount_option_compatibility(path).is_err());
    }

    #[test]
    fn test_check_path_for_mount_option_compatibility_invalid_character() {
        let path = Path::new("/tmp/test\x00");
        assert!(check_path_for_mount_option_compatibility(path).is_err());
    }

    #[test]
    fn test_check_path_for_mount_option_compatibility_non_utf8() {
        // Create a path with invalid UTF-8 sequence
        let invalid_utf8 = vec![0xFF, 0xFF];
        let os_string = OsString::from_vec(invalid_utf8);
        let path = Path::new(&os_string);

        assert!(check_path_for_mount_option_compatibility(path).is_err());
    }
}
