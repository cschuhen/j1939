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
