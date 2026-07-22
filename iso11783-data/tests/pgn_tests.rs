#[cfg(feature = "pgn")]
mod pgn_tests {
    use iso11783_data::strings::pgn;

    #[test]
    fn test_process_data_lookup() {
        let name = pgn::lookup(51968);
        assert_eq!(name, Some("Process Data Message"));
    }

    #[test]
    fn test_unknown_pgn_returns_none() {
        assert_eq!(pgn::lookup(999999), None);
        assert_eq!(pgn::lookup(0xFFFFFFFF), None);
    }

    #[test]
    fn test_pgn_list_sorted() {
        let list = pgn::PGN_LIST;
        for i in 1..list.len() {
            assert!(
                list[i - 1].0 < list[i].0,
                "PGN_LIST not sorted at index {}",
                i
            );
        }
    }

    #[test]
    fn test_pgn_list_count() {
        assert_eq!(pgn::PGN_LIST.len(), 3213);
    }

    #[test]
    fn test_first_pgn() {
        assert_eq!(pgn::PGN_LIST[0].0, 0);
        assert_eq!(pgn::PGN_LIST[0].1, "Torque/Speed Control 1");
    }

    #[test]
    fn test_binary_search_efficiency() {
        let mid = pgn::PGN_LIST.len() / 2;
        let pgn_val = pgn::PGN_LIST[mid].0;
        assert_eq!(pgn::lookup(pgn_val), Some(pgn::PGN_LIST[mid].1));
    }

    #[test]
    fn test_known_pgn_values() {
        assert_eq!(pgn::lookup(256), Some("Transmission Control 1"));
        assert_eq!(pgn::lookup(512), Some("Electronic Brake System #1/1"));
    }

    #[test]
    fn test_lookup_returns_static_str() {
        let name: Option<&str> = pgn::lookup(0);
        assert!(name.is_some());
    }
}
