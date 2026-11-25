// Reference outputs from the original WebView Intl options (Node v26.12.0).
use super::*;
#[test]
fn compact_matches_original_intl_in_all_languages() {
    assert_eq!(number(Some(0), "en"), "0", "en 0");
    assert_eq!(number(Some(1), "en"), "1", "en 1");
    assert_eq!(number(Some(999), "en"), "999", "en 999");
    assert_eq!(number(Some(1000), "en"), "1K", "en 1000");
    assert_eq!(number(Some(1650), "en"), "1.7K", "en 1650");
    assert_eq!(number(Some(12345), "en"), "12.3K", "en 12345");
    assert_eq!(number(Some(999949), "en"), "999.9K", "en 999949");
    assert_eq!(number(Some(999950), "en"), "1M", "en 999950");
    assert_eq!(number(Some(1000000), "en"), "1M", "en 1000000");
    assert_eq!(number(Some(123456789), "en"), "123.5M", "en 123456789");
    assert_eq!(number(Some(999999999), "en"), "1B", "en 999999999");
    assert_eq!(number(Some(1000000000000), "en"), "1T", "en 1000000000000");
    assert_eq!(number(Some(0), "zh-CN"), "0", "zh-CN 0");
    assert_eq!(number(Some(1), "zh-CN"), "1", "zh-CN 1");
    assert_eq!(number(Some(999), "zh-CN"), "999", "zh-CN 999");
    assert_eq!(number(Some(1000), "zh-CN"), "1000", "zh-CN 1000");
    assert_eq!(number(Some(1650), "zh-CN"), "1650", "zh-CN 1650");
    assert_eq!(number(Some(12345), "zh-CN"), "1.2万", "zh-CN 12345");
    assert_eq!(number(Some(999949), "zh-CN"), "100万", "zh-CN 999949");
    assert_eq!(number(Some(999950), "zh-CN"), "100万", "zh-CN 999950");
    assert_eq!(number(Some(1000000), "zh-CN"), "100万", "zh-CN 1000000");
    assert_eq!(number(Some(123456789), "zh-CN"), "1.2亿", "zh-CN 123456789");
    assert_eq!(number(Some(999999999), "zh-CN"), "10亿", "zh-CN 999999999");
    assert_eq!(
        number(Some(1000000000000), "zh-CN"),
        "1万亿",
        "zh-CN 1000000000000"
    );
    assert_eq!(number(Some(0), "ja"), "0", "ja 0");
    assert_eq!(number(Some(1), "ja"), "1", "ja 1");
    assert_eq!(number(Some(999), "ja"), "999", "ja 999");
    assert_eq!(number(Some(1000), "ja"), "1000", "ja 1000");
    assert_eq!(number(Some(1650), "ja"), "1650", "ja 1650");
    assert_eq!(number(Some(12345), "ja"), "1.2万", "ja 12345");
    assert_eq!(number(Some(999949), "ja"), "100万", "ja 999949");
    assert_eq!(number(Some(999950), "ja"), "100万", "ja 999950");
    assert_eq!(number(Some(1000000), "ja"), "100万", "ja 1000000");
    assert_eq!(number(Some(123456789), "ja"), "1.2億", "ja 123456789");
    assert_eq!(number(Some(999999999), "ja"), "10億", "ja 999999999");
    assert_eq!(number(Some(1000000000000), "ja"), "1兆", "ja 1000000000000");
    assert_eq!(number(Some(0), "ko"), "0", "ko 0");
    assert_eq!(number(Some(1), "ko"), "1", "ko 1");
    assert_eq!(number(Some(999), "ko"), "999", "ko 999");
    assert_eq!(number(Some(1000), "ko"), "1천", "ko 1000");
    assert_eq!(number(Some(1650), "ko"), "1.7천", "ko 1650");
    assert_eq!(number(Some(12345), "ko"), "1.2만", "ko 12345");
    assert_eq!(number(Some(999949), "ko"), "100만", "ko 999949");
    assert_eq!(number(Some(999950), "ko"), "100만", "ko 999950");
    assert_eq!(number(Some(1000000), "ko"), "100만", "ko 1000000");
    assert_eq!(number(Some(123456789), "ko"), "1.2억", "ko 123456789");
    assert_eq!(number(Some(999999999), "ko"), "10억", "ko 999999999");
    assert_eq!(number(Some(1000000000000), "ko"), "1조", "ko 1000000000000");
    assert_eq!(number(Some(0), "fr"), "0", "fr 0");
    assert_eq!(number(Some(1), "fr"), "1", "fr 1");
    assert_eq!(number(Some(999), "fr"), "999", "fr 999");
    assert_eq!(number(Some(1000), "fr"), "1 k", "fr 1000");
    assert_eq!(number(Some(1650), "fr"), "1,7 k", "fr 1650");
    assert_eq!(number(Some(12345), "fr"), "12,3 k", "fr 12345");
    assert_eq!(number(Some(999949), "fr"), "999,9 k", "fr 999949");
    assert_eq!(number(Some(999950), "fr"), "1 M", "fr 999950");
    assert_eq!(number(Some(1000000), "fr"), "1 M", "fr 1000000");
    assert_eq!(number(Some(123456789), "fr"), "123,5 M", "fr 123456789");
    assert_eq!(number(Some(999999999), "fr"), "1 Md", "fr 999999999");
    assert_eq!(
        number(Some(1000000000000), "fr"),
        "1 Bn",
        "fr 1000000000000"
    );
    assert_eq!(number(Some(0), "de"), "0", "de 0");
    assert_eq!(number(Some(1), "de"), "1", "de 1");
    assert_eq!(number(Some(999), "de"), "999", "de 999");
    assert_eq!(number(Some(1000), "de"), "1000", "de 1000");
    assert_eq!(number(Some(1650), "de"), "1650", "de 1650");
    assert_eq!(number(Some(12345), "de"), "12.345", "de 12345");
    assert_eq!(number(Some(999949), "de"), "999.949", "de 999949");
    assert_eq!(number(Some(999950), "de"), "999.950", "de 999950");
    assert_eq!(number(Some(1000000), "de"), "1 Mio.", "de 1000000");
    assert_eq!(number(Some(123456789), "de"), "123,5 Mio.", "de 123456789");
    assert_eq!(number(Some(999999999), "de"), "1 Mrd.", "de 999999999");
    assert_eq!(
        number(Some(1000000000000), "de"),
        "1 Bio.",
        "de 1000000000000"
    );
    assert_eq!(number(Some(0), "ru"), "0", "ru 0");
    assert_eq!(number(Some(1), "ru"), "1", "ru 1");
    assert_eq!(number(Some(999), "ru"), "999", "ru 999");
    assert_eq!(number(Some(1000), "ru"), "1 тыс.", "ru 1000");
    assert_eq!(number(Some(1650), "ru"), "1,7 тыс.", "ru 1650");
    assert_eq!(number(Some(12345), "ru"), "12,3 тыс.", "ru 12345");
    assert_eq!(number(Some(999949), "ru"), "999,9 тыс.", "ru 999949");
    assert_eq!(number(Some(999950), "ru"), "1 млн", "ru 999950");
    assert_eq!(number(Some(1000000), "ru"), "1 млн", "ru 1000000");
    assert_eq!(number(Some(123456789), "ru"), "123,5 млн", "ru 123456789");
    assert_eq!(number(Some(999999999), "ru"), "1 млрд", "ru 999999999");
    assert_eq!(
        number(Some(1000000000000), "ru"),
        "1 трлн",
        "ru 1000000000000"
    );
}
#[test]
fn currencies_match_original_intl_in_all_languages() {
    assert_eq!(money(Some(0), "USD", "en"), "$0.00", "en USD 0");
    assert_eq!(money(Some(50), "USD", "en"), "$0.50", "en USD 50");
    assert_eq!(
        money(Some(123456), "USD", "en"),
        "$1,234.56",
        "en USD 123456"
    );
    assert_eq!(
        money(Some(123450), "USD", "en"),
        "$1,234.50",
        "en USD 123450"
    );
    assert_eq!(money(Some(0), "EUR", "en"), "€0.00", "en EUR 0");
    assert_eq!(money(Some(50), "EUR", "en"), "€0.50", "en EUR 50");
    assert_eq!(
        money(Some(123456), "EUR", "en"),
        "€1,234.56",
        "en EUR 123456"
    );
    assert_eq!(
        money(Some(123450), "EUR", "en"),
        "€1,234.50",
        "en EUR 123450"
    );
    assert_eq!(money(Some(0), "JPY", "en"), "¥0", "en JPY 0");
    assert_eq!(money(Some(50), "JPY", "en"), "¥1", "en JPY 50");
    assert_eq!(money(Some(123456), "JPY", "en"), "¥1,235", "en JPY 123456");
    assert_eq!(money(Some(123450), "JPY", "en"), "¥1,235", "en JPY 123450");
    assert_eq!(money(Some(0), "KRW", "en"), "₩0", "en KRW 0");
    assert_eq!(money(Some(50), "KRW", "en"), "₩1", "en KRW 50");
    assert_eq!(money(Some(123456), "KRW", "en"), "₩1,235", "en KRW 123456");
    assert_eq!(money(Some(123450), "KRW", "en"), "₩1,235", "en KRW 123450");
    assert_eq!(money(Some(0), "CNY", "en"), "CN¥0.00", "en CNY 0");
    assert_eq!(money(Some(50), "CNY", "en"), "CN¥0.50", "en CNY 50");
    assert_eq!(
        money(Some(123456), "CNY", "en"),
        "CN¥1,234.56",
        "en CNY 123456"
    );
    assert_eq!(
        money(Some(123450), "CNY", "en"),
        "CN¥1,234.50",
        "en CNY 123450"
    );
    assert_eq!(money(Some(0), "USD", "zh-CN"), "US$0.00", "zh-CN USD 0");
    assert_eq!(money(Some(50), "USD", "zh-CN"), "US$0.50", "zh-CN USD 50");
    assert_eq!(
        money(Some(123456), "USD", "zh-CN"),
        "US$1,234.56",
        "zh-CN USD 123456"
    );
    assert_eq!(
        money(Some(123450), "USD", "zh-CN"),
        "US$1,234.50",
        "zh-CN USD 123450"
    );
    assert_eq!(money(Some(0), "EUR", "zh-CN"), "€0.00", "zh-CN EUR 0");
    assert_eq!(money(Some(50), "EUR", "zh-CN"), "€0.50", "zh-CN EUR 50");
    assert_eq!(
        money(Some(123456), "EUR", "zh-CN"),
        "€1,234.56",
        "zh-CN EUR 123456"
    );
    assert_eq!(
        money(Some(123450), "EUR", "zh-CN"),
        "€1,234.50",
        "zh-CN EUR 123450"
    );
    assert_eq!(money(Some(0), "JPY", "zh-CN"), "JP¥0", "zh-CN JPY 0");
    assert_eq!(money(Some(50), "JPY", "zh-CN"), "JP¥1", "zh-CN JPY 50");
    assert_eq!(
        money(Some(123456), "JPY", "zh-CN"),
        "JP¥1,235",
        "zh-CN JPY 123456"
    );
    assert_eq!(
        money(Some(123450), "JPY", "zh-CN"),
        "JP¥1,235",
        "zh-CN JPY 123450"
    );
    assert_eq!(money(Some(0), "KRW", "zh-CN"), "₩0", "zh-CN KRW 0");
    assert_eq!(money(Some(50), "KRW", "zh-CN"), "₩1", "zh-CN KRW 50");
    assert_eq!(
        money(Some(123456), "KRW", "zh-CN"),
        "₩1,235",
        "zh-CN KRW 123456"
    );
    assert_eq!(
        money(Some(123450), "KRW", "zh-CN"),
        "₩1,235",
        "zh-CN KRW 123450"
    );
    assert_eq!(money(Some(0), "CNY", "zh-CN"), "¥0.00", "zh-CN CNY 0");
    assert_eq!(money(Some(50), "CNY", "zh-CN"), "¥0.50", "zh-CN CNY 50");
    assert_eq!(
        money(Some(123456), "CNY", "zh-CN"),
        "¥1,234.56",
        "zh-CN CNY 123456"
    );
    assert_eq!(
        money(Some(123450), "CNY", "zh-CN"),
        "¥1,234.50",
        "zh-CN CNY 123450"
    );
    assert_eq!(money(Some(0), "USD", "ja"), "$0.00", "ja USD 0");
    assert_eq!(money(Some(50), "USD", "ja"), "$0.50", "ja USD 50");
    assert_eq!(
        money(Some(123456), "USD", "ja"),
        "$1,234.56",
        "ja USD 123456"
    );
    assert_eq!(
        money(Some(123450), "USD", "ja"),
        "$1,234.50",
        "ja USD 123450"
    );
    assert_eq!(money(Some(0), "EUR", "ja"), "€0.00", "ja EUR 0");
    assert_eq!(money(Some(50), "EUR", "ja"), "€0.50", "ja EUR 50");
    assert_eq!(
        money(Some(123456), "EUR", "ja"),
        "€1,234.56",
        "ja EUR 123456"
    );
    assert_eq!(
        money(Some(123450), "EUR", "ja"),
        "€1,234.50",
        "ja EUR 123450"
    );
    assert_eq!(money(Some(0), "JPY", "ja"), "￥0", "ja JPY 0");
    assert_eq!(money(Some(50), "JPY", "ja"), "￥1", "ja JPY 50");
    assert_eq!(money(Some(123456), "JPY", "ja"), "￥1,235", "ja JPY 123456");
    assert_eq!(money(Some(123450), "JPY", "ja"), "￥1,235", "ja JPY 123450");
    assert_eq!(money(Some(0), "KRW", "ja"), "₩0", "ja KRW 0");
    assert_eq!(money(Some(50), "KRW", "ja"), "₩1", "ja KRW 50");
    assert_eq!(money(Some(123456), "KRW", "ja"), "₩1,235", "ja KRW 123456");
    assert_eq!(money(Some(123450), "KRW", "ja"), "₩1,235", "ja KRW 123450");
    assert_eq!(money(Some(0), "CNY", "ja"), "元 0.00", "ja CNY 0");
    assert_eq!(money(Some(50), "CNY", "ja"), "元 0.50", "ja CNY 50");
    assert_eq!(
        money(Some(123456), "CNY", "ja"),
        "元 1,234.56",
        "ja CNY 123456"
    );
    assert_eq!(
        money(Some(123450), "CNY", "ja"),
        "元 1,234.50",
        "ja CNY 123450"
    );
    assert_eq!(money(Some(0), "USD", "ko"), "US$0.00", "ko USD 0");
    assert_eq!(money(Some(50), "USD", "ko"), "US$0.50", "ko USD 50");
    assert_eq!(
        money(Some(123456), "USD", "ko"),
        "US$1,234.56",
        "ko USD 123456"
    );
    assert_eq!(
        money(Some(123450), "USD", "ko"),
        "US$1,234.50",
        "ko USD 123450"
    );
    assert_eq!(money(Some(0), "EUR", "ko"), "€0.00", "ko EUR 0");
    assert_eq!(money(Some(50), "EUR", "ko"), "€0.50", "ko EUR 50");
    assert_eq!(
        money(Some(123456), "EUR", "ko"),
        "€1,234.56",
        "ko EUR 123456"
    );
    assert_eq!(
        money(Some(123450), "EUR", "ko"),
        "€1,234.50",
        "ko EUR 123450"
    );
    assert_eq!(money(Some(0), "JPY", "ko"), "JP¥0", "ko JPY 0");
    assert_eq!(money(Some(50), "JPY", "ko"), "JP¥1", "ko JPY 50");
    assert_eq!(
        money(Some(123456), "JPY", "ko"),
        "JP¥1,235",
        "ko JPY 123456"
    );
    assert_eq!(
        money(Some(123450), "JPY", "ko"),
        "JP¥1,235",
        "ko JPY 123450"
    );
    assert_eq!(money(Some(0), "KRW", "ko"), "₩0", "ko KRW 0");
    assert_eq!(money(Some(50), "KRW", "ko"), "₩1", "ko KRW 50");
    assert_eq!(money(Some(123456), "KRW", "ko"), "₩1,235", "ko KRW 123456");
    assert_eq!(money(Some(123450), "KRW", "ko"), "₩1,235", "ko KRW 123450");
    assert_eq!(money(Some(0), "CNY", "ko"), "CN¥0.00", "ko CNY 0");
    assert_eq!(money(Some(50), "CNY", "ko"), "CN¥0.50", "ko CNY 50");
    assert_eq!(
        money(Some(123456), "CNY", "ko"),
        "CN¥1,234.56",
        "ko CNY 123456"
    );
    assert_eq!(
        money(Some(123450), "CNY", "ko"),
        "CN¥1,234.50",
        "ko CNY 123450"
    );
    assert_eq!(money(Some(0), "USD", "fr"), "0,00 $US", "fr USD 0");
    assert_eq!(money(Some(50), "USD", "fr"), "0,50 $US", "fr USD 50");
    assert_eq!(
        money(Some(123456), "USD", "fr"),
        "1 234,56 $US",
        "fr USD 123456"
    );
    assert_eq!(
        money(Some(123450), "USD", "fr"),
        "1 234,50 $US",
        "fr USD 123450"
    );
    assert_eq!(money(Some(0), "EUR", "fr"), "0,00 €", "fr EUR 0");
    assert_eq!(money(Some(50), "EUR", "fr"), "0,50 €", "fr EUR 50");
    assert_eq!(
        money(Some(123456), "EUR", "fr"),
        "1 234,56 €",
        "fr EUR 123456"
    );
    assert_eq!(
        money(Some(123450), "EUR", "fr"),
        "1 234,50 €",
        "fr EUR 123450"
    );
    assert_eq!(money(Some(0), "JPY", "fr"), "0 JPY", "fr JPY 0");
    assert_eq!(money(Some(50), "JPY", "fr"), "1 JPY", "fr JPY 50");
    assert_eq!(
        money(Some(123456), "JPY", "fr"),
        "1 235 JPY",
        "fr JPY 123456"
    );
    assert_eq!(
        money(Some(123450), "JPY", "fr"),
        "1 235 JPY",
        "fr JPY 123450"
    );
    assert_eq!(money(Some(0), "KRW", "fr"), "0 ₩", "fr KRW 0");
    assert_eq!(money(Some(50), "KRW", "fr"), "1 ₩", "fr KRW 50");
    assert_eq!(money(Some(123456), "KRW", "fr"), "1 235 ₩", "fr KRW 123456");
    assert_eq!(money(Some(123450), "KRW", "fr"), "1 235 ₩", "fr KRW 123450");
    assert_eq!(money(Some(0), "CNY", "fr"), "0,00 CNY", "fr CNY 0");
    assert_eq!(money(Some(50), "CNY", "fr"), "0,50 CNY", "fr CNY 50");
    assert_eq!(
        money(Some(123456), "CNY", "fr"),
        "1 234,56 CNY",
        "fr CNY 123456"
    );
    assert_eq!(
        money(Some(123450), "CNY", "fr"),
        "1 234,50 CNY",
        "fr CNY 123450"
    );
    assert_eq!(money(Some(0), "USD", "de"), "0,00 $", "de USD 0");
    assert_eq!(money(Some(50), "USD", "de"), "0,50 $", "de USD 50");
    assert_eq!(
        money(Some(123456), "USD", "de"),
        "1.234,56 $",
        "de USD 123456"
    );
    assert_eq!(
        money(Some(123450), "USD", "de"),
        "1.234,50 $",
        "de USD 123450"
    );
    assert_eq!(money(Some(0), "EUR", "de"), "0,00 €", "de EUR 0");
    assert_eq!(money(Some(50), "EUR", "de"), "0,50 €", "de EUR 50");
    assert_eq!(
        money(Some(123456), "EUR", "de"),
        "1.234,56 €",
        "de EUR 123456"
    );
    assert_eq!(
        money(Some(123450), "EUR", "de"),
        "1.234,50 €",
        "de EUR 123450"
    );
    assert_eq!(money(Some(0), "JPY", "de"), "0 ¥", "de JPY 0");
    assert_eq!(money(Some(50), "JPY", "de"), "1 ¥", "de JPY 50");
    assert_eq!(money(Some(123456), "JPY", "de"), "1.235 ¥", "de JPY 123456");
    assert_eq!(money(Some(123450), "JPY", "de"), "1.235 ¥", "de JPY 123450");
    assert_eq!(money(Some(0), "KRW", "de"), "0 ₩", "de KRW 0");
    assert_eq!(money(Some(50), "KRW", "de"), "1 ₩", "de KRW 50");
    assert_eq!(money(Some(123456), "KRW", "de"), "1.235 ₩", "de KRW 123456");
    assert_eq!(money(Some(123450), "KRW", "de"), "1.235 ₩", "de KRW 123450");
    assert_eq!(money(Some(0), "CNY", "de"), "0,00 CN¥", "de CNY 0");
    assert_eq!(money(Some(50), "CNY", "de"), "0,50 CN¥", "de CNY 50");
    assert_eq!(
        money(Some(123456), "CNY", "de"),
        "1.234,56 CN¥",
        "de CNY 123456"
    );
    assert_eq!(
        money(Some(123450), "CNY", "de"),
        "1.234,50 CN¥",
        "de CNY 123450"
    );
    assert_eq!(money(Some(0), "USD", "ru"), "0,00 $", "ru USD 0");
    assert_eq!(money(Some(50), "USD", "ru"), "0,50 $", "ru USD 50");
    assert_eq!(
        money(Some(123456), "USD", "ru"),
        "1 234,56 $",
        "ru USD 123456"
    );
    assert_eq!(
        money(Some(123450), "USD", "ru"),
        "1 234,50 $",
        "ru USD 123450"
    );
    assert_eq!(money(Some(0), "EUR", "ru"), "0,00 €", "ru EUR 0");
    assert_eq!(money(Some(50), "EUR", "ru"), "0,50 €", "ru EUR 50");
    assert_eq!(
        money(Some(123456), "EUR", "ru"),
        "1 234,56 €",
        "ru EUR 123456"
    );
    assert_eq!(
        money(Some(123450), "EUR", "ru"),
        "1 234,50 €",
        "ru EUR 123450"
    );
    assert_eq!(money(Some(0), "JPY", "ru"), "0 ¥", "ru JPY 0");
    assert_eq!(money(Some(50), "JPY", "ru"), "1 ¥", "ru JPY 50");
    assert_eq!(money(Some(123456), "JPY", "ru"), "1 235 ¥", "ru JPY 123456");
    assert_eq!(money(Some(123450), "JPY", "ru"), "1 235 ¥", "ru JPY 123450");
    assert_eq!(money(Some(0), "KRW", "ru"), "0 ₩", "ru KRW 0");
    assert_eq!(money(Some(50), "KRW", "ru"), "1 ₩", "ru KRW 50");
    assert_eq!(money(Some(123456), "KRW", "ru"), "1 235 ₩", "ru KRW 123456");
    assert_eq!(money(Some(123450), "KRW", "ru"), "1 235 ₩", "ru KRW 123450");
    assert_eq!(money(Some(0), "CNY", "ru"), "0,00 CN¥", "ru CNY 0");
    assert_eq!(money(Some(50), "CNY", "ru"), "0,50 CN¥", "ru CNY 50");
    assert_eq!(
        money(Some(123456), "CNY", "ru"),
        "1 234,56 CN¥",
        "ru CNY 123456"
    );
    assert_eq!(
        money(Some(123450), "CNY", "ru"),
        "1 234,50 CN¥",
        "ru CNY 123450"
    );
}

#[test]
fn localized_dates_match_original_fields() {
    use chrono::TimeZone;
    assert_eq!(
        date(
            chrono::Utc.with_ymd_and_hms(2026, 11, 5, 3, 7, 0).unwrap(),
            "en"
        )
        .replace('\u{202f}', " "),
        "Nov 5, 03:07 AM",
        "en"
    );
    assert_eq!(
        date(
            chrono::Utc.with_ymd_and_hms(2026, 11, 5, 3, 7, 0).unwrap(),
            "zh-CN"
        )
        .replace('\u{202f}', " "),
        "11月5日 03:07",
        "zh-CN"
    );
    assert_eq!(
        date(
            chrono::Utc.with_ymd_and_hms(2026, 11, 5, 3, 7, 0).unwrap(),
            "ja"
        )
        .replace('\u{202f}', " "),
        "11月5日 03:07",
        "ja"
    );
    assert_eq!(
        date(
            chrono::Utc.with_ymd_and_hms(2026, 11, 5, 3, 7, 0).unwrap(),
            "ko"
        )
        .replace('\u{202f}', " "),
        "11월 5일 오전 03:07",
        "ko"
    );
    assert_eq!(
        date(
            chrono::Utc.with_ymd_and_hms(2026, 11, 5, 3, 7, 0).unwrap(),
            "fr"
        )
        .replace('\u{202f}', " "),
        "5 nov., 03:07",
        "fr"
    );
    assert_eq!(
        date(
            chrono::Utc.with_ymd_and_hms(2026, 11, 5, 3, 7, 0).unwrap(),
            "de"
        )
        .replace('\u{202f}', " "),
        "5. Nov., 03:07",
        "de"
    );
    assert_eq!(
        date(
            chrono::Utc.with_ymd_and_hms(2026, 11, 5, 3, 7, 0).unwrap(),
            "ru"
        )
        .replace('\u{202f}', " "),
        "5 нояб., 03:07",
        "ru"
    );
}
