use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("not a folder: {0}")]
    NotADirectory(PathBuf),
    #[error("not found: {0}")]
    NotFound(PathBuf),
    #[error("not an audio file: {0}")]
    NotAudioFile(PathBuf),
    #[error("output folder {dest} overlaps the source folder {source_dir}")]
    OutputInsideSource { dest: PathBuf, source_dir: PathBuf },
    #[error("no output folder chosen")]
    NoOutputDir,
    #[error("unsupported audio format")]
    UnsupportedFormat,
    #[error("could not decode audio: {0}")]
    Decode(String),
    #[error("file has {0} channels and multichannel downmix is off")]
    TooManyChannels(usize),
    #[error("resampling failed: {0}")]
    Resample(String),
    #[error("could not write WAV: {0}")]
    Encode(String),
    #[error("not enough disk space: need {needed} bytes, {available} available")]
    InsufficientSpace { needed: u64, available: u64 },
    #[error("the file has no audible sound")]
    NoAudio,
    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub(crate) fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> CoreError + '_ {
    move |source| CoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}
