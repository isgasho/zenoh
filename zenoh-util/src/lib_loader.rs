//
// Copyright (c) 2017, 2020 ADLINK Technology Inc.
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ADLINK zenoh team, <zenoh@adlink-labs.tech>
//
use crate::core::{ZError, ZErrorKind, ZResult};
use crate::{zconfigurable, zerror, zerror2};
use libloading::Library;
use log::{debug, trace, warn};
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::path::{Path, PathBuf};

zconfigurable! {
    /// The libraries prefix for the current platform (usually: `"lib"`)
    pub static ref LIB_PREFIX: String = DLL_PREFIX.to_string();
    /// The libraries suffix for the current platform (`".dll"` or `".so"` or `".dylib"`...)
    pub static ref LIB_SUFFIX: String = DLL_SUFFIX.to_string();
}

/// LibLoader allows search for librairies and to load them.
#[derive(Clone, Debug)]
pub struct LibLoader {
    search_paths: Vec<PathBuf>,
}

impl LibLoader {
    /// Creates a new [LibLoader] with a set of paths where the libraries will be searched for.
    /// If `exe_parent_dir`is true, the parent directory of the current executable is also added
    /// to the set of paths for search.
    pub fn new(search_dirs: &[&str], exe_parent_dir: bool) -> ZResult<LibLoader> {
        let mut search_paths: Vec<PathBuf> = vec![];
        for s in search_dirs {
            match shellexpand::full(s) {
                Ok(cow_str) => {
                    match PathBuf::from(&*cow_str).canonicalize() {
                        Ok(path) => search_paths.push(path),
                        Err(err) => debug!("Cannot search for libraries in {}: {}", cow_str, err)
                    }
                },
                Err(err) => warn!("Cannot search for libraries in '{}': {} ", s, err)
            }
        }

        if exe_parent_dir {
            match std::env::args().next() {
                Some(path) => match Path::new(&path).parent() {
                    Some(p) => search_paths.push(p.canonicalize().unwrap()),
                    None => warn!("This executable ({}) has no parent !!. Can't search plugins in its parent directory.", path),
                },
                None => warn!("This executable name was not found in args. Can't find it's parent to search plugins."),
            }
        }

        Ok(LibLoader { search_paths })
    }

    /// Return the list of search paths used by this [LibLoader]
    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }

    /// Load a library from the specified path.
    pub fn load_file(path: &str) -> ZResult<(Library, PathBuf)> {
        let path = Self::str_to_canonical_path(path)?;

        if !path.exists() {
            zerror!(ZErrorKind::Other {
                descr: format!("Library file '{}' doesn't exist", path.display())
            })
        } else if !path.is_file() {
            zerror!(ZErrorKind::Other {
                descr: format!("Library file '{}' is not a file", path.display())
            })
        } else {
            Library::new(path.clone())
                .map_err(|e| {
                    zerror2!(ZErrorKind::Other {
                        descr: e.to_string()
                    })
                })
                .map(|lib| (lib, path))
        }
    }

    /// Search and load all librairies with filename starting with [struct@LIB_PREFIX]+`prefix` and ending with [struct@LIB_SUFFIX].
    /// The result is a list of tuple with:
    ///    * the [Library]
    ///    * its full path
    ///    * its short name (i.e. filename stripped of prefix and suffix)
    pub fn load_all_with_prefix(&self, prefix: Option<&str>) -> Vec<(Library, PathBuf, String)> {
        let lib_prefix = format!("{}{}", *LIB_PREFIX, prefix.unwrap_or(""));
        log::debug!(
            "Search for libraries {}*{} to load in {:?}",
            lib_prefix,
            *LIB_SUFFIX,
            self.search_paths
        );

        let mut result = vec![];
        for dir in &self.search_paths {
            trace!("Search plugins in dir {:?} ", dir);
            match dir.read_dir() {
                Ok(read_dir) => {
                    for entry in read_dir {
                        if let Ok(entry) = entry {
                            if let Ok(filename) = entry.file_name().into_string() {
                                if filename.starts_with(&lib_prefix)
                                    && filename.ends_with(&*LIB_SUFFIX)
                                {
                                    let name = &filename
                                        [(lib_prefix.len())..(filename.len() - LIB_SUFFIX.len())];
                                    let path = entry.path();
                                    if !result.iter().any(|(_, _, n)| n == name) {
                                        match Library::new(path.as_os_str()) {
                                            Ok(lib) => result.push((lib, path, name.to_string())),
                                            Err(err) => warn!("{}", err),
                                        }
                                    } else {
                                        debug!(
                                            "Do not load plugin {} from {:?} : already loaded.",
                                            name, path
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => debug!(
                    "Failed to read in directory {:?} ({}). Can't use it to search for libraries.",
                    dir, err
                ),
            }
        }
        result
    }

    fn str_to_canonical_path(s: &str) -> ZResult<PathBuf> {
        shellexpand::full(s)
            .map_err(|err| {
                zerror2!(ZErrorKind::Other {
                    descr: err.to_string()
                })
            })
            .and_then(|cow_str| {
                PathBuf::from(cow_str.into_owned())
                    .canonicalize()
                    .map_err(|err| {
                        zerror2!(ZErrorKind::Other {
                            descr: err.to_string()
                        })
                    })
            })
    }
}