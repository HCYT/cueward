use cueward_core::{AttachmentKind, AttachmentSegment, Cue};

use crate::notes::{AudioAttachment, AudioNote, MEDIA_MATCH_WINDOW_SECS};

use super::match_key;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AudioTranscriptBlock {
    pub(super) index: usize,
    pub(super) filename: String,
    pub(super) text: String,
}

pub(super) fn labels_for_audio(attachments: &[AudioAttachment], placeholder_count: usize) -> Vec<String> {
    attachments
        .iter()
        .take(placeholder_count)
        .map(|attachment| {
            attachment
                .title
                .clone()
                .unwrap_or_else(|| attachment.filename.clone())
        })
        .collect()
}

pub(super) fn materialize_audio_attachments(
    attachments: &[AudioAttachment],
    placeholder_count: usize,
) -> Vec<AudioAttachment> {
    attachments
        .iter()
        .take(placeholder_count)
        .cloned()
        .collect()
}

pub(super) fn collect_audio_transcript_blocks(
    attachments: &[AudioAttachment],
    offset: usize,
    placeholder_count: usize,
) -> Vec<AudioTranscriptBlock> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .filter_map(|(idx, attachment)| {
            let text = attachment.transcript_text.as_deref()?.trim();
            if text.is_empty() {
                return None;
            }

            Some(AudioTranscriptBlock {
                index: offset + idx + 1,
                filename: attachment.filename.clone(),
                text: text.to_string(),
            })
        })
        .collect()
}

pub(super) fn append_audio_transcripts(content: &str, blocks: &[AudioTranscriptBlock]) -> String {
    if blocks.is_empty() {
        return content.to_string();
    }

    let mut sections = Vec::with_capacity(blocks.len());
    for block in blocks {
        sections.push(format!(
            "[Attachment {} Transcript: {}]\n{}",
            block.index, block.filename, block.text
        ));
    }

    format!("{content}\n\n{}", sections.join("\n\n"))
}

pub(super) fn build_audio_segments(
    attachments: &[AudioAttachment],
    offset: usize,
    placeholder_count: usize,
) -> Vec<AttachmentSegment> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .map(|(idx, attachment)| AttachmentSegment {
            index: offset + idx + 1,
            kind: AttachmentKind::Audio,
            title: attachment.title.clone(),
            url: None,
            latitude: None,
            longitude: None,
            filename: Some(attachment.filename.clone()),
            path: Some(attachment.path.display().to_string()),
            sha256: attachment.sha256.clone(),
            duration_seconds: attachment.duration_seconds,
            transcript_text: attachment.transcript_text.clone(),
            ocr_text: None,
            has_ocr: false,
        })
        .collect()
}

pub(super) fn match_audio_note<'a>(cue: &Cue, notes: &'a [AudioNote]) -> Option<&'a AudioNote> {
    let cue_ts = cue.timestamp.timestamp();
    let cue_title = cue.title.as_deref();

    notes
        .iter()
        .filter(|note| (note.timestamp - cue_ts).abs() <= MEDIA_MATCH_WINDOW_SECS)
        .min_by_key(|note| match_key(cue_title, cue_ts, note.title.as_deref(), note.timestamp))
}
