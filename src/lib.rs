//! libmicrovmi is a cross-platform unified virtual machine introsection interface, following a simple design to keep interoperability at heart.
//!
//! Click on this [book 📖](https://libmicrovmi.github.io/) to find our project documentation.

#![allow(clippy::upper_case_acronyms)]

mod driver;
mod memory;

pub mod api;
pub mod capi;
pub mod errors;
pub mod microvmi;
// reexport
pub use crate::microvmi::Microvmi;

#[macro_use]
extern crate log;
#[macro_use]
extern crate bitflags;
