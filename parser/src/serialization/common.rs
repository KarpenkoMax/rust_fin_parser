
/// Форматирует целочисленное значение баланса (копейки) в человекочитаемый формат
pub(crate) fn format_minor_units<T>(value: T, decimal_separator: char) -> String
where
    T: Into<i128>,
{
    let v: i128 = value.into();
    let v = v.abs() as u128;
    let units = v / 100;
    let frac = v % 100;

    format!("{units}{decimal_separator}{frac:02}")
}