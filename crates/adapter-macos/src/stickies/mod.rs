use plist::Value;
use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::MacosError;

mod color;
mod display;
mod geometry;
mod state;

use self::color::{color_dictionary, StickyColorPreset};
use self::geometry::{parse_expanded_size, parse_frame, StickyFrame, StickySize};
use self::state::{
    derive_sticky_title, ensure_update_fields, find_entry_index, load_state_entries_for_mutation,
    max_z_order, parse_saved_state, parse_saved_state_value, read_sticky_body, saved_state_path,
    sticky_dir, sticky_rtf_path, stickies_root_dir, write_saved_state, write_sticky_body,
    StickyStateEntry, TITLE_KEY,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StickiesNote {
    pub id: String,
    pub title: String,
    pub body: String,
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
    let mut raw = color_dictionary(0.996078431372549, 0.9176470588235294, 0.2392156862745098, 1.0);
    raw.clear();
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

fn clone_state_entry(
    template: Option<&StickyStateEntry>,
    id: &str,
    title: &str,
    z_order: i64,
) -> StickyStateEntry {
    match template {
        Some(template) => {
            let mut raw = template.raw.clone();
            raw.insert("UUID".into(), Value::String(id.into()));
            raw.insert(TITLE_KEY.into(), Value::String(title.into()));
            raw.insert("ZOrder".into(), Value::Integer(z_order.into()));
            if let Some(frame) = raw.get("Frame").and_then(Value::as_string) {
                if let Ok(parsed) = parse_frame(frame) {
                    raw.insert("Frame".into(), Value::String(parsed.offset(24, -24).to_state_value()));
                }
            }
            StickyStateEntry { id: id.into(), raw }
        }
        None => default_state_entry(id, title),
    }
}

fn create_sticky_in_root(root: &Path, title: &str, body: &str) -> Result<StickiesNote, MacosError> {
    let id = uuid::Uuid::new_v4().to_string().to_uppercase();
    let mut entries = load_state_entries_for_mutation(root)?;
    let next_z_order = max_z_order(&entries) + 1;
    let template = entries.first().cloned();
    entries.push(clone_state_entry(template.as_ref(), &id, title, next_z_order));
    let body_path = sticky_rtf_path(root, &id);
    write_sticky_body(&body_path, body)?;
    if let Err(err) = write_saved_state(&saved_state_path(root), &entries) {
        let _ = fs::remove_dir_all(sticky_dir(root, &id));
        return Err(err);
    }

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
    let index = find_entry_index(&entries, id)?;
    let body_path = sticky_rtf_path(root, id);
    if !body_path.exists() {
        return Err(MacosError::Other(format!("sticky body not found: {id}")));
    }

    let current_body = read_sticky_body(&body_path)?;
    let original_raw = entries[index].raw.clone();
    let next_body = body.unwrap_or(&current_body).to_string();

    if let Some(title) = title {
        entries[index]
            .raw
            .insert(TITLE_KEY.into(), Value::String(title.to_string()));
    }

    if body.is_some() {
        write_sticky_body(&body_path, &next_body)?;
    }
    if let Err(err) = write_saved_state(&saved_state_path(root), &entries) {
        entries[index].raw = original_raw;
        if body.is_some() {
            let _ = write_sticky_body(&body_path, &current_body);
        }
        return Err(err);
    }

    let next_title = entries[index]
        .raw
        .get(TITLE_KEY)
        .and_then(Value::as_string)
        .map(str::to_string)
        .unwrap_or_else(|| derive_sticky_title(None, &next_body, id));

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

    write_saved_state(&saved_state_path(root), &entries)?;
    let dir = sticky_dir(root, id);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|err| MacosError::Other(format!("failed to delete sticky dir: {err}")))?;
    }
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
mod tests;
