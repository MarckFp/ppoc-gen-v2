use dioxus::prelude::*;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
struct Bundle(HashMap<String, String>);

static EN_JSON: &str = include_str!("../assets/i18n/en.json");
static ES_JSON: &str = include_str!("../assets/i18n/es.json");
static FR_JSON: &str = include_str!("../assets/i18n/fr.json");
static DE_JSON: &str = include_str!("../assets/i18n/de.json");

static BUNDLES: Lazy<HashMap<&'static str, Bundle>> = Lazy::new(|| {
    let mut m = HashMap::new();
    let parse = |s: &str| serde_json::from_str::<Bundle>(s).unwrap_or(Bundle(HashMap::new()));
    m.insert("en", parse(EN_JSON));
    m.insert("es", parse(ES_JSON));
    m.insert("fr", parse(FR_JSON));
    m.insert("de", parse(DE_JSON));
    m
});

#[derive(Clone)]
pub struct I18nState {
    pub lang: String,        // "en" | "es" | "fr" | "de" | "system"
    pub date_format: String, // e.g., "YYYY-MM-DD" | "DD/MM/YYYY" | "MM/DD/YYYY" | "DD MMM YYYY"
}

impl Default for I18nState {
    fn default() -> Self { Self { lang: detect_system_lang(), date_format: "YYYY-MM-DD".into() } }
}

#[cfg(target_arch = "wasm32")]
fn detect_system_lang() -> String {
    web_sys::window()
        .and_then(|w| w.navigator().language())
        .unwrap_or_else(|| "en".into())
        .split('-')
        .next()
        .unwrap_or("en")
        .to_lowercase()
}
#[cfg(not(target_arch = "wasm32"))]
fn detect_system_lang() -> String {
    std::env::var("LANG")
        .unwrap_or_else(|_| "en".into())
        .split('.').next().unwrap_or("en")
        .split('_').next().unwrap_or("en")
        .to_lowercase()
}

pub fn provide_i18n_from_config() {
    let initial = initial_state_from_config();
    let sig: Signal<I18nState> = use_signal(|| initial);
    provide_context(sig);
}

pub fn use_i18n() -> Signal<I18nState> { use_context::<Signal<I18nState>>() }

pub fn t(key: &str) -> String {
    let st = use_i18n().read().clone();
    let lang = if st.lang == "system" { detect_system_lang() } else { st.lang.clone() };
    let bundles = &*BUNDLES;
    bundles
        .get(lang.as_str())
        .and_then(|b| b.0.get(key).cloned())
        .or_else(|| bundles.get("en").and_then(|b| b.0.get(key).cloned()))
        .unwrap_or_else(|| key.to_string())
}

pub fn set_lang(new_lang: &str) {
    let mut sig = use_i18n();
    let mut guard = sig.write();
    guard.lang = match new_lang { "system"|"es"|"fr"|"de"|"en" => new_lang.to_string(), _ => "en".into() };
}

pub fn set_date_format(fmt: &str) {
    let mut sig = use_i18n();
    let mut guard = sig.write();
    guard.date_format = fmt.to_string();
}

// ===== Weekday helpers (centralized) =====
// Localized weekday names array based on current language.
#[cfg(target_arch = "wasm32")]
pub fn weekdays_for_locale() -> Vec<String> {
    // Read from i18n JSON: Monday..Sunday using common.* keys
    vec![
        t("common.monday"),
        t("common.tuesday"),
        t("common.wednesday"),
        t("common.thursday"),
        t("common.friday"),
        t("common.saturday"),
        t("common.sunday"),
    ]
}

#[cfg(not(target_arch = "wasm32"))]
pub fn weekdays_for_locale() -> Vec<String> {
    vec![
        t("common.monday"),
        t("common.tuesday"),
        t("common.wednesday"),
        t("common.thursday"),
        t("common.friday"),
        t("common.saturday"),
        t("common.sunday"),
    ]
}

// Map localized weekday name to index 1..=7 (Mon=1..Sun=7)
#[allow(dead_code)]
pub fn weekday_index_from_name(name: &str) -> u32 {
    let lower = name.to_lowercase();
    // First try dynamic localized names from weekdays_for_locale
    let names = weekdays_for_locale(); // Monday..Sunday
    if let Some(pos) = names.iter().position(|s| s.to_lowercase() == lower) {
        return (pos as u32) + 1;
    }
    // Fallback to known names across common locales
    match lower.as_str() {
        // Monday
        "monday" | "lunes" | "lundi" | "montag" => 1,
        // Tuesday
        "tuesday" | "martes" | "mardi" | "dienstag" => 2,
        // Wednesday (accept accented and unaccented spanish)
        "wednesday" | "miércoles" | "miercoles" | "mercredi" | "mittwoch" => 3,
        // Thursday
        "thursday" | "jueves" | "jeudi" | "donnerstag" => 4,
        // Friday
        "friday" | "viernes" | "vendredi" | "freitag" => 5,
        // Saturday (accept accented and unaccented spanish)
        "saturday" | "sábado" | "sabado" | "samedi" | "samstag" => 6,
        // Sunday (default)
        _ => 7,
    }
}

// Compute weekday index for a given date (1=Mon..7=Sun), cross-target
#[cfg(not(target_arch = "wasm32"))]
pub fn weekday_index_for_date(y: i32, m: u32, d: u32) -> u32 {
    use chrono::Datelike;
    chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap().weekday().number_from_monday()
}

#[cfg(target_arch = "wasm32")]
pub fn weekday_index_for_date(y: i32, m: u32, d: u32) -> u32 {
    // JS Date: months 0-based, get_day(): 0=Sunday..6=Saturday
    let date = js_sys::Date::new_with_year_month_day(y as u32, (m as i32) - 1, d as i32);
    let w = date.get_day() as u32; // 0..6
    ((w + 6) % 7) + 1 // 1=Mon .. 7=Sun
}

// Localized weekday display name for a date
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn weekday_name_for_date(y: i32, m: u32, d: u32) -> String {
    let idx = weekday_index_for_date(y, m, d) as usize; // 1..7
    let names = weekdays_for_locale();
    names[(idx - 1).min(6)].clone()
}

#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
pub fn weekday_name_for_date(y: i32, m: u32, d: u32) -> String {
    let idx = weekday_index_for_date(y, m, d) as usize; // 1..7
    let names = weekdays_for_locale();
    names[(idx - 1).min(6)].clone()
}

// Format a YYYY-MM-DD string according to configured format and locale
#[allow(dead_code)]
pub fn format_date_ymd(ymd: &str) -> String {
    let st = use_i18n().read().clone();
    let lang = if st.lang == "system" { detect_system_lang() } else { st.lang.clone() };
    let parts: Vec<&str> = ymd.split('-').collect();
    if parts.len() != 3 { return ymd.to_string(); }
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    let (yi, mi, di) = (
        y.parse::<i32>().unwrap_or(1970),
        m.parse::<u32>().unwrap_or(1),
        d.parse::<u32>().unwrap_or(1),
    );
    match st.date_format.as_str() {
        "YYYY-MM-DD" => format!("{:04}-{:02}-{:02}", yi, mi, di),
        "DD/MM/YYYY" => format!("{:02}/{:02}/{:04}", di, mi, yi),
        "MM/DD/YYYY" => format!("{:02}/{:02}/{:04}", mi, di, yi),
        "DD MMM YYYY" => format!("{:02} {} {:04}", di, month_name(mi, yi, &lang, true), yi),
        _ => format!("{:04}-{:02}-{:02}", yi, mi, di),
    }
}

#[cfg(target_arch = "wasm32")]
fn month_name(month: u32, _year: i32, _locale: &str, short: bool) -> String {
    // Use i18n JSON months
    let key = if short { format!("months.short.{}", month) } else { format!("months.long.{}", month) };
    t(&key)
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn month_name(month: u32, _year: i32, _locale: &str, short: bool) -> String {
    let key = if short { format!("months.short.{}", month) } else { format!("months.long.{}", month) };
    t(&key)
}

// === Theme application ===
#[cfg(target_arch = "wasm32")]
pub fn apply_theme(theme: &str) {
    use web_sys::window;
    if let Some(doc) = window().and_then(|w| w.document()) {
        if let Some(el) = doc.document_element() {
            // Read current class attribute, remove any existing 'dark', then conditionally add it
            let current = el.get_attribute("class").unwrap_or_default();
            let mut parts: Vec<&str> = current.split_whitespace().filter(|c| *c != "dark").collect();
            if theme.eq_ignore_ascii_case("dark") {
                parts.push("dark");
            }
            let new_cls = parts.join(" ");
            let _ = el.set_attribute("class", &new_cls);
        }
    }
}
#[cfg(all(not(target_arch = "wasm32")))]
pub fn apply_theme(_theme: &str) { /* no-op on native for now */ }

#[cfg(target_arch = "wasm32")]
fn get_cfg() -> Option<(String, String)> {
    use crate::db::wasm_store as backend;
    backend::get_configuration().map(|c| (c.language, c.date_format))
}
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
fn get_cfg() -> Option<(String, String)> {
    use crate::db::dao;
    if let Ok(c) = dao::get_configuration() { Some((c.language, c.date_format)) } else { None }
}
#[cfg(all(not(target_arch = "wasm32"), not(feature = "native-db")))]
fn get_cfg() -> Option<(String, String)> { None }

fn initial_state_from_config() -> I18nState {
    if let Some((lang, fmt)) = get_cfg() { I18nState { lang, date_format: fmt } } else { I18nState::default() }
}
