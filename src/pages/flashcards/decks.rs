use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::features::{
    auth::models::UserSession,
    flashcards::{
        delete_deck, list_decks_for_project, list_generation_jobs_for_deck, CreateDeck,
        DeckSummary, StartGenerationJob, DEFAULT_PROMPT_TEMPLATE,
    },
    projects::handlers::{get_project, list_project_files},
};

#[component]
pub fn DecksPage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());

    let params = use_params_map();
    let project_id = move || params.with(|p| p.get("id").and_then(|id| id.parse::<i64>().ok()));

    let project_resource = LocalResource::new(move || {
        let id = project_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid project"))?;
            get_project(id).await
        }
    });

    let decks_resource = LocalResource::new(move || {
        let id = project_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid project"))?;
            list_decks_for_project(id).await
        }
    });

    let files_resource = LocalResource::new(move || {
        let id = project_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid project"))?;
            list_project_files(id).await
        }
    });

    let show_create_modal = RwSignal::new(false);
    let show_generate_modal = RwSignal::new(false);
    let selected_deck_id = RwSignal::new(None::<i64>);
    let selected_file_id = RwSignal::new(None::<i64>);
    let custom_prompt = RwSignal::new(String::from(DEFAULT_PROMPT_TEMPLATE));

    let create_deck_action = ServerAction::<CreateDeck>::new();
    let start_job_action = ServerAction::<StartGenerationJob>::new();

    // Resource to track generation jobs for the selected deck
    let jobs_resource = LocalResource::new(move || {
        let deck_id = selected_deck_id.get();
        async move {
            if let Some(deck_id) = deck_id {
                list_generation_jobs_for_deck(deck_id).await.ok()
            } else {
                None
            }
        }
    });

    Effect::new(move |_| {
        if create_deck_action.value().get().is_some() {
            show_create_modal.set(false);
            decks_resource.refetch();
        }
    });

    Effect::new(move |_| {
        if start_job_action.value().get().is_some() {
            show_generate_modal.set(false);
            jobs_resource.refetch();
        }
    });

    let open_generate_modal = move |deck_id: i64| {
        selected_deck_id.set(Some(deck_id));
        selected_file_id.set(None);
        jobs_resource.refetch();
        show_generate_modal.set(true);
    };

    // Poll for job status updates when modal is open
    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| {
        if show_generate_modal.get() {
            let _handle = leptos::task::spawn_local(async move {
                loop {
                    gloo_timers::future::sleep(std::time::Duration::from_secs(3)).await;
                    if show_generate_modal.get() {
                        jobs_resource.refetch();
                    } else {
                        break;
                    }
                }
            });
        }
    });

    view! {
        <section class="mx-auto flex min-h-screen max-w-5xl flex-col gap-8 px-6 py-16">
            <Show
                when=move || user.get().is_some()
                fallback=move || view! {
                    <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-6 text-slate-300">
                        "Please log in to view decks."
                    </div>
                }
            >
                <Show
                    when=move || project_resource.get().is_some()
                    fallback=move || view! { <p class="text-sm text-slate-400">"Loading project..."</p> }
                >
                    {move || match project_resource.get() {
                        Some(Ok(project)) => view! {
                            <div class="space-y-2">
                                <a
                                    class="text-sm text-slate-400 hover:text-white"
                                    href=format!("/projects/{}", project.id)
                                >"← Back to project"</a>
                                <h1 class="text-4xl font-semibold text-white">"Flashcard Decks"</h1>
                                <p class="text-slate-300">{format!("Project: {}", project.name)}</p>
                            </div>

                            <div class="flex items-center justify-between">
                                <h2 class="text-lg font-semibold text-white">"Your Decks"</h2>
                                <button
                                    class="inline-flex items-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950"
                                    on:click=move |_| show_create_modal.set(true)
                                >
                                    "+ New Deck"
                                </button>
                            </div>

                            <Show
                                when=move || decks_resource.get().is_some()
                                fallback=move || view! { <p class="text-sm text-slate-400">"Loading decks..."</p> }
                            >
                                {move || match decks_resource.get() {
                                    Some(Ok(decks)) if decks.is_empty() => view! {
                                        <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-12 text-center">
                                            <p class="text-slate-400">"No decks yet. Create one to get started!"</p>
                                        </div>
                                    }.into_any(),
                                    Some(Ok(decks)) => view! {
                                        <div class="grid gap-4 md:grid-cols-2">
                                            {decks.into_iter().map(|deck| {
                                                let deck_id = deck.id;
                                                view! {
                                                    <DeckCard
                                                        deck=deck
                                                        on_generate=move || open_generate_modal(deck_id)
                                                        on_delete=move || {
                                                            leptos::task::spawn_local(async move {
                                                                if let Ok(_) = delete_deck(deck_id).await {
                                                                    decks_resource.refetch();
                                                                }
                                                            });
                                                        }
                                                    />
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any(),
                                    Some(Err(err)) => view! {
                                        <p class="text-sm text-rose-300">{err.to_string()}</p>
                                    }.into_any(),
                                    None => view! { <span></span> }.into_any(),
                                }}
                            </Show>
                        }.into_any(),
                        Some(Err(err)) => view! {
                            <p class="text-sm text-rose-300">{err.to_string()}</p>
                        }.into_any(),
                        None => view! { <span></span> }.into_any(),
                    }}
                </Show>
            </Show>

            // Create Deck Modal
            <Show when=move || show_create_modal.get()>
                <div
                    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
                    on:click=move |_| show_create_modal.set(false)
                >
                    <div
                        class="w-full max-w-md rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <div class="border-b border-slate-800 px-6 py-4">
                            <h3 class="text-lg font-semibold text-white">"Create New Deck"</h3>
                        </div>
                        <div class="p-6">
                            <ActionForm action=create_deck_action>
                                <div class="space-y-4">
                                    <input type="hidden" name="project_id" value=move || project_id().unwrap_or(0) />

                                    <label class="flex flex-col gap-2 text-sm text-slate-300">
                                        "Deck Name"
                                        <input
                                            class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100"
                                            type="text"
                                            name="name"
                                            placeholder="e.g., Algorithms Midterm"
                                            required
                                        />
                                    </label>

                                    <label class="flex flex-col gap-2 text-sm text-slate-300">
                                        "Description (optional)"
                                        <textarea
                                            class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100 min-h-[80px]"
                                            name="description"
                                            placeholder="Brief description..."
                                        ></textarea>
                                    </label>

                                    <div class="flex gap-3 pt-2">
                                        <button
                                            class="flex-1 rounded-full border border-slate-700 px-6 py-2 text-sm font-semibold text-slate-300"
                                            type="button"
                                            on:click=move |_| show_create_modal.set(false)
                                        >
                                            "Cancel"
                                        </button>
                                        <button
                                            class="flex-1 rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950"
                                            type="submit"
                                        >
                                            "Create Deck"
                                        </button>
                                    </div>
                                </div>
                            </ActionForm>
                        </div>
                    </div>
                </div>
            </Show>

            // Generate Flashcards Modal
            <Show when=move || show_generate_modal.get()>
                <div
                    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
                    on:click=move |_| show_generate_modal.set(false)
                >
                    <div
                        class="w-full max-w-2xl rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl max-h-[90vh] overflow-y-auto"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <div class="border-b border-slate-800 px-6 py-4">
                            <h3 class="text-lg font-semibold text-white">"Generate Flashcards from PDF"</h3>
                        </div>
                        <div class="p-6 space-y-6">
                            // Show recent generation jobs
                            <Show when=move || jobs_resource.get().is_some()>
                                {move || {
                                    jobs_resource.get().and_then(|jobs_opt| {
                                        jobs_opt.map(|jobs| {
                                            if jobs.is_empty() {
                                                return view! {
                                                    <div class="text-sm text-slate-400">"No generation jobs yet for this deck."</div>
                                                }.into_any();
                                            }
                                            view! {
                                                <div class="space-y-2">
                                                    <h4 class="text-sm font-semibold text-slate-300">"Recent Generation Jobs"</h4>
                                                    <div class="space-y-2">
                                                        {jobs.iter().take(3).map(|job| {
                                                            let status_class = match job.status.as_str() {
                                                                "pending" => "border-yellow-500/40 bg-yellow-500/10 text-yellow-200",
                                                                "processing" => "border-blue-500/40 bg-blue-500/10 text-blue-200",
                                                                "completed" => "border-emerald-500/40 bg-emerald-500/10 text-emerald-200",
                                                                "failed" => "border-rose-500/40 bg-rose-500/10 text-rose-200",
                                                                _ => "border-slate-500/40 bg-slate-500/10 text-slate-200",
                                                            };
                                                            let status_text = match job.status.as_str() {
                                                                "pending" => "Waiting to start...".to_string(),
                                                                "processing" => "Generating flashcards...".to_string(),
                                                                "completed" => format!("Completed - {} cards generated", job.cards_generated),
                                                                "failed" => job.error_message.as_deref().unwrap_or("Generation failed").to_string(),
                                                                _ => "Unknown status".to_string(),
                                                            };
                                                            view! {
                                                                <div class={format!("rounded-xl border px-4 py-2 text-sm {}", status_class)}>
                                                                    <div class="flex items-center justify-between">
                                                                        <span>{status_text}</span>
                                                                        <span class="text-xs opacity-70">{job.created_at.clone()}</span>
                                                                    </div>
                                                                </div>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                            }.into_any()
                                        })
                                    })
                                }}
                            </Show>

                            // Form to start a new generation job
                            <ActionForm action=start_job_action>
                                <div class="space-y-4">
                                    <input type="hidden" name="deck_id" value=move || selected_deck_id.get().unwrap_or(0) />

                                    <Show when=move || files_resource.get().is_some()>
                                        {move || files_resource.get().map(|result| match result {
                                            Ok(files) => {
                                                let file_options: Vec<_> = files.into_iter().map(|file| {
                                                    let id = file.id.to_string();
                                                    let name = file.original_filename.clone();
                                                    view! {
                                                        <option value=id>{name}</option>
                                                    }
                                                }).collect();

                                                view! {
                                                    <div class="flex flex-col gap-2 text-sm text-slate-300">
                                                        <label>"Select PDF"</label>
                                                        <select
                                                            class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100"
                                                            name="file_id"
                                                            required
                                                        >
                                                            <option value="">"Choose a PDF..."</option>
                                                            {file_options}
                                                        </select>
                                                    </div>
                                                }.into_any()
                                            },
                                            Err(err) => view! {
                                                <div class="text-sm text-rose-300">{err.to_string()}</div>
                                            }.into_any()
                                        })}
                                    </Show>

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

                                    <Show when=move || start_job_action.pending().get()>
                                        <div class="rounded-xl border border-blue-500/40 bg-blue-500/10 px-4 py-2 text-sm text-blue-200">
                                            "Starting generation job..."
                                        </div>
                                    </Show>

                                    <Show when=move || {
                                        start_job_action.value().get().as_ref().and_then(|r| r.as_ref().err()).is_some()
                                    }>
                                        {move || start_job_action.value().get().as_ref().and_then(|r| r.as_ref().err()).map(|err| view! {
                                            <div class="rounded-xl border border-rose-500/40 bg-rose-500/10 px-4 py-2 text-sm text-rose-200">
                                                {err.to_string()}
                                            </div>
                                        })}
                                    </Show>

                                    <Show when=move || {
                                        start_job_action.value().get().as_ref().and_then(|r| r.as_ref().ok()).is_some()
                                    }>
                                        {move || start_job_action.value().get().as_ref().and_then(|r| r.as_ref().ok()).map(|_job| view! {
                                            <div class="rounded-xl border border-emerald-500/40 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-200">
                                                "Generation job started! It will run in the background. You can close this modal and check back later."
                                            </div>
                                        })}
                                    </Show>

                                    <div class="flex gap-3 pt-2">
                                        <button
                                            class="flex-1 rounded-full border border-slate-700 px-6 py-2 text-sm font-semibold text-slate-300"
                                            type="button"
                                            on:click=move |_| show_generate_modal.set(false)
                                        >
                                            "Close"
                                        </button>
                                        <button
                                            class="flex-1 rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950 disabled:opacity-50"
                                            type="submit"
                                            disabled=move || start_job_action.pending().get()
                                        >
                                            "Start Generation"
                                        </button>
                                    </div>
                                </div>
                            </ActionForm>
                        </div>
                    </div>
                </div>
            </Show>
        </section>
    }
}

#[component]
fn DeckCard(
    deck: DeckSummary,
    on_generate: impl Fn() + 'static + Copy,
    on_delete: impl Fn() + 'static + Copy,
) -> impl IntoView {
    view! {
        <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6 space-y-4">
            <div>
                <h3 class="text-lg font-semibold text-white">{deck.name.clone()}</h3>
                {deck.description.as_ref().map(|desc| view! {
                    <p class="mt-1 text-sm text-slate-400">{desc.clone()}</p>
                })}
                <p class="mt-2 text-xs text-slate-500">
                    {format!("{} cards • Created {}", deck.card_count, deck.created_at)}
                </p>
            </div>

            <div class="flex gap-2">
                <a
                    class="flex-1 text-center rounded-full border border-slate-700 px-4 py-2 text-sm font-semibold text-slate-300 hover:border-slate-400"
                    href=format!("/decks/{}", deck.id)
                >
                    "Study"
                </a>
                <button
                    class="flex-1 rounded-full border border-slate-700 px-4 py-2 text-sm font-semibold text-slate-300 hover:border-slate-400"
                    on:click=move |_| on_generate()
                >
                    "+ Generate"
                </button>
                <button
                    class="rounded-full border border-rose-700 px-4 py-2 text-sm font-semibold text-rose-300 hover:border-rose-400"
                    on:click=move |_| {
                        #[cfg(target_arch = "wasm32")]
                        {
                            let confirmed = web_sys::window()
                                .and_then(|w: web_sys::Window| w.confirm_with_message("Delete this deck and all its cards?").ok())
                                .unwrap_or(false);
                            if confirmed {
                                on_delete();
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            on_delete();
                        }
                    }
                >
                    "Delete"
                </button>
            </div>
        </div>
    }
}
