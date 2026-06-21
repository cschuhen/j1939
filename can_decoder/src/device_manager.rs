use std::collections::HashMap;
use crate::types::{PGN, RawFrame};

#[derive(Debug, Clone)]
pub struct Device {
    pub address: u8,
    pub name: Option<String>,
    pub last_seen_timestamp: u64,
    pub is_claimed: bool,
}

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Claimed { address: u8, name: String, timestamp: u64 },
    Conflict { address: u8, name1: String, name2: String, timestamp: u64 },
    Expired { address: u8, timestamp: u64 },
}

pub struct DeviceManager {
    devices: HashMap<u8, Device>,
    parameter_cache: HashMap<u8, HashMap<PGN, Vec<u8>>>,
    ttl_microseconds: u64,
}

impl DeviceManager {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            devices: HashMap::new(),
            parameter_cache: HashMap::new(),
            ttl_microseconds: ttl_seconds * 1_000_000,
        }
    }

    pub fn update(&mut self, timestamp: u64, address: u8, name: Option<String>) -> Vec<DeviceEvent> {
        let mut events = Vec::new();
        
        if let Some(device) = self.devices.get_mut(&address) {
            // Check for name change/conflict
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
        } else {
            // New device
            self.devices.insert(address, Device {
                address,
                name,
                last_seen_timestamp: timestamp,
                is_claimed: true,
            });
        }

        // Check for expirations
        let expired_addresses: Vec<u8> = self.devices.iter()
            .filter(|(_, d)| timestamp - d.last_seen_timestamp > self.ttl_microseconds)
            .map(|(addr, _)| *addr)
            .collect();

        for addr in expired_addresses {
            if let Some(device) = self.devices.remove(&addr) {
                events.push(DeviceEvent::Expired {
                    address: device.address,
                    timestamp,
                });
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
            self.devices.insert(address, Device {
                address,
                name: Some(name),
                last_seen_timestamp: timestamp,
                is_claimed: true,
            });
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
}
