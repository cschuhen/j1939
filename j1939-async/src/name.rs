#![deny(warnings)]

mod name;
mod name_manager;
mod name_table;

pub use name::identiy_from_bytes;
pub use name::Name;
pub use name_manager::NameManager;
pub type Manager = name_manager::NameManager;
