#![cfg_attr(feature = "bundle", windows_subsystem = "windows")]

use dioxus::prelude::*;

use components::Navbar;
use views::{Blog, Home};

mod components;
mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
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
        document::Link { rel: "icon", href: asset!("/assets/favicon.ico") }
        document::Stylesheet { href: asset!("/assets/styling/main.css") }
        document::Stylesheet  { href: asset!("/assets/tailwind.css") }
        document::Stylesheet  { href: "https://cdn.jsdelivr.net/npm/flowbite@2.5.2/dist/flowbite.min.css" }

        Router::<Route> {}
    }
}
