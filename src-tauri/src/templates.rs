//! Prompt templates and the skills directory (P4).
//!
//! Both are file-backed and code-free, like backend specs: a prompt template is a `*.md` file
//! under `~/.autodev/templates/` (the composer expands `/name` to its body), and the skills
//! directory is `~/.autodev/skills/` — when it has any content it is added to every agent's
//! context via a spawn hook. Hermetic (`_from(dir)`) cores; the argless wrappers use the real
//! data dir.

use std::path::Path;

use serde::Serialize;

use crate::error::AppResult;

/// A reusable prompt template: `name` is the filename stem, `body` the file contents.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplate {
    pub name: String,
    pub body: String,
}

/// List the `*.md` templates in `data_dir/templates`, sorted by name. A missing directory or
/// an unreadable file yields no entry rather than an error.
pub fn list_templates_from(data_dir: &Path) -> Vec<PromptTemplate> {
    let dir = data_dir.join("templates");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    let mut out: Vec<PromptTemplate> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("md"))
        .filter_map(|p| {
            let name = p.file_stem()?.to_str()?.to_string();
            let body = std::fs::read_to_string(&p).ok()?;
            Some(PromptTemplate {
                name,
                body: body.trim_end().to_string(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Absolute path of the skills directory (`data_dir/skills`) if it exists and holds at least
/// one entry, else `None` — there's nothing worth adding to an agent's context otherwise.
pub fn skills_dir_from(data_dir: &Path) -> Option<String> {
    let dir = data_dir.join("skills");
    let mut entries = std::fs::read_dir(&dir).ok()?;
    entries.next().map(|_| dir.to_string_lossy().to_string())
}

/// List templates from the real data directory.
pub fn list_templates() -> AppResult<Vec<PromptTemplate>> {
    Ok(list_templates_from(&crate::state::data_dir()?))
}

/// The skills directory from the real data directory, if present and non-empty.
pub fn skills_dir() -> AppResult<Option<String>> {
    Ok(skills_dir_from(&crate::state::data_dir()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_data_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "autodev-tmpl-{tag}-{:?}",
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lists_md_templates_sorted_ignoring_other_files() {
        let data = temp_data_dir("list");
        let tdir = data.join("templates");
        std::fs::create_dir_all(&tdir).unwrap();
        std::fs::write(tdir.join("refactor.md"), "Refactor this for clarity.\n").unwrap();
        std::fs::write(tdir.join("bugfix.md"), "Find and fix the bug.").unwrap();
        std::fs::write(tdir.join("notes.txt"), "ignored").unwrap();

        let list = list_templates_from(&data);
        assert_eq!(
            list.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["bugfix", "refactor"]
        );
        assert_eq!(list[1].body, "Refactor this for clarity."); // trailing newline trimmed
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn missing_templates_dir_is_empty() {
        let data = temp_data_dir("empty");
        assert!(list_templates_from(&data).is_empty());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn skills_dir_present_only_when_it_has_content() {
        let data = temp_data_dir("skills");
        // No skills dir → None.
        assert!(skills_dir_from(&data).is_none());
        // Empty skills dir → None.
        let sdir = data.join("skills");
        std::fs::create_dir_all(&sdir).unwrap();
        assert!(skills_dir_from(&data).is_none());
        // With content → Some(absolute path).
        std::fs::write(sdir.join("style.md"), "house style").unwrap();
        assert_eq!(
            skills_dir_from(&data),
            Some(sdir.to_string_lossy().to_string())
        );
        std::fs::remove_dir_all(&data).ok();
    }
}
