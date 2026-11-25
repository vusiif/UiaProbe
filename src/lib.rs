//! UiaProbe core library for Windows UI Automation structure perception.

pub mod cli;
pub mod coords;
pub mod error;
pub mod filter;
pub mod hash;
pub mod model;
pub mod uia;

pub use error::{ProbeError, Result};
