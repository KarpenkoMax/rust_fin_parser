
/// Форматирует целочисленное значение баланса (копейки) в человекочитаемый формат
pub(super) fn format_minor_units<T>(value: T, decimal_separator: char) -> String
where
    T: Into<i128>,
{
    let v: i128 = value.into();
    let v = v.unsigned_abs();
    let units = v / 100;
    let frac = v % 100;

    format!("{units}{decimal_separator}{frac:02}")
}

#[cfg(test)]
mod tests {
    use super::format_minor_units;

    #[test]
    fn formats_zero() {
        assert_eq!(format_minor_units(0_i32, '.'), "0.00");
    }

    #[test]
    fn formats_less_than_one_unit() {
        assert_eq!(format_minor_units(1_i32, '.'), "0.01");
        assert_eq!(format_minor_units(10_i32, '.'), "0.10");
        assert_eq!(format_minor_units(99_i32, '.'), "0.99");
    }

    #[test]
    fn formats_whole_units_and_fraction() {
        assert_eq!(format_minor_units(100_i32, '.'), "1.00");
        assert_eq!(format_minor_units(101_i32, '.'), "1.01");
        assert_eq!(format_minor_units(12345_i32, '.'), "123.45");
        assert_eq!(format_minor_units(123456_i64, '.'), "1234.56");
    }

    #[test]
    fn uses_provided_decimal_separator() {
        assert_eq!(format_minor_units(12345_i32, ','), "123,45");
        assert_eq!(format_minor_units(5_i32, ','), "0,05");
    }

    #[test]
    fn works_with_different_numeric_types() {
        assert_eq!(format_minor_units(12345_u64, '.'), "123.45");
        assert_eq!(format_minor_units(12345_i128, '.'), "123.45");
    }

    #[test]
    fn ignores_sign_and_formats_absolute_value() {
        assert_eq!(format_minor_units(-12345_i32, '.'), "123.45");
        assert_eq!(format_minor_units(-5_i64, ','), "0,05");
    }
}

