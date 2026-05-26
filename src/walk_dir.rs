// Copyright 2026 System76 <info@system76.com>
// SPDX-License-Identifier: MIT

//! Search for files within multiple directories. Follows symlinks, avoids loops, and
//! limits the max depth to 5.

use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    path::PathBuf,
};

const MAX_DEPTH: usize = 5;

pub struct Iter {
    directories_to_walk: VecDeque<(PathBuf, usize)>,
    actively_walking: Option<VecDeque<(PathBuf, usize)>>,
    visited: BTreeSet<PathBuf>,
}

impl Iter {
    /// Directories will be processed in order.
    #[inline]
    pub fn new<I: Iterator<Item = PathBuf>>(directories_to_walk: I) -> Self {
        Self {
            directories_to_walk: directories_to_walk.map(|dir| (dir, 0)).collect(),
            actively_walking: None,
            visited: BTreeSet::default(),
        }
    }
}

impl Iterator for Iter {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        'outer: loop {
            let mut paths = match self.actively_walking.take() {
                Some(dir) => dir,
                None => {
                    while let Some((mut path, depth)) = self.directories_to_walk.pop_front() {
                        path = path.canonicalize().map_or(path, |canonical| canonical);
                        self.visited.insert(path.clone());
                        match fs::read_dir(&path) {
                            Ok(dir) => {
                                self.actively_walking = Some({
                                    // Pre-sort the walked directories as order of parsing affects appid matches.
                                    let mut entries = dir
                                        .filter_map(Result::ok)
                                        .map(|entry| (entry.path(), depth))
                                        .collect::<VecDeque<_>>();
                                    entries.make_contiguous().sort_unstable();
                                    entries
                                });

                                continue 'outer;
                            }

                            // Skip directories_to_walk which could not be read or that were already visited
                            _ => continue,
                        }
                    }

                    return None;
                }
            };

            'inner: while let Some((mut path, mut depth)) = paths.pop_front() {
                if !path.exists() {
                    continue 'inner;
                }

                if path.is_dir() {
                    depth += 1;

                    if MAX_DEPTH == depth {
                        continue;
                    }

                    path = match path.canonicalize() {
                        Ok(canonicalized) => canonicalized,
                        Err(_) => continue 'inner,
                    };
                }

                if let Ok(metadata) = path.metadata() {
                    if metadata.is_dir() {
                        // Skip visited directories to mitigate against file system loops
                        if self.visited.insert(path.clone()) {
                            self.directories_to_walk.push_front((path, depth));
                        }
                    } else if metadata.is_file() {
                        self.actively_walking = Some(paths);
                        return Some(path);
                    }
                }
            }
        }
    }
}
