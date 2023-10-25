//! `is_ignored` module provides an API for applying the ignore rules to a
//! specific path, rather than to all paths in a directory tree.

use crate::dir::IgnoreBuilder;
use std::path::Path;

/// Determines whether the given path is ignored, respecting all the ignore files
/// in any parent directories of the given path.
///
/// NOTE: This API ignores any errors encountered while parsing the ignore files.
pub fn is_path_ignored(path: &Path) -> bool {
    let ig_root = IgnoreBuilder::new().build();
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
