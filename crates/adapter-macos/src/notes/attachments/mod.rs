mod image;
mod web_preview;

use std::collections::HashMap;

use cueward_core::{AttachmentKind, AttachmentSegment, Cue, CueSource};

use super::{
    ATTACHMENT_LABEL, AttachmentOcrBlock, MEDIA_MATCH_WINDOW_SECS, MediaAttachment, MediaNote,
    WebPreviewNote,
};
use image::{collect_attachment_ocr_blocks, materialize_attachments};
use web_preview::{
    build_web_preview_segments, labels_for_web_previews, match_web_preview_note,
};

pub(crate) fn enrich_cues_with_attachments(
    cues: &mut [Cue],
    media_notes: &[MediaNote],
    web_preview_notes: &[WebPreviewNote],
) {
    for cue in cues.iter_mut() {
        if !matches!(cue.source, CueSource::Notes) {
            continue;
        }

        let placeholder_count = attachment_placeholder_count(&cue.content);
        if placeholder_count == 0 {
            continue;
        }

        let mut labels = Vec::new();
        let mut segments = Vec::new();

        if let Some(media_note) = match_media_note(cue, media_notes) {
            if !media_note.attachments.is_empty() {
                let remaining = placeholder_count.saturating_sub(labels.len());
                let attachments = materialize_attachments(&media_note.attachments, remaining);
                if !attachments.is_empty() {
                    labels.extend(
                        attachments
                            .iter()
                            .map(|attachment| attachment.filename.clone()),
                    );

                    let ocr_blocks = collect_attachment_ocr_blocks(&attachments, remaining);
                    if !ocr_blocks.is_empty() {
                        cue.content = append_attachment_ocr(&cue.content, &ocr_blocks);
                    }

                    segments.extend(build_image_segments(&attachments, Some(&ocr_blocks)));
                }
            }
        }

        if labels.len() < placeholder_count {
            if let Some(web_preview_note) = match_web_preview_note(cue, web_preview_notes) {
                if !web_preview_note.attachments.is_empty() {
                    let remaining = placeholder_count.saturating_sub(labels.len());
                    labels.extend(labels_for_web_previews(
                        &web_preview_note.attachments,
                        remaining,
                    ));
                    segments.extend(build_web_preview_segments(
                        &web_preview_note.attachments,
                        remaining,
                    ));
                }
            }
        }

        if !labels.is_empty() {
            cue.content = replace_attachment_labels(&cue.content, &labels, placeholder_count);
        }

        if segments.len() < placeholder_count {
            segments.extend(build_unresolved_segments_from(segments.len(), placeholder_count));
        }

        cue.attachment_segments = segments;
    }
}

#[cfg(test)]
pub(crate) fn enrich_cues_with_media(cues: &mut [Cue], media_notes: &[MediaNote]) {
    for cue in cues.iter_mut() {
        if !matches!(cue.source, CueSource::Notes) {
            continue;
        }

        let placeholder_count = attachment_placeholder_count(&cue.content);
        if placeholder_count == 0 {
            continue;
        }

        let Some(media_note) = match_media_note(cue, media_notes) else {
            continue;
        };

        if media_note.attachments.is_empty() {
            continue;
        }

        cue.content = replace_attachment_labels(
            &cue.content,
            &media_note
                .attachments
                .iter()
                .map(|attachment| attachment.filename.clone())
                .collect::<Vec<_>>(),
            placeholder_count,
        );

        cue.attachment_segments = build_image_segments(&media_note.attachments, None);
    }
}

pub(crate) fn attachment_placeholder_count(content: &str) -> usize {
    content.matches(ATTACHMENT_LABEL).count()
}

fn match_media_note<'a>(cue: &Cue, media_notes: &'a [MediaNote]) -> Option<&'a MediaNote> {
    let cue_ts = cue.timestamp.timestamp();
    let cue_title = cue.title.as_deref();

    media_notes
        .iter()
        .filter(|note| (note.timestamp - cue_ts).abs() <= MEDIA_MATCH_WINDOW_SECS)
        .min_by_key(|note| match_key(cue_title, cue_ts, note.title.as_deref(), note.timestamp))
}

pub(super) fn match_key(
    cue_title: Option<&str>,
    cue_ts: i64,
    note_title: Option<&str>,
    note_ts: i64,
) -> (i32, i64) {
    let title_penalty = match (cue_title, note_title) {
        (Some(cue_title), Some(note_title)) if cue_title == note_title => 0,
        (Some(_), Some(_)) => 1,
        _ => 2,
    };
    (title_penalty, (note_ts - cue_ts).abs())
}

fn replace_attachment_labels(body: &str, labels: &[String], placeholder_count: usize) -> String {
    if placeholder_count == 0 {
        return body.to_string();
    }

    let mut result = body.to_string();
    for (idx, label) in labels.iter().enumerate() {
        if idx >= placeholder_count {
            break;
        }

        let replacement = format!("[Attachment {}: {}]", idx + 1, label);
        result = result.replacen(ATTACHMENT_LABEL, &replacement, 1);
    }

    result
}

fn append_attachment_ocr(content: &str, blocks: &[AttachmentOcrBlock]) -> String {
    if blocks.is_empty() {
        return content.to_string();
    }

    let mut sections = Vec::with_capacity(blocks.len());
    for block in blocks {
        sections.push(format!(
            "[Attachment {} OCR: {}]\n{}",
            block.index, block.filename, block.text
        ));
    }

    format!("{content}\n\n{}", sections.join("\n\n"))
}

fn build_image_segments(
    attachments: &[MediaAttachment],
    ocr_blocks: Option<&[AttachmentOcrBlock]>,
) -> Vec<AttachmentSegment> {
    let ocr_by_attachment: HashMap<String, &AttachmentOcrBlock> = ocr_blocks
        .unwrap_or(&[])
        .iter()
        .map(|block| {
            (
                attachment_lookup_key(block.sha256.as_deref(), &block.filename),
                block,
            )
        })
        .collect();

    attachments
        .iter()
        .enumerate()
        .map(|(idx, attachment)| {
            let key = attachment_lookup_key(attachment.sha256.as_deref(), &attachment.filename);
            let ocr = ocr_by_attachment.get(&key);
            AttachmentSegment {
                index: idx + 1,
                kind: AttachmentKind::Image,
                title: None,
                url: None,
                filename: Some(attachment.filename.clone()),
                path: Some(attachment.path.display().to_string()),
                sha256: attachment.sha256.clone(),
                ocr_text: ocr.map(|block| block.text.clone()),
                has_ocr: ocr.is_some(),
            }
        })
        .collect()
}

fn build_unresolved_segments_from(
    existing_count: usize,
    placeholder_count: usize,
) -> Vec<AttachmentSegment> {
    (existing_count..placeholder_count)
        .map(|idx| AttachmentSegment {
            index: idx + 1,
            kind: AttachmentKind::Unresolved,
            title: None,
            url: None,
            filename: None,
            path: None,
            sha256: None,
            ocr_text: None,
            has_ocr: false,
        })
        .collect()
}

fn attachment_lookup_key(sha256: Option<&str>, filename: &str) -> String {
    sha256.unwrap_or(filename).to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};
    use cueward_core::{AttachmentKind, Cue, CueSource};

    use super::{
        AttachmentOcrBlock, MediaAttachment, MediaNote, append_attachment_ocr,
        attachment_placeholder_count, build_image_segments, enrich_cues_with_media,
        replace_attachment_labels,
    };

    #[test]
    fn replace_attachment_labels_uses_filenames() {
        let body = "[Attachment]\n[Attachment]";
        let attachments = vec![
            MediaAttachment {
                filename: "one.png".into(),
                path: PathBuf::from("/tmp/one.png"),
                sha256: None,
            },
            MediaAttachment {
                filename: "two.jpg".into(),
                path: PathBuf::from("/tmp/two.jpg"),
                sha256: None,
            },
        ];

        let replaced = replace_attachment_labels(
            body,
            &attachments
                .iter()
                .map(|attachment| attachment.filename.clone())
                .collect::<Vec<_>>(),
            2,
        );

        assert_eq!(replaced, "[Attachment 1: one.png]\n[Attachment 2: two.jpg]");
    }

    #[test]
    fn enrich_cues_with_media_adds_paths_and_filenames() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 9, 23, 42, 54).unwrap();
        let mut cues = vec![Cue {
            source: CueSource::Notes,
            timestamp,
            content: "[Attachment]".into(),
            url: None,
            title: Some("新增備忘錄".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        }];
        let media_notes = vec![MediaNote {
            timestamp: timestamp.timestamp() - 1,
            title: None,
            attachments: vec![MediaAttachment {
                filename: "scan.jpg".into(),
                path: PathBuf::from("/tmp/scan.jpg"),
                sha256: Some("abc123".into()),
            }],
        }];

        enrich_cues_with_media(&mut cues, &media_notes);

        assert_eq!(cues[0].content, "[Attachment 1: scan.jpg]");
        assert_eq!(cues[0].attachment_segments.len(), 1);
        assert!(matches!(
            cues[0].attachment_segments[0].kind,
            AttachmentKind::Image
        ));
        assert_eq!(
            cues[0].attachment_segments[0].filename.as_deref(),
            Some("scan.jpg")
        );
        assert_eq!(
            cues[0].attachment_segments[0].path.as_deref(),
            Some("/tmp/scan.jpg")
        );
        assert_eq!(
            cues[0].attachment_segments[0].sha256.as_deref(),
            Some("abc123")
        );
        assert!(!cues[0].attachment_segments[0].has_ocr);
    }

    #[test]
    fn append_attachment_ocr_adds_readable_sections() {
        let content = "[Attachment 1: image.png]";
        let blocks = vec![AttachmentOcrBlock {
            index: 1,
            filename: "image.png".into(),
            sha256: Some("hash1".into()),
            text: "detected text".into(),
        }];

        let combined = append_attachment_ocr(content, &blocks);

        assert_eq!(
            combined,
            "[Attachment 1: image.png]\n\n[Attachment 1 OCR: image.png]\ndetected text"
        );
    }

    #[test]
    fn attachment_placeholder_count_counts_remaining_plain_labels() {
        assert_eq!(
            attachment_placeholder_count("[Attachment]\n[Attachment]"),
            2
        );
        assert_eq!(attachment_placeholder_count("[Attachment 1: image.png]"), 0);
    }

    #[test]
    fn build_attachment_segments_includes_ocr_fields() {
        let attachments = vec![MediaAttachment {
            filename: "image.png".into(),
            path: PathBuf::from("/tmp/image.png"),
            sha256: Some("hash1".into()),
        }];
        let blocks = vec![AttachmentOcrBlock {
            index: 1,
            filename: "image.png".into(),
            sha256: Some("hash1".into()),
            text: "detected text".into(),
        }];

        let segments = build_image_segments(&attachments, Some(&blocks));

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].index, 1);
        assert!(matches!(segments[0].kind, AttachmentKind::Image));
        assert_eq!(segments[0].filename.as_deref(), Some("image.png"));
        assert_eq!(segments[0].path.as_deref(), Some("/tmp/image.png"));
        assert_eq!(segments[0].sha256.as_deref(), Some("hash1"));
        assert_eq!(segments[0].ocr_text.as_deref(), Some("detected text"));
        assert!(segments[0].has_ocr);
    }

    #[test]
    fn build_attachment_segments_prefers_sha256_over_filename() {
        let attachments = vec![
            MediaAttachment {
                filename: "image.png".into(),
                path: PathBuf::from("/tmp/one.png"),
                sha256: Some("hash-a".into()),
            },
            MediaAttachment {
                filename: "image.png".into(),
                path: PathBuf::from("/tmp/two.png"),
                sha256: Some("hash-b".into()),
            },
        ];
        let blocks = vec![
            AttachmentOcrBlock {
                index: 1,
                filename: "image.png".into(),
                sha256: Some("hash-a".into()),
                text: "first".into(),
            },
            AttachmentOcrBlock {
                index: 2,
                filename: "image.png".into(),
                sha256: Some("hash-b".into()),
                text: "second".into(),
            },
        ];

        let segments = build_image_segments(&attachments, Some(&blocks));

        assert_eq!(segments[0].ocr_text.as_deref(), Some("first"));
        assert_eq!(segments[1].ocr_text.as_deref(), Some("second"));
    }

    #[test]
    fn append_attachment_ocr_keeps_original_attachment_indices() {
        let content = "[Attachment 1: one.png]\n[Attachment 2: two.png]";
        let blocks = vec![AttachmentOcrBlock {
            index: 2,
            filename: "two.png".into(),
            sha256: Some("hash2".into()),
            text: "detected text".into(),
        }];

        let combined = append_attachment_ocr(content, &blocks);

        assert!(combined.contains("[Attachment 2 OCR: two.png]"));
    }

    #[test]
    fn enrich_cues_with_attachments_emits_unresolved_when_no_media_matches() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 10, 8, 0, 0).unwrap();
        let mut cues = vec![Cue {
            source: CueSource::Notes,
            timestamp,
            content: "[Attachment]".into(),
            url: None,
            title: Some("Unresolved".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        }];

        super::enrich_cues_with_attachments(&mut cues, &[], &[]);

        assert_eq!(cues[0].attachment_segments.len(), 1);
        assert!(matches!(
            cues[0].attachment_segments[0].kind,
            AttachmentKind::Unresolved
        ));
        assert_eq!(cues[0].attachment_segments[0].filename, None);
        assert_eq!(cues[0].attachment_segments[0].path, None);
        assert!(!cues[0].attachment_segments[0].has_ocr);
    }

    #[test]
    fn enrich_cues_with_attachments_emits_unresolved_when_match_has_no_attachments() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 10, 8, 0, 0).unwrap();
        let mut cues = vec![Cue {
            source: CueSource::Notes,
            timestamp,
            content: "[Attachment]\n[Attachment]".into(),
            url: None,
            title: Some("Still unresolved".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        }];
        let media_notes = vec![MediaNote {
            timestamp: timestamp.timestamp(),
            title: Some("Still unresolved".into()),
            attachments: Vec::new(),
        }];

        super::enrich_cues_with_attachments(&mut cues, &media_notes, &[]);

        assert_eq!(cues[0].attachment_segments.len(), 2);
        assert!(cues[0]
            .attachment_segments
            .iter()
            .all(|segment| matches!(segment.kind, AttachmentKind::Unresolved)));
        assert!(cues[0]
            .attachment_segments
            .iter()
            .all(|segment| segment.path.is_none() && segment.filename.is_none()));
    }

    #[test]
    fn enrich_cues_with_attachments_combines_media_and_web_preview_segments() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 10, 9, 0, 0).unwrap();
        let mut cues = vec![Cue {
            source: CueSource::Notes,
            timestamp,
            content: "[Attachment]\n[Attachment]".into(),
            url: None,
            title: Some("Mixed note".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        }];
        let media_notes = vec![MediaNote {
            timestamp: timestamp.timestamp(),
            title: Some("Mixed note".into()),
            attachments: vec![MediaAttachment {
                filename: "scan.jpg".into(),
                path: PathBuf::from("/tmp/scan.jpg"),
                sha256: Some("abc123".into()),
            }],
        }];
        let web_preview_notes = vec![crate::notes::WebPreviewNote {
            timestamp: timestamp.timestamp(),
            title: Some("Mixed note".into()),
            attachments: vec![crate::notes::WebPreviewAttachment {
                title: Some("Cursor Docs".into()),
                url: "https://docs.cursor.com/guides/working-with-context".into(),
            }],
        }];

        super::enrich_cues_with_attachments(&mut cues, &media_notes, &web_preview_notes);

        assert_eq!(
            cues[0].content,
            "[Attachment 1: scan.jpg]\n[Attachment 2: Cursor Docs]"
        );
        assert_eq!(cues[0].attachment_segments.len(), 2);
        assert!(matches!(
            cues[0].attachment_segments[0].kind,
            AttachmentKind::Image
        ));
        assert!(matches!(
            cues[0].attachment_segments[1].kind,
            AttachmentKind::WebPreview
        ));
    }
}
