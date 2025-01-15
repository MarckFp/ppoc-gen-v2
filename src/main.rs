#![cfg_attr(feature = "bundle", windows_subsystem = "windows")]

use dioxus::prelude::*;
// Components
use components::Layout;
use views::{Blog, Home};
use std::env;

mod components;
mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Layout)]
    #[route("/")]
    Home {},
    #[route("/blog/:id")]
    Blog { id: i32 },
}

fn main() {
    let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());

    match env::var("SENTRY_DSN_URL") {
        Ok(val) => {
            let _guard = sentry::init((val, sentry::ClientOptions {
                environment: Some(app_env.into()),
                release: sentry::release_name!(),
                traces_sample_rate: 1.0,
                ..Default::default()
            }));
        },
        Err(e) => println!("Error SENTRY_DSN_URL: {}", e),
    }

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: asset!("/assets/icons/favicon.ico") }
        document::Link {
            rel: "icon",
            href: asset!("/assets/icons/favicon-32x32.png"),
            sizes: "32x32",
        }
        document::Link {
            rel: "icon",
            href: asset!("/assets/icons/favicon-16x16.png"),
            sizes: "16x16",
        }
        document::Link {
            rel: "apple-touch-icon",
            href: asset!("/assets/icons/apple-touch-icon.png"),
            sizes: "180x180",
        }
        document::Link { rel: "manifest", href: asset!("/assets/manifest.json") }
        document::Stylesheet { href: asset!("/assets/styling/main.css") }
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        document::Stylesheet { href: "https://cdn.jsdelivr.net/npm/flowbite@2.5.2/dist/flowbite.min.css" }

        Router::<Route> {}
    }
}
