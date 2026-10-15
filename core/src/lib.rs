//! Core conversion logic for the SP-404 MKII converter. No UI code lives here.

pub mod config;
pub mod convert;
pub mod decode;
pub mod encode;
pub mod error;
pub mod naming;
pub mod process;
pub mod scan;

pub use error::{CoreError, Result};
