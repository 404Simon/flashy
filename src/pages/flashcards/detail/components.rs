use leptos::prelude::*;

use crate::features::{
    flashcards::{
        list_flashcards_by_file, markdown::markdown_to_html, FileCardGroup, GenerationJobWithFile,
        StartGenerationJob, DEFAULT_PROMPT_TEMPLATE,
    },
    projects::models::ProjectFile,
};

// ============= ACTION BAR =============

#[component]
pub fn ActionBar(
    on_rename: impl Fn() + 'static + Copy,
    on_delete: impl Fn() + 'static + Copy,
    on_generate: impl Fn() + 'static + Copy,
    deck_id: i64,
) -> impl IntoView {
    view! {
        <div class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
            <div class="flex flex-wrap gap-3">
                <button
                    class="inline-flex items-center rounded-full border border-slate-700 px-6 py-2 text-sm font-semibold text-slate-300 hover:border-slate-400 hover:bg-slate-800"
                    on:click=move |_| on_rename()
                >
                    "Rename"
                </button>
                <button
                    class="inline-flex items-center rounded-full border border-rose-700 px-6 py-2 text-sm font-semibold text-rose-300 hover:border-rose-400 hover:bg-rose-950"
                    on:click=move |_| on_delete()
                >
                    "Delete Deck"
                </button>
            </div>
            <div class="flex gap-3">
                <a
                    class="inline-flex items-center rounded-full border border-slate-700 px-6 py-2 text-sm font-semibold text-slate-300 hover:border-slate-400 hover:bg-slate-800"
                    href=format!("/decks/{}/study", deck_id)
                >
                    "Study Cards"
                </a>
                <button
                    class="inline-flex items-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950 hover:bg-slate-100"
                    on:click=move |_| on_generate()
                >
                    "+ Add Cards via AI"
                </button>
            </div>
        </div>
    }
}

// ============= GENERATION JOBS LIST =============

#[component]
pub fn GenerationJobsList(jobs: Signal<Vec<GenerationJobWithFile>>) -> impl IntoView {
    view! {
        <div class="space-y-2">
            {move || {
                let job_list = jobs.get();
                if job_list.is_empty() {
                    view! {
                        <p class="text-sm text-slate-400">"No jobs found."</p>
                    }.into_any()
                } else {
                    view! {
                        <div class="space-y-2">
                            {job_list.into_iter().map(|job| {
                                view! { <GenerationJobCard job=job /> }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

#[component]
fn GenerationJobCard(job: GenerationJobWithFile) -> impl IntoView {
    let status_class = match job.status.as_str() {
        "pending" => "border-yellow-500/40 bg-yellow-500/10 text-yellow-200",
        "processing" => "border-blue-500/40 bg-blue-500/10 text-blue-200",
        "completed" => "border-emerald-500/40 bg-emerald-500/10 text-emerald-200",
        "failed" => "border-rose-500/40 bg-rose-500/10 text-rose-200",
        _ => "border-slate-500/40 bg-slate-500/10 text-slate-200",
    };

    let segment_suffix = job
        .segment_label
        .as_ref()
        .map(|label| format!(" · {}", label))
        .unwrap_or_default();
    let status_text = match job.status.as_str() {
        "pending" => format!("Waiting to start - {}{}", job.file_name, segment_suffix),
        "processing" => format!(
            "Generating flashcards from {}{}",
            job.file_name, segment_suffix
        ),
        "completed" => format!(
            "Completed - {} cards generated from {}{}",
            job.cards_generated, job.file_name, segment_suffix
        ),
        "failed" => job
            .error_message
            .as_deref()
            .map(|msg| format!("Failed: {} ({}{})", msg, job.file_name, segment_suffix))
            .unwrap_or_else(|| format!("Generation failed ({}{})", job.file_name, segment_suffix)),
        _ => format!("Unknown status ({}{})", job.file_name, segment_suffix),
    };

    view! {
        <div class={format!("rounded-xl border px-4 py-3 {}", status_class)}>
            <div class="flex items-center justify-between">
                <div class="flex-1">
                    <p class="text-sm font-medium">{status_text}</p>
                    <p class="text-xs opacity-70 mt-1">{job.created_at.clone()}</p>
                </div>
                <Show when=move || job.status == "processing">
                    <div class="ml-3">
                        <svg class="animate-spin h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                        </svg>
                    </div>
                </Show>
            </div>
        </div>
    }
}

// ============= FILE CARD GROUP LIST =============

#[component]
pub fn FileCardGroupList(
    files: Vec<FileCardGroup>,
    _deck_id: i64,
    show_modal: RwSignal<bool>,
    selected_file: RwSignal<Option<FileCardGroup>>,
) -> impl IntoView {
    let open_modal = move |file: FileCardGroup| {
        selected_file.set(Some(file));
        show_modal.set(true);
    };

    view! {
        <div class="space-y-3">
            {files.into_iter().map(|file| {
                let file_clone = file.clone();
                view! {
                    <button
                        class="w-full rounded-2xl border border-slate-800 bg-slate-900/50 px-6 py-4 text-left hover:border-slate-600 hover:bg-slate-900/70 transition-colors"
                        on:click=move |_| open_modal(file_clone.clone())
                    >
                        <h3 class="text-base font-semibold text-white">{file.file_name.clone()}</h3>
                        <p class="text-xs text-slate-400 mt-1">
                            {format!("{} cards • Generated {}", file.card_count, file.created_at)}
                        </p>
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
pub fn FileCardsModal(
    file: Signal<Option<FileCardGroup>>,
    deck_id: i64,
    on_close: impl Fn() + 'static + Copy + Send,
) -> impl IntoView {
    // Store the file_id separately to avoid tracking the full file signal
    let file_id = StoredValue::new(file.get_untracked().map(|f| f.file_id));

    let cards_resource = LocalResource::new(move || async move {
        if let Some(fid) = file_id.get_value() {
            list_flashcards_by_file(deck_id, fid).await.ok()
        } else {
            None
        }
    });

    // MathJax rendering effect - runs when cards are loaded
    Effect::new(move |_| {
        let _ = cards_resource.get();

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

    view! {
        <div
            class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
            on:click=move |_| on_close()
        >
            <div
                class="w-full max-w-3xl rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl max-h-[85vh] overflow-hidden flex flex-col"
                on:click=move |ev| ev.stop_propagation()
            >
                <div class="border-b border-slate-800 px-6 py-4">
                    {move || file.get().map(|f| view! {
                        <div>
                            <h3 class="text-lg font-semibold text-white">{f.file_name.clone()}</h3>
                            <p class="text-sm text-slate-400 mt-1">
                                {format!("{} flashcards", f.card_count)}
                            </p>
                        </div>
                    })}
                </div>
                <div class="flex-1 overflow-y-auto p-6">
                    <Show
                        when=move || cards_resource.get().is_some()
                        fallback=move || view! { <p class="text-sm text-slate-400">"Loading cards..."</p> }
                    >
                        {move || match cards_resource.get().flatten() {
                            Some(cards) if cards.is_empty() => view! {
                                <p class="text-sm text-slate-400">"No cards found."</p>
                            }.into_any(),
                            Some(cards) => view! {
                                <div class="space-y-4">
                                    {cards.into_iter().map(|card| {
                                        let front_html = markdown_to_html(&card.front);
                                        let back_html = markdown_to_html(&card.back);
                                        view! {
                                            <div class="rounded-xl border border-slate-700 bg-slate-900/50 p-5">
                                                <div class="space-y-3">
                                                    <div>
                                                        <p class="text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">"Question"</p>
                                                        <div
                                                            class="mathjax-content text-sm text-white leading-relaxed prose prose-invert max-w-none"
                                                            inner_html=front_html
                                                        ></div>
                                                    </div>
                                                    <div class="border-t border-slate-800 pt-3">
                                                        <p class="text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">"Answer"</p>
                                                        <div
                                                            class="mathjax-content text-sm text-slate-300 leading-relaxed prose prose-invert max-w-none"
                                                            inner_html=back_html
                                                        ></div>
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any(),
                            None => view! {
                                <p class="text-sm text-slate-400">"No cards available."</p>
                            }.into_any(),
                        }}
                    </Show>
                </div>
                <div class="border-t border-slate-800 px-6 py-4">
                    <button
                        class="w-full rounded-full border border-slate-700 px-6 py-2 text-sm font-semibold text-slate-300 hover:border-slate-400"
                        on:click=move |_| on_close()
                    >
                        "Close"
                    </button>
                </div>
            </div>
        </div>
    }
}

// ============= GENERATION MODAL =============

#[component]
pub fn GenerationModal(
    deck_id: Signal<Option<i64>>,
    files: Signal<Vec<ProjectFile>>,
    action: ServerAction<StartGenerationJob>,
    on_close: impl Fn() + 'static + Copy + Send,
) -> impl IntoView {
    let selected_file_id = RwSignal::new(None::<i64>);
    let custom_prompt = RwSignal::new(String::from(DEFAULT_PROMPT_TEMPLATE));

    view! {
        <div
            class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
            on:click=move |_| on_close()
        >
            <div
                class="w-full max-w-2xl rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl max-h-[90vh] overflow-y-auto"
                on:click=move |ev| ev.stop_propagation()
            >
                <div class="border-b border-slate-800 px-6 py-4">
                    <h3 class="text-lg font-semibold text-white">"Generate Flashcards from PDF"</h3>
                </div>
                <div class="p-6">
                    <ActionForm action=action>
                        <div class="space-y-4">
                            <input type="hidden" name="deck_id" value=move || deck_id.get().unwrap_or(0) />

                            <div class="flex flex-col gap-2 text-sm text-slate-300">
                                <label>"Select PDF"</label>
                                <select
                                    class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100"
                                    name="file_id"
                                    required
                                    on:change=move |ev| {
                                        let value = event_target_value(&ev);
                                        selected_file_id.set(value.parse::<i64>().ok());
                                    }
                                >
                                    <option value="">"Choose a PDF..."</option>
                                    {move || {
                                        files.get().into_iter().map(|file| {
                                            let id = file.id.to_string();
                                            let name = file.original_filename.clone();
                                            view! {
                                                <option value=id>{name}</option>
                                            }
                                        }).collect_view()
                                    }}
                                </select>
                            </div>

                            <label class="flex flex-col gap-2 text-sm text-slate-300">
                                "Custom Prompt (optional)"
                                <textarea
                                    class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100 font-mono text-xs min-h-[200px]"
                                    name="prompt_template"
                                    placeholder=DEFAULT_PROMPT_TEMPLATE
                                    prop:value=move || custom_prompt.get()
                                    on:input=move |ev| custom_prompt.set(event_target_value(&ev))
                                ></textarea>
                                <span class="text-xs text-slate-500">"Leave empty to use default prompt. Use $DECK_TITLE$ and $DOCUMENT_TEXT$ as placeholders."</span>
                            </label>

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
                                        "Generation job started! Cards will appear here when ready."
                                    </div>
                                })}
                            </Show>

                            <div class="flex gap-3 pt-2">
                                <button
                                    class="flex-1 rounded-full border border-slate-700 px-6 py-2 text-sm font-semibold text-slate-300"
                                    type="button"
                                    on:click=move |_| on_close()
                                >
                                    "Close"
                                </button>
                                <button
                                    class="flex-1 rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950 disabled:opacity-50"
                                    type="submit"
                                    disabled=move || action.pending().get()
                                >
                                    "Start Generation"
                                </button>
                            </div>
                        </div>
                    </ActionForm>
                </div>
            </div>
        </div>
    }
}
