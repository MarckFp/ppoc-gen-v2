use dioxus::prelude::*;
use crate::i18n::{set_lang, set_date_format};
use crate::i18n::t;

#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))] mod backend { pub use crate::db::dao::{configuration_is_set, update_configuration}; }
#[cfg(target_arch = "wasm32")] use crate::db::wasm_store as backend;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "native-db")))]
#[allow(dead_code)]
mod backend { pub fn update_configuration(_: &str, _: &str, _: &str, _: &str, _: &str, _: &str) {} pub fn configuration_is_set() -> bool { false } }

#[component]
pub fn Landpage() -> Element {
    // Simple form state
    let mut name = use_signal(|| String::new());
    let mut theme = use_signal(|| String::from("System"));
    // Access global configured signal (provided in App)
    let mut configured: Signal<bool> = use_context();
    let mut week_start = use_signal(|| String::from("monday"));
    let mut language = use_signal(|| String::from("system"));
    let mut date_format = use_signal(|| String::from("YYYY-MM-DD"));

    let submit = move |_| {
        let n = name.read().trim().to_string();
        if n.is_empty() { return; }
        // Persist configuration (default name order first_last, choose week_start)
    backend::update_configuration(&n, &theme.read(), "first_last", &week_start.read(), &language.read(), &date_format.read());
    // Flip global flag so components react immediately and update i18n
        configured.set(true);
    set_lang(&language.read());
    set_date_format(&date_format.read());
        // On web builds, force a reload so Home renders instead of Landpage
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(win) = web_sys::window() {
                let _ = win.location().reload();
            }
        }
    };

    rsx! { div { class: "min-h-full flex flex-col items-center justify-start pt-10",
        div { class: "w-full max-w-xl space-y-8",
            div { class: "space-y-2",
                h1 { class: "text-3xl font-bold tracking-tight", {t("landpage.title")} }
                p { class: "text-sm text-slate-600 dark:text-slate-300", {t("landpage.subtitle")} }
            }
            div { class: "bg-slate-50 dark:bg-slate-900/50 border border-slate-200 dark:border-slate-700 rounded-lg p-6 shadow-sm space-y-5",
                div { class: "flex flex-col gap-2",
                    label { class: "text-sm font-medium text-slate-700 dark:text-slate-200", {t("landpage.name")} }
                    input { class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-800 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", value: name.read().clone(), oninput: move |e| name.set(e.value()) }
                }
                div { class: "flex flex-col gap-2",
                    label { class: "text-sm font-medium text-slate-700 dark:text-slate-200", {t("landpage.theme")} }
                    select { class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-800 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", value: theme.read().clone(), oninput: move |e| theme.set(e.value()),
                        option { value: "System", {t("common.system")} }
                        option { value: "Light", {t("common.light")} }
                        option { value: "Dark", {t("common.dark")} }
                    }
                }
                div { class: "flex flex-col gap-2",
                    label { class: "text-sm font-medium text-slate-700 dark:text-slate-200", "Language" }
                    select { class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-800 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", value: language.read().clone(), oninput: move |e| language.set(e.value()),
                        option { value: "system", {t("common.system")} }
                        option { value: "en", "English" }
                        option { value: "es", "Español" }
                        option { value: "fr", "Français" }
                        option { value: "de", "Deutsch" }
                    }
                }
                div { class: "flex flex-col gap-2",
                    label { class: "text-sm font-medium text-slate-700 dark:text-slate-200", "Date format" }
                    select { class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-800 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", value: date_format.read().clone(), oninput: move |e| date_format.set(e.value()),
                        option { value: "YYYY-MM-DD", "YYYY-MM-DD (2025-06-01)" }
                        option { value: "DD/MM/YYYY", "DD/MM/YYYY (01/06/2025)" }
                        option { value: "MM/DD/YYYY", "MM/DD/YYYY (06/01/2025)" }
                        option { value: "DD MMM YYYY", "DD MMM YYYY (01 Jun 2025)" }
                    }
                }
                div { class: "flex flex-col gap-2",
                    label { class: "text-sm font-medium text-slate-700 dark:text-slate-200", {t("landpage.week_start")} }
                    select { class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-800 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", value: week_start.read().clone(), oninput: move |e| week_start.set(e.value()),
                        option { value: "monday", {t("common.monday")} }
                        option { value: "sunday", {t("common.sunday")} }
                    }
                }
                button { class: "inline-flex items-center gap-2 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium px-4 py-2 transition disabled:opacity-50 disabled:cursor-not-allowed", onclick: submit, {t("landpage.save")} }
            }
        }
    } }
}
