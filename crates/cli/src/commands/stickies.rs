use std::process;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum StickiesAction {
    /// List stickies
    List,
    /// Create a sticky
    Create {
        /// Sticky title
        #[arg(long)]
        title: String,
        /// Sticky body
        #[arg(long)]
        body: String,
    },
    /// Update a sticky
    Update {
        /// Sticky id
        #[arg(long)]
        id: String,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New body
        #[arg(long)]
        body: Option<String>,
    },
    /// Delete a sticky
    Delete {
        /// Sticky id
        #[arg(long)]
        id: String,
    },
}

pub(crate) fn dispatch(_action: StickiesAction) {
    eprintln!("error: stickies commands not implemented yet");
    process::exit(1);
}
