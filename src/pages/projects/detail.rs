use leptos::prelude::*;
use leptos_router::hooks::{use_params_map, use_query_map};

use crate::features::{
    auth::models::UserSession,
    projects::handlers::{get_project, get_project_file_text, list_project_files},
};

#[component]
pub fn ProjectDetailPage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());
    let params = use_params_map();
    let query = use_query_map();
    let project_id = move || params.with(|p| p.get("id").and_then(|id| id.parse::<i64>().ok()));
    #[allow(clippy::redundant_closure)]
    let project_id_signal = Signal::derive(move || project_id());

    let project_resource = LocalResource::new(move || {
        let id = project_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid project"))?;
            get_project(id).await
        }
    });

    let files_resource = LocalResource::new(move || {
        let id = project_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid project"))?;
            list_project_files(id).await
        }
    });

    let uploaded = move || query.with(|q| q.get("uploaded").is_some());
    let selected_file_id = RwSignal::new(None::<i64>);
    let pdf_modal_url = RwSignal::new(None::<String>);

    let text_resource = LocalResource::new(move || {
        let file_id = selected_file_id.get();
        async move {
            match file_id {
                Some(id) => get_project_file_text(id).await,
                None => Ok(String::new()),
            }
        }
    });

    let open_text_modal = move |file_id: i64| {
        pdf_modal_url.set(None);
        selected_file_id.set(Some(file_id));
    };

    let open_pdf_modal = move |project_id: i64, file_id: i64| {
        selected_file_id.set(None);
        pdf_modal_url.set(Some(format!(
            "/api/projects/{project_id}/files/{file_id}/pdf"
        )));
    };

    let close_modals = move || {
        selected_file_id.set(None);
        pdf_modal_url.set(None);
    };

    view! {
        <>
        <section class="mx-auto flex min-h-screen max-w-5xl flex-col gap-8 px-6 py-16">
            <Show
                when=move || user.get().is_some()
                fallback=move || view! {
                    <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-6 text-slate-300">
                        "Please log in to view this project."
                    </div>
                }
            >
                <Show
                    when=move || project_resource.get().is_some()
                    fallback=move || view! { <p class="text-sm text-slate-400">"Loading project..."</p> }
                >
                    {move || -> AnyView { match project_resource.get() {
                        Some(Ok(project)) => view! {
                            <div class="space-y-2">
                                <a class="text-sm text-slate-400 hover:text-white" href="/projects">"← Back to projects"</a>
                                <h1 class="text-4xl font-semibold text-white">{project.name}</h1>
                                {project.description.as_ref().map(|desc| view! {
                                    <p class="text-slate-300">{desc.clone()}</p>
                                })}
                                <div class="pt-2">
                                    <a
                                        class="inline-flex items-center rounded-full border border-slate-700 px-5 py-2 text-sm hover:border-slate-400"
                                        href=format!("/projects/{}/decks", project.id)
                                    >
                                        "View Flashcard Decks →"
                                    </a>
                                </div>
                            </div>

                            <div class="grid gap-6 lg:grid-cols-[1.1fr_0.9fr]">
                                <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                                    <div class="flex items-center justify-between">
                                        <h2 class="text-lg font-semibold text-white">"Lecture uploads"</h2>
                                        <span class="text-xs text-slate-500">{format!("Created {}", project.created_at)}</span>
                                    </div>
                                    <p class="mt-2 text-sm text-slate-400">
                                        "Upload PDF slides only."
                                    </p>
                                    {move || -> AnyView { if uploaded() {
                                        view! {
                                            <div class="mt-4 rounded-xl border border-emerald-500/40 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-200">
                                                "Upload complete. Text extraction finished."
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div class="hidden"></div> }.into_any()
                                    }}}
                                    <form
                                        class="mt-6 space-y-4"
                                        method="post"
                                        enctype="multipart/form-data"
                                        action=move || project_id_signal
                                            .get()
                                            .map(|id| format!("/api/projects/{id}/upload"))
                                            .unwrap_or_default()
                                    >
                                        <label class="flex flex-col gap-2 text-sm text-slate-300">
                                            "PDF file"
                                            <input
                                                class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100"
                                                type="file"
                                                name="file"
                                                accept="application/pdf,.pdf"
                                                required
                                            />
                                        </label>
                                        <div class="pt-1">
                                            <button class="inline-flex items-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950" type="submit">"Upload slides"</button>
                                        </div>
                                    </form>
                                </div>

                                <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                                    <h2 class="text-lg font-semibold text-white">"Text extraction"</h2>
                                    <p class="mt-2 text-sm text-slate-400">
                                        "We’ll summarize the upload here for future study sessions."
                                    </p>
                                    <Show
                                        when=move || files_resource.get().is_some()
                                        fallback=move || view! { <p class="mt-4 text-sm text-slate-400">"Loading files..."</p> }
                                    >
                                        {move || -> AnyView { match files_resource.get() {
                                            Some(Ok(files)) if files.is_empty() => view! {
                                                <> <p class="mt-4 text-sm text-slate-400">"No PDFs uploaded yet."</p> </>
                                            }.into_any(),
                                            Some(Ok(files)) => view! {
                                                <>
                                                    <ul class="mt-4 space-y-3">
                                                        {files.into_iter().map(|file| {
                                                            let preview = file.text_preview.clone().unwrap_or_default();
                                                            let has_preview = !preview.trim().is_empty();
                                                            view! {
                                                                <li
                                                                    class="rounded-xl border border-slate-800 bg-slate-900/40 p-4 transition hover:border-slate-700 hover:bg-slate-900/70"
                                                                    on:click=move |_| {
                                                                        open_text_modal(file.id);
                                                                    }
                                                                >
                                                                    <div class="flex items-start justify-between gap-3">
                                                                        <div>
                                                                            <p class="text-sm font-semibold text-white">{file.original_filename}</p>
                                                                            <p class="text-xs text-slate-500">{format!("{} • {}", format_bytes(file.file_size), file.created_at)}</p>
                                                                        </div>
                                                                        <button
                                                                            class="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-300 hover:border-slate-400"
                                                                            on:click=move |ev| {
                                                                                ev.stop_propagation();
                                                                                if let Some(project_id) = project_id_signal.get() {
                                                                                    open_pdf_modal(project_id, file.id);
                                                                                }
                                                                            }
                                                                        >
                                                                            "PDF"
                                                                        </button>
                                                                    </div>
                                                                    {move || -> AnyView { if has_preview {
                                                                        view! {
                                                                            <p class="mt-3 max-h-32 overflow-hidden text-xs text-slate-400 whitespace-pre-line">{preview.clone()}</p>
                                                                        }.into_any()
                                                                    } else {
                                                                        view! {
                                                                            <p class="mt-3 text-xs text-slate-500">"No extractable text found."</p>
                                                                        }.into_any()
                                                                    }}}
                                                                </li>
                                                            }
                                                        }).collect_view()}
                                                    </ul>
                                                </>
                                            }.into_any(),
                                            Some(Err(err)) => view! {
                                                <> <p class="mt-4 text-sm text-rose-300">{err.to_string()}</p> </>
                                            }.into_any(),
                                            None => view! { <> </> }.into_any(),
                                        }}}
                                    </Show>
                                </div>
                            </div>
                        }.into_any(),
                        Some(Err(err)) => view! { <> <p class="text-sm text-rose-300">{err.to_string()}</p> </> }.into_any(),
                        None => view! { <> </> }.into_any(),
                    }}}
                </Show>
            </Show>
        </section>

        <Show when=move || pdf_modal_url.get().is_some()>
            {move || match pdf_modal_url.get() {
                Some(url) => view! {
                    <div
                        class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
                        on:click=move |_| close_modals()
                    >
                        <div
                            class="w-full max-w-5xl rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl"
                            on:click=move |ev| ev.stop_propagation()
                        >
                            <div class="flex items-center justify-between border-b border-slate-800 px-6 py-4">
                                <p class="text-sm font-semibold text-white">"PDF Preview"</p>
                                <button
                                    class="rounded-full border border-slate-700 px-4 py-1 text-xs text-slate-300 hover:border-slate-400"
                                    on:click=move |_| close_modals()
                                >
                                    "Close"
                                </button>
                            </div>
                            <div class="h-[70vh] w-full bg-slate-900">
                                <iframe class="h-full w-full" src=url title="PDF preview"></iframe>
                            </div>
                        </div>
                    </div>
                }.into_any(),
                None => view! { <span></span> }.into_any(),
            }}
        </Show>

        <Show when=move || selected_file_id.get().is_some()>
            <div
                class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
                on:click=move |_| close_modals()
            >
                <div
                    class="w-full max-w-3xl rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl"
                    on:click=move |ev| ev.stop_propagation()
                >
                    <div class="flex items-center justify-between border-b border-slate-800 px-6 py-4">
                        <p class="text-sm font-semibold text-white">"Extracted text"</p>
                        <button
                            class="rounded-full border border-slate-700 px-4 py-1 text-xs text-slate-300 hover:border-slate-400"
                            on:click=move |_| close_modals()
                        >
                            "Close"
                        </button>
                    </div>
                    <div class="max-h-[70vh] overflow-auto px-6 py-4">
                        <Show
                            when=move || text_resource.get().is_some()
                            fallback=move || view! { <p class="text-sm text-slate-400">"Loading text..."</p> }
                        >
                            {move || match text_resource.get() {
                                Some(Ok(text)) if text.trim().is_empty() => view! {
                                    <p class="text-sm text-slate-400">"No extractable text found."</p>
                                }.into_any(),
                                Some(Ok(text)) => view! {
                                    <pre class="whitespace-pre-wrap text-sm text-slate-200">{text}</pre>
                                }.into_any(),
                                Some(Err(err)) => view! {
                                    <p class="text-sm text-rose-300">{err.to_string()}</p>
                                }.into_any(),
                                None => view! { <span></span> }.into_any(),
                            }}
                        </Show>
                    </div>
                </div>
            </div>
        </Show>
        </>
    }
}

fn format_bytes(bytes: i64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut index = 0usize;

    while size >= 1024.0 && index < units.len() - 1 {
        size /= 1024.0;
        index += 1;
    }

    if index == 0 {
        format!("{bytes} {}", units[index])
    } else {
        format!("{size:.1} {}", units[index])
    }
}
