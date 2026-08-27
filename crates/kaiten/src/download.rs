//! Saving card attachments to disk — shared by `card file get` and the MCP
//! `download_file` tool (issue #12).

use std::path::{Path, PathBuf};

use kaiten_client::{CardFile, FileRef, KaitenClient};

use crate::error::CliError;

/// What was saved where; the JSON output of both front ends.
#[derive(Debug, serde::Serialize)]
pub(crate) struct SavedFile {
    pub path: String,
    pub name: String,
    /// Bytes on disk.
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// The attachment's own name reduced to a plain file name: the last path
/// component (either separator), or `file-<ref>` when that is empty, `.`
/// or `..` — a server-supplied name must never escape the target directory.
pub(crate) fn safe_file_name(file: &CardFile) -> String {
    let last = file.name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if last.is_empty() || last == "." || last == ".." {
        format!("file-{}", FileRef::from(file))
    } else {
        last.to_owned()
    }
}

/// The attachment addressed by `file_ref`, or an error that lists what the
/// card actually has.
pub(crate) fn find_file<'a>(
    card_id: u64,
    files: &'a [CardFile],
    file_ref: &FileRef,
) -> Result<&'a CardFile, CliError> {
    files.iter().find(|f| file_ref.matches(f)).ok_or_else(|| {
        let available = if files.is_empty() {
            "(none)".to_owned()
        } else {
            files
                .iter()
                .map(|f| format!("{} ({})", FileRef::from(f), f.name))
                .collect::<Vec<_>>()
                .join(", ")
        };
        CliError::InvalidArg(format!(
            "card {card_id} has no file `{file_ref}`; available: {available}"
        ))
    })
}

/// Where to save: an existing directory joined with `name`, an explicit file
/// path as is, or `default_dir/name`. A path written with a trailing
/// separator means "directory" — if it does not exist, say so instead of
/// silently writing a file under that name.
pub(crate) fn target_path(
    requested: Option<&Path>,
    default_dir: &Path,
    name: &str,
) -> Result<PathBuf, CliError> {
    match requested {
        Some(p) if p.is_dir() => Ok(p.join(name)),
        Some(p) if p.as_os_str().to_string_lossy().ends_with(['/', '\\']) => Err(
            CliError::InvalidArg(format!("directory {} does not exist", p.display())),
        ),
        Some(p) => Ok(p.to_path_buf()),
        None => Ok(default_dir.join(name)),
    }
}

/// Refuse to clobber an existing file unless `force`; `hint` names the
/// caller's override (`--force`, `overwrite=true`).
pub(crate) fn ensure_writable(path: &Path, force: bool, hint: &str) -> Result<(), CliError> {
    if path.exists() && !force {
        return Err(CliError::InvalidArg(format!(
            "{} already exists; pass {hint} to overwrite",
            path.display()
        )));
    }
    Ok(())
}

/// Download `file` to `target` and describe the result; `path` is absolute so
/// an MCP client can find the file regardless of the server's working
/// directory, and a write failure names the target.
pub(crate) async fn save(
    client: &KaitenClient,
    file: &CardFile,
    target: &Path,
) -> Result<SavedFile, CliError> {
    let bytes = client.files().download(file).await?;
    std::fs::write(target, &bytes).map_err(|e| {
        CliError::Io(std::io::Error::new(
            e.kind(),
            format!("cannot write {}: {e}", target.display()),
        ))
    })?;
    let path = std::path::absolute(target).unwrap_or_else(|_| target.to_path_buf());
    Ok(SavedFile {
        path: path.display().to_string(),
        name: file.name.clone(),
        size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        mime_type: file.mime_type.clone(),
    })
}

/// `card file list --json`: the raw file plus the `uid` a newer-storage file
/// is addressed by (see [`FileRef`]), so scripts need not re-derive it.
#[derive(Debug, serde::Serialize)]
pub(crate) struct FileListEntry<'a> {
    #[serde(flatten)]
    pub file: &'a CardFile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
}

impl<'a> From<&'a CardFile> for FileListEntry<'a> {
    fn from(file: &'a CardFile) -> Self {
        let uid = match FileRef::from(file) {
            FileRef::Uid(uid) => Some(uid),
            FileRef::Id(_) => None,
        };
        Self { file, uid }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(json: &str) -> CardFile {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn safe_file_name_strips_directories_and_falls_back() {
        assert_eq!(
            safe_file_name(&file(r#"{"id": 1, "name": "report.xlsx"}"#)),
            "report.xlsx"
        );
        assert_eq!(
            safe_file_name(&file(r#"{"id": 1, "name": "../../etc/passwd"}"#)),
            "passwd"
        );
        assert_eq!(
            safe_file_name(&file(r#"{"id": 1, "name": "a\\b\\c.txt"}"#)),
            "c.txt"
        );
        assert_eq!(
            safe_file_name(&file(r#"{"id": 61256602, "name": ""}"#)),
            "file-61256602"
        );
        assert_eq!(
            safe_file_name(&file(
                r#"{"id": "6a8e66af-0000-0000-0000-000000000000", "name": "..",
                    "url": "/api/v1/cards/cu/files/6a8e66af-0000-0000-0000-000000000000"}"#
            )),
            "file-6a8e66af-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn find_file_matches_id_or_uid_and_lists_available_on_miss() {
        let files = vec![
            file(r#"{"id": 61256602, "name": "probe-attach.txt"}"#),
            file(
                r#"{"id": "6a8e66af-0000-0000-0000-000000000000", "name": "report.xlsx",
                    "url": "/api/v1/cards/cu/files/6a8e66af-0000-0000-0000-000000000000"}"#,
            ),
        ];
        assert_eq!(
            find_file(67_089_469, &files, &FileRef::Id(61_256_602))
                .unwrap()
                .name,
            "probe-attach.txt"
        );
        assert_eq!(
            find_file(
                67_089_469,
                &files,
                &FileRef::Uid("6A8E66AF-0000-0000-0000-000000000000".into())
            )
            .unwrap()
            .name,
            "report.xlsx"
        );
        let err = find_file(67_089_469, &files, &FileRef::Id(999))
            .unwrap_err()
            .to_string();
        assert!(err.contains("no file `999`"), "{err}");
        assert!(err.contains("67089469"), "{err}");
        assert!(err.contains("61256602 (probe-attach.txt)"), "{err}");
        assert!(
            err.contains("6a8e66af-0000-0000-0000-000000000000 (report.xlsx)"),
            "{err}"
        );
        let err = find_file(1, &[], &FileRef::Id(1)).unwrap_err().to_string();
        assert!(err.contains("(none)"), "{err}");
    }

    #[test]
    fn target_path_joins_name_into_existing_dir_or_keeps_explicit_path_or_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            target_path(Some(dir.path()), Path::new("/default"), "a.txt").unwrap(),
            dir.path().join("a.txt")
        );
        let explicit = dir.path().join("renamed.bin");
        assert_eq!(
            target_path(Some(&explicit), Path::new("/default"), "a.txt").unwrap(),
            explicit
        );
        assert_eq!(
            target_path(None, dir.path(), "a.txt").unwrap(),
            dir.path().join("a.txt")
        );
    }

    /// `-o ./downloads/` says "directory": if it does not exist, say so instead
    /// of writing a file called `downloads`.
    #[test]
    fn target_path_rejects_a_missing_directory_named_with_a_trailing_separator() {
        let dir = tempfile::tempdir().unwrap();
        let missing = format!("{}/nodir/", dir.path().display());
        let err = target_path(Some(Path::new(&missing)), Path::new("/default"), "a.txt")
            .unwrap_err()
            .to_string();
        assert!(err.contains("does not exist"), "{err}");
        assert!(err.contains("nodir"), "{err}");
    }

    #[test]
    fn ensure_writable_refuses_existing_unless_force() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.txt");
        assert!(ensure_writable(&path, false, "--force").is_ok());
        std::fs::write(&path, "old").unwrap();
        let err = ensure_writable(&path, false, "--force")
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists"), "{err}");
        assert!(err.contains("--force"), "{err}");
        assert!(ensure_writable(&path, true, "--force").is_ok());
    }
}
