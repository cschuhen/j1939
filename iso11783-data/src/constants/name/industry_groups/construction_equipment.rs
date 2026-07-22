/// Source: NAME lookup tables (Manufacturer IDs, Industry Groups, Global NAME Functions, IG Specific NAME Function) rev 1, downloaded 2026-07-24
pub const NON_SPECIFIC_SYSTEM: u32 = 0;
pub const SKID_STEER_LOADER: u32 = 1;
pub const ARTICULATED_DUMP_TRUCK: u32 = 2;
pub const BACKHOE: u32 = 3;
pub const CRAWLER: u32 = 4;
pub const EXCAVATOR: u32 = 5;
pub const FORKLIFT: u32 = 6;
pub const FOUR_WHEEL_DRIVE_LOADER: u32 = 7;
pub const GRADER: u32 = 8;
pub const MILLING_MACHINE: u32 = 9;
pub const RECYCLER_AND_SOIL_STABILIZER: u32 = 10;
pub const BINDING_AGENT_SPREADER: u32 = 11;
pub const PAVER: u32 = 12;
pub const FEEDER: u32 = 13;
pub const SCREENING_PLANT: u32 = 14;
pub const STACKER: u32 = 15;
pub const ROLLER: u32 = 16;
pub const CRUSHER: u32 = 17;
pub const NOT_AVAILABLE: u32 = 127;



/// Vehicle system: non specific system
pub mod non_specific_system {
    pub const SUPPLEMENTAL_ENGINE_CONTROL_SENSING: u16 = 128;
    pub const LASER_RECEIVER: u16 = 129;
    pub const LAND_LEVELING_SYSTEM_OPERATOR_INTERFACE: u16 = 130;
    pub const LAND_LEVELING_ELECTRIC_MAST: u16 = 131;
    pub const SINGLE_LAND_LEVELING_SYSTEM_SUPERVISOR: u16 = 132;
    pub const LAND_LEVELING_SYSTEM_DISPLAY: u16 = 133;
    pub const LASER_TRACER: u16 = 134;
    pub const LOADER_CONTROL: u16 = 135;
    pub const SLOPE_SENSOR: u16 = 136;
    pub const LIFTARM_CONTROL: u16 = 137;
    pub const SUPPLEMENTAL_SENSOR_PROCESSING_UNITS: u16 = 138;
    pub const HYDRAULIC_SYSTEM_PLANNER: u16 = 139;
    pub const HYDRAULIC_VALVE_CONTROLLER: u16 = 140;
    pub const JOYSTICK_CONTROL: u16 = 141;
    pub const ROTATION_SENSOR: u16 = 142;
    pub const SONIC_SENSOR: u16 = 143;
    pub const SURVEY_TOTAL_STATION_TARGET: u16 = 144;
    pub const HEADING_SENSOR: u16 = 145;
    pub const ALARM_DEVICE: u16 = 146;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: skid steer loader
pub mod skid_steer_loader {
    pub const MAIN_CONTROLLER: u16 = 128;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: articulated dump truck
pub mod articulated_dump_truck {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: backhoe
pub mod backhoe {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: crawler
pub mod crawler {
    pub const BLADE_CONTROLLER: u16 = 128;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: excavator
pub mod excavator {
    pub const SLOPE_SENSOR: u16 = 128;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: forklift
pub mod forklift {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: four wheel drive loader
pub mod four_wheel_drive_loader {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: grader
pub mod grader {
    pub const HFWD_CONTROLLER: u16 = 128;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: milling machine
pub mod milling_machine {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: recycler and soil stabilizer
pub mod recycler_and_soil_stabilizer {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: binding agent spreader
pub mod binding_agent_spreader {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: paver
pub mod paver {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: feeder
pub mod feeder {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: screening plant
pub mod screening_plant {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: stacker
pub mod stacker {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: roller
pub mod roller {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: crusher
pub mod crusher {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: not available
pub mod not_available {
    pub const NOT_AVAILABLE: u16 = 255;
}

