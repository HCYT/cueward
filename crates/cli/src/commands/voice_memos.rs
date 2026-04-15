use clap::Subcommand;
use std::process;

#[derive(Subcommand)]
pub(crate) enum VoiceMemosAction {
    /// List voice memos
    List,
    /// Read one voice memo by id
    Read {
        /// Voice memo id
        #[arg(long)]
        id: String,
    },
}

pub(crate) fn dispatch(_action: VoiceMemosAction) {
    eprintln!("error: voice memos commands not implemented yet");
    process::exit(1);
}
