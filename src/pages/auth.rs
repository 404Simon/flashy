use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params_map};

use crate::features::auth::{
    handlers::{LoginUser, RegisterUser},
    models::UserSession,
};

#[component]
pub fn LoginPage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let login_action = ServerAction::<LoginUser>::new();
    let navigate = use_navigate();

    Effect::new(move |_| {
        if let Some(Ok(_)) = login_action.value().get() {
            user_resource.refetch();
            navigate("/", Default::default());
        }
    });

    let error = move || {
        login_action
            .value()
            .get()
            .and_then(|res| res.err())
            .map(|e| match e {
                ServerFnError::ServerError(msg) => msg,
                other => other.to_string(),
            })
    };

    view! {
        <section class="mx-auto flex min-h-screen max-w-3xl flex-col justify-center gap-8 px-6 py-16">
            <div class="space-y-3">
                <p class="text-sm uppercase tracking-[0.3em] text-slate-400">"Welcome back"</p>
                <h1 class="text-4xl font-semibold text-white">"Login"</h1>
                <p class="text-slate-300">"Your deck progress waits on the other side."</p>
            </div>
            <div class="space-y-4 rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                <ActionForm action=login_action>
                    <div class="space-y-4">
                        <Show when=move || error().is_some()>
                            <p class="rounded-xl border border-red-800 bg-red-900/50 px-4 py-2 text-sm text-red-300">
                                {error().unwrap_or_default()}
                            </p>
                        </Show>
                        <label class="flex flex-col gap-2 text-sm text-slate-300">
                            "Username"
                            <input class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100" type="text" name="username" required/>
                        </label>
                        <label class="flex flex-col gap-2 text-sm text-slate-300">
                            "Password"
                            <input class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100" type="password" name="password" required/>
                        </label>
                        <div class="pt-2">
                            <button class="inline-flex items-center justify-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950" type="submit">"Login"</button>
                        </div>
                    </div>
                </ActionForm>
            </div>
            <p class="text-sm text-slate-400">"Need an invite? Ask your admin for a link."</p>
        </section>
    }
}

#[component]
pub fn RegisterPage() -> impl IntoView {
    let params = use_params_map();
    let token = move || params.with(|p| p.get("token").unwrap_or_default());
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let register_action = ServerAction::<RegisterUser>::new();
    let navigate = use_navigate();

    Effect::new(move |_| {
        if let Some(Ok(_)) = register_action.value().get() {
            user_resource.refetch();
            navigate("/", Default::default());
        }
    });

    view! {
        <section class="mx-auto flex min-h-screen max-w-3xl flex-col justify-center gap-8 px-6 py-16">
            <div class="space-y-3">
                <p class="text-sm uppercase tracking-[0.3em] text-slate-400">"Invite accepted"</p>
                <h1 class="text-4xl font-semibold text-white">"Create your account"</h1>
                <p class="text-slate-300">"This invite unlocks Flashy for you."</p>
            </div>
            <div class="space-y-4 rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                <ActionForm action=register_action>
                    <div class="space-y-4">
                        <input type="hidden" name="invite_token" value=token/>
                        <label class="flex flex-col gap-2 text-sm text-slate-300">
                            "Username"
                            <input class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100" type="text" name="username" required/>
                        </label>
                        <label class="flex flex-col gap-2 text-sm text-slate-300">
                            "Email (optional)"
                            <input class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100" type="email" name="email"/>
                        </label>
                        <label class="flex flex-col gap-2 text-sm text-slate-300">
                            "Password"
                            <input class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100" type="password" name="password" required minlength="8"/>
                        </label>
                        <div class="pt-2">
                            <button class="inline-flex items-center justify-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950" type="submit">"Create account"</button>
                        </div>
                    </div>
                </ActionForm>
            </div>
        </section>
    }
}
