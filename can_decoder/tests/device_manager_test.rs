use can_decoder::device_manager::{DeviceManager, DeviceEvent};
use can_decoder::types::PGN;

#[tokio::test]
async fn test_device_manager_new() {
    let dm = DeviceManager::new(10);
    // No way to check private fields directly without making them pub or adding getters,
    // but we can test through public methods.
}

#[tokio::test]
async fn test_device_manager_update_new_device() {
    let mut dm = DeviceManager::new(10);
    let events = dm.update(1000, 0x20, Some("Engine".to_string()));
    
    assert!(events.is_empty());
    let device = dm.get_device(0x20).unwrap();
    assert_eq!(device.address, 0x20);
    assert_eq!(device.name, Some("Engine".to_string()));
    assert_eq!(device.is_claimed, true);
}

#[tokio::test]
async fn test_device_manager_update_name_change_conflict() {
    let mut dm = DeviceManager::new(10);
    dm.update(1000, 0x20, Some("Engine".to_string()));
    
    // Change name of already claimed device
    let events = dm.update(2000, 0x20, Some("Motor".to_string()));
    
    assert_eq!(events.len(), 1);
    if let DeviceEvent::Conflict { address, name1, name2, .. } = &events[0] {
        assert_eq!(*address, 0x20);
        assert_eq!(name1, "Engine");
        assert_eq!(name2, "Motor");
    } else {
        panic!("Expected Conflict event");
    }
}

#[tokio::test]
async fn test_device_manager_expiration() {
    let mut dm = DeviceManager::new(1); // 1 second TTL
    dm.update(1000, 0x20, Some("Engine".to_string()));
    
    // Update with timestamp 3 seconds later
    let events = dm.update(4000, 0x20, Some("Engine".to_string()));
    
    assert_eq!(events.len(), 1);
    if let DeviceEvent::Expired { address, .. } = &events[0] {
        assert_eq!(*address, 0x20);
    } else {
        panic!("Expected Expired event");
    }
    
    assert!(dm.get_device(0x20).is_none());
}

#[tokio::test]
async fn test_device_manager_handle_claim_conflict() {
    let mut dm = DeviceManager::new(10);
    dm.update(1000, 0x20, Some("Engine".to_string()));
    
    // Claiming with different name
    let events = dm.handle_claim(0x20, "Motor".to_string(), 2000);
    
    assert_eq!(events.len(), 1);
    if let DeviceEvent::Conflict { address, name1, name2, .. } = &events[0] {
        assert_eq!(*address, 0x20);
        assert_eq!(name1, "Engine");
        assert_eq!(name2, "Motor");
    } else {
        panic!("Expected Conflict event");
    }
}

#[tokio::test]
async fn test_device_manager_parameter_cache() {
    let mut dm = DeviceManager::new(10);
    let pgn = PGN { priority: 0, pgn: 0x123 };
    let data = vec![0x01, 0x02, 0x03];
    
    dm.update_parameter(0x20, pgn.clone(), data.clone());
    
    assert_eq!(dm.get_parameter(0x20, pgn), Some(&data));
    assert_eq!(dm.get_parameter(0x20, PGN { priority: 0, pgn: 0x456 }), None);
}
