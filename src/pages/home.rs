use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::features::auth::{handlers::LogoutUser, models::UserSession};

/// Renders the home page of the application.
#[component]
pub fn HomePage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());
    let logout_action = ServerAction::<LogoutUser>::new();
    let navigate = use_navigate();

    Effect::new(move |_| {
        if let Some(Ok(())) = logout_action.value().get() {
            user_resource.refetch();
            navigate("/login", Default::default());
        }
    });

    view! {
        <section class="mx-auto flex min-h-screen max-w-5xl flex-col justify-center gap-10 px-6 py-16">
            <header class="space-y-4">
                <p class="text-sm uppercase tracking-[0.3em] text-slate-400">"Study, distilled"</p>
                <h1 class="text-5xl font-semibold leading-tight text-white md:text-6xl">
                    "Flashy turns lecture slides into sharp, recall-ready cards."
                </h1>
                <p class="max-w-2xl text-lg text-slate-300">
                    "Create decks from your class PDFs, quiz yourself, and track what sticks."
                </p>
            </header>
            <Show
                when=move || user.get().is_some()
                fallback=move || view! {
                    <div class="flex flex-col gap-3 rounded-2xl border border-slate-800 bg-slate-900/40 p-6">
                        <p class="text-slate-300">"You are not logged in."</p>
                        <div class="flex flex-wrap gap-3">
                            <a class="inline-flex items-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950" href="/login">"Login"</a>
                            <span class="text-xs text-slate-400">"Invite required to register."</span>
                        </div>
                    </div>
                }
            >
                {move || {
                    let user = user.get().unwrap();
                    view! {
                        <div class="flex flex-wrap items-center gap-4 rounded-2xl border border-slate-800 bg-slate-900/40 p-6">
                            <p class="text-slate-300">{format!("Logged in as {}", user.username)}</p>
                            <Show when=move || user.is_admin>
                                <a class="rounded-full border border-slate-700 px-5 py-2 text-sm hover:border-slate-400" href="/admin/invites">"Manage invites"</a>
                            </Show>
                            <a class="rounded-full border border-slate-700 px-5 py-2 text-sm hover:border-slate-400" href="/projects">"My projects"</a>
                            <button
                                class="rounded-full border border-slate-700 px-5 py-2 text-sm hover:border-slate-400"
                                on:click=move |_| {
                                    logout_action.dispatch(LogoutUser {});
                                }
                            >
                                "Logout"
                            </button>
                        </div>
                    }
                }}
            </Show>
        </section>
    }
}
