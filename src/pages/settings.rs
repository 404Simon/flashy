use leptos::prelude::*;

use crate::features::auth::handlers::{ChangePassword, RevokeConnectedApplication};

#[component]
pub fn SettingsPage() -> impl IntoView {
    let change_password_action = ServerAction::<ChangePassword>::new();
    let (success_message, set_success_message) = signal(Option::<String>::None);
    let connected_apps = LocalResource::new(|| async {
        crate::features::auth::handlers::list_connected_applications().await
    });
    let revoke_action = ServerAction::<RevokeConnectedApplication>::new();

    Effect::new(move |_| {
        if let Some(Ok(_)) = change_password_action.value().get() {
            set_success_message.set(Some("Password changed successfully".to_string()));
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(())) = revoke_action.value().get() {
            connected_apps.refetch();
        }
    });

    view! {
        <section class="mx-auto flex min-h-screen max-w-3xl flex-col gap-8 px-6 py-16">
            <div class="space-y-3">
                <p class="text-sm uppercase tracking-[0.3em] text-slate-400">"Account"</p>
                <h1 class="text-4xl font-semibold text-white">"Settings"</h1>
                <p class="text-slate-300">"Manage your account preferences"</p>
            </div>
            <a class="text-sm text-slate-400 hover:text-white" href="/">"← Back to home"</a>

            <div class="space-y-6">
                <div class="space-y-4 rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <div class="space-y-2">
                        <h2 class="text-xl font-semibold text-white">"Change Password"</h2>
                        <p class="text-sm text-slate-400">"Update your password to keep your account secure"</p>
                    </div>

                    {move || {
                        success_message
                            .get()
                            .map(|msg| {
                                view! {
                                    <div class="rounded-xl border border-green-800 bg-green-950/30 px-4 py-3 text-sm text-green-400">
                                        {msg}
                                    </div>
                                }
                            })
                    }}

                    {move || {
                        change_password_action
                            .value()
                            .get()
                            .and_then(|v| v.err())
                            .map(|err| {
                                view! {
                                    <div class="rounded-xl border border-red-800 bg-red-950/30 px-4 py-3 text-sm text-red-400">
                                        {err.to_string()}
                                    </div>
                                }
                            })
                    }}

                    <ActionForm
                        action=change_password_action
                        on:submit=move |_| {
                            set_success_message.set(None);
                        }
                    >

                        <div class="space-y-4">
                            <label class="flex flex-col gap-2 text-sm text-slate-300">
                                "Current Password"
                                <input
                                    class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100"
                                    type="password"
                                    name="current_password"
                                    required
                                    autocomplete="current-password"
                                />
                            </label>
                            <label class="flex flex-col gap-2 text-sm text-slate-300">
                                "New Password"
                                <input
                                    class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100"
                                    type="password"
                                    name="new_password"
                                    required
                                    minlength="8"
                                    autocomplete="new-password"
                                />
                            </label>
                            <p class="text-xs text-slate-500">"Minimum 8 characters required"</p>
                            <div class="pt-2">
                                <button
                                    class="inline-flex items-center justify-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950 transition-opacity hover:opacity-90 disabled:opacity-50"
                                    type="submit"
                                    disabled=move || change_password_action.pending().get()
                                >
                                    {move || {
                                        if change_password_action.pending().get() {
                                            "Changing..."
                                        } else {
                                            "Change Password"
                                        }
                                    }}

                                </button>
                            </div>
                        </div>
                    </ActionForm>
                </div>
                <div class="space-y-4 rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <div class="space-y-2">
                        <h2 class="text-xl font-semibold text-white">"Connected applications"</h2>
                        <p class="text-sm text-slate-400">"Applications with read-only access to your Flashy content."</p>
                    </div>
                    <Suspense fallback=move || view! { <p class="text-sm text-slate-400">"Loading…"</p> }>
                        {move || Suspend::new(async move {
                            match connected_apps.await {
                                Ok(apps) if apps.is_empty() => view! { <p class="text-sm text-slate-400">"No connected applications."</p> }.into_any(),
                                Ok(apps) => view! { <ul class="divide-y divide-slate-800">{apps.into_iter().map(|app| view! {
                                    <li class="flex items-center justify-between gap-4 py-4">
                                        <div><p class="font-medium text-white">{app.display_name}</p><p class="max-w-md truncate text-xs text-slate-500">{app.client_id.clone()}</p></div>
                                        <ActionForm action=revoke_action><input type="hidden" name="client_id" value=app.client_id/><button class="rounded-full border border-red-800 px-4 py-2 text-sm text-red-300" type="submit">"Disconnect"</button></ActionForm>
                                    </li>
                                }).collect_view()}</ul> }.into_any(),
                                Err(_) => view! { <p class="text-sm text-red-400">"Could not load connected applications."</p> }.into_any(),
                            }
                        })}
                    </Suspense>
                </div>
            </div>
        </section>
    }
}
