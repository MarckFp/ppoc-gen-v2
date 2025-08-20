use dioxus::prelude::*;
use crate::i18n::t;

#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
mod backend {
    use crate::db::dao;
    #[derive(Clone)]
    pub struct Configuration { pub congregation_name: String, pub theme: String, pub name_order: String, pub week_start: String, pub language: String, pub date_format: String }
    pub fn get_configuration() -> Option<Configuration> { dao::get_configuration().ok().map(|c| Configuration { congregation_name: c.congregation_name, theme: c.theme, name_order: c.name_order, week_start: c.week_start, language: c.language, date_format: c.date_format }) }
    pub fn update_configuration(name: &str, theme: &str, name_order: &str, week_start: &str, language: &str, date_format: &str) { let _ = dao::update_configuration(name, theme, name_order, week_start, language, date_format); }
    pub fn export_data() -> Option<String> { dao::export_data().ok() }
    pub fn import_data(json: &str) -> bool { dao::import_data(json).is_ok() }
    pub fn reset_data() -> bool { dao::reset_data().is_ok() }
}
#[cfg(target_arch = "wasm32")]
use crate::db::wasm_store as backend;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "native-db")))]
#[allow(dead_code)]
mod backend {
    #[derive(Clone)]
    pub struct Configuration { pub congregation_name: String, pub theme: String, pub name_order: String, pub week_start: String, pub language: String, pub date_format: String }
    pub fn get_configuration() -> Option<Configuration> { None }
    pub fn update_configuration(_name: &str, _theme: &str, _name_order: &str, _week_start: &str, _language: &str, _date_format: &str) {}
    pub fn export_data() -> Option<String> { Some("{}".to_string()) }
    pub fn import_data(_json: &str) -> bool { true }
    pub fn reset_data() -> bool { true }
}

#[component]
pub fn Configuration() -> Element {
    let mut name = use_signal(|| String::new());
    let mut theme = use_signal(|| String::from("System"));
    let mut name_order = use_signal(|| String::from("first_last"));
    let mut week_start = use_signal(|| String::from("monday"));
    let mut saved = use_signal(|| false);
    let mut language = use_signal(|| String::from("system"));
    let mut date_format = use_signal(|| String::from("YYYY-MM-DD"));
    let mut confirm_import = use_signal(|| false);
    let mut import_error = use_signal(|| Option::<String>::None);
    let mut confirm_reset = use_signal(|| false);
    // Access global configured flag from App to toggle after a reset
    let mut configured: Signal<bool> = use_context();

    // Load existing configuration on mount (web or native-db builds)
    use_effect(move || {
    #[cfg(any(target_arch = "wasm32", all(feature = "native-db", not(target_arch = "wasm32"))))]
        if let Some(cfg) = backend::get_configuration() {
            name.set(cfg.congregation_name);
            theme.set(cfg.theme);
            name_order.set(cfg.name_order);
            week_start.set(cfg.week_start);
            language.set(cfg.language);
            date_format.set(cfg.date_format);
        }
    });

    let on_save = move |_| {
        let n = name.read().trim().to_string();
        if n.is_empty() { return; }
    backend::update_configuration(&n, &theme.read(), &name_order.read(), &week_start.read(), &language.read(), &date_format.read());
        crate::i18n::set_lang(&language.read());
        crate::i18n::set_date_format(&date_format.read());
        crate::i18n::apply_theme(&theme.read());
        saved.set(true);
    };

    // Export handler
    // Helper to get export JSON on all targets
    #[cfg(target_arch = "wasm32")]
    fn get_export_json() -> Option<String> { Some(backend::export_data()) }
    #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
    fn get_export_json() -> Option<String> { backend::export_data() }
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "native-db")))]
    fn get_export_json() -> Option<String> { backend::export_data() }

    let on_export = move |_| {
    if let Some(_json) = get_export_json() {
            #[cfg(target_arch = "wasm32")]
            {
                if let Some(win) = web_sys::window() {
                    if let Some(doc) = win.document() {
                        if let Ok(a) = doc.create_element("a") {
                            use web_sys::wasm_bindgen::JsCast;
                let href = format!("data:application/json;charset=utf-8,{}", urlencoding::encode(&_json));
                            a.set_attribute("href", &href).ok();
                            a.set_attribute("download", "dx_app_export.json").ok();
                            if let Ok(ae) = a.dyn_into::<web_sys::HtmlElement>() { ae.click(); }
                        }
                    }
                }
            }
            #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
            {
        let path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join("dx_app_export.json");
        let _ = std::fs::write(path, _json);
            }
        }
    };

    // File-based import

    // Import button: trigger hidden file input (web) or open confirm (native)
    let on_import_click = move |_| {
        import_error.set(None);
        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::{window, HtmlElement};
            use web_sys::wasm_bindgen::JsCast;
            if let Some(doc) = window().and_then(|w| w.document()) {
                if let Some(el) = doc.get_element_by_id("importFile") {
                    if let Ok(btn) = el.dyn_into::<HtmlElement>() {
                        btn.click();
                        return;
                    }
                }
            }
        }
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            // Native fallback: confirm and read default path
            confirm_import.set(true);
        }
    };

    // Confirmed import: read file (web) or read from default path (native stub)
    let do_import = move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::{window, HtmlInputElement, FileReader, Event};
            use web_sys::wasm_bindgen::JsCast;
            if let Some(doc) = window().and_then(|w| w.document()) {
                if let Some(el) = doc.get_element_by_id("importFile") {
                    if let Ok(input) = el.dyn_into::<HtmlInputElement>() {
                        if let Some(files) = input.files() {
                            if files.length() > 0 {
                                if let Some(file) = files.get(0) {
                                    let reader = FileReader::new().unwrap();
                                    let fr_c = reader.clone();
                                    let mut confirm_copy = confirm_import.clone();
                                    let mut import_err_copy = import_error.clone();
                                    let onload = web_sys::wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: Event| {
                                        let result = fr_c.result().unwrap_or(web_sys::wasm_bindgen::JsValue::from_str(""));
                                        let text = result.as_string().unwrap_or_default();
                                        if backend::import_data(&text) { confirm_copy.set(false); } else { import_err_copy.set(Some(t("config.import_invalid_file"))); }
                                    }) as Box<dyn FnMut(_)>);
                                    reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                                    onload.forget();
                                    let _ = reader.read_as_text(&file);
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            import_error.set(Some(t("config.import_choose_file")));
        }
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            let path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join("dx_app_export.json");
            if let Ok(text) = std::fs::read_to_string(path) {
                let ok = backend::import_data(&text);
                if ok { confirm_import.set(false); } else { import_error.set(Some(t("config.import_invalid_file"))); }
            } else {
                import_error.set(Some(t("config.import_could_not_read")));
            }
        }
    };

    // File input node for web import
    let import_box = {
        // Hidden file input; when a file is chosen, open confirmation modal
        let mut confirm_copy = confirm_import.clone();
        rsx!(
            input {
                id: "importFile",
                r#type: "file",
                accept: ".json",
                class: "hidden",
                onchange: move |_| {
                    // Only on web will this be useful, native path uses default file
                    confirm_copy.set(true);
                },
            }
        )
    };

    rsx! {
        // Centered responsive card
        div { class: "min-h-[70vh] flex items-center justify-center",
            div { class: "w-full max-w-lg mx-auto space-y-6",
                div { class: "flex items-center justify-between",
                    a {
                        href: "/",
                        class: "inline-flex items-center gap-2 h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                        span { "←" }
                        span { class: "hidden sm:inline", {t("nav.home")} }
                    }
                }
                div { class: "rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-5 sm:p-6 space-y-5",
                    div { class: "space-y-1",
                        h1 { class: "text-xl sm:text-2xl font-semibold", {t("config.title")} }
                        p { class: "text-sm text-slate-600 dark:text-slate-300",
                            {t("config.subtitle")}
                        }
                    }
                    div { class: "flex flex-col gap-2",
                        label { class: "text-sm font-medium text-slate-700 dark:text-slate-200",
                            {t("landpage.name")}
                        }
                        input {
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            value: name.read().clone(),
                            oninput: move |e| name.set(e.value()),
                        }
                    }
                    div { class: "flex flex-col gap-2",
                        label { class: "text-sm font-medium text-slate-700 dark:text-slate-200",
                            {t("landpage.theme")}
                        }
                        select {
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            value: theme.read().clone(),
                            oninput: move |e| theme.set(e.value()),
                            option { value: "System", {t("common.system")} }
                            option { value: "Light", {t("common.light")} }
                            option { value: "Dark", {t("common.dark")} }
                        }
                    }
                    div { class: "flex flex-col gap-2",
                        label { class: "text-sm font-medium text-slate-700 dark:text-slate-200",
                            {t("config.name_order")}
                        }
                        select {
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            value: name_order.read().clone(),
                            oninput: move |e| name_order.set(e.value()),
                            option { value: "first_last", {t("config.name_order_first_last")} }
                            option { value: "last_first", {t("config.name_order_last_first")} }
                        }
                    }
                    div { class: "flex flex-col gap-2",
                        label { class: "text-sm font-medium text-slate-700 dark:text-slate-200",
                            {t("config.language")}
                        }
                        select {
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            value: language.read().clone(),
                            oninput: move |e| language.set(e.value()),
                            option { value: "system", "System" }
                            option { value: "en", "English" }
                            option { value: "es", "Español" }
                            option { value: "fr", "Français" }
                            option { value: "de", "Deutsch" }
                        }
                    }
                    div { class: "flex flex-col gap-2",
                        label { class: "text-sm font-medium text-slate-700 dark:text-slate-200",
                            {t("config.date_format")}
                        }
                        select {
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            value: date_format.read().clone(),
                            oninput: move |e| date_format.set(e.value()),
                            option { value: "YYYY-MM-DD", "YYYY-MM-DD (2025-06-01)" }
                            option { value: "DD/MM/YYYY", "DD/MM/YYYY (01/06/2025)" }
                            option { value: "MM/DD/YYYY", "MM/DD/YYYY (06/01/2025)" }
                            option { value: "DD MMM YYYY", "DD MMM YYYY (01 Jun 2025)" }
                        }
                    }
                    div { class: "flex flex-col gap-2",
                        label { class: "text-sm font-medium text-slate-700 dark:text-slate-200",
                            {t("config.week_start")}
                        }
                        select {
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            value: week_start.read().clone(),
                            oninput: move |e| week_start.set(e.value()),
                            option { value: "monday", {t("common.monday")} }
                            option { value: "sunday", {t("common.sunday")} }
                        }
                    }
                    div { class: "flex flex-col items-stretch gap-2",
                        button {
                            class: "inline-flex justify-center items-center gap-2 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium px-4 py-2 transition disabled:opacity-50 disabled:cursor-not-allowed w-full",
                            disabled: name.read().trim().is_empty(),
                            onclick: on_save,
                            {t("config.save")}
                        }
                        {saved().then(|| rsx! {
                            span { class: "text-sm text-green-600 text-center", {t("config.saved")} }
                        })}
                    }
                    div { class: "pt-2 border-t border-slate-200 dark:border-slate-700 mt-2 space-y-3",
                        h2 { class: "text-sm font-semibold text-slate-700 dark:text-slate-200",
                            {t("config.data")}
                        }
                        div { class: "flex flex-col sm:flex-row gap-3 items-stretch justify-center w-full",
                            // Export button
                            button {
                                class: "inline-flex items-center justify-center gap-2 rounded-md bg-emerald-600 hover:bg-emerald-500 text-white text-sm font-medium px-4 py-2 transition w-full sm:w-44 h-10",
                                onclick: on_export,
                                span { "⬇️" }
                                span { class: "sm:inline", {t("config.export")} }
                            }
                            // Import button + file input (web)
                            div { class: "flex flex-col gap-2",
                                {import_box}
                                button {
                                    class: "inline-flex items-center justify-center gap-2 rounded-md bg-red-600 hover:bg-red-500 text-white text-sm font-medium px-4 py-2 transition w-full sm:w-44 h-10",
                                    onclick: on_import_click,
                                    span { "⬆️" }
                                    span { class: "sm:inline", {t("config.import")} }
                                }
                            }
                            // Delete all data button
                            button {
                                class: "inline-flex items-center justify-center gap-2 rounded-md bg-slate-700 hover:bg-slate-600 text-white text-sm font-medium px-4 py-2 transition w-full sm:w-44 h-10",
                                onclick: move |_| confirm_reset.set(true),
                                span { "🗑️" }
                                span { class: "sm:inline", {t("config.delete_all")} }
                            }
                            {import_error.read().as_ref().map(|e| rsx! {
                                p { class: "text-sm text-red-600 text-center w-full", {e.clone()} }
                            })}
                        }
                    }
                }
            }
        }

        {confirm_import().then(|| rsx! {
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                    h2 { class: "text-lg font-semibold", {t("config.confirm_import_title")} }
                    p { class: "text-sm text-slate-600 dark:text-slate-300",
                        {t("config.confirm_import_message")}
                    }
                    div { class: "flex items-center justify-end gap-2",
                        button {
                            class: "inline-flex items-center h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                            onclick: move |_| confirm_import.set(false),
                            {t("common.cancel")}
                        }
                        button {
                            class: "inline-flex items-center h-9 px-3 rounded-md bg-red-600 hover:bg-red-500 text-white text-sm font-medium transition",
                            onclick: do_import,
                            {t("config.import")}
                        }
                    }
                }
            }
        })}

        // mark as not configured and send user to home so Landpage shows
        {confirm_reset().then(|| rsx! {
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                    h2 { class: "text-lg font-semibold", {t("config.confirm_delete_all_title")} }
                    p { class: "text-sm text-slate-600 dark:text-slate-300",
                        {t("config.confirm_delete_all_message")}
                    }
                    div { class: "flex items-center justify-end gap-2",
                        button {
                            class: "inline-flex items-center h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                            onclick: move |_| confirm_reset.set(false),
                            {t("common.cancel")}
                        }
                        button {
                            class: "inline-flex items-center h-9 px-3 rounded-md bg-red-600 hover:bg-red-500 text-white text-sm font-medium transition",
                            onclick: move |_| {
                                if backend::reset_data() {
                                    configured.set(false);
                                    confirm_reset.set(false);
                                    #[cfg(target_arch = "wasm32")]
                                    {
                                        if let Some(win) = web_sys::window() {
                                            let _ = win.location().set_href("/");
                                        }
                                    }
                                }
                            },
                            {t("config.delete_all")}
                        }
                    }
                }
            }
        })}
    }
}
