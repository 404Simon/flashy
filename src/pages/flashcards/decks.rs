use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::features::{
    auth::models::UserSession,
    flashcards::{list_decks_for_project, CreateDeck, DeckSummary},
    projects::handlers::get_project,
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

    let show_create_modal = RwSignal::new(false);

    let create_deck_action = ServerAction::<CreateDeck>::new();

    Effect::new(move |_| {
        if create_deck_action.value().get().is_some() {
            show_create_modal.set(false);
            decks_resource.refetch();
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
                                                view! {
                                                    <DeckCard deck=deck />
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
        </section>
    }
}

#[component]
fn DeckCard(deck: DeckSummary) -> impl IntoView {
    view! {
        <a
            href=format!("/decks/{}", deck.id)
            class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6 space-y-2 hover:border-slate-600 hover:bg-slate-900/70 transition-colors block"
        >
            <h3 class="text-lg font-semibold text-white">{deck.name.clone()}</h3>
            {deck.description.as_ref().map(|desc| view! {
                <p class="text-sm text-slate-400">{desc.clone()}</p>
            })}
            <p class="text-xs text-slate-500">
                {format!("{} cards • Created {}", deck.card_count, deck.created_at)}
            </p>
        </a>
    }
}
