use crate::Route;
use dioxus::prelude::*;
use dioxus_free_icons::Icon;

//Icons
use dioxus_free_icons::icons::hi_solid_icons::HiHome;
use dioxus_free_icons::icons::hi_solid_icons::HiBookOpen;

#[component]
pub fn Bottombar() -> Element {
    let nav = navigator();
    let NAV_ELEMENTS = 2;

    let grid_class = format!("grid-cols-{}", NAV_ELEMENTS);
    rsx! {
        div { id: "navbar", class: "fixed bottom-0 left-0 z-50 w-full h-16 bg-white border-t border-gray-200 dark:bg-gray-700 dark:border-gray-600",
            div { class: "grid h-full max-w-lg {grid_class} mx-auto font-medium",
                button  { class: "inline-flex flex-col items-center justify-center px-5 hover:bg-gray-50 dark:hover:bg-gray-800 group",
                    onclick: move |_| {
                        nav.push(Route::Home{});
                    },
                    Icon {
                        width: 30,
                        height: 30,
                        fill: "black",
                        icon: HiHome,
                    },
                    span { class: "text-sm text-gray-500 dark:text-gray-400 group-hover:text-blue-600 dark:group-hover:text-blue-500",
                        "Home"
                    }
                }
                button  { class: "inline-flex flex-col items-center justify-center px-5 hover:bg-gray-50 dark:hover:bg-gray-800 group",
                    onclick: move |_| {
                        nav.push(Route::Blog { id: 1 });
                    },
                    Icon {
                        width: 30,
                        height: 30,
                        fill: "black",
                        icon: HiBookOpen,
                    },
                    span { class: "text-sm text-gray-500 dark:text-gray-400 group-hover:text-blue-600 dark:group-hover:text-blue-500",
                        "Blog"
                    }
                }
            }
        }

        Outlet::<Route> {}
    }
}
