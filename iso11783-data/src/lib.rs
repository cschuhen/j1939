#![no_std]

#[cfg(feature = "pgn")]
#[path = "constants/pgn.rs"]
mod pgn;

#[cfg(feature = "isobus_params")]
#[path = "constants/isobus_params.rs"]
mod isobus_params;

#[cfg(feature = "task_controller_ddi")]
#[path = "constants/task_controller_ddi.rs"]
mod task_controller_ddi;

#[cfg(feature = "name")]
#[path = "constants/name/mod.rs"]
mod name;

// Strings modules (lookup tables and functions)
#[cfg(feature = "pgn")]
#[path = "strings/pgn.rs"]
mod strings_pgn;

#[cfg(feature = "isobus_params")]
#[path = "strings/isobus_params.rs"]
mod strings_isobus_params;

#[cfg(feature = "task_controller_ddi")]
#[path = "strings/task_controller_ddi.rs"]
mod strings_task_controller_ddi;

#[cfg(feature = "name")]
#[path = "strings/name.rs"]
mod strings_name;

// Constants modules (named constants)
#[cfg(any(
    feature = "pgn",
    feature = "isobus_params",
    feature = "task_controller_ddi",
    feature = "name"
))]
pub mod constants {
    #[cfg(feature = "pgn")]
    pub mod pgn {
        pub use crate::pgn::*;
    }

    #[cfg(feature = "task_controller_ddi")]
    pub mod task_controller_ddi {
        pub use crate::task_controller_ddi::*;
    }

    #[cfg(feature = "name")]
    pub mod name {
        pub use crate::name::*;
    }
}

// Strings module (lookup tables and functions)
#[cfg(any(
    feature = "pgn",
    feature = "isobus_params",
    feature = "task_controller_ddi",
    feature = "name"
))]
pub mod strings {
    #[cfg(feature = "pgn")]
    pub mod pgn {
        pub use crate::strings_pgn::*;
    }

    #[cfg(feature = "isobus_params")]
    pub mod isobus_params {
        pub use crate::strings_isobus_params::*;
    }

    #[cfg(feature = "task_controller_ddi")]
    pub mod task_controller_ddi {
        pub use crate::strings_task_controller_ddi::*;
    }

    #[cfg(feature = "name")]
    pub mod name {
        pub use crate::strings_name::*;
    }
}
