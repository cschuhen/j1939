# J1939 Address Claiming Specification

J1939 networks require dynamic or static network management to ensure that no two Electronic Control Units (ECUs) share the same Source Address (SA) on a single bus. This is accomplished using the **Address Claiming Procedure** (defined in SAE J1939-81).

---

## Key Messages & Protocol Constants

### 1. Address Claimed (AC) Message
* **PGN**: `60928` (`0xEE00`)
* **Format**: PDU1 (can be sent point-to-point or broadcast to `255`)
* **Data Length**: 8 bytes
* **Payload**: The 64-bit ECU NAME (stored in Little-Endian format).
* **Source Address**: The address being claimed (usually `0` to `253`), or `254` (Null Address) if the ECU is unable to claim an address.

### 2. Request PGN Message
* **PGN**: `59904` (`0xEA00`)
* **Format**: PDU1
* **Data Length**: 3 bytes
* **Payload**: The PGN being requested (3-byte, Little-Endian).
* **Requesting Address Claims**: To request all active nodes to announce their address claims, a node transmits a Request PGN frame targeting PGN `60928`.
  * **Payload bytes**: `[0x00, 0xEE, 0x00]` (Little-Endian representation of `60928` / `0x00EE00`).
  * **Destination Address**: Can be sent to a specific ECU (`0`-`253`) or broadcast to the Global Address (`255`).

---

## Critical Address Assignments

* **Addresses `0` to `253`**: Valid, claimable source addresses for operational nodes.
* **Null Address (`254` / `0xFE`)**: Used by nodes that have lost conflict resolution, cannot claim a preferred address, and are not arbitrary-address capable. A node transmitting with SA `254` cannot send operational messages.
* **Global Address (`255` / `0xFF`)**: Used as a destination address for broadcasts (all nodes). Never used as a source address.

---

## The Address Claiming Procedure

```mermaid
sequenceDiagram
    participant ECU A as New ECU (SA: 128)
    participant ECU B as Active ECU (SA: 128)
    participant Bus as CAN Bus

    Note over ECU A: Power Up / Start Claim
    ECU A->>Bus: Request PGN 60928 (Broadcast to 255)
    Note over ECU B: Receives Request
    ECU B->>Bus: Address Claimed (SA: 128, NAME: B)
    Note over ECU A: Compares NAME A vs NAME B

    alt NAME A < NAME B (ECU A Wins)
        ECU A->>Bus: Address Claimed (SA: 128, NAME: A)
        Note over ECU B: Receives Claim
        Note over ECU B: NAME B > NAME A (ECU B Loses)
        alt ECU B is Arbitrary Address Capable
            Note over ECU B: Claim Next Available Address
            ECU B->>Bus: Address Claimed (SA: 129, NAME: B)
        else ECU B is NOT Arbitrary Address Capable
            ECU B->>Bus: Address Claimed (SA: 254, NAME: B)
            Note over ECU B: Go Offline
        end
    else NAME A > NAME B (ECU A Loses)
        Note over ECU A: ECU A immediately relinquishes address
        alt ECU A is Arbitrary Address Capable
            Note over ECU A: Claim Next Available Address
            ECU A->>Bus: Address Claimed (SA: 129, NAME: A)
        else ECU A is NOT Arbitrary Address Capable
            ECU A->>Bus: Address Claimed (SA: 254, NAME: A)
            Note over ECU A: Go Offline
        end
    end
```

### Step-by-Step Execution

1. **Request Current Claims (Optional but Recommended)**:
   * Upon startup, a node should broadcast a Request PGN frame for PGN `60928` (`Address Claimed`) to discover existing nodes.
2. **Transmit Preferred Address Claim**:
   * The node broadcasts an Address Claimed frame (PGN `60928`) with its Source Address (SA) set to its preferred address and its 64-bit NAME in the payload.
3. **Wait for Challenges (T2 Timeout / 250 ms)**:
   * The node must start a **250 ms timer (T2)**.
   * During this period, the node cannot send operational messages.
   * If no other ECU challenges the address within 250 ms, the address is successfully claimed, and the node transitions to the operational state.
4. **Respond to Future Requests**:
   * Any time a Request for PGN `60928` is received, the ECU must respond by re-transmitting its Address Claimed frame.

---

## Conflict Resolution Logic

If a node receives an Address Claimed message from another node claiming the same Source Address:

* **Compare NAMEs**: Both nodes treat their 64-bit NAME as a single **64-bit unsigned integer**.
* **Lesser Numerical Value Wins**: The node with the smaller numerical NAME value has higher priority.
* **Winner Action**:
  * If the active node wins, it immediately re-transmits its Address Claimed frame to re-assert its claim.
  * If the incoming node wins, the active node must immediately cease transmitting on that address.
* **Loser Action**:
  * **If Arbitrary Address Capable (Bit 63 of NAME is `1`)**: The losing node must select a different, unclaimed address (typically searching upwards through the address space) and broadcast a new Address Claimed frame.
  * **If NOT Arbitrary Address Capable (Bit 63 of NAME is `0`)**: The losing node must broadcast an Address Claimed frame with the Source Address set to `254` (Null Address) and enter an offline state (no operational messages).

---

## State Transition Details for Implementers

* **State: Initialize**: Send Request for Address Claimed. Wait for responses to populate local network map.
* **State: Claiming**: Broadcast Address Claimed for preferred SA. Start 250 ms timer.
* **State: Active/Operational**: Timer expired without conflict. Send/receive normal parameter data.
* **State: Offline (Cannot Claim)**: Conflict lost and not arbitrary-capable. Node only responds to PGN requests with Address Claimed (SA = 254) and does not transmit parameters.
