use dioxus::prelude::*;
use web_sys::window;

//Components
use crate::components::Sidebar;
use crate::components::Bottombar;

fn is_mobile() -> bool {
    let user_agent_check = window()
        .and_then(|w| w.navigator().user_agent().ok())
        .map(|ua| ua.contains("Mobile") || ua.contains("Android") || ua.contains("iPhone"))
        .unwrap_or(false);

    let size_check = window()
        .map(|w| w.inner_width().unwrap().as_f64().unwrap_or(0.0) <= 768.0)
        .unwrap_or(false);

    user_agent_check || size_check
}

#[component]
pub fn Layout() -> Element {
    if is_mobile() {
        rsx! {
            Bottombar {}
        }
    } else {
        rsx! {
            Sidebar {}
        }
    }
}
