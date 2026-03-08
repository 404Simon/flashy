use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::features::{
    auth::models::UserSession,
    flashcards::{
        delete_deck, get_deck, list_files_with_cards_for_deck,
        list_generation_jobs_with_files_for_deck, update_deck,
    },
};

mod components;
use components::{ActionBar, FileCardGroupList, GenerationJobsList};

#[component]
pub fn DeckDetailPage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());

    let params = use_params_map();
    let deck_id = Signal::derive(move || {
        params.with(|p| p.get("deck_id").and_then(|id| id.parse::<i64>().ok()))
    });

    let deck_resource = LocalResource::new(move || {
        let id = deck_id.get();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid deck"))?;
            get_deck(id).await
        }
    });

    let files_resource = LocalResource::new(move || {
        let id = deck_id.get();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid deck"))?;
            list_files_with_cards_for_deck(id).await
        }
    });

    let jobs_resource = LocalResource::new(move || {
        let id = deck_id.get();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid deck"))?;
            list_generation_jobs_with_files_for_deck(id).await
        }
    });

    // Modals
    let show_rename_modal = RwSignal::new(false);
    let show_cards_modal = RwSignal::new(false);
    let selected_file = RwSignal::new(None::<crate::features::flashcards::FileCardGroup>);
    let rename_name = RwSignal::new(String::new());
    let rename_description = RwSignal::new(String::new());

    // Poll for job updates
    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| {
        let _ = deck_id.get_untracked();
        let _handle = leptos::task::spawn_local(async move {
            loop {
                gloo_timers::future::sleep(std::time::Duration::from_secs(5)).await;
                if deck_id.get_untracked().is_some() {
                    jobs_resource.refetch();
                    files_resource.refetch();
                } else {
                    break;
                }
            }
        });
    });

    let handle_rename = move || {
        if let Some(Ok(deck)) = deck_resource.get() {
            rename_name.set(deck.name.clone());
            rename_description.set(deck.description.clone().unwrap_or_default());
            show_rename_modal.set(true);
        }
    };

    let handle_rename_submit = move || {
        if let Some(id) = deck_id.get_untracked() {
            let name = rename_name.get();
            let desc = rename_description.get();
            let description = if desc.trim().is_empty() {
                None
            } else {
                Some(desc)
            };

            leptos::task::spawn_local(async move {
                if update_deck(id, name, description).await.is_ok() {
                    deck_resource.refetch();
                    show_rename_modal.set(false);
                }
            });
        }
    };

    let handle_delete = move || {
        if let Some(id) = deck_id.get_untracked() {
            if let Some(Ok(deck)) = deck_resource.get() {
                #[cfg(target_arch = "wasm32")]
                {
                    let confirmed = web_sys::window()
                        .and_then(|w| {
                            w.confirm_with_message(&format!(
                                "Delete deck '{}' and all its cards?",
                                deck.name
                            ))
                            .ok()
                        })
                        .unwrap_or(false);

                    if confirmed {
                        let project_id = deck.project_id;
                        leptos::task::spawn_local(async move {
                            if delete_deck(id).await.is_ok() {
                                leptos_router::hooks::use_navigate()(
                                    &format!("/projects/{}/decks", project_id),
                                    Default::default(),
                                );
                            }
                        });
                    }
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let project_id = deck.project_id;
                    leptos::task::spawn_local(async move {
                        if delete_deck(id).await.is_ok() {
                            leptos_router::hooks::use_navigate()(
                                &format!("/projects/{}/decks", project_id),
                                Default::default(),
                            );
                        }
                    });
                }
            }
        }
    };

    view! {
        <section class="mx-auto flex min-h-screen max-w-6xl flex-col gap-8 px-6 py-16">
            <Show
                when=move || user.get().is_some()
                fallback=move || view! {
                    <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-6 text-slate-300">
                        "Please log in to view this deck."
                    </div>
                }
            >
                <Show
                    when=move || deck_resource.get().is_some()
                    fallback=move || view! { <p class="text-sm text-slate-400">"Loading deck..."</p> }
                >
                    {move || match deck_resource.get() {
                        Some(Ok(deck)) => view! {
                            <div class="space-y-8">
                                // Header with back link
                                <div class="space-y-2">
                                    <a
                                        class="text-sm text-slate-400 hover:text-white"
                                        href=format!("/projects/{}/decks", deck.project_id)
                                    >"← Back to decks"</a>
                                    <h1 class="text-4xl font-semibold text-white">{deck.name.clone()}</h1>
                                    {deck.description.as_ref().map(|desc| view! {
                                        <p class="text-slate-300">{desc.clone()}</p>
                                    })}
                                </div>

                                // Action Bar
                                <ActionBar
                                    on_rename=handle_rename
                                    on_delete=handle_delete
                                    deck_id=deck.id
                                />

                                // Processing Jobs Section
                                <Show when=move || {
                                    jobs_resource.get()
                                        .and_then(|r| r.ok())
                                        .map(|jobs| jobs.iter().any(|j| j.status == "pending" || j.status == "processing"))
                                        .unwrap_or(false)
                                }>
                                    <div class="rounded-2xl border border-blue-500/40 bg-blue-500/5 p-6 space-y-4">
                                        <h2 class="text-lg font-semibold text-blue-200">"Processing PDFs"</h2>
                                        <GenerationJobsList
                                            jobs=Signal::derive(move || {
                                                jobs_resource.get()
                                                    .and_then(|r| r.ok())
                                                    .map(|jobs| jobs.into_iter()
                                                        .filter(|j| j.status == "pending" || j.status == "processing")
                                                        .collect::<Vec<_>>())
                                                    .unwrap_or_default()
                                            })
                                        />
                                    </div>
                                </Show>

                                // Generated PDFs Section
                                <div class="space-y-4">
                                    <h2 class="text-lg font-semibold text-white">"Generated Flashcards"</h2>
                                    <Show
                                        when=move || files_resource.get().is_some()
                                        fallback=move || view! { <p class="text-sm text-slate-400">"Loading files..."</p> }
                                    >
                                        {move || match files_resource.get() {
                                            Some(Ok(files)) if files.is_empty() => view! {
                                                <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-12 text-center">
                                                    <p class="text-slate-400">"No flashcards generated yet. Click 'Add Cards via AI' to get started!"</p>
                                                </div>
                                            }.into_any(),
                                            Some(Ok(files)) => view! {
                                                <FileCardGroupList
                                                    files=files
                                                    _deck_id=deck.id
                                                    show_modal=show_cards_modal
                                                    selected_file=selected_file
                                                />
                                            }.into_any(),
                                            Some(Err(err)) => view! {
                                                <p class="text-sm text-rose-300">{err.to_string()}</p>
                                            }.into_any(),
                                            None => view! { <span></span> }.into_any(),
                                        }}
                                    </Show>
                                </div>

                                // Recent Generation History
                                <Show when=move || {
                                    jobs_resource.get()
                                        .and_then(|r| r.ok())
                                        .map(|jobs| jobs.iter().any(|j| j.status == "completed" || j.status == "failed"))
                                        .unwrap_or(false)
                                }>
                                    <div class="space-y-4">
                                        <h2 class="text-lg font-semibold text-white">"Generation History"</h2>
                                        <GenerationJobsList
                                            jobs=Signal::derive(move || {
                                                jobs_resource.get()
                                                    .and_then(|r| r.ok())
                                                    .map(|jobs| jobs.into_iter()
                                                        .filter(|j| j.status == "completed" || j.status == "failed")
                                                        .take(5)
                                                        .collect::<Vec<_>>())
                                                    .unwrap_or_default()
                                            })
                                        />
                                    </div>
                                </Show>
                            </div>
                        }.into_any(),
                        Some(Err(err)) => view! {
                            <p class="text-sm text-rose-300">{err.to_string()}</p>
                        }.into_any(),
                        None => view! { <span></span> }.into_any(),
                    }}
                </Show>
            </Show>

            // Rename Modal
            <Show when=move || show_rename_modal.get()>
                <div
                    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
                    on:click=move |_| show_rename_modal.set(false)
                >
                    <div
                        class="w-full max-w-md rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <div class="border-b border-slate-800 px-6 py-4">
                            <h3 class="text-lg font-semibold text-white">"Rename Deck"</h3>
                        </div>
                        <div class="p-6">
                            <div class="space-y-4">
                                <label class="flex flex-col gap-2 text-sm text-slate-300">
                                    "Deck Name"
                                    <input
                                        class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100"
                                        type="text"
                                        placeholder="e.g., Algorithms Midterm"
                                        prop:value=move || rename_name.get()
                                        on:input=move |ev| rename_name.set(event_target_value(&ev))
                                    />
                                </label>

                                <label class="flex flex-col gap-2 text-sm text-slate-300">
                                    "Description (optional)"
                                    <textarea
                                        class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100 min-h-[80px]"
                                        placeholder="Brief description..."
                                        prop:value=move || rename_description.get()
                                        on:input=move |ev| rename_description.set(event_target_value(&ev))
                                    ></textarea>
                                </label>

                                <div class="flex gap-3 pt-2">
                                    <button
                                        class="flex-1 rounded-full border border-slate-700 px-6 py-2 text-sm font-semibold text-slate-300"
                                        type="button"
                                        on:click=move |_| show_rename_modal.set(false)
                                    >
                                        "Cancel"
                                    </button>
                                    <button
                                        class="flex-1 rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950"
                                        type="button"
                                        on:click=move |_| handle_rename_submit()
                                    >
                                        "Save"
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </Show>

            // Cards Modal
            <Show when=move || show_cards_modal.get()>
                {move || {
                    let deck_id_val = deck_id.get().unwrap_or(0);
                    view! {
                        <components::FileCardsModal
                            file=Signal::derive(move || selected_file.get())
                            deck_id=deck_id_val
                            on_close=move || show_cards_modal.set(false)
                        />
                    }
                }}
            </Show>
        </section>
    }
}
