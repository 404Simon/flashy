use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::features::{
    auth::models::UserSession,
    flashcards::{get_deck, list_flashcards, markdown::markdown_to_html, Flashcard},
    projects::get_file_name,
};

#[component]
pub fn DeckViewerPage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());

    let params = use_params_map();
    let deck_id = move || params.with(|p| p.get("deck_id").and_then(|id| id.parse::<i64>().ok()));

    let deck_resource = LocalResource::new(move || {
        let id = deck_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid deck"))?;
            get_deck(id).await
        }
    });

    let cards_resource = LocalResource::new(move || {
        let id = deck_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid deck"))?;
            list_flashcards(id).await
        }
    });

    let current_card_index = RwSignal::new(0usize);
    let show_answer = RwSignal::new(false);

    let next_card = move || {
        show_answer.set(false);
        if let Some(Ok(cards)) = cards_resource.get() {
            if current_card_index.get() < cards.len() - 1 {
                current_card_index.update(|i| *i += 1);
            }
        }
    };

    let prev_card = move || {
        show_answer.set(false);
        if current_card_index.get() > 0 {
            current_card_index.update(|i| *i -= 1);
        }
    };

    let toggle_answer = move || {
        show_answer.update(|s| *s = !*s);
    };

    view! {
        <section class="mx-auto flex min-h-screen max-w-5xl flex-col gap-8 px-6 py-16">
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
                            <div class="space-y-2">
                                <div class="flex items-center justify-between">
                                    <a
                                        class="text-sm text-slate-400 hover:text-white"
                                        href=format!("/decks/{}", deck.id)
                                    >"← Back to deck"</a>
                                    <a
                                        class="inline-flex items-center rounded-full border border-slate-700 px-4 py-1.5 text-xs font-semibold text-slate-300 hover:border-slate-400 hover:bg-slate-900"
                                        href=format!("/api/decks/{}/download/anki", deck.id)
                                        download
                                    >
                                        "↓ Download as Anki"
                                    </a>
                                </div>
                                <h1 class="text-4xl font-semibold text-white">{deck.name.clone()}</h1>
                                {deck.description.as_ref().map(|desc| view! {
                                    <p class="text-slate-300">{desc.clone()}</p>
                                })}
                            </div>

                            <Show
                                when=move || cards_resource.get().is_some()
                                fallback=move || view! { <p class="text-sm text-slate-400">"Loading cards..."</p> }
                            >
                                {move || match cards_resource.get() {
                                    Some(Ok(cards)) if cards.is_empty() => view! {
                                        <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-12 text-center">
                                            <p class="text-slate-400">"No flashcards in this deck yet."</p>
                                        </div>
                                    }.into_any(),
                                    Some(Ok(cards)) => {
                                        let total_cards = cards.len();
                                        let cards_clone = cards.clone();
                                        view! {
                                            <div class="space-y-6">
                                                <div class="flex items-center justify-between text-sm text-slate-400">
                                                    <span>{move || format!("Card {} of {}", current_card_index.get() + 1, total_cards)}</span>
                                                    <span>{format!("{} cards total", total_cards)}</span>
                                                </div>

                                                <FlashcardViewer
                                                    card=Signal::derive(move || cards_clone.get(current_card_index.get()).cloned())
                                                    show_answer=show_answer.read_only()
                                                    on_toggle=toggle_answer
                                                />

                                                <div class="flex items-center justify-between gap-4">
                                                    <button
                                                        class="inline-flex items-center rounded-full border border-slate-700 px-6 py-2 text-sm font-semibold text-slate-300 hover:border-slate-400 disabled:opacity-50 disabled:cursor-not-allowed"
                                                        on:click=move |_| prev_card()
                                                        disabled=move || current_card_index.get() == 0
                                                    >
                                                        "← Previous"
                                                    </button>

                                                    <button
                                                        class="inline-flex items-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950"
                                                        on:click=move |_| toggle_answer()
                                                    >
                                                        {move || if show_answer.get() { "Hide Answer" } else { "Show Answer" }}
                                                    </button>

                                                    <button
                                                        class="inline-flex items-center rounded-full border border-slate-700 px-6 py-2 text-sm font-semibold text-slate-300 hover:border-slate-400 disabled:opacity-50 disabled:cursor-not-allowed"
                                                        on:click=move |_| next_card()
                                                        disabled={
                                                            let total = total_cards;
                                                            move || current_card_index.get() >= total - 1
                                                        }
                                                    >
                                                        "Next →"
                                                    </button>
                                                </div>
                                            </div>
                                        }.into_any()
                                    },
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
        </section>
    }
}

#[component]
fn FlashcardViewer(
    card: Signal<Option<Flashcard>>,
    show_answer: ReadSignal<bool>,
    on_toggle: impl Fn() + 'static + Copy + Send + Sync,
) -> impl IntoView {
    // Fetch file name if card has a file_id
    let file_name_resource = LocalResource::new(move || async move {
        if let Some(c) = card.get() {
            if let Some(file_id) = c.file_id {
                get_file_name(file_id).await.ok()
            } else {
                None
            }
        } else {
            None
        }
    });

    // Refetch when card changes
    Effect::new(move |_| {
        let _ = card.get();
        file_name_resource.refetch();
    });

    // MathJax rendering effect
    Effect::new(move |_| {
        let _ = show_answer.get();
        let _ = card.get();

        // Trigger MathJax re-render
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::prelude::*;

            #[wasm_bindgen]
            extern "C" {
                #[wasm_bindgen(js_namespace = ["window", "MathJax"], js_name = typesetPromise)]
                fn mathjax_typeset() -> js_sys::Promise;
            }

            // Small delay to ensure DOM is updated
            let _ = leptos::task::spawn_local(async {
                gloo_timers::future::TimeoutFuture::new(50).await;
                let _ = mathjax_typeset();
            });
        }
    });

    let front_html = Signal::derive(move || {
        card.get()
            .map(|c| markdown_to_html(&c.front))
            .unwrap_or_default()
    });

    let back_html = Signal::derive(move || {
        card.get()
            .map(|c| markdown_to_html(&c.back))
            .unwrap_or_default()
    });

    view! {
        <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-8">
            {move || match card.get() {
                Some(card) => {
                    view! {
                        <div class="space-y-8">
                            <div class="min-h-[200px]">
                                <h3 class="mb-4 text-sm font-semibold uppercase tracking-wider text-slate-500">"Question"</h3>
                                <div
                                    class="mathjax-content text-lg text-white prose prose-invert max-w-none"
                                    inner_html=move || front_html.get()
                                ></div>
                            </div>

                            <Show when=move || show_answer.get()>
                                <div class="border-t border-slate-800 pt-8">
                                    <h3 class="mb-4 text-sm font-semibold uppercase tracking-wider text-slate-500">"Answer"</h3>
                                    <div
                                        class="mathjax-content text-base text-slate-200 prose prose-invert max-w-none"
                                        inner_html=move || back_html.get()
                                    ></div>

                                    <div class="mt-4 space-y-1 text-xs text-slate-500">
                                        {card.document_reference.as_ref().map(|doc_ref| view! {
                                            <div>"Reference: " {doc_ref.clone()}</div>
                                        })}
                                        {move || file_name_resource.get().flatten().map(|name| view! {
                                            <div>"Source: " {name}</div>
                                        })}
                                    </div>
                                </div>
                            </Show>

                            <Show when=move || !show_answer.get()>
                                <button
                                    class="w-full rounded-xl border border-dashed border-slate-700 py-8 text-slate-500 hover:border-slate-500 hover:text-slate-400"
                                    on:click=move |_| on_toggle()
                                >
                                    "Click to reveal answer"
                                </button>
                            </Show>
                        </div>
                    }.into_any()
                },
                None => view! {
                    <p class="text-center text-slate-400">"No card selected"</p>
                }.into_any(),
            }}
        </div>
    }
}
