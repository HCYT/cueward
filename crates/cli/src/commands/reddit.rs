use std::process;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum RedditAction {
    /// Read a subreddit feed
    Feed {
        /// Subreddit name, e.g. rust or r/rust
        subreddit: String,
        /// Max posts to return
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Read a Reddit post and its top-level comments
    Post {
        /// Reddit post URL
        url: String,
    },
    /// Search Reddit posts
    Search {
        /// Search query
        query: String,
        /// Restrict search to a subreddit, e.g. rust or r/rust
        #[arg(long)]
        subreddit: Option<String>,
        /// Max posts to return
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

pub(crate) fn dispatch(_action: RedditAction) {
    eprintln!("error: reddit command not implemented yet");
    process::exit(1);
}
