/// Source: NAME lookup tables (Manufacturer IDs, Industry Groups, Global NAME Functions, IG Specific NAME Function) rev 1, downloaded 2026-07-24
pub const NON_SPECIFIC_SYSTEM: u32 = 0;
pub const SYSTEM_TOOLS: u32 = 10;
pub const SAFETY_SYSTEMS: u32 = 20;
pub const GATEWAY: u32 = 25;
pub const POWER_MANAGEMENT_AND_LIGHTING_SYSTEMS: u32 = 30;
pub const STEERING_SYSTEMS: u32 = 40;
pub const PROPULSION_SYSTEMS: u32 = 50;
pub const NAVIGATION_SYSTEMS: u32 = 60;
pub const COMMUNICATIONS_SYSTEMS: u32 = 70;
pub const INSTRUMENTATION_GENERAL_SYSTEMS: u32 = 80;
pub const ENVIRONMENTAL_HVAC_SYSTEMS: u32 = 90;
pub const DECK_CARGO_AND_FISHING_EQUIPMENT_SYSTEMS: u32 = 100;
pub const NOT_AVAILABLE: u32 = 127;



/// Vehicle system: non specific system
pub mod non_specific_system {
    pub const ALARM_SYSTEM_CONTROL_FOR_MARINE_ENGINES: u16 = 128;
    pub const PROTECTION_SYSTEM_FOR_MARINE_ENGINES: u16 = 129;
    pub const DISPLAY_FOR_PROTECTION_SYSTEM_FOR_MARINE_ENGINES: u16 = 130;
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: system tools
pub mod system_tools {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: safety systems
pub mod safety_systems {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: gateway
pub mod gateway {
}

/// Vehicle system: power management and lighting systems
pub mod power_management_and_lighting_systems {
    pub const SWITCH: u16 = 130;
    pub const LOAD: u16 = 140;
}

/// Vehicle system: steering systems
pub mod steering_systems {
    pub const FOLLOW_UP_CONTROLLER: u16 = 130;
    pub const MODE_CONTROLLER: u16 = 140;
    pub const AUTOMATIC_STEERING_CONTROLLER: u16 = 150;
    pub const HEADING_SENSORS: u16 = 160;
}

/// Vehicle system: propulsion systems
pub mod propulsion_systems {
    pub const ENGINEROOM_MONITORING: u16 = 130;
    pub const ENGINE_INTERFACE: u16 = 140;
    pub const ENGINE_CONTROLLER: u16 = 150;
    pub const ENGINE_GATEWAY: u16 = 160;
    pub const CONTROL_HEAD: u16 = 170;
    pub const ACTUATOR: u16 = 180;
    pub const GAUGE_INTERFACE: u16 = 190;
    pub const GAUGE_LARGE: u16 = 200;
    pub const GAUGE_SMALL: u16 = 210;
    pub const PROPULSION_SENSORS_GATEWAY: u16 = 220;
}

/// Vehicle system: navigation systems
pub mod navigation_systems {
    pub const SOUNDER_DEPTH: u16 = 130;
    pub const GLOBAL_NAVIGATION_SATELLITE_SYSTEM_GNSS: u16 = 145;
    pub const LORAN_C: u16 = 150;
    pub const SPEED_SENSORS: u16 = 155;
    pub const TURN_RATE_INDICATOR: u16 = 160;
    pub const INTEGRATED_NAVIGATION: u16 = 170;
    pub const RADAR_AND_OR_RADAR_PLOTTING: u16 = 200;
    pub const ELECTRONIC_CHART_DISPLAY_INFORMATION_SYSTEM_ECDIS: u16 = 205;
    pub const ELECTRONIC_CHART_SYSTEM_ECS: u16 = 210;
    pub const DIRECTION_FINDER: u16 = 220;
}

/// Vehicle system: communications systems
pub mod communications_systems {
    pub const EMERGENCY_POSITION_INDICATING_BEACON_EPIRB: u16 = 130;
    pub const AUTOMATIC_IDENTIFICATION_SYSTEM: u16 = 140;
    pub const DIGITAL_SELECTIVE_CALLING_DSC: u16 = 150;
    pub const DATA_RECEIVER: u16 = 160;
    pub const SATELLITE: u16 = 170;
    pub const RADIO_TELEPHONE_MF_HF: u16 = 180;
    pub const RADIO_TELEPHONE_VHF: u16 = 190;
}

/// Vehicle system: instrumentation general systems
pub mod instrumentation_general_systems {
    pub const TIME_DATE_SYSTEMS: u16 = 130;
    pub const VOYAGE_DATA_RECORDER: u16 = 140;
    pub const INTEGRATED_INSTRUMENTATION: u16 = 150;
    pub const GENERAL_PURPOSE_DISPLAYS: u16 = 160;
    pub const GENERAL_SENSOR_BOX: u16 = 170;
    pub const WEATHER_INSTRUMENTS: u16 = 180;
    pub const TRANSDUCER_GENERAL: u16 = 190;
    pub const NMEA_0183_CONVERTER: u16 = 200;
}

/// Vehicle system: environmental hvac systems
pub mod environmental_hvac_systems {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: deck cargo and fishing equipment systems
pub mod deck_cargo_and_fishing_equipment_systems {
    pub const NOT_AVAILABLE: u16 = 255;
}

/// Vehicle system: not available
pub mod not_available {
    pub const NOT_AVAILABLE: u16 = 255;
}

