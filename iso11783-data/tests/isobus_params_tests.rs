#[cfg(feature = "isobus_params")]
mod params_tests {
    use iso11783_data::strings::isobus_params;

    #[test]
    fn test_minimum_control_function() {
        let name = isobus_params::lookup(0);
        assert_eq!(name, Some("Minimum Control Function"));
    }

    #[test]
    fn test_ut_server() {
        let name = isobus_params::lookup(1);
        assert_eq!(name, Some("Universal Terminal (UT) Server"));
    }

    #[test]
    fn test_ut_client() {
        let name = isobus_params::lookup(2);
        assert_eq!(name, Some("Universal Terminal (UT) Client"));
    }

    #[test]
    fn test_tim_server() {
        let name = isobus_params::lookup(15);
        assert_eq!(name, Some("Tractor Implement Management (TIM) Server"));
    }

    #[test]
    fn test_unknown_value_returns_none() {
        assert_eq!(isobus_params::lookup(999), None);
    }

    #[test]
    fn test_param_list_sorted() {
        let list = isobus_params::PARAM_NAME_LIST;
        for i in 1..list.len() {
            assert!(
                list[i - 1].0 < list[i].0,
                "PARAM_NAME_LIST not sorted at index {}",
                i
            );
        }
    }

    #[test]
    fn test_param_list_count() {
        assert_eq!(isobus_params::PARAM_NAME_LIST.len(), 30);
    }
}
