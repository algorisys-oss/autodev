use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// A project is a directory on disk that agents can be pointed at and that can be
/// `@`-mentioned to give an agent context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    /// Display name, derived from the directory's basename.
    pub name: String,
    /// Absolute path to the project directory.
    pub path: String,
}

/// A workspace groups the projects a user works on together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub projects: Vec<Project>,
}

/// The whole persisted set of workspaces.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStore {
    pub workspaces: Vec<Workspace>,
}

/// Directories never worth walking for `@`-mention file listing.
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    ".codegraph",
    ".next",
    ".svelte-kit",
    "build",
    ".venv",
    "__pycache__",
];

/// Cap on files returned by a single mention resolution, so a huge repo does not
/// flood the agent's context.
const MAX_MENTION_FILES: usize = 500;

fn store_path(dir: &Path) -> PathBuf {
    dir.join("workspaces.json")
}

/// Load the workspace store from `dir`, returning an empty store if absent.
pub fn load_store_from(dir: &Path) -> AppResult<WorkspaceStore> {
    match fs::read_to_string(store_path(dir)) {
        Ok(raw) => Ok(serde_json::from_str(&raw)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(WorkspaceStore::default()),
        Err(e) => Err(AppError::Io(e)),
    }
}

/// Write the workspace store to `dir`, creating `dir` if needed.
pub fn save_store_to(dir: &Path, store: &WorkspaceStore) -> AppResult<()> {
    fs::create_dir_all(dir)?;
    fs::write(store_path(dir), serde_json::to_string_pretty(store)?)?;
    Ok(())
}

/// Turn a name into a url-safe slug used as a workspace id.
fn slugify(name: &str) -> String {
    let s: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "workspace".to_string()
    } else {
        s
    }
}

/// Build an id from `name` that does not collide with any existing workspace id.
fn unique_id(store: &WorkspaceStore, name: &str) -> String {
    let base = slugify(name);
    if !store.workspaces.iter().any(|w| w.id == base) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !store.workspaces.iter().any(|w| w.id == candidate) {
            return candidate;
        }
        n += 1;
    }
}

impl WorkspaceStore {
    fn find_mut(&mut self, id: &str) -> AppResult<&mut Workspace> {
        self.workspaces
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or_else(|| AppError::NotFound(format!("workspace {id}")))
    }

    /// Create a new, empty workspace and return it.
    pub fn create_workspace(&mut self, name: &str) -> Workspace {
        let ws = Workspace {
            id: unique_id(self, name),
            name: name.trim().to_string(),
            projects: Vec::new(),
        };
        self.workspaces.push(ws.clone());
        ws
    }

    /// Remove a workspace by id.
    pub fn delete_workspace(&mut self, id: &str) -> AppResult<()> {
        let before = self.workspaces.len();
        self.workspaces.retain(|w| w.id != id);
        if self.workspaces.len() == before {
            return Err(AppError::NotFound(format!("workspace {id}")));
        }
        Ok(())
    }

    /// Add a project directory to a workspace. The project name is the directory
    /// basename. Rejects a path that is not an existing directory or a duplicate.
    pub fn add_project(&mut self, workspace_id: &str, path: &str) -> AppResult<Workspace> {
        let p = Path::new(path);
        if !p.is_dir() {
            return Err(AppError::NotFound(format!("directory {path}")));
        }
        let abs = fs::canonicalize(p)?;
        let abs_str = abs.to_string_lossy().to_string();
        let name = abs
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| abs_str.clone());

        let ws = self.find_mut(workspace_id)?;
        if ws.projects.iter().any(|pr| pr.path == abs_str) {
            return Err(AppError::Conflict(format!(
                "project already added: {abs_str}"
            )));
        }
        ws.projects.push(Project {
            name,
            path: abs_str,
        });
        Ok(ws.clone())
    }

    /// Remove a project from a workspace by its name.
    pub fn remove_project(
        &mut self,
        workspace_id: &str,
        project_name: &str,
    ) -> AppResult<Workspace> {
        let ws = self.find_mut(workspace_id)?;
        let before = ws.projects.len();
        ws.projects.retain(|p| p.name != project_name);
        if ws.projects.len() == before {
            return Err(AppError::NotFound(format!("project {project_name}")));
        }
        Ok(ws.clone())
    }
}

/// Result of resolving an `@`-mention: the matched project and the files under it
/// an agent should see (relative paths, capped and sorted).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedMention {
    pub project: Project,
    pub files: Vec<String>,
    /// True if the listing hit `MAX_MENTION_FILES` and was truncated.
    pub truncated: bool,
}

/// Normalize a token so "Bridge Bench UI", "bridge-bench-ui" and "bridgebenchui"
/// all compare equal.
fn normalize(token: &str) -> String {
    token
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Find the project in `ws` whose name matches `token` (ignoring case, spaces, and
/// hyphens) and list its files.
pub fn resolve_mention(ws: &Workspace, token: &str) -> Option<ResolvedMention> {
    let want = normalize(token);
    let project = ws.projects.iter().find(|p| normalize(&p.name) == want)?;
    let (files, truncated) = list_files(Path::new(&project.path));
    Some(ResolvedMention {
        project: project.clone(),
        files,
        truncated,
    })
}

/// Walk a directory, skipping ignored dirs, returning relative file paths sorted,
/// capped at `MAX_MENTION_FILES`.
fn list_files(root: &Path) -> (Vec<String>, bool) {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut truncated = false;

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if file_type.is_dir() {
                if IGNORED_DIRS.contains(&name.as_str()) {
                    continue;
                }
                stack.push(path);
            } else if file_type.is_file() {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_string_lossy().to_string());
                    if out.len() >= MAX_MENTION_FILES {
                        truncated = true;
                        out.sort();
                        return (out, truncated);
                    }
                }
            }
        }
    }
    out.sort();
    (out, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "autodev-ws-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_and_persist_roundtrips() {
        let dir = temp_dir("persist");
        let mut store = WorkspaceStore::default();
        let ws = store.create_workspace("Bridge Mind");
        assert_eq!(ws.id, "bridge-mind");
        save_store_to(&dir, &store).unwrap();

        let reloaded = load_store_from(&dir).unwrap();
        assert_eq!(reloaded, store);
        assert_eq!(reloaded.workspaces.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unique_ids_do_not_collide() {
        let mut store = WorkspaceStore::default();
        let a = store.create_workspace("app");
        let b = store.create_workspace("app");
        assert_eq!(a.id, "app");
        assert_eq!(b.id, "app-2");
    }

    #[test]
    fn add_and_remove_projects() {
        let dir = temp_dir("projects");
        let proj = dir.join("my-api");
        fs::create_dir_all(&proj).unwrap();

        let mut store = WorkspaceStore::default();
        let ws = store.create_workspace("work");
        let updated = store.add_project(&ws.id, proj.to_str().unwrap()).unwrap();
        assert_eq!(updated.projects.len(), 1);
        assert_eq!(updated.projects[0].name, "my-api");

        // adding the same path again is a conflict
        assert!(store.add_project(&ws.id, proj.to_str().unwrap()).is_err());

        let removed = store.remove_project(&ws.id, "my-api").unwrap();
        assert!(removed.projects.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn add_project_rejects_missing_dir() {
        let mut store = WorkspaceStore::default();
        let ws = store.create_workspace("work");
        assert!(store.add_project(&ws.id, "/no/such/path/here").is_err());
    }

    #[test]
    fn resolve_mention_matches_fuzzily_and_lists_files() {
        let dir = temp_dir("mention");
        let proj = dir.join("bridge-bench-ui");
        fs::create_dir_all(proj.join("src")).unwrap();
        fs::create_dir_all(proj.join("node_modules/dep")).unwrap();
        fs::write(proj.join("src/main.ts"), "x").unwrap();
        fs::write(proj.join("readme.md"), "y").unwrap();
        fs::write(proj.join("node_modules/dep/index.js"), "z").unwrap();

        let mut store = WorkspaceStore::default();
        let ws = store.create_workspace("work");
        store.add_project(&ws.id, proj.to_str().unwrap()).unwrap();
        let ws = &store.workspaces[0];

        // "Bridge Bench UI" normalizes to the same as "bridge-bench-ui"
        let resolved = resolve_mention(ws, "Bridge Bench UI").expect("should match");
        assert_eq!(resolved.project.name, "bridge-bench-ui");
        assert!(resolved.files.contains(&"src/main.ts".to_string()));
        assert!(resolved.files.contains(&"readme.md".to_string()));
        // node_modules is skipped
        assert!(!resolved.files.iter().any(|f| f.contains("node_modules")));
        assert!(!resolved.truncated);

        assert!(resolve_mention(ws, "nonexistent").is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
