/// Source: NAME lookup tables (Manufacturer IDs, Industry Groups, Global NAME Functions, IG Specific NAME Function) rev 1, downloaded 2026-07-24
pub const NON_SPECIFIC_SYSTEM: u32 = 0;
pub const TRACTOR: u32 = 1;
pub const TILLAGE: u32 = 2;
pub const SECONDARY_TILLAGE: u32 = 3;
pub const PLANTERS_SEEDERS: u32 = 4;
pub const FERTILIZERS: u32 = 5;
pub const SPRAYERS: u32 = 6;
pub const HARVESTERS: u32 = 7;
pub const ROOT_HARVESTERS: u32 = 8;
pub const FORAGE: u32 = 9;
pub const IRRIGATION: u32 = 10;
pub const TRANSPORT_TRAILER: u32 = 11;
pub const FARM_YARD_OPERATIONS: u32 = 12;
pub const POWERED_AUXILIARY_DEVICES: u32 = 13;
pub const SPECIAL_CROPS: u32 = 14;
pub const EARTH_WORK: u32 = 15;
pub const SKIDDER: u32 = 16;
pub const SENSOR_SYSTEMS: u32 = 17;
pub const TIMBER_HARVESTERS: u32 = 19;
pub const FORWARDERS: u32 = 20;
pub const TIMBER_LOADERS: u32 = 21;
pub const TIMBER_PROCESSING_MACHINES: u32 = 22;
pub const MULCHERS: u32 = 23;
pub const UTILITY_VEHICLES: u32 = 24;
pub const SLURRY_MANURE_APPLICATORS: u32 = 25;
pub const FEEDERS_MIXERS: u32 = 26;
pub const WEEDERS_NON_CHEMICAL_WEED_CONTROL_THE_DEVICE_CLASS_WEEDERS_IS_U: u32 = 27;
pub const TURF_AND_LAWN_CARE_MOWERS: u32 = 28;
pub const PRODUCT_MATERIAL_HANDLING: u32 = 29;
pub const NOT_AVAILABLE: u32 = 127;



/// Vehicle system: non specific system
pub mod non_specific_system {
    pub const NON_VIRTUAL_TERMINAL_DISPLAY: u16 = 128;
    pub const OPERATOR_CONTROLS_MACHINE_SPECIFIC: u16 = 129;
    pub const TASK_CONTROLLER_MAPPING_COMPUTER: u16 = 130;
    pub const POSITION_CONTROL: u16 = 131;
    pub const MACHINE_CONTROL: u16 = 132;
    pub const FOREIGN_OBJECT_DETECTION: u16 = 133;
    pub const TRACTOR_ECU: u16 = 134;
    pub const SEQUENCE_CONTROL_MASTER: u16 = 135;
    pub const PRODUCT_DOSING: u16 = 136;
    pub const PRODUCT_TREATMENT: u16 = 137;
    pub const RESERVED: u16 = 138;
    pub const DATA_LOGGER: u16 = 139;
    pub const DECISION_SUPPORT: u16 = 140;
    pub const LIGHTING_CONTROLLER: u16 = 141;
    pub const TIM_SERVER: u16 = 142;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: tractor
pub mod tractor {
    pub const AUXILIARY_VALVE_CONTROL: u16 = 129;
    pub const REAR_HITCH_CONTROL: u16 = 130;
    pub const FRONT_HITCH_CONTROL: u16 = 131;
    pub const TRACTOR_MACHINE_CONTROL: u16 = 132;
    pub const CENTER_HITCH_CONTROL: u16 = 134;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: tillage
pub mod tillage {
    pub const TILLAGE_MACHINE_CONTROL: u16 = 132;
    pub const TILLAGE_DEPTH_CONTROL: u16 = 135;
    pub const FRAME_CONTROL: u16 = 136;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: secondary tillage
pub mod secondary_tillage {
    pub const SECONDARY_TILLAGE_MACHINE_CONTROL: u16 = 132;
    pub const SECONDARY_TILLAGE_DEPTH_CONTROL: u16 = 135;
    pub const FRAME_CONTROL: u16 = 136;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: planters seeders
pub mod planters_seeders {
    pub const SEED_RATE_CONTROL: u16 = 128;
    pub const SECTION_ON_OFF_CONTROL: u16 = 129;
    pub const POSITION_CONTROL: u16 = 131;
    pub const PLANTERS_SEEDERS_MACHINE_CONTROL: u16 = 132;
    pub const PRODUCT_FLOW: u16 = 133;
    pub const PRODUCT_LEVEL: u16 = 134;
    pub const DEPTH_CONTROL: u16 = 135;
    pub const FRAME_CONTROL: u16 = 136;
    pub const DOWN_PRESSURE: u16 = 137;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: fertilizers
pub mod fertilizers {
    pub const FERTILIZE_RATE_CONTROL: u16 = 128;
    pub const SECTION_ON_OFF_CONTROL: u16 = 129;
    pub const PRODUCT_PRESSURE: u16 = 130;
    pub const POSITION_CONTROL: u16 = 131;
    pub const FERTILIZERS_MACHINE_CONTROL: u16 = 132;
    pub const PRODUCT_FLOW: u16 = 133;
    pub const PRODUCT_LEVEL: u16 = 134;
    pub const HEIGHT_DEPTH_CONTROL: u16 = 135;
    pub const FRAME_CONTROL: u16 = 136;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: sprayers
pub mod sprayers {
    pub const SPRAY_RATE_CONTROL: u16 = 128;
    pub const SECTION_ON_OFF_CONTROL: u16 = 129;
    pub const PRODUCT_PRESSURE: u16 = 130;
    pub const POSITION_CONTROL: u16 = 131;
    pub const SPRAYERS_MACHINE_CONTROL: u16 = 132;
    pub const PRODUCT_FLOW: u16 = 133;
    pub const PRODUCT_LEVEL: u16 = 134;
    pub const BOOM_HEIGHT_CONTROL: u16 = 135;
    pub const FRAME_CONTROL: u16 = 136;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: harvesters
pub mod harvesters {
    pub const TAILING_MONITOR: u16 = 128;
    pub const HEADER_CONTROL: u16 = 129;
    pub const PRODUCT_LOSS_MONITOR: u16 = 130;
    pub const PRODUCT_MOISTURE: u16 = 131;
    pub const HARVESTER_MACHINE_CONTROL: u16 = 132;
    pub const PRODUCT_FLOW: u16 = 133;
    pub const PRODUCT_LEVEL: u16 = 134;
    pub const HEADER_HEIGHT_CONTROL: u16 = 135;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: root harvesters
pub mod root_harvesters {
    pub const ROOT_HARVESTERS_MACHINE_CONTROL: u16 = 132;
    pub const PRODUCT_FLOW: u16 = 133;
    pub const PRODUCT_LEVEL: u16 = 134;
    pub const DEPTH_CONTROL: u16 = 135;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: forage
pub mod forage {
    pub const TWINE_WRAPPER_CONTROL: u16 = 128;
    pub const PRODUCT_PACKAGING_CONTROL: u16 = 129;
    pub const PRODUCT_MOISTURE: u16 = 131;
    pub const FORAGE_MACHINE_CONTROL: u16 = 132;
    pub const PRODUCT_FLOW: u16 = 133;
    pub const WORKING_HEIGHT_CONTROL: u16 = 135;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: irrigation
pub mod irrigation {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: transport trailer
pub mod transport_trailer {
    pub const TRANSPORT_MACHINE_CONTROL: u16 = 132;
    pub const UNLOAD_CONTROL: u16 = 136;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: farm yard operations
pub mod farm_yard_operations {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: powered auxiliary devices
pub mod powered_auxiliary_devices {
    pub const POWERED_DEVICES_MACHINE_CONTROL: u16 = 132;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: special crops
pub mod special_crops {
    pub const SPECIAL_CROP_MACHINE_CONTROL: u16 = 132;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: earth work
pub mod earth_work {
    pub const MATERIAL_RATE_CONTROL: u16 = 128;
    pub const EARTHWORKS_MACHINE_CONTROL: u16 = 132;
    pub const MATERIAL_FLOW: u16 = 133;
    pub const MATERIAL_LEVEL: u16 = 134;
    pub const DEPTH_CONTROL: u16 = 135;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: skidder
pub mod skidder {
    pub const SKIDDER_MACHINE_CONTROL: u16 = 132;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: sensor systems
pub mod sensor_systems {
    pub const GUIDANCE_FEELER: u16 = 128;
    pub const CAMERA_SYSTEM: u16 = 129;
    pub const CROP_SCOUTING: u16 = 130;
    pub const MATERIAL_PROPERTIES_SENSING: u16 = 131;
    pub const INERTIAL_MEASUREMENT_UNIT_IMU: u16 = 132;
    pub const PRODUCT_FLOW: u16 = 133;
    pub const PRODUCT_LEVEL: u16 = 134;
    pub const PRODUCT_MASS: u16 = 135;
    pub const VIBRATION_KNOCK: u16 = 136;
    pub const WEATHER_INSTRUMENTS: u16 = 137;
    pub const SOIL_SCOUTING: u16 = 138;
}

/// Vehicle system: timber harvesters
pub mod timber_harvesters {
    pub const TIMBER_HARVESTORS_MACHINE_CONTROL: u16 = 132;
}

/// Vehicle system: forwarders
pub mod forwarders {
    pub const FORWARDERS_MACHINE_CONTROL: u16 = 132;
}

/// Vehicle system: timber loaders
pub mod timber_loaders {
    pub const TIMBER_LOADERS_MACHINE_CONTROL: u16 = 132;
}

/// Vehicle system: timber processing machines
pub mod timber_processing_machines {
    pub const TIMBER_PROCESSING_MACHINE_CONTROL: u16 = 132;
}

/// Vehicle system: mulchers
pub mod mulchers {
    pub const MULCHER_MACHINE_CONTROL: u16 = 132;
}

/// Vehicle system: utility vehicles
pub mod utility_vehicles {
    pub const UTILITY_MACHINE_CONTROL: u16 = 132;
}

/// Vehicle system: slurry manure applicators
pub mod slurry_manure_applicators {
    pub const SLURRY_MANURE_RATE_CONTROL: u16 = 128;
    pub const SECTION_ON_OFF_CONTROL: u16 = 129;
    pub const PRODUCT_PRESSURE: u16 = 130;
    pub const SLURRY_MANURE_MACHINE_CONTROL: u16 = 132;
    pub const PRODUCT_FLOW: u16 = 133;
    pub const PRODUCT_LEVEL: u16 = 134;
    pub const BOOM_HEIGHT_CONTROL: u16 = 135;
}

/// Vehicle system: feeders mixers
pub mod feeders_mixers {
    pub const FEEDER_MIXER_RATE_CONTROL: u16 = 128;
    pub const SECTION_ON_OFF_CONTROL: u16 = 129;
    pub const PRODUCT_PRESSURE: u16 = 130;
    pub const FEEDER_MIXER_MACHINE_CONTROL: u16 = 132;
    pub const PRODUCT_FLOW: u16 = 133;
    pub const PRODUCT_LEVEL: u16 = 134;
    pub const BOOM_HEIGHT_CONTROL: u16 = 135;
}

/// Vehicle system: weeders non chemical weed control the device class weeders is u
pub mod weeders_non_chemical_weed_control_the_device_class_weeders_is_u {
    pub const WEEDER_MACHINE_CONTROL: u16 = 132;
}

/// Vehicle system: turf and lawn care mowers
pub mod turf_and_lawn_care_mowers {
    pub const TURF_AND_LAWN_CARE_MOWERS_MACHINE_CONTROL: u16 = 132;
}

/// Vehicle system: product material handling
pub mod product_material_handling {
    pub const PRODUCT_MATERIAL_HANDLING_MACHINE_CONTROL: u16 = 132;
    pub const PRODUCT_MATERIAL_HANDLING_PRODUCT_FLOW: u16 = 133;
    pub const PRODUCT_MATERIAL_HANDLING_PRODUCT_LEVEL: u16 = 134;
}

/// Vehicle system: not available
pub mod not_available {
    pub const NOT_AVAILABLE: u16 = 255;
}

