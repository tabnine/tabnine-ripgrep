//! `is_ignored` module provides an API for applying the ignore rules to a
//! specific path, rather than to all paths in a directory tree.

use crate::dir::{Ignore, IgnoreBuilder};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Determines whether the given path is ignored, respecting all the ignore files
/// in any parent directories of the given path.
///
/// NOTE: This API ignores any errors encountered while parsing the ignore files.
pub fn is_path_ignored(path: &Path) -> bool {
    let ig_root = IgnoreBuilder::new()
        .add_custom_ignore_filename(".tabnineignore")
        .build();
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
}

impl GitignoreCache {
    /**
    Creates a new GitignoreCache.
    **/
    pub fn new() -> GitignoreCache {
        GitignoreCache { ignores: HashMap::new() }
    }

    /**
    Returns whether the given path is ignored, respecting all the ignore files
    in any parent directories of the given path.
    **/
    pub fn is_ignored(&mut self, path: &Path) -> bool {
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

    fn get_ignore(&mut self, path: &Path) -> Option<&Ignore> {
        let parent = Self::find_parent_path_with_ignore(path)?;
        match self.ignores.entry(parent.clone()) {
            Entry::Occupied(e) => Some(e.into_mut()),
            Entry::Vacant(e) => {
                let ig = Self::build_ignore_for_path(&parent);
                Some(e.insert(ig))
            }
        }
    }

    fn build_ignore_for_path(path: &Path) -> Ignore {
        let ig_root = IgnoreBuilder::new().build();
        let mut cur_ig = ig_root.clone();
        let ancestors = path.ancestors().collect::<Vec<&Path>>();
        for ancestor in ancestors.iter().rev() {
            let ig = ig_root.add_parents(ancestor).0;

            let (igtmp, _e) = ig.add_child(ancestor);

            cur_ig = igtmp;
        }
        return cur_ig;
    }

    fn find_parent_path_with_ignore(mut path: &Path) -> Option<PathBuf> {
        loop {
            if path.is_dir() {
                if path.join(".gitignore").exists() {
                    return Some(path.to_path_buf());
                }

                if path.join(".ignore").exists() {
                    return Some(path.to_path_buf());
                }

                if path.join(".tabnineignore").exists() {
                    return Some(path.to_path_buf());
                }
            }

            path = path.parent()?;
        }
    }
}

#[cfg(test)]
mod tests {
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

        assert!(is_path_ignored(&td.path().join("foo/bar/baz/a_foo.txt")));
        assert!(!is_path_ignored(&td.path().join("foo/bar/baz/a_foo_1.txt")));
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

        assert!(!is_path_ignored(&td.path().join("foo/bar/baz/a_foo.txt")));
        assert!(is_path_ignored(&td.path().join("foo/bar/baz/b_foo.txt")));
    }

    #[test]
    fn ignore_exclude() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("foo/bar/baz"));
        wfile(td.path().join("foo/.ignore"), "**/*foo.txt\n!**/a_foo.txt");
        wfile(td.path().join("foo/bar/baz/a_foo.txt"), "");
        wfile(td.path().join("foo/bar/baz/b_foo.txt"), "");

        assert!(!is_path_ignored(&td.path().join("foo/bar/baz/a_foo.txt")));
        assert!(is_path_ignored(&td.path().join("foo/bar/baz/b_foo.txt")));
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

        assert!(is_path_ignored(&td.path().join("bar/a.txt")));
        assert!(!is_path_ignored(&td.path().join("zibi/a.txt")));
    }

    #[test]
    fn gitignore_exclude() {
        let td = TempDir::new().unwrap();
        mkdirp(td.path().join("foo/bar/baz"));
        mkdirp(td.path().join("foo/.git"));
        wfile(td.path().join("foo/.gitignore"), "**/*foo.txt\n!**/a_foo.txt");
        wfile(td.path().join("foo/bar/baz/a_foo.txt"), "");
        wfile(td.path().join("foo/bar/baz/b_foo.txt"), "");

        assert!(!is_path_ignored(&td.path().join("foo/bar/baz/a_foo.txt")));
        assert!(is_path_ignored(&td.path().join("foo/bar/baz/b_foo.txt")));
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

        assert!(is_path_ignored(&td.path().join("foo/bar/baz/a_foo.txt")));
        assert!(is_path_ignored(&td.path().join("foo/bar/baz/zibi.txt")));
        assert!(!is_path_ignored(&td.path().join("foo/b_foo.txt")));
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

        assert!(is_path_ignored(&td.path().join("foo/bar.txt")));
        assert!(!is_path_ignored(&td.path().join("foo/bar/baz/bar.txt")));
        assert!(!is_path_ignored(&td.path().join("foo/bar/baz/zibi.txt")));
    }
}
