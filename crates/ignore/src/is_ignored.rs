//! `is_ignored` module provides an API for applying the ignore rules to a
//! specific path, rather than to all paths in a directory tree.
use log::warn;

use crate::dir::{Ignore, IgnoreBuilder};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Determines whether the given path is ignored, respecting all the ignore files
/// in any parent directories of the given path.
///
/// NOTE: This API ignores any errors encountered while parsing the ignore files.
pub fn is_path_ignored(
    path: &Path,
    additional_ignore_filenames: Option<&[&str]>,
) -> bool {
    let mut builder = IgnoreBuilder::new();
    if let Some(additional_ignore_filenames) = additional_ignore_filenames {
        for &filename in additional_ignore_filenames {
            builder.add_custom_ignore_filename(filename);
        }
    }
    let ig_root = builder.build();
    let mut cur_ig = ig_root.clone();
    let ancestors = path.ancestors().skip(1).collect::<Vec<&Path>>();
    for ancestor in ancestors.iter().rev() {
        let ig = ig_root.add_parents(ancestor).0;

        if cur_ig.matched(ancestor, ancestor.is_dir()).is_ignore() {
            return true;
        }
        let (igtmp, _e) = ig.add_child(ancestor);

        cur_ig = igtmp;
    }
    cur_ig.matched(path, path.is_dir()).is_ignore()
}

/**
Efficiently cache ignores, so that you do not have to constantly re-create them
**/
pub struct GitignoreCache {
    ignores: HashMap<PathBuf, Ignore>,
    additional_ignore_filenames: Option<Vec<String>>,
    absolute_ignore: Option<Ignore>, // Store absolute path ignores separately
}

impl GitignoreCache {
    /**
    Creates a new GitignoreCache.
    **/
    pub fn new(additional_ignore_filenames: Option<Vec<String>>) -> Self {
        let absolute_ignore =
            if let Some(ref filenames) = additional_ignore_filenames {
                Self::build_absolute_ignore(filenames)
            } else {
                None
            };

        GitignoreCache {
            ignores: HashMap::new(),
            additional_ignore_filenames,
            absolute_ignore,
        }
    }

    /**
    Returns whether the given path is ignored, respecting all the ignore files
    in any parent directories of the given path.
    **/
    pub fn is_ignored(&mut self, path: &Path) -> bool {
        // First check against absolute ignores
        if let Some(ref ignore) = self.absolute_ignore {
            if ignore.matched(path, path.is_dir()).is_ignore() {
                return true;
            }
        }

        // Then check directory-based ignores
        let Some(result) = self.get_ignore(path) else {
            return false;
        };

        let ancestors = path.ancestors().collect::<Vec<&Path>>();
        for ancestor in ancestors.iter().rev() {
            if result.matched(ancestor, ancestor.is_dir()).is_ignore() {
                return true;
            }
        }

        false
    }

    // Create an Ignore just for absolute paths - called once during initialization
    fn build_absolute_ignore(filenames: &[String]) -> Option<Ignore> {
        let mut builder = IgnoreBuilder::new();
        let mut has_valid_ignore = false;

        for filename in filenames {
            let path_obj = Path::new(filename);
            if path_obj.is_absolute() && path_obj.exists() {
                let (gitignore, error) =
                    crate::gitignore::Gitignore::new(path_obj);
                if let Some(err) = &error {
                    warn!(
                        "Warning: Error parsing ignore file at {:?}: {}",
                        path_obj, err
                    );
                }
                builder.add_ignore(gitignore);
                has_valid_ignore = true;
            }
        }

        if has_valid_ignore {
            Some(builder.build())
        } else {
            None
        }
    }

    fn get_ignore(&mut self, path: &Path) -> Option<&Ignore> {
        let parent = self.find_parent_path_with_ignore(path)?;
        match self.ignores.entry(parent.clone()) {
            Entry::Occupied(e) => Some(e.into_mut()),
            Entry::Vacant(e) => {
                let ig = Self::build_ignore_for_path(
                    &parent,
                    self.additional_ignore_filenames.as_ref(),
                );
                Some(e.insert(ig))
            }
        }
    }

    fn build_ignore_for_path(
        path: &Path,
        additional_ignore_filenames: Option<&Vec<String>>,
    ) -> Ignore {
        let mut builder = IgnoreBuilder::new();
        if let Some(additional_ignore_filenames) = additional_ignore_filenames
        {
            for filename in additional_ignore_filenames {
                let path_obj = Path::new(filename);
                // Only handle non-absolute paths here
                if !path_obj.is_absolute() {
                    builder.add_custom_ignore_filename(filename);
                }
            }
        }
        let ig_root = builder.build();
        let mut cur_ig = ig_root.clone();
        let ancestors = path.ancestors().collect::<Vec<&Path>>();
        for ancestor in ancestors.iter().rev() {
            let ig = ig_root.add_parents(ancestor).0;

            let (igtmp, _e) = ig.add_child(ancestor);

            cur_ig = igtmp;
        }
        return cur_ig;
    }

    fn find_parent_path_with_ignore(
        &mut self,
        mut path: &Path,
    ) -> Option<PathBuf> {
        loop {
            if path.is_dir() {
                if path.join(".gitignore").exists() {
                    return Some(path.to_path_buf());
                }

                if path.join(".ignore").exists() {
                    return Some(path.to_path_buf());
                }

                if let Some(additional_ignore_filenames) =
                    &self.additional_ignore_filenames
                {
                    for filename in additional_ignore_filenames {
                        if path.join(filename).exists() {
                            return Some(path.to_path_buf());
                        }
                    }
                }
            }

            path = path.parent()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GitignoreCache;
    use crate::is_ignored::is_path_ignored;
    use crate::tests::TempDir;
    use std::io::Write;
    use std::path::Path;

    fn wfile<P: AsRef<Path>>(path: P, contents: &str) {
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }

    fn mkdirp<P: AsRef<Path>>(path: P) {
        std::fs::create_dir_all(path).unwrap();
    }

    #[test]
    fn ignore() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("foo/bar/baz"));
        wfile(td.path().join("foo/.ignore"), "**/*foo.txt");
        wfile(td.path().join("foo/bar/baz/a_foo.txt"), "something");

        assert!(is_path_ignored(
            &td.path().join("foo/bar/baz/a_foo.txt"),
            None
        ));
        assert!(!is_path_ignored(
            &td.path().join("foo/bar/baz/a_foo_1.txt"),
            None
        ));
    }
    #[test]
    fn ignore_tabnine() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("foo/bar/baz"));
        wfile(
            td.path().join("foo/.tabnineignore"),
            "**/*foo.txt\n!**/a_foo.txt",
        );
        wfile(td.path().join("foo/bar/baz/a_foo.txt"), "something");
        wfile(td.path().join("foo/bar/baz/b_foo.txt"), "");

        assert!(!is_path_ignored(
            &td.path().join("foo/bar/baz/a_foo.txt"),
            Some(&[".tabnineignore"])
        ));
        assert!(is_path_ignored(
            &td.path().join("foo/bar/baz/b_foo.txt"),
            Some(&[".tabnineignore"])
        ));
    }

    #[test]
    fn ignore_exclude() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("foo/bar/baz"));
        wfile(td.path().join("foo/.ignore"), "**/*foo.txt\n!**/a_foo.txt");
        wfile(td.path().join("foo/bar/baz/a_foo.txt"), "");
        wfile(td.path().join("foo/bar/baz/b_foo.txt"), "");

        assert!(!is_path_ignored(
            &td.path().join("foo/bar/baz/a_foo.txt"),
            None
        ));
        assert!(is_path_ignored(
            &td.path().join("foo/bar/baz/b_foo.txt"),
            None
        ));
    }

    #[test]
    fn gitignore() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("bar/zibi"));
        mkdirp(td.path().join("zibi"));
        mkdirp(td.path().join(".git"));

        wfile(td.path().join(".gitignore"), "bar");
        wfile(td.path().join("bar/a.txt"), "");
        wfile(td.path().join("zibi/a.txt"), "");

        assert!(is_path_ignored(&td.path().join("bar/a.txt"), None));
        assert!(!is_path_ignored(&td.path().join("zibi/a.txt"), None));
    }

    #[test]
    fn gitignore_exclude() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("foo/bar/baz"));
        mkdirp(td.path().join("foo/.git"));
        wfile(td.path().join("foo/.gitignore"), "**/*foo.txt\n!**/a_foo.txt");
        wfile(td.path().join("foo/bar/baz/a_foo.txt"), "");
        wfile(td.path().join("foo/bar/baz/b_foo.txt"), "");

        assert!(!is_path_ignored(
            &td.path().join("foo/bar/baz/a_foo.txt"),
            None
        ));
        assert!(is_path_ignored(
            &td.path().join("foo/bar/baz/b_foo.txt"),
            None
        ));
    }

    #[test]
    fn multiple_ignore_files() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("foo/bar/baz"));
        mkdirp(td.path().join("foo/.git"));
        wfile(td.path().join("foo/.gitignore"), "bar/**/*foo.txt");
        wfile(td.path().join("foo/bar/.ignore"), "baz");

        wfile(td.path().join("foo/bar/baz/a_foo.txt"), "");
        wfile(td.path().join("foo/bar/baz/zibi.txt"), "");
        wfile(td.path().join("foo/b_foo.txt"), "");

        assert!(is_path_ignored(
            &td.path().join("foo/bar/baz/a_foo.txt"),
            None
        ));
        assert!(is_path_ignored(
            &td.path().join("foo/bar/baz/zibi.txt"),
            None
        ));
        assert!(!is_path_ignored(&td.path().join("foo/b_foo.txt"), None));
    }

    #[test]
    fn should_resolve_ignore_rules_correctly() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("foo/.git"));
        mkdirp(td.path().join("foo/bar/baz"));

        wfile(td.path().join("foo/.gitignore"), "/bar.txt");

        wfile(td.path().join("foo/bar.txt"), "");
        wfile(td.path().join("foo/bar/baz/bar.txt"), "");
        wfile(td.path().join("foo/bar/baz/zibi.txt"), "");

        assert!(is_path_ignored(&td.path().join("foo/bar.txt"), None));
        assert!(!is_path_ignored(
            &td.path().join("foo/bar/baz/bar.txt"),
            None
        ));
        assert!(!is_path_ignored(
            &td.path().join("foo/bar/baz/zibi.txt"),
            None
        ));
    }

    #[test]
    fn gitignore_cache_with_absolute_path() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("project/src"));

        // Create test files
        wfile(td.path().join("project/src/test.go"), "");
        wfile(td.path().join("project/src/test.txt"), "");

        // Create an ignore file at a completely separate location
        let ignore_dir = TempDir::new().unwrap();
        let abs_ignore_path = ignore_dir.path().join("global_ignore");
        wfile(&abs_ignore_path, "*.go");

        // Test the GitignoreCache with absolute path
        let abs_path_str = abs_ignore_path.to_str().unwrap().to_string();
        let mut cache = GitignoreCache::new(Some(vec![abs_path_str]));

        assert!(cache.is_ignored(&td.path().join("project/src/test.go")));
        assert!(!cache.is_ignored(&td.path().join("project/src/test.txt")));
    }

    #[test]
    fn gitignore_cache_with_mixed_paths() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("project/src"));

        // Create a relative ignore file
        wfile(td.path().join("project/.customignore"), "*.txt");

        // Create test files
        wfile(td.path().join("project/src/test.go"), "");
        wfile(td.path().join("project/src/test.txt"), "");
        wfile(td.path().join("project/src/test.md"), "");

        // Create an ignore file at a completely separate location
        let ignore_dir = TempDir::new().unwrap();
        let abs_ignore_path = ignore_dir.path().join("global_ignore");
        wfile(&abs_ignore_path, "*.go");

        // Test the GitignoreCache with both absolute and custom ignore files
        let abs_path_str = abs_ignore_path.to_str().unwrap().to_string();
        let mut cache = GitignoreCache::new(Some(vec![
            abs_path_str,
            ".customignore".to_string(),
        ]));

        assert!(cache.is_ignored(&td.path().join("project/src/test.go")));
        assert!(cache.is_ignored(&td.path().join("project/src/test.txt")));
        assert!(!cache.is_ignored(&td.path().join("project/src/test.md")));
    }

    #[test]
    fn gitignore_cache_with_nonexistent_path() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("project/src"));

        // Create test files
        wfile(td.path().join("project/src/test.go"), "");

        // Test with a nonexistent absolute path - should be ignored
        let nonexistent_path = "/path/that/does/not/exist/ignore";
        let mut cache =
            GitignoreCache::new(Some(vec![nonexistent_path.to_string()]));

        // Should not crash and should not ignore the file
        assert!(!cache.is_ignored(&td.path().join("project/src/test.go")));
    }

    #[test]
    fn gitignore_cache_with_valid_and_invalid_absolute_paths() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("project/src"));

        // Create test files
        wfile(td.path().join("project/src/test.go"), "");
        wfile(td.path().join("project/src/test.txt"), "");

        // Create a valid ignore file
        let ignore_dir = TempDir::new().unwrap();
        let abs_ignore_path = ignore_dir.path().join("global_ignore");
        wfile(&abs_ignore_path, "*.go");

        // Test with both valid and invalid paths
        let abs_path_str = abs_ignore_path.to_str().unwrap().to_string();
        let nonexistent_path = "/path/that/does/not/exist/ignore";
        let mut cache = GitignoreCache::new(Some(vec![
            nonexistent_path.to_string(),
            abs_path_str,
        ]));

        // The valid path should still work even with an invalid one in the list
        assert!(cache.is_ignored(&td.path().join("project/src/test.go")));
        assert!(!cache.is_ignored(&td.path().join("project/src/test.txt")));
    }
}
