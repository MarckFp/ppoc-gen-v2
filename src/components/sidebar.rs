use crate::Route;
use dioxus::prelude::*;
use dioxus_free_icons::Icon;

//Icons
use dioxus_free_icons::icons::hi_solid_icons::HiHome;
use dioxus_free_icons::icons::hi_solid_icons::HiBookOpen;

#[component]
pub fn Sidebar() -> Element {
    let nav = navigator();

    rsx! {
        aside { class: "fixed top-0 left-0 z-40 w-64 h-screen transition-transform -translate-x-full sm:translate-x-0",
            div { class: "h-full px-3 py-4 overflow-y-auto bg-gray-50 dark:bg-gray-800",
                a { class: "flex items-center ps-2.5 mb-5",
                    span { class:"self-center text-xl font-semibold whitespace-nowrap dark:text-black",
                        "DxApp"
                    }
                },
                ul { class: "space-y-2 font-medium",
                    li {
                        a { class: "flex items-center p-2 text-gray-900 rounded-lg dark:text-white hover:bg-gray-100 dark:hover:bg-gray-700 group",
                            onclick: move |_| {
                                nav.push(Route::Home{});
                            },
                            Icon {
                                width: 30,
                                height: 30,
                                fill: "black",
                                icon: HiHome,
                            },
                            span { class: "ms-3",
                                "Home"
                            }
                        }
                    },
                    li {
                        a { class: "flex items-center p-2 text-gray-900 rounded-lg dark:text-white hover:bg-gray-100 dark:hover:bg-gray-700 group",
                            onclick: move |_| {
                                nav.push(Route::Blog { id: 1 });
                            },
                            Icon {
                                width: 30,
                                height: 30,
                                fill: "black",
                                icon: HiBookOpen,
                            },
                            span { class: "ms-3",
                                "Blog"
                            }
                        }
                    }
                }
            }
        },

        div { class: "p-4 sm:ml-64",
            Outlet::<Route> {}
        }
    }
}
