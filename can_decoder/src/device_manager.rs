use std::collections::HashMap;

use j1939_async::name::Name;

type PGN = u32;

#[derive(Debug, Clone)]
pub struct Device {
    pub address: u8,
    pub name: Option<String>,
    pub last_seen_timestamp: u64,
    pub is_claimed: bool,
}

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Claimed {
        address: u8,
        name: String,
        timestamp: u64,
    },
    Conflict {
        address: u8,
        name1: String,
        name2: String,
        timestamp: u64,
    },
    Expired {
        address: u8,
        timestamp: u64,
    },
}

pub struct DeviceManager {
    devices: HashMap<u8, Device>,
    parameter_cache: HashMap<u8, HashMap<PGN, Vec<u8>>>,
    name_table: HashMap<u8, u64>,
    ttl_microseconds: u64,
}

impl DeviceManager {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            devices: HashMap::new(),
            parameter_cache: HashMap::new(),
            name_table: HashMap::new(),
            ttl_microseconds: ttl_seconds * 1_000_000,
        }
    }

    pub fn update(
        &mut self,
        timestamp: u64,
        address: u8,
        name: Option<String>,
    ) -> Vec<DeviceEvent> {
        let mut events = Vec::new();

        if let Some(device) = self.devices.get_mut(&address) {
            // Check if this device is about to expire before we refresh it
            if timestamp - device.last_seen_timestamp > self.ttl_microseconds {
                // Device was last seen more than TTL ago, emit expiration and remove
                events.push(DeviceEvent::Expired {
                    address: device.address,
                    timestamp,
                });
                self.devices.remove(&address);
                self.name_table.remove(&address);
            } else {
                // Not expired yet - update normally
                if let Some(new_name) = name.clone() {
                    if let Some(old_name) = &device.name {
                        if old_name != &new_name && device.is_claimed {
                            events.push(DeviceEvent::Conflict {
                                address,
                                name1: old_name.clone(),
                                name2: new_name.clone(),
                                timestamp,
                            });
                        }
                    }
                    device.name = Some(new_name);
                }
                device.last_seen_timestamp = timestamp;
                device.is_claimed = true;
            }
        } else {
            // New device (not a refresh of expired one)
            self.devices.insert(
                address,
                Device {
                    address,
                    name,
                    last_seen_timestamp: timestamp,
                    is_claimed: true,
                },
            );
        }

        // Check for expirations of OTHER devices (not the one we just updated)
        let expired_addresses: Vec<u8> = self
            .devices
            .iter()
            .filter(|(_, d)| timestamp - d.last_seen_timestamp > self.ttl_microseconds)
            .map(|(addr, _)| *addr)
            .collect();

        for addr in expired_addresses {
            if let Some(device) = self.devices.remove(&addr) {
                events.push(DeviceEvent::Expired {
                    address: device.address,
                    timestamp,
                });
                self.name_table.remove(&addr);
            }
        }

        events
    }

    pub fn handle_claim(&mut self, address: u8, name: String, timestamp: u64) -> Vec<DeviceEvent> {
        let mut events = Vec::new();
        if let Some(device) = self.devices.get_mut(&address) {
            if device.is_claimed && device.name.as_ref() != Some(&name) {
                events.push(DeviceEvent::Conflict {
                    address,
                    name1: device.name.clone().unwrap_or_default(),
                    name2: name.clone(),
                    timestamp,
                });
            }
            device.name = Some(name);
            device.is_claimed = true;
            device.last_seen_timestamp = timestamp;
        } else {
            self.devices.insert(
                address,
                Device {
                    address,
                    name: Some(name),
                    last_seen_timestamp: timestamp,
                    is_claimed: true,
                },
            );
        }
        events
    }

    pub fn get_device(&self, address: u8) -> Option<&Device> {
        self.devices.get(&address)
    }

    pub fn update_parameter(&mut self, address: u8, pgn: PGN, data: Vec<u8>) {
        self.parameter_cache
            .entry(address)
            .or_default()
            .insert(pgn, data);
    }

    pub fn get_parameter(&self, address: u8, pgn: PGN) -> Option<&Vec<u8>> {
        self.parameter_cache.get(&address)?.get(&pgn)
    }

    /// Parse 8 bytes into a J1939 u64 NAME using the j1939-async library.
    pub fn parse_name_from_bytes(bytes: &[u8]) -> Result<u64, String> {
        if bytes.len() < 8 {
            return Err(format!(
                "Expected 8 bytes for NAME parsing, got {}",
                bytes.len()
            ));
        }
        let name = Name::from_bytes(bytes).map_err(|_| "Failed to parse NAME from bytes")?;
        Ok(name.raw())
    }

    /// Store a u64 NAME for a given address, parsed from raw bytes.
    pub fn set_name_from_bytes(&mut self, address: u8, bytes: &[u8]) -> Result<(), String> {
        let name_u64 = Self::parse_name_from_bytes(bytes)?;
        self.name_table.insert(address, name_u64);
        Ok(())
    }

    /// Store a u64 NAME for a given address directly.
    pub fn set_name_u64(&mut self, address: u8, name_u64: u64) {
        self.name_table.insert(address, name_u64);
    }

    /// Retrieve the stored u64 NAME for a given address.
    pub fn get_name_u64(&self, address: u8) -> Option<u64> {
        self.name_table.get(&address).copied()
    }

    /// Remove the stored NAME for a given address (e.g., on device expiration).
    pub fn remove_name(&mut self, address: u8) {
        self.name_table.remove(&address);
    }
}
