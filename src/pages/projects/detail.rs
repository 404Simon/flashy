use leptos::html::Form;
use leptos::prelude::*;
use leptos_router::hooks::{use_params_map, use_query_map};
use std::collections::BTreeSet;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::config_handlers::get_app_config;
use crate::features::{
    auth::models::UserSession,
    flashcards::{list_decks_for_project, StartGenerationJob},
    projects::handlers::{
        delete_project_file, get_project, get_project_file_outline, get_project_file_text,
        get_segment_stats, list_project_files,
    },
    projects::models::{PdfTocEntry, SegmentRange, SegmentStats},
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
    let segment_modal_file = RwSignal::new(None::<crate::features::projects::models::ProjectFile>);
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
        segment_modal_file.set(None);
        pdf_modal_url.set(Some(format!(
            "/api/projects/{project_id}/files/{file_id}/pdf"
        )));
    };

    let open_segment_modal = move |file: crate::features::projects::models::ProjectFile| {
        selected_file_id.set(None);
        pdf_modal_url.set(None);
        segment_modal_file.set(Some(file));
    };

    let close_modals = move || {
        selected_file_id.set(None);
        pdf_modal_url.set(None);
        segment_modal_file.set(None);
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
                                                    let pdf_button: AnyView = if !is_processing && !is_failed {
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
                                                        }
                                                        .into_any()
                                                    } else {
                                                        view! { <span></span> }.into_any()
                                                    };

                                                    let segment_button: AnyView = if !is_processing && !is_failed {
                                                        let file_for_segment = file.clone();
                                                        view! {
                                                            <button
                                                                class="rounded-full border border-indigo-700/60 px-3 py-1 text-xs text-indigo-200 hover:border-indigo-400"
                                                                on:click=move |ev| {
                                                                    ev.stop_propagation();
                                                                    open_segment_modal(file_for_segment.clone());
                                                                }
                                                            >
                                                                "Generate ✨"
                                                            </button>
                                                        }
                                                        .into_any()
                                                    } else {
                                                        view! { <span></span> }.into_any()
                                                    };

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
                                                                    {pdf_button}
                                                                    {segment_button}
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

        <Show when=move || segment_modal_file.get().is_some()>
            {move || {
                let project_id_value = project_id_signal.get().unwrap_or_default();
                segment_modal_file.get().map(|file| view! {
                    <SegmentModal
                        file=file
                        project_id=project_id_value
                        on_close=move || close_modals()
                    />
                })
            }}
        </Show>
        </>
    }
}

#[component]
fn SegmentModal(
    file: crate::features::projects::models::ProjectFile,
    project_id: i64,
    on_close: impl Fn() + 'static + Copy + Send,
) -> impl IntoView {
    let segment_alive = Arc::new(AtomicBool::new(true));
    {
        let segment_alive = Arc::clone(&segment_alive);
        on_cleanup(move || segment_alive.store(false, Ordering::Relaxed));
    }

    let action = ServerAction::<StartGenerationJob>::new();
    let outline_resource =
        LocalResource::new(move || async move { get_project_file_outline(file.id).await });

    let decks_resource =
        LocalResource::new(move || async move { list_decks_for_project(project_id).await.ok() });

    let outline_entries = RwSignal::new(Vec::<PdfTocEntry>::new());
    let total_pages = RwSignal::new(None::<i64>);
    let selected_ids = RwSignal::new(BTreeSet::<String>::new());
    let selected_ranges = RwSignal::new(Vec::<SegmentRange>::new());
    let toc_request_id = RwSignal::new(0u64);
    let toc_timed_out = RwSignal::new(false);
    let stats = RwSignal::new(None::<SegmentStats>);
    let stats_loading = RwSignal::new(false);
    let stats_error = RwSignal::new(None::<String>);
    let stats_request_id = RwSignal::new(0u64);
    let selected_deck_id = RwSignal::new(None::<i64>);
    let segment_label = RwSignal::new(String::new());

    Effect::new(move |_| {
        if let Some(Ok(outline)) = outline_resource.get() {
            outline_entries.set(outline.entries.clone());
            total_pages.set(Some(outline.total_pages));
        }
    });

    #[cfg(target_arch = "wasm32")]
    let segment_alive_for_toc = Arc::clone(&segment_alive);
    Effect::new(move |_| {
        if outline_resource.get().is_some() {
            let request_id = toc_request_id.get_untracked() + 1;
            toc_request_id.set(request_id);
            toc_timed_out.set(false);
            return;
        }

        let request_id = toc_request_id.get_untracked() + 1;
        toc_request_id.set(request_id);
        toc_timed_out.set(false);

        #[cfg(target_arch = "wasm32")]
        {
            let toc_request_id = toc_request_id;
            let toc_timed_out = toc_timed_out;
            let segment_alive = Arc::clone(&segment_alive_for_toc);
            leptos::task::spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(25_000).await;
                if !segment_alive.load(Ordering::Relaxed) {
                    return;
                }
                if toc_request_id.get_untracked() == request_id {
                    toc_timed_out.set(true);
                }
            });
        }
    });

    Effect::new(move |_| {
        let entries = outline_entries.get();
        let selected = selected_ids.get();
        let ranges = ranges_from_selection(&entries, &selected);
        selected_ranges.set(ranges);
    });

    let segment_alive_for_stats = Arc::clone(&segment_alive);
    Effect::new(move |_| {
        let ranges = selected_ranges.get();
        if ranges.is_empty() {
            stats.set(None);
            stats_loading.set(false);
            stats_error.set(None);
            return;
        }

        let request_id = stats_request_id.get_untracked() + 1;
        stats_request_id.set(request_id);
        stats_loading.set(true);
        stats_error.set(None);

        let file_id = file.id;
        let stats = stats;
        let stats_loading = stats_loading;
        let stats_error = stats_error;
        let stats_request_id = stats_request_id;
        let segment_alive = Arc::clone(&segment_alive_for_stats);
        leptos::task::spawn_local(async move {
            match get_segment_stats(file_id, ranges).await {
                Ok(result) => {
                    if !segment_alive.load(Ordering::Relaxed) {
                        return;
                    }
                    if stats_request_id.get_untracked() == request_id {
                        stats.set(Some(result));
                        stats_loading.set(false);
                    }
                }
                Err(err) => {
                    if !segment_alive.load(Ordering::Relaxed) {
                        return;
                    }
                    if stats_request_id.get_untracked() == request_id {
                        stats_error.set(Some(err.to_string()));
                        stats_loading.set(false);
                    }
                }
            }
        });
    });

    let page_count = Signal::derive(move || {
        let ranges = selected_ranges.get();
        count_pages(&ranges)
    });

    let can_generate = Signal::derive(move || {
        selected_deck_id.get().is_some()
            && !selected_ranges.get().is_empty()
            && !action.pending().get()
    });

    view! {
        <div
            class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
            on:click=move |_| on_close()
        >
            <div
                class="w-full max-w-6xl rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl"
                on:click=move |ev| ev.stop_propagation()
            >
                <div class="flex items-center justify-between border-b border-slate-800 px-6 py-4">
                    <div>
                        <p class="text-xs uppercase tracking-wider text-slate-500">"Segment PDF"</p>
                        <h3 class="text-lg font-semibold text-white">{file.original_filename.clone()}</h3>
                    </div>
                    <button
                        class="rounded-full border border-slate-700 px-4 py-1 text-xs text-slate-300 hover:border-slate-400"
                        on:click=move |_| on_close()
                    >
                        "Close"
                    </button>
                </div>

                <div class="grid gap-6 p-6 lg:grid-cols-[minmax(0,1.3fr)_minmax(0,0.9fr)]">
                    <div class="space-y-4">
                        <div class="flex items-center justify-between">
                            <div>
                                <p class="text-sm font-semibold text-slate-200">"Table of contents"</p>
                                <p class="text-xs text-slate-500">
                                    {move || total_pages.get().map(|p| format!("{} pages total", format_number(p))).unwrap_or_default()}
                                </p>
                            </div>
                            <div class="flex gap-2">
                                <button
                                    class="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-300 hover:border-slate-400"
                                    type="button"
                                    on:click=move |_| {
                                        let entries = outline_entries.get();
                                        selected_ids.update(|set| {
                                            set.clear();
                                            for entry in entries {
                                                set.insert(entry.id);
                                            }
                                        });
                                    }
                                >
                                    "Select all"
                                </button>
                                <button
                                    class="rounded-full border border-slate-800 px-3 py-1 text-xs text-slate-400 hover:border-slate-600"
                                    type="button"
                                    on:click=move |_| {
                                        selected_ids.update(|set| set.clear());
                                    }
                                >
                                    "Clear"
                                </button>
                            </div>
                        </div>

                        <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-4">
                            <Show
                                when=move || outline_resource.get().is_some() || toc_timed_out.get()
                                fallback=move || view! {
                                    <div class="flex items-center gap-3 text-sm text-slate-400">
                                        <span class="h-4 w-4 animate-spin rounded-full border-2 border-slate-500 border-t-transparent"></span>
                                        "Analyzing table of contents..."
                                    </div>
                                }
                            >
                                {move || {
                                    if toc_timed_out.get() {
                                        view! {
                                            <div class="space-y-2 text-sm text-slate-400">
                                                <p>"Table of contents is taking too long."</p>
                                                <button
                                                    type="button"
                                                    class="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-300 hover:border-slate-400"
                                                    on:click=move |_| {
                                                        toc_timed_out.set(false);
                                                        outline_resource.refetch();
                                                    }
                                                >
                                                    "Try again"
                                                </button>
                                            </div>
                                        }.into_any()
                                    } else {
                                        match outline_resource.get() {
                                            Some(Ok(outline)) if outline.entries.is_empty() => view! {
                                                <p class="text-sm text-slate-400">
                                                    "No embedded table of contents found in this PDF."
                                                </p>
                                            }.into_any(),
                                            Some(Ok(_outline)) => view! {
                                                <div class="max-h-[50vh] space-y-2 overflow-auto pr-2">
                                                    {move || outline_entries.get().into_iter().enumerate().map(|(index, entry)| {
                                                        let entry_id_change = entry.id.clone();
                                                        let entry_index = index;
                                                        let indent = indent_class(entry.level);
                                                        view! {
                                                            <label class="flex items-start gap-3 rounded-xl border border-transparent px-3 py-2 hover:border-slate-700/60 hover:bg-slate-900/40">
                                                                <input
                                                                    type="checkbox"
                                                                    class="mt-1 h-4 w-4 rounded border-slate-600 bg-slate-950 text-indigo-400"
                                                                    prop:checked=move || {
                                                                        let entries = outline_entries.get();
                                                                        let selected = selected_ids.get();
                                                                        if entry_index >= entries.len() {
                                                                            return false;
                                                                        }
                                                                        entry_fully_selected(&entries, &selected, entry_index)
                                                                    }
                                                                    on:change=move |ev| {
                                                                        let checked = event_target_checked(&ev);
                                                                        let entries = outline_entries.get();
                                                                        if entry_index >= entries.len() {
                                                                            return;
                                                                        }
                                                                        let descendants = descendant_ids(&entries, entry_index);
                                                                        let ancestors = ancestor_indices(&entries, entry_index);
                                                                        selected_ids.update(|set| {
                                                                            if checked {
                                                                                set.insert(entry_id_change.clone());
                                                                                for id in descendants.iter() {
                                                                                    set.insert(id.clone());
                                                                                }
                                                                            } else {
                                                                                set.remove(&entry_id_change);
                                                                                for id in descendants.iter() {
                                                                                    set.remove(id);
                                                                                }
                                                                            }

                                                                            for ancestor_index in ancestors {
                                                                                let ancestor_id = entries[ancestor_index].id.clone();
                                                                                let ancestor_descendants = descendant_ids(&entries, ancestor_index);
                                                                                let all_selected = ancestor_descendants
                                                                                    .iter()
                                                                                    .all(|id| set.contains(id));
                                                                                if all_selected {
                                                                                    set.insert(ancestor_id);
                                                                                } else {
                                                                                    set.remove(&ancestor_id);
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                />
                                                                <div class={format!("{} flex-1", indent)}>
                                                                    <p class="text-sm text-slate-100">{entry.title.clone()}</p>
                                                                    <p class="text-xs text-slate-500">
                                                                        {format!("pp. {}–{}", entry.start_page, entry.end_page)}
                                                                    </p>
                                                                </div>
                                                            </label>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            }.into_any(),
                                            Some(Err(err)) => view! {
                                                <p class="text-sm text-rose-300">{err.to_string()}</p>
                                            }.into_any(),
                                            None => view! { <span></span> }.into_any(),
                                        }
                                    }
                                }}
                            </Show>
                        </div>
                    </div>

                    <div class="space-y-4">
                        <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-5 space-y-4">
                            <div>
                                <p class="text-sm font-semibold text-slate-200">"Segment summary"</p>
                                <p class="text-xs text-slate-500">
                                    {move || {
                                        let selected_count = selected_ids.get().len();
                                        if selected_count == 0 {
                                            "Select chapters to build a segment.".to_string()
                                        } else {
                                            format!(
                                                "{} chapter{} selected",
                                                selected_count,
                                                if selected_count == 1 { "" } else { "s" }
                                            )
                                        }
                                    }}
                                </p>
                            </div>

                            <div class="grid grid-cols-2 gap-4">
                                <div class="rounded-xl border border-slate-800 bg-slate-950/70 p-4">
                                    <p class="text-xs uppercase tracking-wider text-slate-500">"Pages"</p>
                                    <p class="mt-2 text-2xl font-semibold text-white">
                                        {move || format_number(page_count.get())}
                                    </p>
                                </div>
                                <div class="rounded-xl border border-slate-800 bg-slate-950/70 p-4">
                                    <p class="text-xs uppercase tracking-wider text-slate-500">"Words"</p>
                                    <div class="mt-2 flex items-center gap-2">
                                        {move || {
                                            if stats_loading.get() {
                                                view! {
                                                    <span class="h-4 w-4 animate-spin rounded-full border-2 border-slate-500 border-t-transparent"></span>
                                                    <span class="text-sm text-slate-400">"Calculating..."</span>
                                                }.into_any()
                                            } else if let Some(stats) = stats.get() {
                                                view! {
                                                    <p class="text-2xl font-semibold text-white">{format_number(stats.word_count)}</p>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <p class="text-2xl font-semibold text-slate-500">"—"</p>
                                                }.into_any()
                                            }
                                        }}
                                    </div>
                                    {move || stats_error.get().map(|err| view! {
                                        <p class="mt-2 text-xs text-rose-300">{err}</p>
                                    })}
                                </div>
                            </div>

                            <label class="flex flex-col gap-2 text-sm text-slate-300">
                                "Segment label (optional)"
                                <input
                                    class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100"
                                    type="text"
                                    placeholder="e.g., Chapters 1-3"
                                    prop:value=move || segment_label.get()
                                    on:input=move |ev| segment_label.set(event_target_value(&ev))
                                />
                            </label>
                        </div>

                        <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-5 space-y-4">
                            <div>
                                <p class="text-sm font-semibold text-slate-200">"Generate flashcards"</p>
                                <p class="text-xs text-slate-500">
                                    "Pick a deck and create cards from this segment."
                                </p>
                            </div>

                            <div class="flex flex-col gap-2 text-sm text-slate-300">
                                <label>"Choose deck"</label>
                                <select
                                    class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100"
                                    on:change=move |ev| {
                                        let value = event_target_value(&ev);
                                        selected_deck_id.set(value.parse::<i64>().ok());
                                    }
                                >
                                    <option value="">"Select a deck..."</option>
                                    {move || {
                                        decks_resource.get().flatten().unwrap_or_default().into_iter().map(|deck| {
                                            let id = deck.id.to_string();
                                            let name = deck.name.clone();
                                            view! { <option value=id>{name}</option> }
                                        }).collect_view()
                                    }}
                                </select>
                                {move || {
                                    let decks = decks_resource.get().flatten().unwrap_or_default();
                                    if decks.is_empty() {
                                        view! {
                                            <p class="text-xs text-slate-500">
                                                "No decks found for this project yet."
                                            </p>
                                        }.into_any()
                                    } else {
                                        view! { <span></span> }.into_any()
                                    }
                                }}
                            </div>

                            <Show when=move || action.pending().get()>
                                <div class="rounded-xl border border-blue-500/40 bg-blue-500/10 px-4 py-2 text-sm text-blue-200">
                                    "Starting generation job..."
                                </div>
                            </Show>

                            <Show when=move || {
                                action.value().get().as_ref().and_then(|r| r.as_ref().err()).is_some()
                            }>
                                {move || action.value().get().as_ref().and_then(|r| r.as_ref().err()).map(|err| view! {
                                    <div class="rounded-xl border border-rose-500/40 bg-rose-500/10 px-4 py-2 text-sm text-rose-200">
                                        {err.to_string()}
                                    </div>
                                })}
                            </Show>

                            <Show when=move || {
                                action.value().get().as_ref().and_then(|r| r.as_ref().ok()).is_some()
                            }>
                                {move || action.value().get().as_ref().and_then(|r| r.as_ref().ok()).map(|_job| view! {
                                    <div class="rounded-xl border border-emerald-500/40 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-200">
                                        "Generation job started! Check your deck for new cards."
                                    </div>
                                })}
                            </Show>

                            <button
                                class="w-full rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950 disabled:opacity-50"
                                type="button"
                                on:click=move |_| {
                                    if let Some(deck_id) = selected_deck_id.get() {
                                        let ranges = selected_ranges.get();
                                        if ranges.is_empty() {
                                            return;
                                        }
                                        let label = segment_label.get();
                                        action.dispatch(StartGenerationJob {
                                            deck_id,
                                            file_id: file.id,
                                            prompt_template: None,
                                            segment_label: if label.trim().is_empty() { None } else { Some(label) },
                                            segment_ranges: Some(ranges),
                                        });
                                    }
                                }
                                disabled=move || !can_generate.get()
                            >
                                "Generate ✨"
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
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

fn ranges_from_selection(
    entries: &[PdfTocEntry],
    selected: &BTreeSet<String>,
) -> Vec<SegmentRange> {
    let mut ranges: Vec<SegmentRange> = entries
        .iter()
        .filter(|entry| selected.contains(&entry.id))
        .map(|entry| SegmentRange {
            start_page: entry.start_page,
            end_page: entry.end_page,
        })
        .collect();

    merge_ranges_for_ui(&mut ranges)
}

fn descendant_ids(entries: &[PdfTocEntry], index: usize) -> Vec<String> {
    if index >= entries.len() {
        return Vec::new();
    }

    let level = entries[index].level;
    let mut ids = Vec::new();

    for entry in entries.iter().skip(index + 1) {
        if entry.level <= level {
            break;
        }
        ids.push(entry.id.clone());
    }

    ids
}

fn ancestor_indices(entries: &[PdfTocEntry], index: usize) -> Vec<usize> {
    if index == 0 || index >= entries.len() {
        return Vec::new();
    }

    let mut ancestors = Vec::new();
    let mut current_level = entries[index].level;

    for i in (0..index).rev() {
        let level = entries[i].level;
        if level < current_level {
            ancestors.push(i);
            current_level = level;
            if current_level == 0 {
                break;
            }
        }
    }

    ancestors
}

fn entry_fully_selected(
    entries: &[PdfTocEntry],
    selected: &BTreeSet<String>,
    index: usize,
) -> bool {
    if index >= entries.len() {
        return false;
    }

    let descendants = descendant_ids(entries, index);
    if descendants.is_empty() {
        return selected.contains(&entries[index].id);
    }

    descendants.iter().all(|id| selected.contains(id))
}

fn merge_ranges_for_ui(ranges: &mut [SegmentRange]) -> Vec<SegmentRange> {
    ranges.sort_by_key(|range| (range.start_page, range.end_page));

    let mut merged: Vec<SegmentRange> = Vec::new();
    for range in ranges.iter() {
        if let Some(last) = merged.last_mut() {
            if range.start_page <= last.end_page + 1 {
                last.end_page = last.end_page.max(range.end_page);
            } else {
                merged.push(range.clone());
            }
        } else {
            merged.push(range.clone());
        }
    }

    merged
}

fn count_pages(ranges: &[SegmentRange]) -> i64 {
    ranges
        .iter()
        .map(|range| (range.end_page - range.start_page + 1).max(0))
        .sum()
}

fn indent_class(level: i64) -> &'static str {
    match level {
        0 => "pl-0",
        1 => "pl-4",
        2 => "pl-8",
        3 => "pl-12",
        _ => "pl-16",
    }
}
