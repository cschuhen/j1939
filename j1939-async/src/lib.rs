#![cfg_attr(not(test), no_std)]

#[cfg(feature = "defmt")]
extern crate defmt;

#[cfg(feature = "defmt")]
use defmt as _;

pub mod can;
pub mod error;
pub mod name;
pub mod pgn;
pub mod process_data;
pub mod stack;
pub mod string_utils;

#[cfg_attr(feature = "embassy", path = "os/embassy.rs")]
mod os;

pub use crate::can::Id;


