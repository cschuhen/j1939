/// PGN entry: maps a numeric PGN value to its human-readable name.
#[derive(Debug, Clone)]
pub struct PgnEntry {
    pub pgn: u32,
    pub name: String,
}

/// ISOBUS parameter NAME entry: maps a numeric value to its human-readable name.
#[derive(Debug, Clone)]
pub struct ParamNameEntry {
    pub value: u32,
    pub name: String,
}

/// Task Controller DDI entry with full metadata.
#[derive(Debug, Clone)]
pub struct DdiEntry {
    pub ddi: u16,
    pub name: String,
    pub unit: Option<String>,
    pub resolution: f64,
    pub offset: f64,
}

/// Manufacturer ID entry: maps a numeric manufacturer ID to its name.
#[derive(Debug, Clone)]
pub struct ManufacturerIdEntry {
    pub id: u8,
    pub name: String,
}

/// Industry Group entry: maps an industry group ID to its description.
#[derive(Debug, Clone)]
pub struct IndustryGroupEntry {
    pub id: u8,
    pub description: String,
}

/// Global NAME Function entry: maps a function ID to its description.
#[derive(Debug, Clone)]
pub struct GlobalFunctionEntry {
    pub id: u16,
    pub description: String,
}

/// IG Specific NAME Function entry: maps (industry_group, vehicle_system, function) to description.
#[derive(Debug, Clone)]
pub struct IgSpecificFunctionEntry {
    pub industry_group_id: u8,
    pub vehicle_system_id: u8,
    pub function_id: u16,
    pub description: String,
}

/// Vehicle System entry: maps (industry_group, vehicle_system) to description.
#[derive(Debug, Clone)]
pub struct VehicleSystemEntry {
    pub industry_group_id: u8,
    pub vehicle_system_id: u8,
    pub description: String,
}
