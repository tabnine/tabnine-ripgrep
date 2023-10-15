//! Exposes the `is_ignored` API to determine whether a path is ignored.
use crate::dir::IgnoreBuilder;
use std::path::Path;

/// Determines whether the given path is ignored.
pub fn is_ignored(path: &Path) -> bool {
    let (ignore, _e) = IgnoreBuilder::new().build().add_parents(path);
    let mut cur_ig = ignore.clone();
    for ancestor in path.ancestors() {
        if cur_ig.matched(ancestor, ancestor.is_dir()).is_ignore() {
            return true;
        }
        let (ig, _e) = cur_ig.add_child(ancestor);
        cur_ig = ig;
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::is_ignored::is_ignored;
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
        let tempdir = TempDir::new().unwrap();
        mkdirp(tempdir.path().join("kaki/pipi/poopoo"));
        wfile(tempdir.path().join("kaki/.ignore"), "**/*kaki.txt");
        wfile(tempdir.path().join("kaki/pipi/poopoo/a_kaki.txt"), "something");

        assert!(is_ignored(
            &tempdir.path().join("kaki/pipi/poopoo/a_kaki.txt")
        ));
        assert!(!is_ignored(
            &tempdir.path().join("kaki/pipi/poopoo/a_kaki_1.txt")
        ));
    }

    #[test]
    fn ignore_exclude() {
        let tempdir = TempDir::new().unwrap();
        mkdirp(tempdir.path().join("kaki/pipi/poopoo"));
        wfile(
            tempdir.path().join("kaki/.ignore"),
            "**/*kaki.txt\n!**/a_kaki.txt",
        );
        wfile(tempdir.path().join("kaki/pipi/poopoo/a_kaki.txt"), "");
        wfile(tempdir.path().join("kaki/pipi/poopoo/b_kaki.txt"), "");

        assert!(!is_ignored(
            &tempdir.path().join("kaki/pipi/poopoo/a_kaki.txt")
        ));
        assert!(is_ignored(
            &tempdir.path().join("kaki/pipi/poopoo/b_kaki.txt")
        ));
    }

    #[test]
    fn gitignore() {
        let tempdir = TempDir::new().unwrap();
        mkdirp(tempdir.path().join("pipi/zibi"));
        mkdirp(tempdir.path().join("zibi"));
        mkdirp(tempdir.path().join(".git"));

        wfile(tempdir.path().join(".ignore"), "pipi");
        wfile(tempdir.path().join("pipi/a.txt"), "");
        wfile(tempdir.path().join("zibi/a.txt"), "");

        assert!(is_ignored(&tempdir.path().join("pipi/a.txt")));
        assert!(!is_ignored(&tempdir.path().join("zibi/a.txt")));
    }

    #[test]
    fn gitignore_exclude() {
        let tempdir = TempDir::new().unwrap();
        mkdirp(tempdir.path().join("kaki/pipi/poopoo"));
        mkdirp(tempdir.path().join("kaki/.git"));
        wfile(
            tempdir.path().join("kaki/.gitignore"),
            "**/*kaki.txt\n!**/a_kaki.txt",
        );
        wfile(tempdir.path().join("kaki/pipi/poopoo/a_kaki.txt"), "");
        wfile(tempdir.path().join("kaki/pipi/poopoo/b_kaki.txt"), "");

        assert!(!is_ignored(
            &tempdir.path().join("kaki/pipi/poopoo/a_kaki.txt")
        ));
        assert!(is_ignored(
            &tempdir.path().join("kaki/pipi/poopoo/b_kaki.txt")
        ));
    }

    #[test]
    fn multiple_ignore_files() {
        let tempdir = TempDir::new().unwrap();
        mkdirp(tempdir.path().join("kaki/pipi/poopoo"));
        mkdirp(tempdir.path().join("kaki/.git"));
        wfile(tempdir.path().join("kaki/.gitignore"), "pipi/**/*kaki.txt");
        wfile(tempdir.path().join("kaki/pipi/.ignore"), "poopoo");

        wfile(tempdir.path().join("kaki/pipi/poopoo/a_kaki.txt"), "");
        wfile(tempdir.path().join("kaki/pipi/poopoo/zibi.txt"), "");
        wfile(tempdir.path().join("kaki/b_kaki.txt"), "");

        assert!(is_ignored(
            &tempdir.path().join("kaki/pipi/poopoo/a_kaki.txt")
        ));
        assert!(is_ignored(&tempdir.path().join("kaki/pipi/poopoo/zibi.txt")));
        assert!(!is_ignored(&tempdir.path().join("kaki/b_kaki.txt")));
    }
}
