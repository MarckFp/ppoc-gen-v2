use dioxus::prelude::*;
#[allow(unused_imports)]
use crate::i18n::format_date_ymd;
use crate::i18n::t;
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))] use crate::db::dao;
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))] use crate::db::dao::Absence as NativeAbsence;
#[cfg(target_arch = "wasm32")] use crate::db::wasm_store as wasm_backend;
#[cfg(target_arch = "wasm32")] use wasm_backend::Absence as WebAbsence;
#[cfg(target_arch = "wasm32")] use wasm_backend::Publisher as WebPublisher;

const PAGE_SIZE: usize = 25;

#[derive(Clone, PartialEq)]
enum ConfirmAction { DeleteOne(i64), DeleteMany(Vec<i64>) }

#[derive(PartialEq, Clone)]
struct AbsenceForm { id: Option<i64>, publisher_id: String, start_date: String, end_date: String, description: String }

#[derive(Clone)]
struct AbsenceItem { id: i64, title: String, subtitle: String, _publisher_id: i64 }

#[component]
#[allow(unused_mut)]
pub fn Absences() -> Element {
    let mut list = use_signal(|| Vec::<AbsenceItem>::new());
    let mut raw = use_signal(|| Vec::<AbsenceItem>::new());
    let mut query = use_signal(|| String::new());
    let mut modal_open = use_signal(|| false);
    let mut form = use_signal(|| AbsenceForm { id: None, publisher_id: String::new(), start_date: String::new(), end_date: String::new(), description: String::new() });
    let mut error = use_signal(|| Option::<String>::None);
    let mut current_page = use_signal(|| 0usize);
    let mut selected = use_signal(|| Vec::<i64>::new());
    let mut confirm_action = use_signal(|| Option::<ConfirmAction>::None);
    let mut select_mode = use_signal(|| false);
    // publishers for selector
    let mut publishers = use_signal(|| Vec::<(i64, String)>::new());

    use_effect(move || {
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            // load configuration for name order
            let order = dao::get_configuration().ok().map(|c| c.name_order).unwrap_or_else(|| "first_last".into());
            if let Ok(items) = dao::list_publishers() {
                let mapped = items.into_iter().map(|p| if order == "last_first" { (p.id, format!("{} {}", p.last_name, p.first_name)) } else { (p.id, format!("{} {}", p.first_name, p.last_name)) }).collect();
                publishers.set(mapped);
            }
            let today_effect = chrono::Local::now().date_naive();
            let _ = dao::cleanup_expired_absences(today_effect);
            if let Ok(items) = dao::list_future_absences(chrono::Local::now().date_naive()) {
                let name_lookup = publishers.read().clone();
                let mapped = items.into_iter().map(|a: NativeAbsence| {
                    let name = name_lookup.iter().find(|(id, _)| *id == a.publisher_id).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("#{:?}", a.publisher_id));
                    AbsenceItem { id: a.id, _publisher_id: a.publisher_id, title: format!("{} • {} → {}", name, format_date_ymd(&a.start_date.to_string()), format_date_ymd(&a.end_date.to_string())), subtitle: a.description.clone().unwrap_or_default() }
                }).collect::<Vec<_>>();
                raw.set(mapped.clone()); list.set(mapped);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let mut ps = wasm_backend::list_publishers();
            // build labels based on configuration
            let order = wasm_backend::get_name_order();
            let mapped = ps.drain(..).map(|p: WebPublisher| if order == "last_first" { (p.id, format!("{} {}", p.last_name, p.first_name)) } else { (p.id, format!("{} {}", p.first_name, p.last_name)) }).collect::<Vec<_>>();
            publishers.set(mapped);
            let now = js_sys::Date::new_0();
            let today_effect = format!("{:04}-{:02}-{:02}", now.get_full_year() as i32, now.get_month() as u32 + 1, now.get_date() as u32);
            wasm_backend::cleanup_expired_absences(&today_effect);
            let name_lookup = publishers.read().clone();
            let mapped = wasm_backend::list_future_absences(&today_effect).into_iter().map(|a: WebAbsence| {
                let name = name_lookup.iter().find(|(id, _)| *id == a.publisher_id).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("#{:?}", a.publisher_id));
                AbsenceItem { id: a.id, _publisher_id: a.publisher_id, title: format!("{} • {} → {}", name, format_date_ymd(&a.start_date), format_date_ymd(&a.end_date)), subtitle: a.description.unwrap_or_default() }
            }).collect::<Vec<_>>();
            raw.set(mapped.clone()); list.set(mapped);
        }
    });

    let mut apply_filter = {
        let mut list = list.clone();
        let raw = raw.clone();
        let query = query.clone();
        move || {
            let q = query.read().to_lowercase();
            if q.is_empty() { list.set(raw.read().clone()); return; }
            let items = raw.read().iter().cloned().filter(|i| i.title.to_lowercase().contains(&q) || i.subtitle.to_lowercase().contains(&q)).collect::<Vec<_>>();
            list.set(items);
            current_page.set(0);
        }
    };

    let open_create = move |_| {
        error.set(None);
        form.set(AbsenceForm { id: None, publisher_id: String::new(), start_date: String::new(), end_date: String::new(), description: String::new() });
        modal_open.set(true);
    };

    let on_submit = move |_| {
    error.set(None);
    let f = form.read().clone();
    if f.publisher_id.trim().is_empty() || f.start_date.trim().is_empty() || f.end_date.trim().is_empty() { error.set(Some(t("absences.error_required"))); return; }
        let pid = f.publisher_id.parse::<i64>().unwrap_or(0);
    if pid <= 0 { error.set(Some(t("absences.error_invalid_publisher"))); return; }
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            if let Some(id) = f.id { let _ = dao::update_absence(id, pid, chrono::NaiveDate::parse_from_str(&f.start_date, "%Y-%m-%d").unwrap(), chrono::NaiveDate::parse_from_str(&f.end_date, "%Y-%m-%d").unwrap(), if f.description.trim().is_empty() { None } else { Some(&*f.description) }); }
            else { let _ = dao::create_absence(pid, chrono::NaiveDate::parse_from_str(&f.start_date, "%Y-%m-%d").unwrap(), chrono::NaiveDate::parse_from_str(&f.end_date, "%Y-%m-%d").unwrap(), if f.description.trim().is_empty() { None } else { Some(&*f.description) }); }
            let today_submit = chrono::Local::now().date_naive();
            if let Ok(items) = dao::list_future_absences(today_submit) {
                let name_lookup = publishers.read().clone();
                let mapped = items.into_iter().map(|a: NativeAbsence| {
                    let name = name_lookup.iter().find(|(id, _)| *id == a.publisher_id).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("#{:?}", a.publisher_id));
                    AbsenceItem { id: a.id, _publisher_id: a.publisher_id, title: format!("{} • {} → {}", name, format_date_ymd(&a.start_date.to_string()), format_date_ymd(&a.end_date.to_string())), subtitle: a.description.clone().unwrap_or_default() }
                }).collect::<Vec<_>>(); raw.set(mapped.clone()); list.set(mapped);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let now = js_sys::Date::new_0();
            let today_submit = format!("{:04}-{:02}-{:02}", now.get_full_year() as i32, now.get_month() as u32 + 1, now.get_date() as u32);
            if let Some(id) = f.id { wasm_backend::update_absence(id, pid, &f.start_date, &f.end_date, if f.description.trim().is_empty() { None } else { Some(&*f.description) }); }
            else { let _ = wasm_backend::create_absence(pid, &f.start_date, &f.end_date, if f.description.trim().is_empty() { None } else { Some(&*f.description) }); }
            let name_lookup = publishers.read().clone();
            let mapped = wasm_backend::list_future_absences(&today_submit).into_iter().map(|a: WebAbsence| {
                let name = name_lookup.iter().find(|(id, _)| *id == a.publisher_id).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("#{:?}", a.publisher_id));
                AbsenceItem { id: a.id, _publisher_id: a.publisher_id, title: format!("{} • {} → {}", name, format_date_ymd(&a.start_date), format_date_ymd(&a.end_date)), subtitle: a.description.unwrap_or_default() }
            }).collect::<Vec<_>>(); raw.set(mapped.clone()); list.set(mapped);
        }
        modal_open.set(false);
    };

    let mut delete_absence = move |_id: i64| {
    #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
    { let _ = dao::delete_absence(_id); let today_del = chrono::Local::now().date_naive(); if let Ok(items) = dao::list_future_absences(today_del) { let name_lookup = publishers.read().clone(); let mapped = items.into_iter().map(|a: NativeAbsence| { let name = name_lookup.iter().find(|(id, _)| *id == a.publisher_id).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("#{:?}", a.publisher_id)); AbsenceItem { id: a.id, _publisher_id: a.publisher_id, title: format!("{} • {} → {}", name, format_date_ymd(&a.start_date.to_string()), format_date_ymd(&a.end_date.to_string())), subtitle: a.description.clone().unwrap_or_default() } }).collect::<Vec<_>>(); raw.set(mapped.clone()); list.set(mapped); } }
        #[cfg(target_arch = "wasm32")]
    { wasm_backend::delete_absence(_id); let now = js_sys::Date::new_0(); let today_del = format!("{:04}-{:02}-{:02}", now.get_full_year() as i32, now.get_month() as u32 + 1, now.get_date() as u32); let name_lookup = publishers.read().clone(); let mapped = wasm_backend::list_future_absences(&today_del).into_iter().map(|a: WebAbsence| { let name = name_lookup.iter().find(|(id, _)| *id == a.publisher_id).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("#{:?}", a.publisher_id)); AbsenceItem { id: a.id, _publisher_id: a.publisher_id, title: format!("{} • {} → {}", name, format_date_ymd(&a.start_date), format_date_ymd(&a.end_date)), subtitle: a.description.unwrap_or_default() } }).collect::<Vec<_>>(); raw.set(mapped.clone()); list.set(mapped); }
    };
    rsx! {
        div { class: "min-h-[70vh] flex items-start justify-center",
            div { class: "w-full max-w-2xl mx-auto space-y-5",
                div { class: "flex items-center justify-between",
                    a {
                        href: "/",
                        class: "inline-flex items-center gap-2 h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                        span { "←" }
                        span { class: "hidden sm:inline", {t("nav.home")} }
                    }
                    button {
                        class: "inline-flex items-center gap-2 h-9 px-3 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition",
                        onclick: open_create,
                        span { "➕" }
                        span { class: "hidden sm:inline", {t("common.new")} }
                    }
                }
                div { class: "rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-4 sm:p-5 space-y-4",
                    div { class: "flex flex-col sm:flex-row gap-2 sm:items-center sm:justify-between",
                        h1 { class: "text-xl sm:text-2xl font-semibold", {t("nav.absences")} }
                        input {
                            class: "h-10 w-full sm:w-64 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            placeholder: t("common.search_placeholder"),
                            value: query.read().clone(),
                            oninput: move |e| {
                                query.set(e.value());
                                apply_filter();
                                current_page.set(0);
                            },
                        }
                    }
                    {
                        // pagination + selection bar
                        let all_items = list.read().clone();
                        let total = all_items.len();
                        let pages = if total == 0 { 1 } else { ((total - 1) / PAGE_SIZE) + 1 };
                        let page = current_page.read().clone().min(pages - 1);
                        let start = page * PAGE_SIZE;
                        let end = core::cmp::min(start + PAGE_SIZE, total);
                        let page_items = all_items[start..end].to_vec();
                        let page_ids: Vec<i64> = page_items.iter().map(|p| p.id).collect();
                        let sel = selected.read().clone();
                        let all_selected_on_page = !page_ids.is_empty()

                            // controls row
                            // left: selection toggle + (conditional) select-all and bulk delete
                            // right: pagination controls
                            // list
                            && page_ids.iter().all(|id| sel.contains(id));
                        rsx! {
                            div { class: "flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between",
                                div { class: "flex items-center gap-3",
                                    button {
                                        class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600",
                                        onclick: move |_| select_mode.set(!select_mode()),
                                        {if select_mode() { t("common.done") } else { t("common.select") }}
                                    }
                                    {select_mode().then(|| rsx! {
                                        div { class: "flex items-center gap-3",
                                            label { class: "inline-flex items-center gap-2 text-sm text-slate-600 dark:text-slate-300",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: all_selected_on_page,
                                                    onchange: move |e| {
                                                        let check = e.value().parse::<bool>().unwrap_or(false);
                                                        let mut v = selected.read().clone();
                                                        if check {
                                                            for id in page_ids.clone() {
                                                                if !v.contains(&id) {
                                                                    v.push(id);
                                                                }
                                                            }
                                                        } else {
                                                            v.retain(|id| !page_ids.contains(id));
                                                        }
                                                        selected.set(v);
                                                    },
                                                }
                                                span { {t("common.select_all_page")} }
                                            }
                                            {(selected.read().len() > 0).then(|| rsx! {
                                                button {
                                                    class: "inline-flex items-center gap-2 h-9 px-3 rounded-md bg-red-600 hover:bg-red-500 text-white text-sm font-medium transition",
                                                    onclick: move |_| {
                                                        confirm_action.set(Some(ConfirmAction::DeleteMany(selected.read().clone())))
                                                    },
                                                    span { "🗑️" }
                                                    span { class: "hidden sm:inline",
                                                        {format!("{} ({})", t("common.delete_selected"), selected.read().len())}
                                                    }
                                                }
                                            })}
                                        }
                                    })}
                                }
                                div { class: "flex items-center gap-2 text-sm text-slate-600 dark:text-slate-300",
                                    span {
                                        {
                                            format!(
                                                "{}–{} {} {}",
                                                if total == 0 { 0 } else { start + 1 },
                                                end,
                                                t("common.of"),
                                                total,
                                            )
                                        }
                                    }
                                    div { class: "flex items-center gap-1",
                                        button {
                                            class: "h-8 px-2 rounded-md border border-slate-300 dark:border-slate-600 disabled:opacity-50",
                                            disabled: page == 0,
                                            onclick: move |_| {
                                                if page > 0 {
                                                    current_page.set(page - 1);
                                                }
                                            },
                                            {t("common.prev")}
                                        }
                                        button {
                                            class: "h-8 px-2 rounded-md border border-slate-300 dark:border-slate-600 disabled:opacity-50",
                                            disabled: page + 1 >= pages,
                                            onclick: move |_| {
                                                if page + 1 < pages {
                                                    current_page.set(page + 1);
                                                }
                                            },
                                            {t("common.next")}
                                        }
                                    }
                                }
                            }
                            {
                                if page_items.is_empty() {
                                    rsx! {
                                        div { class: "text-sm text-slate-600 dark:text-slate-300", {t("absences.empty")} }
                                    }
                                } else {
                                    rsx! {
                                        ul { class: "divide-y divide-slate-200 dark:divide-slate-700",
                                            for p in page_items.into_iter() {
                                                li { class: "py-3 flex items-center justify-between gap-3",
                                                    div { class: "flex items-center gap-3 min-w-0 w-full",
                                                        {select_mode().then(|| rsx! {
                                                            input {
                                                                r#type: "checkbox",
                                                                checked: selected.read().contains(&p.id),
                                                                onchange: move |_| {
                                                                    let mut v = selected.read().clone();
                                                                    if let Some(pos) = v.iter().position(|x| *x == p.id) {
                                                                        v.remove(pos);
                                                                    } else {
                                                                        v.push(p.id);
                                                                    }
                                                                    selected.set(v);
                                                                },
                                                            }
                                                        })}
                                                        div {
                                                            class: "min-w-0 flex-1 cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-700/30 rounded-md px-3 -mx-3 py-2",
                                                            onclick: move |_| {
                                                                error.set(None);
                                                                #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
                                                                {
                                                                    let t = chrono::Local::now().date_naive();
                                                                    if let Ok(items) = dao::list_future_absences(t) {
                                                                        if let Some(a) = items.into_iter().find(|x| x.id == p.id) {
                                                                            form.set(AbsenceForm {
                                                                                id: Some(a.id),
                                                                                publisher_id: a.publisher_id.to_string(),
                                                                                start_date: a.start_date.to_string(),
                                                                                end_date: a.end_date.to_string(),
                                                                                description: a.description.unwrap_or_default(),
                                                                            });
                                                                            modal_open.set(true);
                                                                        }
                                                                    }
                                                                }
                                                                #[cfg(target_arch = "wasm32")]
                                                                {
                                                                    let now = js_sys::Date::new_0();
                                                                    let t = format!(
                                                                        "{:04}-{:02}-{:02}",
                                                                        now.get_full_year() as i32,
                                                                        now.get_month() as u32 + 1,
                                                                        now.get_date() as u32,
                                                                    );
                                                                    if let Some(a) = wasm_backend::list_future_absences(&t)
                                                                        .into_iter()
                                                                        .find(|x| x.id == p.id)
                                                                    {
                                                                        form.set(AbsenceForm {
                                                                            id: Some(a.id),
                                                                            publisher_id: a.publisher_id.to_string(),
                                                                            start_date: a.start_date.to_string(),
                                                                            end_date: a.end_date.to_string(),
                                                                            description: a.description.unwrap_or_default(),
                                                                        });
                                                                        modal_open.set(true);
                                                                    }
                                                                }
                                                            },
                                                            div { class: "font-medium text-slate-800 dark:text-slate-100", {p.title.clone()} }
                                                            div { class: "text-xs text-slate-500", {p.subtitle.clone()} }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        {modal_open().then(|| rsx! {
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                    h2 { class: "text-lg font-semibold",
                        {
                            if form.read().id.is_some() {
                                t("absences.edit_title")
                            } else {
                                t("absences.new_title")
                            }
                        }
                    }
                    {error.read().as_ref().map(|err| rsx! {
                        p { class: "text-red-600 text-sm", {err.clone()} }
                    })}
                    div { class: "grid grid-cols-1 gap-3",
                        select {
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            value: form.read().publisher_id.clone(),
                            onchange: move |e| form.write().publisher_id = e.value(),
                            option { value: "", selected: form.read().publisher_id.is_empty(),
                                {t("absences.select_publisher")}
                            }
                            for (id , name) in publishers.read().iter().cloned() {
                                option {
                                    value: "{id}",
                                    selected: form.read().publisher_id == id.to_string(),
                                    "{name}"
                                }
                            }
                        }
                        input {
                            r#type: "date",
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            placeholder: {t("absences.start_date")},
                            value: form.read().start_date.clone(),
                            oninput: move |e| form.write().start_date = e.value(),
                        }
                        input {
                            r#type: "date",
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            placeholder: {t("absences.end_date")},
                            value: form.read().end_date.clone(),
                            oninput: move |e| form.write().end_date = e.value(),
                        }
                        textarea {
                            class: "rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 w-full h-20",
                            placeholder: {t("absences.description_optional")},
                            value: form.read().description.clone(),
                            oninput: move |e| form.write().description = e.value(),
                        }
                    }
                    div { class: "flex items-center justify-between gap-2",
                        {form.read().id.map(|eid| rsx! {
                            button {
                                class: "inline-flex items-center h-9 px-3 rounded-md border border-red-300 text-red-700 text-sm font-medium transition",
                                onclick: move |_| {
                                    confirm_action.set(Some(ConfirmAction::DeleteOne(eid)));
                                },
                                {t("common.delete")}
                            }
                        })}
                        div { class: "flex items-center gap-2",
                            button {
                                class: "inline-flex items-center h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                                onclick: move |_| modal_open.set(false),
                                {t("common.cancel")}
                            }
                            button {
                                class: "inline-flex items-center h-9 px-3 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition",
                                onclick: on_submit,
                                {if form.read().id.is_some() { t("common.save") } else { t("common.create") }}
                            }
                        }
                    }
                }
            }
        })}

        {
            confirm_action
                .read()
                .as_ref()
                .map(|action| {
                    let message = match action {
                        ConfirmAction::DeleteOne(_id) => t("absences.confirm_delete_one"),
                        ConfirmAction::DeleteMany(ids) => {
                            format!("{} ({})", t("absences.confirm_delete_many"), ids.len())
                        }
                    };
                    rsx! {
                        div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                            div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                                h2 { class: "text-lg font-semibold", {t("common.confirm_delete_title")} }
                                p { class: "text-sm text-slate-600 dark:text-slate-300", {message} }
                                div { class: "flex items-center justify-end gap-2",
                                    button {
                                        class: "inline-flex items-center h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                                        onclick: move |_| confirm_action.set(None),
                                        {t("common.cancel")}
                                    }
                                    button {
                                        class: "inline-flex items-center h-9 px-3 rounded-md bg-red-600 hover:bg-red-500 text-white text-sm font-medium transition",
                                        onclick: move |_| {
                                            let act = confirm_action.read().clone();
                                            match act {
                                                Some(ConfirmAction::DeleteOne(id)) => {
                                                    delete_absence(id);
                                                    let mut v = selected.read().clone();
                                                    v.retain(|x| *x != id);
                                                    selected.set(v);
                                                }
                                                Some(ConfirmAction::DeleteMany(ids)) => {
                                                    let _ = &ids;
                                                    #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
                                                    {
                                                        for id in ids.iter().copied() {
                                                            let _ = dao::delete_absence(id);
                                                        }
                                                        let t = chrono::Local::now().date_naive();
                                                        if let Ok(items) = dao::list_future_absences(t) {
                                                            let name_lookup = publishers.read().clone();
                                                            let mapped = items
                                                                .into_iter()
                                                                .map(|a: NativeAbsence| {
                                                                    let name = name_lookup
                                                                        .iter()
                                                                        .find(|(id, _)| *id == a.publisher_id)
                                                                        .map(|(_, n)| n.clone())
                                                                        .unwrap_or_else(|| format!("#{:?}", a.publisher_id));
                                                                    AbsenceItem {
                                                                        id: a.id,
                                                                        _publisher_id: a.publisher_id,
                                                                        title: format!(
                                                                            "{} • {} → {}",
                                                                            name,
                                                                            format_date_ymd(&a.start_date.to_string()),
                                                                            format_date_ymd(&a.end_date.to_string()),
                                                                        ),
                                                                        subtitle: a.description.clone().unwrap_or_default(),
                                                                    }
                                                                })
                                                                .collect::<Vec<_>>();
                                                            raw.set(mapped.clone());
                                                            list.set(mapped);
                                                        }
                                                    }
                                                    #[cfg(target_arch = "wasm32")]
                                                    {
                                                        for id in ids.iter().copied() {
                                                            wasm_backend::delete_absence(id);
                                                        }
                                                        let now = js_sys::Date::new_0();
                                                        let t = format!(
                                                            "{:04}-{:02}-{:02}",
                                                            now.get_full_year() as i32,
                                                            now.get_month() as u32 + 1,
                                                            now.get_date() as u32,
                                                        );
                                                        let name_lookup = publishers.read().clone();
                                                        let mapped = wasm_backend::list_future_absences(&t)
                                                            .into_iter()
                                                            .map(|a: WebAbsence| {
                                                                let name = name_lookup
                                                                    .iter()
                                                                    .find(|(id, _)| *id == a.publisher_id)
                                                                    .map(|(_, n)| n.clone())
                                                                    .unwrap_or_else(|| format!("#{:?}", a.publisher_id));
                                                                AbsenceItem {
                                                                    id: a.id,
                                                                    _publisher_id: a.publisher_id,
                                                                    title: format!(
                                                                        "{} • {} → {}",
                                                                        name,
                                                                        format_date_ymd(&a.start_date),
                                                                        format_date_ymd(&a.end_date),
                                                                    ),
                                                                    subtitle: a.description.unwrap_or_default(),
                                                                }
                                                            })
                                                            .collect::<Vec<_>>();
                                                        raw.set(mapped.clone());
                                                        list.set(mapped);
                                                    }
                                                    selected.set(vec![]);
                                                }
                                                None => {}
                                            }
                                            confirm_action.set(None);
                                        },
                                        span { "🗑️" }
                                        span { class: "hidden sm:inline", {t("common.delete")} }
                                    }
                                }
                            }
                        }
                    }
                })
        }
    }
}
