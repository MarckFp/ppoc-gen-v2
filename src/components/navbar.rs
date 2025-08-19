use dioxus::prelude::*;

#[component]
pub fn Navbar() -> Element {
    rsx! {
        // Bottom floating bar: fixed, translucent, responsive
        nav { class: "fixed bottom-0 inset-x-0 z-50 border-t border-slate-200 dark:border-slate-700 bg-white/90 dark:bg-slate-900/90 backdrop-blur",
            div { class: "mx-auto w-full max-w-5xl px-3",
                div { class: "h-14 flex items-stretch justify-between gap-1 sm:gap-2",
                    a { href: "/", class: "flex-1 flex items-center justify-center text-sm font-medium text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-md transition",
                        span { class: "hidden sm:inline", "🏠 Home" }
                        span { class: "sm:hidden", "🏠" }
                    }
                    a { href: "/publishers", class: "flex-1 flex items-center justify-center text-sm font-medium text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-md transition",
                        span { class: "hidden sm:inline", "👥 Publishers" }
                        span { class: "sm:hidden", "👥" }
                    }
                    a { href: "/schedules", class: "flex-1 flex items-center justify-center text-sm font-medium text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-md transition",
                        span { class: "hidden sm:inline", "📅 Schedules" }
                        span { class: "sm:hidden", "📅" }
                    }
                    a { href: "/shifts", class: "flex-1 flex items-center justify-center text-sm font-medium text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-md transition",
                        span { class: "hidden sm:inline", "🗓️ Shifts" }
                        span { class: "sm:hidden", "🗓️" }
                    }
                    a { href: "/absences", class: "flex-1 flex items-center justify-center text-sm font-medium text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-md transition",
                        span { class: "hidden sm:inline", "🚫 Absences" }
                        span { class: "sm:hidden", "🚫" }
                    }
                    a { href: "/configuration", class: "flex-1 flex items-center justify-center text-sm font-medium text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-md transition",
                        span { class: "hidden sm:inline", "⚙️ Configuration" }
                        span { class: "sm:hidden", "⚙️" }
                    }
                }
            }
        }
    }
}
