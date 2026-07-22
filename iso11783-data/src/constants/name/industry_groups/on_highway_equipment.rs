/// Source: NAME lookup tables (Manufacturer IDs, Industry Groups, Global NAME Functions, IG Specific NAME Function) rev 1, downloaded 2026-07-24
pub const NON_SPECIFIC_SYSTEM: u32 = 0;
pub const TRACTOR: u32 = 1;
pub const TRAILER: u32 = 2;
pub const NOT_AVAILABLE: u32 = 127;



/// Vehicle system: non specific system
pub mod non_specific_system {
    pub const TACHOGRAPH: u16 = 128;
    pub const DOOR_CONTROLLER: u16 = 129;
    pub const ARTICULATION_TURNTABLE_CONTROL: u16 = 130;
    pub const BODY_TO_VEHICLE_INTERFACE_CONTROL: u16 = 131;
    pub const SLOPE_SENSOR: u16 = 132;
    pub const RETARDER_DISPLAY: u16 = 134;
    pub const DIFFERENTIAL_LOCK_CONTROLLER: u16 = 135;
    pub const LOW_VOLTAGE_DISCONNECT: u16 = 136;
    pub const ROADWAY_INFORMATION: u16 = 137;
    pub const AUTOMATED_DRIVING: u16 = 138;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: tractor
pub mod tractor {
    pub const FORWARD_ROAD_IMAGE_PROCESSING: u16 = 128;
    pub const FIFTH_WHEEL_SMART_SYSTEM: u16 = 129;
    pub const CATALYST_FLUID_SENSOR: u16 = 130;
    pub const ADAPTIVE_FRONT_LIGHTING_SYSTEM: u16 = 131;
    pub const IDLE_CONTROL_SYSTEM: u16 = 132;
    pub const USER_INTERFACE_SYSTEM: u16 = 133;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: trailer
pub mod trailer {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: not available
pub mod not_available {
    pub const NOT_AVAILABLE: u16 = 255;
}

