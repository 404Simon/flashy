use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::features::{
    auth::models::UserSession,
    flashcards::markdown::markdown_to_html,
    projects::get_file_name,
    summaries::{delete_summary, get_summary, get_summary_markdown},
};

#[component]
pub fn SummaryViewerPage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());

    let params = use_params_map();
    let summary_id = move || {
        params.with(|p| {
            p.get("summary_id")
                .and_then(|id| id.parse::<i64>().ok())
        })
    };

    let summary_resource = LocalResource::new(move || {
        let id = summary_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid summary"))?;
            get_summary(id).await
        }
    });

    Effect::new(move |_| {
        if let Some(Ok(summary)) = summary_resource.get() {
            let is_generating = summary.status == "pending" || summary.status == "processing";
            if is_generating {
                set_timeout(
                    move || {
                        summary_resource.refetch();
                    },
                    std::time::Duration::from_secs(2),
                );
            }
        }
    });

    Effect::new(move |_| {
        let _ = summary_resource.get();

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::prelude::*;

            #[wasm_bindgen]
            extern "C" {
                #[wasm_bindgen(js_namespace = ["window", "MathJax"], js_name = typesetPromise)]
                fn mathjax_typeset() -> js_sys::Promise;
            }

            let _ = leptos::task::spawn_local(async {
                gloo_timers::future::TimeoutFuture::new(100).await;
                let _ = mathjax_typeset();
            });
        }
    });

    let download_markdown = move || {
        let sid = summary_id();
        leptos::task::spawn_local(async move {
            if let Some(id) = sid {
                if let Ok(md) = get_summary_markdown(id).await {
                    #[cfg(not(target_arch = "wasm32"))]
                    let _ = &md;

                    #[cfg(target_arch = "wasm32")]
                    {
                        use wasm_bindgen::JsCast;

                        let window = web_sys::window().unwrap();
                        let document = window.document().unwrap();
                        let blob = web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(
                            &wasm_bindgen::JsValue::from_str(&md),
                        ))
                        .unwrap();
                        let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                        let a = document.create_element("a").unwrap();
                        let a: web_sys::HtmlElement = a.dyn_into().unwrap();
                        a.set_attribute("href", &url).unwrap();
                        a.set_attribute("download", "summary.md").unwrap();
                        let _ = a.click();
                        web_sys::Url::revoke_object_url(&url).unwrap();
                    }
                }
            }
        });
    };

    let file_name_resource = LocalResource::new(move || {
        async move {
            if let Some(Ok(summary)) = summary_resource.get() {
                if let Some(file_id) = summary.file_id {
                    get_file_name(file_id).await.ok()
                } else {
                    None
                }
            } else {
                None
            }
        }
    });

    let content_html = Signal::derive(move || {
        summary_resource
            .get()
            .and_then(|r| r.ok())
            .map(|s| markdown_to_html(&s.content_markdown))
            .unwrap_or_default()
    });

    view! {
        <section class="mx-auto flex min-h-screen max-w-5xl flex-col gap-6 px-6 py-16">
            <Show
                when=move || user.get().is_some()
                fallback=move || view! {
                    <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-6 text-slate-300">
                        "Please log in to view this summary."
                    </div>
                }
            >
                <Show
                    when=move || summary_resource.get().is_some()
                    fallback=move || view! { <p class="text-sm text-slate-400">"Loading summary..."</p> }
                >
                    {move || match summary_resource.get() {
                        Some(Ok(summary)) => {
                            let project_id = summary.project_id;
                            let is_generating = summary.status == "pending" || summary.status == "processing";
                            let is_failed = summary.status == "failed";
                            let sid = summary.id;
                            let title = summary.title.clone();
                            let description = summary.description.clone();
                            let segment_label = summary.segment_label.clone();
                            let error_message = summary.error_message.clone();
                            let created_at = summary.created_at.clone();

                            view! {
                                <div class="space-y-2">
                                    <a
                                        class="text-sm text-slate-400 hover:text-white"
                                        href=format!("/projects/{}/summaries", project_id)
                                    >"← Back to summaries"</a>
                                    <h1 class="text-4xl font-semibold text-white">{title.clone()}</h1>
                                    {description.as_ref().map(|desc| view! {
                                        <p class="text-slate-300">{desc.clone()}</p>
                                    })}
                                </div>

                                <Show when=move || !is_generating && !is_failed>
                                    <div class="flex flex-wrap items-center gap-3">
                                        <button
                                            class="inline-flex items-center rounded-full border border-slate-700 px-5 py-2 text-sm font-semibold text-slate-300 hover:border-slate-400 hover:bg-slate-800"
                                            on:click=move |_| download_markdown()
                                        >
                                            "↓ Markdown"
                                        </button>
                                        <button
                                            class="inline-flex items-center rounded-full border border-rose-700 px-5 py-2 text-sm font-semibold text-rose-300 hover:border-rose-400 hover:bg-rose-950"
                                            on:click=move |_| {
                                                if window().confirm_with_message("Delete this summary?").unwrap_or(false) {
                                                    let pid = project_id;
                                                    leptos::task::spawn_local(async move {
                                                        if delete_summary(sid).await.is_ok() {
                                                            leptos_router::hooks::use_navigate()(
                                                                &format!("/projects/{}/summaries", pid),
                                                                Default::default(),
                                                            );
                                                        }
                                                    });
                                                }
                                            }
                                        >
                                            "Delete"
                                        </button>
                                    </div>
                                </Show>

                                <Show when=move || is_generating>
                                    <div class="rounded-2xl border border-blue-500/40 bg-blue-500/5 p-8 text-center space-y-3">
                                        <span class="inline-block h-8 w-8 animate-spin rounded-full border-4 border-blue-400 border-t-transparent"></span>
                                        <p class="text-blue-200 font-medium">"Generating summary..."</p>
                                        <p class="text-xs text-blue-300/70">"This may take a minute depending on the document size."</p>
                                    </div>
                                </Show>

                                <Show when=move || is_failed>
                                    <div class="rounded-2xl border border-rose-500/40 bg-rose-500/5 p-6 space-y-4">
                                        <p class="text-rose-200 font-medium">"Summary generation failed"</p>
                                        {error_message.as_ref().map(|err| view! {
                                            <p class="text-sm text-rose-300/70">{err.clone()}</p>
                                        })}
                                    </div>
                                </Show>

                                <Show when=move || summary.status == "completed">
                                    <article class="rounded-2xl border border-slate-800 bg-slate-900/50 p-8 md:p-12">
                                        <div
                                            class="mathjax-content prose prose-invert prose-slate max-w-none prose-headings:text-white prose-p:text-slate-200 prose-li:text-slate-200 prose-strong:text-white prose-code:text-emerald-300 prose-pre:bg-slate-950 prose-th:text-white prose-td:text-slate-200"
                                            inner_html=move || content_html.get()
                                        ></div>
                                    </article>

                                    <div class="flex flex-wrap gap-4 text-xs text-slate-500">
                                        {move || file_name_resource.get().flatten().map(|name| view! {
                                            <span>"Source: " {name}</span>
                                        })}
                                        {segment_label.as_ref().map(|label| view! {
                                            <span>"Segment: " {label.clone()}</span>
                                        })}
                                        <span>"Created: " {created_at.clone()}</span>
                                    </div>
                                </Show>
                            }.into_any()
                        }
                        Some(Err(err)) => view! {
                            <p class="text-sm text-rose-300">{err.to_string()}</p>
                        }.into_any(),
                        None => view! { <span></span> }.into_any(),
                    }}
                </Show>
            </Show>
        </section>
    }
}
