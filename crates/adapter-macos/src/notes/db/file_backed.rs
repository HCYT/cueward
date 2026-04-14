use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use cueward_core::AttachmentKind;

use crate::MacosError;
use crate::notes::{FileBackedAttachment, FileBackedNote};

use super::{
    apple_to_unix_timestamp, normalize_media_title, notes_group_container_path, open_notes_db,
    resolve_media_path, since_apple_epoch,
};

pub(crate) fn load_file_backed_notes(since: DateTime<Utc>) -> Result<Vec<FileBackedNote>, MacosError> {
    let conn = open_notes_db()?;
    let media_root = notes_group_container_path()?.join("Accounts");

    let mut stmt = conn
        .prepare(
            r#"
        SELECT
            COALESCE(
                note.ZMODIFICATIONDATE,
                note.ZMODIFICATIONDATE1,
                attachment.ZMODIFICATIONDATE,
                attachment.ZMODIFICATIONDATE1
            ),
            note.ZTITLE1,
            attachment.ZTYPEUTI,
            attachment.ZTITLE,
            media.ZFILENAME,
            media.ZIDENTIFIER
        FROM ZICCLOUDSYNCINGOBJECT AS attachment
        JOIN ZICCLOUDSYNCINGOBJECT AS note
            ON attachment.ZNOTE = note.Z_PK
        JOIN ZICCLOUDSYNCINGOBJECT AS media
            ON attachment.ZMEDIA = media.Z_PK
        WHERE attachment.ZTYPEUTI IN ('com.adobe.pdf', 'public.data')
          AND media.ZFILENAME IS NOT NULL
          AND media.ZIDENTIFIER IS NOT NULL
          AND COALESCE(
                note.ZMODIFICATIONDATE,
                note.ZMODIFICATIONDATE1,
                attachment.ZMODIFICATIONDATE,
                attachment.ZMODIFICATIONDATE1
              ) > ?
        "#,
        )
        .map_err(|err| MacosError::Other(format!("failed to prepare file-backed query: {err}")))?;

    let mut rows = stmt
        .query([since_apple_epoch(since)])
        .map_err(|err| MacosError::Other(format!("failed to query file-backed notes: {err}")))?;

    let mut grouped: HashMap<(i64, Option<String>), Vec<FileBackedAttachment>> = HashMap::new();
    while let Some(row) = rows
        .next()
        .map_err(|err| MacosError::Other(format!("failed to read file-backed row: {err}")))?
    {
        let modification_date: f64 = row
            .get(0)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed modification date: {err}")))?;
        let note_title: Option<String> = row
            .get(1)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed note title: {err}")))?;
        let type_uti: String = row
            .get(2)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed type: {err}")))?;
        let attachment_title: Option<String> = row
            .get(3)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed title: {err}")))?;
        let filename: String = row
            .get(4)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed filename: {err}")))?;
        let identifier: String = row
            .get(5)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed identifier: {err}")))?;

        let Some(path) = resolve_media_path(&media_root, &identifier, &filename) else {
            continue;
        };
        let Some(kind) = attachment_kind_from_uti(&type_uti) else {
            continue;
        };
        let Some(attachment) = file_backed_attachment_from_row(kind, attachment_title, filename, path) else {
            continue;
        };

        let timestamp = apple_to_unix_timestamp(modification_date);
        grouped
            .entry((timestamp, normalize_media_title(note_title)))
            .or_default()
            .push(attachment);
    }

    Ok(grouped
        .into_iter()
        .map(|((timestamp, title), attachments)| FileBackedNote {
            timestamp,
            title,
            attachments,
        })
        .collect())
}

fn attachment_kind_from_uti(uti: &str) -> Option<AttachmentKind> {
    match uti {
        "com.adobe.pdf" | "public.pdf" => Some(AttachmentKind::Pdf),
        "public.data" => Some(AttachmentKind::Binary),
        _ => None,
    }
}

fn file_backed_attachment_from_row(
    kind: AttachmentKind,
    title: Option<String>,
    filename: String,
    path: PathBuf,
) -> Option<FileBackedAttachment> {
    if filename.trim().is_empty() {
        return None;
    }

    Some(FileBackedAttachment {
        kind,
        title: normalize_media_title(title),
        filename,
        path,
        sha256: None,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use cueward_core::AttachmentKind;

    use super::{attachment_kind_from_uti, file_backed_attachment_from_row};

    #[test]
    fn file_backed_attachment_from_row_builds_pdf_attachment() {
        let attachment = file_backed_attachment_from_row(
            AttachmentKind::Pdf,
            Some("SK-INFLUX [V MB]_DS_C0919.pdf".into()),
            "SK-INFLUX [V MB]_DS_C0919.pdf".into(),
            PathBuf::from("/tmp/SK-INFLUX [V MB]_DS_C0919.pdf"),
        )
        .expect("pdf attachment");

        assert_eq!(attachment.kind, AttachmentKind::Pdf);
        assert_eq!(
            attachment.title.as_deref(),
            Some("SK-INFLUX [V MB]_DS_C0919.pdf")
        );
        assert_eq!(attachment.filename, "SK-INFLUX [V MB]_DS_C0919.pdf");
        assert_eq!(
            attachment.path,
            PathBuf::from("/tmp/SK-INFLUX [V MB]_DS_C0919.pdf")
        );
    }

    #[test]
    fn file_backed_attachment_from_row_uses_filename_when_title_missing() {
        let attachment = file_backed_attachment_from_row(
            AttachmentKind::Binary,
            None,
            "blob.bin".into(),
            PathBuf::from("/tmp/blob.bin"),
        )
        .expect("binary attachment");

        assert_eq!(attachment.kind, AttachmentKind::Binary);
        assert_eq!(attachment.title, None);
        assert_eq!(attachment.filename, "blob.bin");
    }

    #[test]
    fn attachment_kind_from_uti_maps_pdf_and_binary() {
        assert_eq!(attachment_kind_from_uti("com.adobe.pdf"), Some(AttachmentKind::Pdf));
        assert_eq!(attachment_kind_from_uti("public.pdf"), Some(AttachmentKind::Pdf));
        assert_eq!(attachment_kind_from_uti("public.data"), Some(AttachmentKind::Binary));
        assert_eq!(attachment_kind_from_uti("public.url"), None);
    }
}
