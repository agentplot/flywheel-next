//! The world's git reads over a repository on this computer (185, S28).

use crate::git::{self, Repo};

struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!(
            "flywheel-git-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A line's log is its newest commits first, as many as asked, each with its
/// short hash, subject, author and moment; a line the repository does not have
/// logs nothing rather than failing the page that asked (185, S28).
#[test]
fn a_log_is_the_newest_commits_first_and_nothing_for_a_line_not_there() {
    let scratch = Scratch::new("log");
    let repo = Repo::at(&scratch.0);
    repo.git(&["init", "--initial-branch=main", "--quiet", "."]).unwrap();
    for subject in ["first", "second", "third"] {
        repo.git(&["commit", "--allow-empty", "--quiet", "-m", subject]).unwrap();
    }

    let log = git::log(&repo, "main", 2).unwrap();
    let subjects: Vec<&str> = log.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, ["third", "second"]);
    for commit in &log {
        assert!(commit.hash.len() >= 7 && commit.hash.chars().all(|c| c.is_ascii_hexdigit()), "{commit:?}");
        assert_eq!(commit.author, "flywheel");
        assert!(chrono::DateTime::parse_from_rfc3339(&commit.at).is_ok(), "{commit:?}");
    }

    assert!(git::log(&repo, "bolt/atlas/never-made", 5).unwrap().is_empty());
}
