use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::features::{
    auth::models::UserSession,
    invites::handlers::{list_invites, CreateInvite},
};

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

#[component]
pub fn AdminInvitesPage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());

    let create_action = ServerAction::<CreateInvite>::new();
    let invites_resource = LocalResource::new(move || async move { list_invites().await });

    Effect::new(move |_| {
        if let Some(Ok(_)) = create_action.value().get() {
            invites_resource.refetch();
        }
    });

    view! {
        <section class="mx-auto flex min-h-screen max-w-4xl flex-col justify-center gap-6 px-6 py-16">
            <div class="space-y-2">
                <p class="text-sm uppercase tracking-[0.3em] text-slate-400">"Admin"</p>
                <h1 class="text-4xl font-semibold text-white">"Invites"</h1>
            </div>
            <a class="text-sm text-slate-400 hover:text-white" href="/">"← Back to home"</a>
            <Show when=move || user.get().map(|u| u.is_admin).unwrap_or(false) fallback=move || {
                view! { <p>"You are not authorized to view this page."</p> }
            }>
                <div class="space-y-4 rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <ActionForm action=create_action>
                        <div class="space-y-4">
                            <label class="flex items-center gap-3 text-sm text-slate-300">
                                "Reusable"
                                <input type="checkbox" name="is_reusable" value="true"/>
                            </label>
                            <label class="flex flex-col gap-2 text-sm text-slate-300">
                                "Duration (days, 1-30)"
                                <input class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100" type="number" name="duration_days" min="1" max="30" value="7"/>
                            </label>
                            <div class="pt-2">
                                <button class="inline-flex w-fit items-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950" type="submit">"Create invite"</button>
                            </div>
                        </div>
                    </ActionForm>
                </div>
                <h2 class="text-lg font-semibold text-white">"Existing invites"</h2>
                <Show
                    when=move || invites_resource.get().is_some()
                    fallback=move || view! { <p>"Loading..."</p> }
                >
                    {move || match invites_resource.get() {
                        Some(Ok(invites)) => view! {
                            <div>
                                <ul class="space-y-2 text-sm text-slate-300">
                                    {invites.into_iter().map(|invite| view! {
                                        <li class="rounded-xl border border-slate-800 bg-slate-900/40 px-4 py-3">
                                            <div class="font-mono text-slate-100">{format!("/invite/{}", invite.token)}</div>
                                            <div class="text-xs text-slate-400">
                                                {format!("{} days · uses {}", invite.duration_days, invite.uses)}
                                                {if invite.is_reusable { " · reusable" } else { " · single-use" }}
                                            </div>
                                        </li>
                                    }).collect_view()}
                                </ul>
                            </div>
                        }.into_view(),
                        Some(Err(e)) => view! { <div><p>{e.to_string()}</p></div> }.into_view(),
                        None => view! { <div><span></span></div> }.into_view(),
                    }}
                </Show>
            </Show>
        </section>
    }
}
