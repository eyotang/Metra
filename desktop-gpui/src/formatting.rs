//! Native equivalents of the WebView's Intl number formatters.
use fixed_decimal::{Decimal, SignedRoundingMode, UnsignedRoundingMode};
use icu_decimal::CompactDecimalFormatter;
use icu_experimental::dimension::currency::formatter::CurrencyFormatter;
use icu_locale::Locale;

pub(super) fn number(value: Option<u64>, locale: &str) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    let result = (|| {
        let locale: Locale = locale.parse().ok()?;
        let formatter =
            CompactDecimalFormatter::try_new_short(locale.into(), Default::default()).ok()?;
        let mut decimal = Decimal::from(value);
        let exponent = formatter.compact_exponent_for_magnitude(decimal.nonzero_magnitude_start());
        // ECMA-402 maximumFractionDigits: 1, halfExpand. ICU's convenience
        // format() instead uses two significant digits and halfEven.
        decimal.round_with_mode(
            i16::from(exponent) - 1,
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        );
        let exponent = formatter.compact_exponent_for_magnitude(decimal.nonzero_magnitude_start());
        decimal.multiply_pow10(-i16::from(exponent));
        decimal.trim_end();
        Some(
            formatter
                .format_with_exponent(&decimal, exponent)
                .ok()?
                .to_string(),
        )
    })();
    result.unwrap_or_else(|| value.to_string())
}

pub(super) fn money(value: Option<u64>, currency: &str, locale: &str) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    let result = (|| {
        let locale: Locale = locale.parse().ok()?;
        let use_code = locale.id.language.as_str() == "fr"
            && (currency.eq_ignore_ascii_case("JPY") || currency.eq_ignore_ascii_case("CNY"));
        let currency = currency.parse().ok()?;
        // Preserve the existing French JPY/CNY spelling across CLDR revisions.
        let formatter = if use_code {
            CurrencyFormatter::try_new_code(locale.into(), currency, Default::default()).ok()?
        } else {
            CurrencyFormatter::try_new_symbol(locale.into(), currency, Default::default()).ok()?
        };
        let mut decimal = Decimal::from(value);
        decimal.multiply_pow10(-2);
        Some(formatter.format_fixed_decimal(&decimal).to_string())
    })();
    result.unwrap_or_else(|| format!("{currency} {}.{:02}", value / 100, value % 100))
}

#[cfg(test)]
mod tests;

pub(super) fn date<T: chrono::TimeZone>(value: chrono::DateTime<T>, locale: &str) -> String {
    use chrono::{Datelike, Timelike};
    use icu_datetime::{
        DateTimeFormatter,
        fieldsets::MDT,
        input::{Date, DateTime, Time},
        options::TimePrecision,
    };
    let result = (|| {
        let locale: Locale = locale.parse().ok()?;
        let formatter = DateTimeFormatter::try_new(
            locale.into(),
            MDT::medium().with_time_precision(TimePrecision::Minute),
        )
        .ok()?;
        let date = Date::try_new_iso(value.year(), value.month() as u8, value.day() as u8).ok()?;
        let time = Time::try_new(value.hour() as u8, value.minute() as u8, 0, 0).ok()?;
        let mut rendered = formatter
            .format(&DateTime { date, time })
            .to_string()
            .replace('\u{202f}', " ");
        // Intl requests a two-digit hour, independently of the numeric day.
        if let Some(colon) = rendered.find(':') {
            let start = rendered[..colon]
                .bytes()
                .rposition(|b| !b.is_ascii_digit())
                .map_or(0, |i| i + 1);
            if colon - start == 1 {
                rendered.insert(start, '0');
            }
        }
        Some(rendered)
    })();
    result.unwrap_or_else(|| {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            value.year(),
            value.month(),
            value.day(),
            value.hour(),
            value.minute()
        )
    })
}
