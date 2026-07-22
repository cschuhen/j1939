#[cfg(feature = "task_controller_ddi")]
mod ddi_tests {
    use iso11783_data::strings::task_controller_ddi;

    #[test]
    fn test_ddi_0_lookup() {
        let info = task_controller_ddi::lookup(0);
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.ddi, 0);
        assert_eq!(info.name, "Internal Data Base DDI");
        assert_eq!(info.unit, Some("not defined"));
    }

    #[test]
    fn test_ddi_6_lookup() {
        let info = task_controller_ddi::lookup(6);
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.name, "Setpoint Mass Per Area Application Rate");
        assert_eq!(info.unit, Some("mg/m\u{00B2}"));
    }

    #[test]
    fn test_ddi_2_lookup() {
        let info = task_controller_ddi::lookup(2);
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.resolution, 0.01);
        assert_eq!(info.unit, Some("mm\u{00B3}/m\u{00B2}"));
    }

    #[test]
    fn test_unknown_ddi_returns_none() {
        assert_eq!(task_controller_ddi::lookup(999), None);
        assert_eq!(task_controller_ddi::lookup(65535), None);
    }

    #[test]
    fn test_to_physical_basic() {
        let physical = task_controller_ddi::to_physical(0, 42);
        assert_eq!(physical, Some(42.0));
    }

    #[test]
    fn test_to_physical_with_resolution() {
        let physical = task_controller_ddi::to_physical(2, 100);
        assert_eq!(physical, Some(1.0));
    }

    #[test]
    fn test_to_physical_unknown_ddi() {
        assert_eq!(task_controller_ddi::to_physical(999, 42), None);
    }

    #[test]
    fn test_to_physical_negative_raw() {
        let physical = task_controller_ddi::to_physical(0, -10);
        assert_eq!(physical, Some(-10.0));
    }

    #[test]
    fn test_ddi_list_sorted() {
        let list = task_controller_ddi::DDI_LIST;
        for i in 1..list.len() {
            assert!(
                list[i - 1].ddi < list[i].ddi,
                "DDI_LIST not sorted at index {}",
                i
            );
        }
    }

    #[test]
    fn test_ddi_list_count() {
        assert_eq!(task_controller_ddi::DDI_LIST.len(), 383);
    }

    #[test]
    fn test_first_and_last_ddi() {
        assert_eq!(task_controller_ddi::DDI_LIST[0].ddi, 0);
        let last = task_controller_ddi::DDI_LIST.last().unwrap();
        assert!(last.ddi > 0);
    }

    #[test]
    fn test_all_ddis_have_non_empty_names() {
        for info in task_controller_ddi::DDI_LIST {
            assert!(!info.name.is_empty(), "DDI {} has empty name", info.ddi);
        }
    }
}
