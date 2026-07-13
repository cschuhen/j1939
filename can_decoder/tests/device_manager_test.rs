use can_decoder::device_manager::{DeviceEvent, DeviceManager};

#[tokio::test]
async fn test_device_manager_new() {
    DeviceManager::new(10);
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
    if let DeviceEvent::Conflict {
        address,
        name1,
        name2,
        ..
    } = &events[0]
    {
        assert_eq!(*address, 0x20);
        assert_eq!(name1, "Engine");
        assert_eq!(name2, "Motor");
    } else {
        panic!("Expected Conflict event");
    }
}

#[tokio::test]
async fn test_device_manager_expiration() {
    let mut dm = DeviceManager::new(1); // 1 second TTL (in microseconds: 1_000_000)
    dm.update(1_000_000, 0x20, Some("Engine".to_string()));

    // Update with timestamp 3 seconds later (4_000_000 - 1_000_000 = 3_000_000 > 1_000_000 TTL)
    let events = dm.update(4_000_000, 0x20, Some("Engine".to_string()));

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
    if let DeviceEvent::Conflict {
        address,
        name1,
        name2,
        ..
    } = &events[0]
    {
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
    let pgn = 0x123;
    let data = vec![0x01, 0x02, 0x03];

    dm.update_parameter(0x20, pgn.clone(), data.clone());

    assert_eq!(dm.get_parameter(0x20, pgn), Some(&data));
    assert_eq!(dm.get_parameter(0x20, 0x456), None);
}

#[test]
fn test_parse_name_from_bytes_valid() {
    // NAME from j1939-async tests: [0x6e, 0x83, 0x0d, 0x46, 0x00, 0x3e, 0x00, 0x80]
    // Expected raw value: 0x8000_3e00_460d_836e
    let bytes = [0x6eu8, 0x83, 0x0d, 0x46, 0x00, 0x3e, 0x00, 0x80];
    let name_u64 = DeviceManager::parse_name_from_bytes(&bytes).unwrap();
    assert_eq!(name_u64, 0x8000_3e00_460d_836e);
}

#[test]
fn test_parse_name_from_bytes_too_short() {
    let bytes = [0x01u8, 0x02, 0x03];
    let result = DeviceManager::parse_name_from_bytes(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_parse_name_from_bytes_empty() {
    let bytes: [u8; 0] = [];
    let result = DeviceManager::parse_name_from_bytes(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_set_and_get_name_u64() {
    let mut dm = DeviceManager::new(10);
    let name_u64: u64 = 0x8000_3e00_460d_836e;

    dm.set_name_u64(0x20, name_u64);

    assert_eq!(dm.get_name_u64(0x20), Some(name_u64));
    assert_eq!(dm.get_name_u64(0xFF), None);
}

#[test]
fn test_set_name_from_bytes() {
    let mut dm = DeviceManager::new(10);
    let bytes = [0x6eu8, 0x83, 0x0d, 0x46, 0x00, 0x3e, 0x00, 0x80];

    dm.set_name_from_bytes(0x20, &bytes).unwrap();

    assert_eq!(dm.get_name_u64(0x20), Some(0x8000_3e00_460d_836e));
}

#[test]
fn test_set_name_from_bytes_invalid() {
    let mut dm = DeviceManager::new(10);
    let bytes = [0x01u8, 0x02];

    let result = dm.set_name_from_bytes(0x20, &bytes);
    assert!(result.is_err());
}

#[test]
fn test_remove_name() {
    let mut dm = DeviceManager::new(10);
    dm.set_name_u64(0x20, 0x8000_3e00_460d_836e);

    assert_eq!(dm.get_name_u64(0x20), Some(0x8000_3e00_460d_836e));

    dm.remove_name(0x20);

    assert_eq!(dm.get_name_u64(0x20), None);
}

#[tokio::test]
async fn test_expiration_cleans_up_name() {
    let mut dm = DeviceManager::new(1); // 1 second TTL (in microseconds: 1_000_000)
    dm.set_name_u64(0x20, 0x8000_3e00_460d_836e);
    dm.update(1_000_000, 0x20, Some("Engine".to_string()));

    // Update with timestamp beyond TTL - device expires and name should be cleaned up
    let events = dm.update(4_000_000, 0x20, Some("Engine".to_string()));

    assert_eq!(events.len(), 1);
    if let DeviceEvent::Expired { address, .. } = &events[0] {
        assert_eq!(*address, 0x20);
    } else {
        panic!("Expected Expired event");
    }

    assert!(dm.get_device(0x20).is_none());
    assert_eq!(dm.get_name_u64(0x20), None);
}

#[test]
fn test_multiple_names_stored_independently() {
    let mut dm = DeviceManager::new(10);

    dm.set_name_u64(0x20, 0x8000_3e00_460d_836e);
    dm.set_name_u64(0xF8, 0x1111_2222_3333_4444);

    assert_eq!(dm.get_name_u64(0x20), Some(0x8000_3e00_460d_836e));
    assert_eq!(dm.get_name_u64(0xF8), Some(0x1111_2222_3333_4444));
    assert_eq!(dm.get_name_u64(0x40), None);

    // Overwriting a name should work
    dm.set_name_u64(0x20, 0xDEAD_BEEF_CAFE_BABE);
    assert_eq!(dm.get_name_u64(0x20), Some(0xDEAD_BEEF_CAFE_BABE));
}

#[test]
fn test_parse_all_on_name() {
    // Bytes: [0xFF, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    // from_bytes constructs raw = byte[7]<<56 | ... | byte[1]<<8 | byte[0]
    let bytes = [0xFFu8, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let name_u64 = DeviceManager::parse_name_from_bytes(&bytes).unwrap();
    // byte[1]=0xFE means bits 8-15 are 0xFE, result = 0xFFFFFFFFFFFFFEFF
    assert_eq!(name_u64, 0xFFFFFFFFFFFFFEFF);
}

#[test]
fn test_parse_zero_name() {
    let bytes = [0x00u8; 8];
    let name_u64 = DeviceManager::parse_name_from_bytes(&bytes).unwrap();
    assert_eq!(name_u64, 0);
}
