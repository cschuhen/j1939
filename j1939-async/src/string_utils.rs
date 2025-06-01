use heapless::String;

pub fn format_number(
    val: f32,
    mult: &str,
    unit: &str,
    max_len: usize,
    precision: usize,
) -> String<12> {
    let mut string: String<12>;
    use lexical_core::BUFFER_SIZE;

    let mut buffer = [b'0'; BUFFER_SIZE];
    let bytes = lexical_core::write(val, &mut buffer);
    string = match core::str::from_utf8(bytes).unwrap().try_into() {
        Ok(s) => s,
        Err(_) => String::try_from("ErRoR").unwrap(),
    };

    let max_num_len = max_len - mult.len() - unit.len();

    match string.find('.') {
        Some(dp_pos) => {
            let len_by_precision = dp_pos + precision + 1;
            let target_len = core::cmp::min(len_by_precision, max_num_len);
            while string.len() < target_len {
                _ = string.push('0');
            }
            string.truncate(target_len);
            // 10.12
            // dp is at 2,  len = 5, prec = 5 - 2 - 1 = 2
            /*let precision = string.len()
            let target_by_dp =
            if string.len() > len {
                string.truncate(core::cmp::max(len, dp_pos));
            }*/
        }
        None => {}
    }

    //let len = core::cmp::min(len - mult.len() - unit.len(), utf8.len());

    //let len = core::cmp::min(len - mult.len() - unit.len(), utf8.len());
    //string = utf8[0..len].into();

    _ = string.push_str(mult);
    _ = string.push_str(unit);
    string
}

//#[cfg(test)]
//mod format_number_Tests {

#[test]
fn test_format_numer() {
    assert_eq!(format_number(100.23, "m", "A", 8, 2), "100.23mA");
    assert_eq!(format_number(100.0, "m", "A", 8, 2), "100.00mA");
    assert_eq!(format_number(10.0, "m", "A", 8, 2), "10.00mA");
    assert_eq!(format_number(1.0, "m", "A", 8, 2), "1.00mA");
}
//}
