use plist::{Dictionary, Value};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::MacosError;

const SAVED_STATE_FILENAME: &str = ".SavedStickiesState";
const TITLE_KEY: &str = "CuewardTitle";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StickiesNote {
    pub id: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone)]
struct StickyStateEntry {
    id: String,
    raw: Dictionary,
}

fn stickies_root_dir() -> Result<PathBuf, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME environment variable must be set".into()))?;
    Ok(PathBuf::from(home).join("Library/Containers/com.apple.Stickies/Data/Library/Stickies"))
}

fn saved_state_path(root: &Path) -> PathBuf {
    root.join(SAVED_STATE_FILENAME)
}

fn sticky_dir(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}.rtfd"))
}

fn sticky_rtf_path(root: &Path, id: &str) -> PathBuf {
    sticky_dir(root, id).join("TXT.rtf")
}

fn parse_saved_state_value(value: Value) -> Result<Vec<StickyStateEntry>, MacosError> {
    let entries = value
        .into_array()
        .ok_or_else(|| MacosError::Other("stickies state is not an array".into()))?;

    let mut parsed = Vec::new();
    for entry in entries {
        let raw = entry
            .into_dictionary()
            .ok_or_else(|| MacosError::Other("stickies state entry is not a dictionary".into()))?;
        let id = raw
            .get("UUID")
            .and_then(Value::as_string)
            .map(str::to_string)
            .ok_or_else(|| MacosError::Other("stickies state entry missing UUID".into()))?;
        parsed.push(StickyStateEntry { id, raw });
    }

    Ok(parsed)
}

fn parse_saved_state(path: &Path) -> Result<Vec<StickyStateEntry>, MacosError> {
    let value = Value::from_file(path)
        .map_err(|err| MacosError::Other(format!("failed to decode stickies state: {err}")))?;
    parse_saved_state_value(value)
}

fn write_saved_state(path: &Path, entries: &[StickyStateEntry]) -> Result<(), MacosError> {
    let value = Value::Array(
        entries
            .iter()
            .map(|entry| Value::Dictionary(entry.raw.clone()))
            .collect(),
    );
    value
        .to_file_xml(path)
        .map_err(|err| MacosError::Other(format!("failed to write stickies state: {err}")))
}

fn ensure_update_fields(title: Option<&str>, body: Option<&str>) -> Result<(), MacosError> {
    if title.is_none() && body.is_none() {
        return Err(MacosError::Other("no sticky updates specified".into()));
    }
    Ok(())
}

fn derive_sticky_title(stored_title: Option<&str>, body: &str, id: &str) -> String {
    stored_title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            body.lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| format!("Sticky {}", &id[..id.len().min(8)]))
}

fn read_sticky_body(path: &Path) -> Result<String, MacosError> {
    let output = Command::new("textutil")
        .arg("-convert")
        .arg("txt")
        .arg("-stdout")
        .arg(path)
        .output()
        .map_err(|err| MacosError::Other(format!("textutil: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("textutil failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
}

fn write_sticky_body(path: &Path, body: &str) -> Result<(), MacosError> {
    let parent = path
        .parent()
        .ok_or_else(|| MacosError::Other("invalid sticky path".into()))?;
    fs::create_dir_all(parent)
        .map_err(|err| MacosError::Other(format!("failed to create sticky dir: {err}")))?;

    let mut child = Command::new("textutil")
        .arg("-convert")
        .arg("rtf")
        .arg("-stdin")
        .arg("-stdout")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| MacosError::Other(format!("textutil: {err}")))?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        stdin
            .write_all(body.as_bytes())
            .map_err(|err| MacosError::Other(format!("failed to write sticky body: {err}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| MacosError::Other(format!("textutil: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("textutil failed: {stderr}")));
    }

    fs::write(path, output.stdout)
        .map_err(|err| MacosError::Other(format!("failed to write sticky rtf: {err}")))
}

fn list_stickies_from_root(root: &Path) -> Result<Vec<StickiesNote>, MacosError> {
    let state_path = saved_state_path(root);
    if !state_path.exists() {
        return Err(MacosError::Other("stickies state not found".into()));
    }

    let entries = parse_saved_state(&state_path)?;
    let mut notes = Vec::new();
    for entry in entries {
        let body_path = sticky_rtf_path(root, &entry.id);
        if !body_path.exists() {
            continue;
        }
        let body = read_sticky_body(&body_path)?;
        let stored_title = entry.raw.get(TITLE_KEY).and_then(Value::as_string);
        let title = derive_sticky_title(stored_title, &body, &entry.id);
        notes.push(StickiesNote {
            id: entry.id,
            title,
            body,
        });
    }

    Ok(notes)
}

fn default_state_entry(id: &str, title: &str) -> StickyStateEntry {
    let mut raw = Dictionary::new();
    raw.insert("UUID".into(), Value::String(id.into()));
    raw.insert("ExpandedSize".into(), Value::String("{300, 200}".into()));
    raw.insert("ExpandFrameY".into(), Value::Integer(0.into()));
    raw.insert("Floating".into(), Value::Integer(0.into()));
    raw.insert("Frame".into(), Value::String("{{200, 900}, {300, 200}}".into()));
    raw.insert("SpellCheckingTypes".into(), Value::Integer(9191.into()));
    raw.insert("Translucent".into(), Value::Integer(0.into()));
    raw.insert("ZOrder".into(), Value::Integer(1.into()));
    raw.insert(TITLE_KEY.into(), Value::String(title.into()));
    StickyStateEntry { id: id.into(), raw }
}

fn load_state_entries_for_mutation(root: &Path) -> Result<Vec<StickyStateEntry>, MacosError> {
    let state_path = saved_state_path(root);
    if !state_path.exists() {
        return Ok(Vec::new());
    }
    parse_saved_state(&state_path)
}

fn find_entry_mut<'a>(
    entries: &'a mut [StickyStateEntry],
    id: &str,
) -> Result<&'a mut StickyStateEntry, MacosError> {
    entries
        .iter_mut()
        .find(|entry| entry.id == id)
        .ok_or_else(|| MacosError::Other(format!("sticky not found: {id}")))
}

fn create_sticky_in_root(root: &Path, title: &str, body: &str) -> Result<StickiesNote, MacosError> {
    let id = uuid::Uuid::new_v4().to_string().to_uppercase();
    let mut entries = load_state_entries_for_mutation(root)?;
    entries.push(default_state_entry(&id, title));
    write_sticky_body(&sticky_rtf_path(root, &id), body)?;
    write_saved_state(&saved_state_path(root), &entries)?;

    Ok(StickiesNote {
        id,
        title: title.to_string(),
        body: body.to_string(),
    })
}

fn update_sticky_in_root(
    root: &Path,
    id: &str,
    title: Option<&str>,
    body: Option<&str>,
) -> Result<StickiesNote, MacosError> {
    ensure_update_fields(title, body)?;

    let mut entries = load_state_entries_for_mutation(root)?;
    let entry = find_entry_mut(&mut entries, id)?;
    let body_path = sticky_rtf_path(root, id);
    if !body_path.exists() {
        return Err(MacosError::Other(format!("sticky body not found: {id}")));
    }

    let next_body = match body {
        Some(body) => body.to_string(),
        None => read_sticky_body(&body_path)?,
    };
    let next_title = match title {
        Some(title) => title.to_string(),
        None => entry
            .raw
            .get(TITLE_KEY)
            .and_then(Value::as_string)
            .map(str::to_string)
            .unwrap_or_else(|| derive_sticky_title(None, &next_body, id)),
    };

    entry
        .raw
        .insert(TITLE_KEY.into(), Value::String(next_title.clone()));
    write_sticky_body(&body_path, &next_body)?;
    write_saved_state(&saved_state_path(root), &entries)?;

    Ok(StickiesNote {
        id: id.to_string(),
        title: next_title,
        body: next_body,
    })
}

fn delete_sticky_in_root(root: &Path, id: &str) -> Result<(), MacosError> {
    let mut entries = load_state_entries_for_mutation(root)?;
    let original_len = entries.len();
    entries.retain(|entry| entry.id != id);
    if entries.len() == original_len {
        return Err(MacosError::Other(format!("sticky not found: {id}")));
    }

    let dir = sticky_dir(root, id);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|err| MacosError::Other(format!("failed to delete sticky dir: {err}")))?;
    }
    write_saved_state(&saved_state_path(root), &entries)?;
    Ok(())
}

/// List all Stickies notes.
pub fn list_stickies() -> Result<Vec<StickiesNote>, MacosError> {
    list_stickies_from_root(&stickies_root_dir()?)
}

/// Create a Stickies note.
pub fn create_sticky(title: &str, body: &str) -> Result<StickiesNote, MacosError> {
    create_sticky_in_root(&stickies_root_dir()?, title, body)
}

/// Update a Stickies note by UUID.
pub fn update_sticky(id: &str, title: Option<&str>, body: Option<&str>) -> Result<StickiesNote, MacosError> {
    update_sticky_in_root(&stickies_root_dir()?, id, title, body)
}

/// Delete a Stickies note by UUID.
pub fn delete_sticky(id: &str) -> Result<(), MacosError> {
    delete_sticky_in_root(&stickies_root_dir()?, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_saved_state() -> Value {
        Value::Array(vec![Value::Dictionary({
            let mut raw = Dictionary::new();
            raw.insert(
                "UUID".into(),
                Value::String("DF260009-9714-421B-BB65-D2B413C55F46".into()),
            );
            raw
        })])
    }

    #[test]
    fn parse_saved_state_reads_uuid_entries() {
        let entries = parse_saved_state_value(sample_saved_state()).expect("state");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "DF260009-9714-421B-BB65-D2B413C55F46");
    }

    #[test]
    fn sticky_title_falls_back_from_body_when_missing() {
        let title = derive_sticky_title(None, "第一行\n第二行", "sticky-1");

        assert_eq!(title, "第一行");
    }

    #[test]
    fn sticky_title_falls_back_to_id_when_body_empty() {
        let title = derive_sticky_title(None, "", "DF260009-9714-421B-BB65-D2B413C55F46");

        assert_eq!(title, "Sticky DF260009");
    }

    #[test]
    fn update_requires_title_or_body() {
        let err = ensure_update_fields(None, None).unwrap_err();

        assert_eq!(err.to_string(), "no sticky updates specified");
    }

    #[test]
    fn read_sticky_body_decodes_rtf_via_textutil() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("TXT.rtf");
        write_sticky_body(&path, "第一行\n第二行").expect("write rtf");

        let body = read_sticky_body(&path).expect("read rtf");

        assert_eq!(body, "第一行\n第二行");
    }

    #[test]
    fn create_sticky_writes_rtfd_and_updates_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let sticky = create_sticky_in_root(temp.path(), "臨時待辦", "記得回覆客戶")
            .expect("create sticky");

        assert!(saved_state_path(temp.path()).exists());
        assert!(sticky_rtf_path(temp.path(), &sticky.id).exists());

        let notes = list_stickies_from_root(temp.path()).expect("list");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "臨時待辦");
        assert_eq!(notes[0].body, "記得回覆客戶");
    }

    #[test]
    fn update_sticky_rewrites_title_and_body_for_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let sticky = create_sticky_in_root(temp.path(), "舊標題", "舊內容")
            .expect("create sticky");

        let updated = update_sticky_in_root(
            temp.path(),
            &sticky.id,
            Some("新標題"),
            Some("新內容"),
        )
        .expect("update sticky");

        assert_eq!(updated.title, "新標題");
        assert_eq!(updated.body, "新內容");

        let notes = list_stickies_from_root(temp.path()).expect("list");
        assert_eq!(notes[0].title, "新標題");
        assert_eq!(notes[0].body, "新內容");
    }

    #[test]
    fn delete_sticky_removes_state_and_rtfd() {
        let temp = tempfile::tempdir().expect("tempdir");
        let sticky = create_sticky_in_root(temp.path(), "標題", "內容")
            .expect("create sticky");

        delete_sticky_in_root(temp.path(), &sticky.id).expect("delete sticky");

        assert!(!sticky_dir(temp.path(), &sticky.id).exists());
        let notes = list_stickies_from_root(temp.path()).expect("list");
        assert!(notes.is_empty());
    }
}
