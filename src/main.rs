#![cfg_attr(feature = "bundle", windows_subsystem = "windows")]

use dioxus::prelude::*;
mod i18n;
// Components
use views::{Home, Publishers, Absences, Schedules, Shifts, Configuration};
// Static web: use wasm local storage backend for configuration detection
#[cfg(target_arch = "wasm32")] use crate::db::wasm_store as backend;
#[cfg(not(target_arch = "wasm32"))] mod backend { pub fn configuration_is_set() -> bool { true } }

mod components;
mod views;
mod db; // universal db facade (native sqlite or wasm storage)

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    Home {},
    #[route("/publishers")]
    Publishers {},
    #[route("/absences")]
    Absences {},
    #[route("/schedules")]
    Schedules {},
    #[route("/shifts")]
    Shifts {},
    #[route("/configuration")]
    Configuration {},
}

fn main() {
    #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
    install_panic_hook();
    dioxus::launch(App);
}
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        use std::fs::{OpenOptions, create_dir_all};
        use std::io::Write;
        let mut base = dirs_next::cache_dir().or_else(|| dirs_next::data_local_dir()).unwrap_or(std::env::temp_dir());
        base.push("dx_app");
        let _ = create_dir_all(&base);
    let file = OpenOptions::new().create(true).append(true).open(base.join("panic.log"));
    if let Ok(mut f) = file {
            let _ = writeln!(f, "{} | PANIC: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), info);
        }
    }));
}

#[component]
fn App() -> Element {
    // For static site (wasm), determine if initial configuration exists
    let configured = use_signal(|| backend::configuration_is_set());
    // Provide context so Landpage can flip it after user saves configuration
    provide_context(configured);
    // Provide i18n context (reads initial language/date from configuration if present)
    i18n::provide_i18n_from_config();

    // Apply theme based on saved configuration (web/native)
    use_effect(move || {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(cfg) = backend::get_configuration() { i18n::apply_theme(&cfg.theme); }
            else { i18n::apply_theme("System"); }
        }
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            if let Ok(cfg) = crate::db::dao::get_configuration() { i18n::apply_theme(&cfg.theme); }
            else { i18n::apply_theme("System"); }
        }
        #[cfg(all(not(target_arch = "wasm32"), not(feature = "native-db")))]
        {
            i18n::apply_theme("System");
        }
    });

    rsx! {
        document::Stylesheet { href: asset!("assets/tailwind.css") }
        head {
            document::Meta { name: "description", content: "Dioxus template project" }
            document::Link { rel: "icon", href: asset!("assets/icons/favicon.ico") }
            document::Link {
                rel: "icon",
                href: asset!("assets/icons/favicon-32x32.png"),
                sizes: "32x32",
            }
            document::Link {
                rel: "icon",
                href: asset!("assets/icons/favicon-16x16.png"),
                sizes: "16x16",
            }
            document::Link {
                rel: "apple-touch-icon",
                href: asset!("assets/icons/apple-touch-icon.png"),
                sizes: "180x180",
            }
        
        }
        div { class: "app-layout flex min-h-screen",
            main { class: "main-content flex-1 p-8 bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100",
                Router::<Route> {}
            }
        }
    }
}
