#[cfg(feature = "name")]
mod manufacturer_tests {
    use iso11783_data::strings::name;

    #[test]
    fn test_manufacturer_id_0() {
        let name = name::manufacturer_id_lookup(0);
        assert_eq!(name, Some("For experimental or developmental use only."));
    }

    #[test]
    fn test_manufacturer_id_1() {
        let name = name::manufacturer_id_lookup(1);
        assert!(name.unwrap().contains("Bendix"));
    }

    #[test]
    fn test_manufacturer_id_caterpillar() {
        let name = name::manufacturer_id_lookup(8);
        assert_eq!(name, Some("Caterpillar Inc."));
    }

    #[test]
    fn test_manufacturer_id_john_deere() {
        let name = name::manufacturer_id_lookup(33);
        assert_eq!(name, Some("John Deere"));
    }

    #[test]
    fn test_manufacturer_list_sorted() {
        let list = name::MANUFACTURER_ID_LIST;
        for i in 1..list.len() {
            assert!(
                list[i - 1].0 < list[i].0,
                "MANUFACTURER_ID_LIST not sorted at index {}",
                i
            );
        }
    }

    #[test]
    fn test_manufacturer_list_count() {
        assert_eq!(name::MANUFACTURER_ID_LIST.len(), 256);
    }
}

#[cfg(feature = "name")]
mod industry_group_tests {
    use iso11783_data::strings::name;

    #[test]
    fn test_industry_group_0() {
        let name = name::industry_group_lookup(0);
        assert_eq!(name, Some("Global, applies to all"));
    }

    #[test]
    fn test_industry_group_1() {
        let name = name::industry_group_lookup(1);
        assert_eq!(name, Some("On-Highway Equipment"));
    }

    #[test]
    fn test_industry_group_2() {
        let name = name::industry_group_lookup(2);
        assert_eq!(name, Some("Agricultural and Forestry Equipment"));
    }

    #[test]
    fn test_industry_group_unknown() {
        assert_eq!(name::industry_group_lookup(9), None);
    }

    #[test]
    fn test_industry_group_list_sorted() {
        let list = name::INDUSTRY_GROUP_LIST;
        for i in 1..list.len() {
            assert!(
                list[i - 1].0 < list[i].0,
                "INDUSTRY_GROUP_LIST not sorted at index {}",
                i
            );
        }
    }

    #[test]
    fn test_industry_group_list_count() {
        assert_eq!(name::INDUSTRY_GROUP_LIST.len(), 8);
    }
}

#[cfg(feature = "name")]
mod global_function_tests {
    use iso11783_data::strings::name;

    #[test]
    fn test_global_function_0() {
        let name = name::global_function_lookup(0);
        assert_eq!(name, Some("Engine"));
    }

    #[test]
    fn test_global_function_1() {
        let name = name::global_function_lookup(1);
        assert_eq!(name, Some("Auxiliary Power Unit (APU)"));
    }

    #[test]
    fn test_global_function_unknown() {
        assert_eq!(name::global_function_lookup(999), None);
    }

    #[test]
    fn test_global_function_list_sorted() {
        let list = name::GLOBAL_FUNCTION_LIST;
        for i in 1..list.len() {
            assert!(
                list[i - 1].0 < list[i].0,
                "GLOBAL_FUNCTION_LIST not sorted at index {}",
                i
            );
        }
    }

    #[test]
    fn test_global_function_list_count() {
        assert_eq!(name::GLOBAL_FUNCTION_LIST.len(), 95);
    }
}

#[cfg(feature = "name")]
mod ig_specific_function_tests {
    use iso11783_data::strings::name;

    #[test]
    fn test_ig_specific_reserved() {
        // First entry in IG_SPECIFIC_FUNCTION_LIST has key 128 (function_id=128, vs=0)
        let name = name::ig_specific_function_lookup(0, 0, 128);
        assert_eq!(name, Some("Reserved"));
    }

    #[test]
    fn test_ig_specific_diagnostic_tool() {
        // function_id=129, vs=0
        let name = name::ig_specific_function_lookup(0, 0, 129);
        assert_eq!(name, Some("Off-board diagnostic-service tool"));
    }

    #[test]
    fn test_ig_specific_unknown() {
        assert_eq!(name::ig_specific_function_lookup(9, 9, 999), None);
    }

    #[test]
    fn test_ig_specific_list_sorted() {
        let list = name::IG_SPECIFIC_FUNCTION_LIST;
        for i in 1..list.len() {
            assert!(
                list[i - 1].0 < list[i].0,
                "IG_SPECIFIC_FUNCTION_LIST not sorted at index {}",
                i
            );
        }
    }

    #[test]
    fn test_ig_specific_list_count() {
        assert_eq!(name::IG_SPECIFIC_FUNCTION_LIST.len(), 284);
    }
}

#[cfg(feature = "name")]
mod vehicle_system_tests {
    use iso11783_data::strings::name;

    #[test]
    fn test_vehicle_system_non_specific() {
        // Industry group 0, vehicle system 0 should be "Non-specific System" or similar
        let name = name::vehicle_system_lookup(0, 0);
        assert!(name.is_some());
    }

    #[test]
    fn test_vehicle_system_unknown() {
        assert_eq!(name::vehicle_system_lookup(9, 9), None);
    }

    #[test]
    fn test_vehicle_system_list_sorted() {
        let list = name::VEHICLE_SYSTEM_LIST;
        for i in 1..list.len() {
            assert!(
                list[i - 1].0 < list[i].0,
                "VEHICLE_SYSTEM_LIST not sorted at index {}",
                i
            );
        }
    }

    #[test]
    fn test_vehicle_system_list_count() {
        assert_eq!(name::VEHICLE_SYSTEM_LIST.len(), 70);
    }

    #[test]
    fn test_function() {
        assert_eq!(iso11783_data::constants::name::industry_groups::agricultural_and_forestry_equipment::planters_seeders::SEED_RATE_CONTROL, 128);
        assert_eq!(iso11783_data::constants::name::industry_groups::agricultural_and_forestry_equipment::planters_seeders::PLANTERS_SEEDERS_MACHINE_CONTROL, 132);
        assert_eq!(iso11783_data::constants::name::industry_groups::agricultural_and_forestry_equipment::PLANTERS_SEEDERS , 4);
    }
}
