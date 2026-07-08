# J1939 Data Types, Scaling, & Signal Encoding Specification

This document details how individual signals—known as **Suspect Parameter Numbers (SPNs)**—are packed, scaled, and interpreted within J1939 Parameter Group payloads. It covers scaling formulas, bit-packing layouts, special diagnostic states, and parsing implementation references.

---

## 1. Physical Value Conversion (Scaling)

J1939 parameters are transmitted as raw integer values to maximize CAN bus efficiency. They must be scaled and offset to obtain their physical values (e.g. RPM, Temperature, Pressure).

### The Scaling Formula
$$\text{Physical Value} = (\text{Raw Value} \times \text{Resolution}) + \text{Offset}$$

* **Resolution (Factor)**: The multiplier to scale the raw integer (can be a fraction, e.g. `0.125` or `0.05`).
* **Offset**: The value added after scaling to allow representation of negative numbers (e.g. `-40` for temperatures).

### The Encoding Formula (Inverse)
To encode a physical value back into a raw bus integer:
$$\text{Raw Value} = \text{round}\left(\frac{\text{Physical Value} - \text{Offset}}{\text{Resolution}}\right)$$

### Operational Examples

#### Example A: Engine Speed (SPN 190)
* **Resolution**: `0.125 RPM / bit`
* **Offset**: `0 RPM`
* **Size**: 2 Bytes (16 bits)
* **Raw Value**: `0x1F40` (decimal `8000`)
* **Calculation**:
  $$\text{Engine Speed} = 8000 \times 0.125 + 0 = 1000.0\text{ RPM}$$

#### Example B: Engine Coolant Temperature (SPN 110)
* **Resolution**: `1 °C / bit`
* **Offset**: `-40 °C`
* **Size**: 1 Byte (8 bits)
* **Raw Value**: `0x50` (decimal `80`)
* **Calculation**:
  $$\text{Temperature} = 80 \times 1 + (-40) = 40\text{ °C}$$

---

## 2. Special Parameter States & Ranges

J1939 defines dedicated ranges at the upper end of each data size to indicate special conditions (e.g., sensor errors, or when a parameter is not supported by an ECU). 

### Special Ranges Table

| Field Size | Valid Data Range | Reserved Range | Error Indicator | Not Available / Not Requested |
| :--- | :--- | :--- | :--- | :--- |
| **2 Bits** | `0b00` to `0b01`<br>(0 to 1) | N/A | `0b10`<br>(2 / `0x2`) | `0b11`<br>(3 / `0x3`) |
| **4 Bits** | `0x0` to `0xC`<br>(0 to 12) | `0xD`<br>(13) | `0xE`<br>(14) | `0xF`<br>(15) |
| **8 Bits (1 Byte)** | `0x00` to `0xFA`<br>(0 to 250) | `0xFB` to `0xFD`<br>(251 to 253) | `0xFE`<br>(254) | `0xFF`<br>(255) |
| **16 Bits (2 Bytes)**| `0x0000` to `0xFAFF`<br>(0 to 64255) | `0xFB00` to `0xFDFF`<br>(64256 to 65023) | `0xFE00` to `0xFEFF`<br>(65024 to 65279) | `0xFF00` to `0xFFFF`<br>(65280 to 65535) |
| **24 Bits (3 Bytes)**| `0x000000` to `0xFAFFFF` | `0xFB0000` to `0xFDFFFF` | `0xFE0000` to `0xFEFFFF` | `0xFF0000` to `0xFFFFFF` |
| **32 Bits (4 Bytes)**| `0x00000000` to `0xFAFFFFFF` | `0xFB000000` to `0xFDFFFFFF` | `0xFE000000` to `0xFEFFFFFF` | `0xFF000000` to `0xFFFFFFFF` |

### Parameter State Semantics
* **Valid Data**: The sensor/ECU is operating normally and transmitting valid measurements.
* **Reserved**: Reserved by SAE for future definitions. Do not interpret as valid data.
* **Error Indicator**: The transmitting ECU has detected a fault (e.g. sensor short circuit, wire break, or signal out of physical range bounds).
* **Not Available / Not Requested**: The ECU does not support this parameter, the vehicle configuration does not have the sensor installed, or the data is not currently requested/broadcast.

---

## 3. Bit-Packing & Endianness

* **Endianness**: J1939 uses **Little-Endian (Intel format)** by default for all multi-byte values. The Least Significant Byte (LSB) is transmitted first.
* **Bit Numbering**: J1939 documentation numbers bits **1 to 8** within a byte, where **Bit 1 is the Least Significant Bit (LSb)** and **Bit 8 is the Most Significant Bit (MSb)**.
* **Start Bit Notation**: Mapped as `Byte.Bit` (1-indexed). For example, a start bit of `3.5` means the parameter starts at Byte 3, Bit 5.

### Bit Layout Example (8-Byte Payload)

```
Byte 1: [ Bit 8 (MSB) | Bit 7 | Bit 6 | Bit 5 | Bit 4 | Bit 3 | Bit 2 | Bit 1 (LSB) ]
```

#### Common Packing Scenarios
* **2-Bit Flag at Start Bit `1.1`**: Occupies Byte 1, Bits 1–2. Mask = `0x03`.
* **2-Bit Flag at Start Bit `1.3`**: Occupies Byte 1, Bits 3–4. Mask = `0x0C` (shift right by 2).
* **4-Bit Flag at Start Bit `1.5`**: Occupies Byte 1, Bits 5–8. Mask = `0xF0` (shift right by 4).
* **16-Bit Parameter at Start Bit `2.1`**: Occupies Byte 2 (LSB) and Byte 3 (MSB). Mapped as `(Byte3 << 8) | Byte2`.

---

## 4. Parser Implementation Reference (Rust)

Below is a robust Rust implementation showing how to safely parse signals, handle special indicator states, and encode physical values back to raw byte frames.

```rust
#[derive(Debug, PartialEq)]
pub enum SignalState<T> {
    Valid(T),
    Reserved,
    Error,
    NotAvailable,
}

/// Decode a 16-bit (2-byte) J1939 signal starting at a given byte offset.
/// This handles Little-Endian decoding, range check states, and scaling.
pub fn decode_u16_signal(
    payload: &[u8],
    byte_offset: usize,
    resolution: f64,
    offset: f64,
) -> Option<SignalState<f64>> {
    if byte_offset + 1 >= payload.len() {
        return None; // Payload out of bounds
    }

    // Read 16-bit Little-Endian integer
    let raw = u16::from_le_bytes([payload[byte_offset], payload[byte_offset + 1]]);

    match raw {
        0x0000..=0xFAFF => {
            let physical = (raw as f64) * resolution + offset;
            Some(SignalState::Valid(physical))
        }
        0xFB00..=0xFDFF => Some(SignalState::Reserved),
        0xFE00..=0xFEFF => Some(SignalState::Error),
        0xFF00..=0xFFFF => Some(SignalState::NotAvailable),
    }
}

/// Encode a physical value into a 16-bit Little-Endian slot inside a payload.
/// Handles conversion and injects special states if desired.
pub fn encode_u16_signal(
    payload: &mut [u8],
    byte_offset: usize,
    state: SignalState<f64>,
    resolution: f64,
    offset: f64,
) -> bool {
    if byte_offset + 1 >= payload.len() {
        return false;
    }

    let raw: u16 = match state {
        SignalState::Valid(value) => {
            let converted = ((value - offset) / resolution).round();
            // Clamp value to the valid u16 J1939 range
            if converted < 0.0 {
                0
            } else if converted > 64255.0 {
                0xFAFF
            } else {
                converted as u16
            }
        }
        SignalState::Reserved => 0xFB00,
        SignalState::Error => 0xFE00,
        SignalState::NotAvailable => 0xFF00,
    };

    let bytes = raw.to_le_bytes();
    payload[byte_offset] = bytes[0];
    payload[byte_offset + 1] = bytes[1];
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_temp_valid() {
        // Temp SPN: Resolution 1.0, Offset -40.0. Byte offset 0.
        let payload = [0x50, 0xFF]; // 0x50 = 80 decimal
        let state = decode_u16_signal(&payload, 0, 1.0, -40.0).unwrap();
        assert_eq!(state, SignalState::Valid(40.0));
    }

    #[test]
    fn test_decode_error_state() {
        let payload = [0x00, 0xFE]; // 0xFE00 (Error Indicator)
        let state = decode_u16_signal(&payload, 0, 1.0, -40.0).unwrap();
        assert_eq!(state, SignalState::Error);
    }
}
```
