//! Loading executable extensions (P5).
//!
//! An extension is a `*.js`/`*.mjs` file under `~/.autodev/extensions/`. The Rust side only
//! reads the files (name + source); the frontend executes each as an ES module, passing it an
//! `autodev` API to register hooks and composer commands. Extensions run with the app's full
//! trust — they are the user's own files, like a shell script in their home dir — so loading
//! them is surfaced in the UI rather than sandboxed. Hermetic (`_from(dir)`); the argless
//! wrapper uses the real data dir.

use std::path::Path;

use serde::Serialize;

use crate::error::AppResult;

/// One extension file: its `name` (filename stem) and JavaScript `source`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionFile {
    pub name: String,
    pub source: String,
}

/// List the `*.js`/`*.mjs` extensions in `data_dir/extensions`, sorted by name. A missing
/// directory or an unreadable file yields no entry.
pub fn list_extensions_from(data_dir: &Path) -> Vec<ExtensionFile> {
    let dir = data_dir.join("extensions");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    let mut out: Vec<ExtensionFile> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|x| x.to_str()),
                Some("js") | Some("mjs")
            )
        })
        .filter_map(|p| {
            let name = p.file_stem()?.to_str()?.to_string();
            let source = std::fs::read_to_string(&p).ok()?;
            Some(ExtensionFile { name, source })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// List extensions from the real data directory.
pub fn list_extensions() -> AppResult<Vec<ExtensionFile>> {
    Ok(list_extensions_from(&crate::state::data_dir()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_data_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "autodev-ext-{tag}-{:?}",
            std::thread::current().id()
        ));
        std::fs::create_dir_all(dir.join("extensions")).unwrap();
        dir
    }

    #[test]
    fn lists_js_and_mjs_sorted_ignoring_other_files() {
        let data = temp_data_dir("list");
        let ext = data.join("extensions");
        std::fs::write(ext.join("b-hook.js"), "export default () => {}").unwrap();
        std::fs::write(ext.join("a-cmd.mjs"), "export default () => {}").unwrap();
        std::fs::write(ext.join("readme.txt"), "ignored").unwrap();

        let list = list_extensions_from(&data);
        assert_eq!(
            list.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["a-cmd", "b-hook"]
        );
        assert_eq!(list[0].source, "export default () => {}");
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn missing_extensions_dir_is_empty() {
        let data = std::env::temp_dir().join("autodev-ext-none-xyz");
        assert!(list_extensions_from(&data).is_empty());
    }
}
