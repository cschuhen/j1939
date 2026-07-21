/// CANoT proprietary DDI definitions for can_decoder.
/// This module provides lookup tables and functions for proprietary DDI ranges (0xE000-0xFFFE).

use iso11783_data::task_controller_ddi::DdiInfo;

/// Lookup table for proprietary DDIs in the range 0xE000-0xFFFE.
pub const PROPRIETARY_DDI_LIST: &[DdiInfo] = &[
    DdiInfo { ddi: 0xE000, name: "Total Charge", unit: Some("micro-amp-hours"), resolution: 1.0, offset: 0 },
    DdiInfo { ddi: 0xE001, name: "Current", unit: Some("micro-amp"), resolution: 1.0, offset: 0 },
    DdiInfo { ddi: 0xE002, name: "Voltage", unit: Some("micro-volts"), resolution: 1.0, offset: 0 },
    DdiInfo { ddi: 0xE003, name: "Shunt Resistance", unit: Some("micro-ohms"), resolution: 1.0, offset: 0 },
    DdiInfo { ddi: 0xE004, name: "Shunt Offset", unit: Some("ADC counts (2.5 μV)"), resolution: 1.0, offset: 0 },
    DdiInfo { ddi: 0xE010, name: "Temperature", unit: Some("micro-degrees"), resolution: 1.0, offset: 0 },
    DdiInfo { ddi: 0xE011, name: "Humidity", unit: Some("ppm"), resolution: 1.0, offset: 0 },
    DdiInfo { ddi: 0xE012, name: "Pressure", unit: Some("milli-pascals"), resolution: 1.0, offset: 0 },
];

/// Look up a proprietary DDI by its 16-bit value.
/// Returns None if the DDI is not found in the table.
pub fn lookup(ddi: u16) -> Option<&'static DdiInfo> {
    PROPRIETARY_DDI_LIST.binary_search_by_key(&ddi, |info| info.ddi).ok().map(|idx| &PROPRIETARY_DDI_LIST[idx])
}

/// Convert a raw i32 value to a physical f64 value using the DDI's scale and offset.
/// Formula: physical = (raw_value - offset) * scale
/// Returns None if the DDI is not found in the table.
pub fn to_physical(ddi: u16, raw_value: i32) -> Option<f64> {
    lookup(ddi).map(|info| (raw_value as f64 - info.offset as f64) * info.resolution)
}

/// Check if a DDI falls within the proprietary range (0xE000-0xFFFE).
pub fn is_proprietary(ddi: u16) -> bool {
    ddi >= 0xE000 && ddi <= 0xFFFE
}
