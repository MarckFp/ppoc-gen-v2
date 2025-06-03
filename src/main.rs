#![cfg_attr(feature = "bundle", windows_subsystem = "windows")]

use dioxus::prelude::*;
// Components
use views::{Blog, Home};

mod components;
mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    Home {},
    #[route("/blog/:id")]
    Blog { id: i32 },
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        head {
            meta { name: "description", content: "Dioxus template project" }
            meta { content: "text/html;charset=utf-8", http_equiv: "Content-Type" }
            meta { name: "viewport", content: "width=device-width, initial-scale=1" }
            meta { charset: "UTF-8" }
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
            document::Stylesheet { href: asset!("/assets/styling/main.css") }
            document::Stylesheet { href: asset!("/assets/tailwind.css") }
        }
        Router::<Route> {}
    }
}
