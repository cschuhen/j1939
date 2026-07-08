# J1939 Lower-Layer Specification Index

This documentation suite provides a concise, high-density reference for J1939 lower-layer protocol mechanisms. It is designed to be highly readable for human developers and optimized for quick parsing and code generation by AI agents.

---

## Documentation Directory

The specification is broken down into five core modules:

1. **[CAN Identifier Structure](file:///home/cschuhen/rust/j1939/docs/j1939_can_identifier.md)**
   * Bit-level breakdown of the 29-bit CAN ID.
   * Logic for deriving PDU1 vs PDU2.
   * Formulae for PGN and Destination Address (DA) extraction.
   * Rust-compatible extraction/parsing reference.
2. **[ECU NAME Fields](file:///home/cschuhen/rust/j1939/docs/j1939_ecu_name.md)**
   * Component layout of the 64-bit identifier.
   * Classification of fixed (enumerated) vs. numeric fields.
   * Links to vendor registries and packing/unpacking code.
3. **[Address Claiming Procedure](file:///home/cschuhen/rust/j1939/docs/j1939_address_claiming.md)**
   * Node initialization sequence and claim negotiation rules.
   * Comparison rules (lower 64-bit NAME wins).
   * Requesting active address maps via PGN `59904`.
4. **[Control Protocols & Timing](file:///home/cschuhen/rust/j1939/docs/j1939_control_protocols.md)**
   * Core request and acknowledgment frames.
   * Standard Transport Protocol (TP) flow control (BAM, RTS/CTS).
   * ISO 11783 Extended Transport Protocol (ETP).
   * Exact timeouts (T1, T2, T3, T4, and BAM spacing).
5. **[Data Types & Scaling](file:///home/cschuhen/rust/j1939/docs/j1939_data_types.md)**
   * Physical value conversion formulas (resolution and offset).
   * Range boundaries and special diagnostic/availability indicator states.
   * Little-Endian bit-packing layouts and start bit notation.
   * Rust-compatible packing/unpacking reference.

---

## J1939 Protocol OSI Mapping

| OSI Layer | J1939 Standard | Key Implementation Responsibility |
| :--- | :--- | :--- |
| **Layer 7: Application** | J1939-71 / J1939-73 | Parameter definition (SPNs), Diagnostics (DTCs). |
| **Layer 4: Transport** | J1939-21 / ISO 11783-6 | Multi-packet fragmentation & reassembly (TP / ETP). |
| **Layer 3: Network** | J1939-81 | Dynamic Address Claiming & ECU Name management. |
| **Layer 2: Data Link** | J1939-21 | 29-bit CAN ID formatting, Priority mapping, PGN assignment. |
| **Layer 1: Physical** | J1939-11 / J1939-15 | CAN bus electrical specifications (typically 250 kbps or 500 kbps). |

---

## Quick Reference: Stack Implementation Checklist

When building a J1939 stack, ensure the following requirements are met in order:

* **Verify 29-bit CAN Frame Compatibility**: Ensure the CAN driver does not drop or fail to transmit extended frames.
* **Implement Dynamic Address Claiming**: The stack must wait 250 ms (T2) before sending operational data.
* **Implement the PGN Request Listener**: Respond to incoming PGN `59904` requests for PGN `60928` (Address Claimed) immediately.
* **Construct Transport Session Management**: Set up separate state buffers for standard TP (by Sender Address) and ETP to handle multi-packet reassembly without blocking the main event loop.
* **Establish Rate-Limiting for BAMs**: Enforce a delay between 50 ms and 200 ms when broadcasting multi-frame messages.
