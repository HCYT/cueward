use std::path::PathBuf;
use crate::MacosError;

pub fn home_dir() -> Result<PathBuf, MacosError> {
    dirs::home_dir().ok_or_else(|| MacosError::Other("could not determine home directory".into()))
}
