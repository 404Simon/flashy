use leptos::html::Form;
use leptos::prelude::*;
use leptos_router::hooks::{use_params_map, use_query_map};

use crate::config_handlers::get_app_config;
use crate::features::{
    auth::models::UserSession,
    projects::handlers::{
        delete_project_file, get_project, get_project_file_text, list_project_files,
    },
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

    let config_resource = LocalResource::new(move || async move { get_app_config().await.ok() });

    let uploaded = move || {
        query.with(|q| {
            q.get("uploaded")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0)
        })
    };

    let selected_file_id = RwSignal::new(None::<i64>);
    let pdf_modal_url = RwSignal::new(None::<String>);
    let upload_form_ref = NodeRef::<Form>::new();
    let delete_action = Action::new(|file_id: &i64| {
        let file_id = *file_id;
        async move { delete_project_file(file_id).await }
    });

    let text_resource = LocalResource::new(move || {
        let file_id = selected_file_id.get();
        async move {
            match file_id {
                Some(id) => get_project_file_text(id).await,
                None => Ok(String::new()),
            }
        }
    });

    // Refetch files when delete completes
    Effect::new(move |_| {
        if delete_action.value().get().is_some() {
            files_resource.refetch();
        }
    });

    // Auto-refresh files list to show processing status
    Effect::new(move |_| {
        if let Some(Ok(files)) = files_resource.get() {
            // Check if any files are still processing
            let has_processing = files
                .iter()
                .any(|f| f.processing_status == "pending" || f.processing_status == "processing");
            if has_processing {
                set_timeout(
                    move || {
                        files_resource.refetch();
                    },
                    std::time::Duration::from_secs(2),
                );
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

                            <div class="mx-auto max-w-4xl">
                                <div class="mb-6 flex items-center justify-between">
                                    <h2 class="text-lg font-semibold text-white">"Uploaded files"</h2>
                                    <form
                                        node_ref=upload_form_ref
                                        class="flex items-center gap-2"
                                        method="post"
                                        enctype="multipart/form-data"
                                        action=move || project_id_signal
                                            .get()
                                            .map(|id| format!("/api/projects/{id}/upload"))
                                            .unwrap_or_default()
                                    >
                                        <label class="cursor-pointer">
                                            <input
                                                class="hidden"
                                                type="file"
                                                name="file"
                                                accept="application/pdf,.pdf"
                                                multiple
                                                required
                                                on:change=move |_| {
                                                    if let Some(form) = upload_form_ref.get() {
                                                        let _ = form.submit();
                                                    }
                                                }
                                            />
                                            <span class="inline-flex items-center rounded-full border border-slate-700 bg-slate-900/50 px-4 py-2 text-sm text-slate-300 hover:border-slate-400 hover:bg-slate-900">
                                                "+ Upload PDF"
                                            </span>
                                        </label>
                                    </form>
                                </div>

                                {move || -> AnyView {
                                    let count = uploaded();
                                    if count > 0 {
                                        view! {
                                            <div class="mb-4 rounded-xl border border-emerald-500/40 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-200">
                                                {format!("{} file{} uploaded. Processing in background...", count, if count > 1 { "s" } else { "" })}
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div class="hidden"></div> }.into_any()
                                    }
                                }}

                                <Show
                                    when=move || files_resource.get().is_some()
                                    fallback=move || view! { <p class="text-sm text-slate-400">"Loading files..."</p> }
                                >
                                    {move || -> AnyView { match files_resource.get() {
                                        Some(Ok(files)) if files.is_empty() => view! {
                                            <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-12 text-center">
                                                <p class="text-sm text-slate-400">"No PDFs uploaded yet."</p>
                                                <p class="mt-2 text-xs text-slate-500">"Click the '+ Upload PDF' button to get started."</p>
                                            </div>
                                        }.into_any(),
                                        Some(Ok(files)) => view! {
                                            <ul class="space-y-3">
                                                {files.into_iter().map(|file| {
                                                    let preview = file.text_preview.clone().unwrap_or_default();
                                                    let has_preview = !preview.trim().is_empty();
                                                    let status = file.processing_status.clone();
                                                    let is_processing = status == "pending" || status == "processing";
                                                    let is_failed = status == "failed";
                                                    let word_count = file.word_count;

                                                    view! {
                                                        <li
                                                            class="rounded-xl border border-slate-800 bg-slate-900/40 p-5 transition hover:border-slate-700 hover:bg-slate-900/70"
                                                            class:opacity-60=move || is_processing
                                                        >
                                                            <div class="flex items-start justify-between gap-3">
                                                                <div class="flex-1 cursor-pointer" on:click=move |_| {
                                                                    if !is_processing && !is_failed {
                                                                        open_text_modal(file.id);
                                                                    }
                                                                }>
                                                                    <p class="text-sm font-semibold text-white">{file.original_filename.clone()}</p>
                                                                    <p class="mt-1 text-xs text-slate-500">
                                                                        {format!("{} • {}", format_bytes(file.file_size), file.created_at.clone())}
                                                                    </p>
                                                                    {move || -> AnyView {
                                                                        if let Some(wc) = word_count {
                                                                            let max_words = config_resource.get()
                                                                                .flatten()
                                                                                .map(|c| c.max_context_words)
                                                                                .unwrap_or(12_000);
                                                                            let is_over_limit = wc as usize > max_words;
                                                                            let color_class = if is_over_limit { "text-rose-400" } else { "text-slate-500" };
                                                                            view! {
                                                                                <p class=format!("mt-1 text-xs {}", color_class)>
                                                                                    {format!("{} words", format_number(wc))}
                                                                                    {if is_over_limit { format!(" (exceeds limit of {})", format_number(max_words as i64)) } else { String::new() }}
                                                                                </p>
                                                                            }.into_any()
                                                                        } else {
                                                                            view! { <span></span> }.into_any()
                                                                        }
                                                                    }}

                                                                    {move || -> AnyView {
                                                                        if is_processing {
                                                                            view! {
                                                                                <p class="mt-1 text-xs text-amber-400">"Processing..."</p>
                                                                            }.into_any()
                                                                        } else if is_failed {
                                                                            view! {
                                                                                <p class="mt-1 text-xs text-rose-400">"Processing failed"</p>
                                                                            }.into_any()
                                                                        } else {
                                                                            view! { <span></span> }.into_any()
                                                                        }
                                                                    }}
                                                                </div>
                                                                <div class="flex gap-2">
                                                                    {move || -> AnyView {
                                                                        if !is_processing && !is_failed {
                                                                            view! {
                                                                                <button
                                                                                    class="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-300 hover:border-slate-400"
                                                                                    on:click=move |ev| {
                                                                                        ev.stop_propagation();
                                                                                        if let Some(project_id) = project_id_signal.get() {
                                                                                            open_pdf_modal(project_id, file.id);
                                                                                        }
                                                                                    }
                                                                                >
                                                                                    "View PDF"
                                                                                </button>
                                                                            }.into_any()
                                                                        } else {
                                                                            view! { <span></span> }.into_any()
                                                                        }
                                                                    }}
                                                                    <button
                                                                        class="rounded-full border border-rose-800 px-3 py-1 text-xs text-rose-300 hover:border-rose-600"
                                                                        type="button"
                                                                        on:click=move |ev| {
                                                                            ev.stop_propagation();
                                                                            if window().confirm_with_message("Are you sure you want to delete this file?").unwrap_or(false) {
                                                                                delete_action.dispatch(file.id);
                                                                            }
                                                                        }
                                                                    >
                                                                        "Delete"
                                                                    </button>
                                                                </div>
                                                            </div>
                                                            {move || -> AnyView {
                                                                if !is_processing && !is_failed && has_preview {
                                                                    view! {
                                                                        <p class="mt-3 max-h-32 overflow-hidden text-xs text-slate-400 whitespace-pre-line">{preview.clone()}</p>
                                                                    }.into_any()
                                                                } else if !is_processing && !is_failed && !has_preview {
                                                                    view! {
                                                                        <p class="mt-3 text-xs text-slate-500">"No extractable text found."</p>
                                                                    }.into_any()
                                                                } else {
                                                                    view! { <span></span> }.into_any()
                                                                }
                                                            }}
                                                        </li>
                                                    }
                                                }).collect_view()}
                                            </ul>
                                        }.into_any(),
                                        Some(Err(err)) => view! {
                                            <p class="text-sm text-rose-300">{err.to_string()}</p>
                                        }.into_any(),
                                        None => {
                                            let _: () = view! { <> </> };
                                            ().into_any()
                                        },
                                    }}}
                                </Show>
                            </div>
                        }.into_any(),
                        Some(Err(err)) => view! { <> <p class="text-sm text-rose-300">{err.to_string()}</p> </> }.into_any(),
                        None => {
                            let _: () = view! { <> </> };
                            ().into_any()
                        },
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

fn format_number(num: i64) -> String {
    let s = num.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(*ch);
    }

    result
}
