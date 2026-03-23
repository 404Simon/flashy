use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[component]
pub fn InvitePage() -> impl IntoView {
    let params = use_params_map();
    let token = move || params.with(|p| p.get("token").unwrap_or_default());

    view! {
        <section class="mx-auto flex min-h-screen max-w-3xl flex-col justify-center gap-6 px-6 py-16">
            <div class="space-y-3">
                <p class="text-sm uppercase tracking-[0.3em] text-slate-400">"Invitation"</p>
                <h1 class="text-4xl font-semibold text-white">"You are invited to Flashy"</h1>
                <p class="text-slate-300">"Tap below to create your account."</p>
            </div>
            <a class="inline-flex w-fit items-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950" href=move || format!("/register/{}", token())>
                "Continue to registration"
            </a>
        </section>
    }
}
