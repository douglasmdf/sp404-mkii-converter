//! Core conversion logic for the SP-404 MKII converter. No UI code lives here.

pub mod analyze;
pub mod config;
pub mod convert;
pub mod decode;
pub mod encode;
pub mod error;
pub mod labeling;
pub mod name_tags;
pub mod naming;
pub mod process;
pub mod runner;
pub mod scan;
pub mod space;
pub mod tags;
pub mod undo;

pub use error::{CoreError, Result};
