use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Run `git` in `dir` with `args`, returning stdout or a `Git` error with stderr.
fn run_git(dir: &Path, args: &[&str]) -> AppResult<String> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(AppError::Io)?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(AppError::Git(if msg.is_empty() {
            format!("git {:?} failed", args)
        } else {
            msg
        }));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Is `dir` inside a git work tree?
pub fn is_repo(dir: &Path) -> bool {
    run_git(dir, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s.trim() == "true")
        .unwrap_or(false)
}

/// The currently checked-out branch name in `dir`.
pub fn current_branch(dir: &Path) -> AppResult<String> {
    Ok(run_git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string())
}

/// Branch + whether the work tree has uncommitted changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeStatus {
    pub branch: String,
    pub dirty: bool,
}

pub fn status(path: &Path) -> AppResult<WorktreeStatus> {
    let branch = current_branch(path)?;
    let porcelain = run_git(path, &["status", "--porcelain"])?;
    Ok(WorktreeStatus {
        branch,
        dirty: !porcelain.trim().is_empty(),
    })
}

/// Create a new worktree of `repo` at `path`, checked out on a new branch `branch`.
pub fn create_worktree(repo: &Path, path: &Path, branch: &str) -> AppResult<()> {
    let path_str = path
        .to_str()
        .ok_or_else(|| AppError::Git("non-utf8 path".into()))?;
    run_git(repo, &["worktree", "add", "-b", branch, path_str])?;
    Ok(())
}

/// The diff of `branch` against the repo's current HEAD (what merging would bring in).
pub fn diff(repo: &Path, branch: &str) -> AppResult<String> {
    run_git(repo, &["diff", &format!("HEAD...{branch}")])
}

/// Merge `branch` into the repo's current branch (no fast-forward). Refuses if the
/// target work tree is dirty, so a merge never clobbers uncommitted local work.
pub fn merge(repo: &Path, branch: &str) -> AppResult<String> {
    if status(repo)?.dirty {
        return Err(AppError::Git(
            "target working tree has uncommitted changes; commit or stash first".into(),
        ));
    }
    run_git(
        repo,
        &["merge", "--no-ff", "-m", &format!("Merge {branch}"), branch],
    )
}

/// Remove a worktree. `force` also drops uncommitted changes in it.
pub fn remove_worktree(repo: &Path, path: &Path, force: bool) -> AppResult<()> {
    let path_str = path
        .to_str()
        .ok_or_else(|| AppError::Git("non-utf8 path".into()))?;
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path_str);
    run_git(repo, &args)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "autodev-git-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn commit(dir: &Path, msg: &str) {
        run_git(
            dir,
            &[
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=test",
                "commit",
                "-m",
                msg,
            ],
        )
        .unwrap();
    }

    fn init_repo(dir: &Path) {
        run_git(dir, &["init", "-b", "main"]).unwrap();
        fs::write(dir.join("readme.md"), "hello").unwrap();
        run_git(dir, &["add", "."]).unwrap();
        commit(dir, "init");
    }

    #[test]
    fn is_repo_detects_git() {
        let dir = tmp("isrepo");
        assert!(!is_repo(&dir));
        init_repo(&dir);
        assert!(is_repo(&dir));
        assert_eq!(current_branch(&dir).unwrap(), "main");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn worktree_create_commit_merge_flow() {
        let base = tmp("flow");
        let repo = base.join("repo");
        fs::create_dir_all(&repo).unwrap();
        init_repo(&repo);

        let wt = base.join("wt");
        create_worktree(&repo, &wt, "feature").unwrap();
        assert!(wt.join("readme.md").exists());
        assert_eq!(status(&wt).unwrap().branch, "feature");

        // do work in the worktree and commit it
        fs::write(wt.join("feature.txt"), "new work").unwrap();
        run_git(&wt, &["add", "."]).unwrap();
        commit(&wt, "add feature");
        assert!(!status(&wt).unwrap().dirty);

        // diff from the repo shows the new file
        let d = diff(&repo, "feature").unwrap();
        assert!(d.contains("feature.txt"));

        // merge back into main
        merge(&repo, "feature").unwrap();
        assert!(repo.join("feature.txt").exists());

        remove_worktree(&repo, &wt, false).unwrap();
        assert!(!wt.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn merge_refuses_dirty_target() {
        let base = tmp("dirty");
        let repo = base.join("repo");
        fs::create_dir_all(&repo).unwrap();
        init_repo(&repo);
        create_worktree(&repo, &base.join("wt"), "feature").unwrap();

        // make the target repo dirty
        fs::write(repo.join("readme.md"), "changed").unwrap();
        assert!(status(&repo).unwrap().dirty);
        assert!(merge(&repo, "feature").is_err());
        let _ = fs::remove_dir_all(&base);
    }
}
