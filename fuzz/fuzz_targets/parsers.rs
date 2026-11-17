#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if data.len() > 4096 {
        return;
    }
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = freemkv_i18n::bundled_locale_json(text);
        let _ = freemkv_i18n::get(text);
        let _ = freemkv_i18n::fmt(text, &[("value", text)]);
    }
});
