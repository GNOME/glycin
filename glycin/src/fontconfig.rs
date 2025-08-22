use std::collections::BTreeSet;
use std::ffi::{c_char, CStr};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::OnceLock;

use fontconfig_sys as fc;

pub fn cached_paths() -> &'static Option<BTreeSet<PathBuf>> {
    static DIRS: OnceLock<Option<BTreeSet<PathBuf>>> = OnceLock::new();

    DIRS.get_or_init(dirs)
}

fn dirs() -> Option<BTreeSet<PathBuf>> {
    unsafe {
        if fc::FcInit() != 1 {
            return None;
        }
        let config = NonNull::new(fc::FcConfigGetCurrent())?.as_ptr();

        let mut paths = cache_dirs(config)?;
        paths.append(&mut config_files_and_dirs(config)?);
        paths.append(&mut font_dirs(config)?);

        Some(paths)
    }
}

unsafe fn cache_dirs(config: *mut fc::FcConfig) -> Option<BTreeSet<PathBuf>> {
    str_list_to_set(fc::FcConfigGetCacheDirs(config))
}

unsafe fn config_files_and_dirs(config: *mut fc::FcConfig) -> Option<BTreeSet<PathBuf>> {
    let config_dirs = str_list_to_set(fc::FcConfigGetConfigDirs(config))?;
    let all_config_files = str_list_to_set(fc::FcConfigGetConfigFiles(config))?;

    let mut config_files_without_confd = all_config_files
        .into_iter()
        .filter(|x| {
            x.parent()
                .is_some_and(|p| !config_dirs.contains(p) && !config_dirs.contains(x))
        })
        .collect();

    let mut all_paths = config_dirs;
    all_paths.append(&mut config_files_without_confd);
    Some(all_paths)
}

unsafe fn font_dirs(config: *mut fc::FcConfig) -> Option<BTreeSet<PathBuf>> {
    let font_dirs = str_list_to_set(fc::FcConfigGetFontDirs(config))?;

    Some(
        font_dirs
            .into_iter()
            .filter(|x| !x.starts_with("/usr/"))
            .collect(),
    )
}

unsafe fn str_list_to_set(list: *mut fc::FcStrList) -> Option<BTreeSet<PathBuf>> {
    let list = NonNull::new(list)?.as_ptr();
    let mut vec = BTreeSet::new();
    loop {
        let s = fc::FcStrListNext(list);
        if s.is_null() {
            break;
        } else if let Ok(cs) = CStr::from_ptr(s as *const c_char).to_str() {
            vec.insert(PathBuf::from(cs));
        } else {
            tracing::error!("fontconfig: Invalid path");
        }
    }

    fc::FcStrListDone(list);

    Some(vec)
}
