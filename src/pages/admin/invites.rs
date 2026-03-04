use leptos::prelude::*;

use crate::{
    features::invites::handlers::{list_invites, CreateInvite},
    pages::admin::layout::AdminLayout,
};

#[cfg(target_arch = "wasm32")]
use web_sys;

#[component]
pub fn AdminInvitesPage() -> impl IntoView {
    let create_action = ServerAction::<CreateInvite>::new();
    let invites_resource = LocalResource::new(move || async move { list_invites().await });

    Effect::new(move |_| {
        if let Some(Ok(_)) = create_action.value().get() {
            invites_resource.refetch();
        }
    });

    view! {
        <AdminLayout>
            <div class="space-y-6">
                <div>
                    <h2 class="text-2xl font-semibold text-white mb-2">"Manage Invites"</h2>
                    <p class="text-slate-400 text-sm">"Create and manage invitation links for new users"</p>
                </div>

                <div class="space-y-4 rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <h3 class="text-lg font-medium text-white">"Create new invite"</h3>
                    <ActionForm action=create_action>
                        <div class="space-y-4">
                            <label class="flex items-center gap-3 text-sm text-slate-300">
                                <input type="checkbox" name="is_reusable" value="true" class="rounded border-slate-700 bg-slate-950 text-white focus:ring-2 focus:ring-slate-500"/>
                                "Reusable (can be used multiple times)"
                            </label>
                            <label class="flex flex-col gap-2 text-sm text-slate-300">
                                "Duration (days)"
                                <input
                                    class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100 focus:border-slate-500 focus:outline-none focus:ring-2 focus:ring-slate-500"
                                    type="number"
                                    name="duration_days"
                                    min="1"
                                    max="30"
                                    value="7"
                                    required
                                />
                                <span class="text-xs text-slate-500">"Between 1 and 30 days"</span>
                            </label>
                            <div class="pt-2">
                                <button
                                    class="inline-flex w-fit items-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950 hover:bg-slate-100 transition-colors"
                                    type="submit"
                                >
                                    "Create invite"
                                </button>
                            </div>
                        </div>
                    </ActionForm>
                </div>

                <div>
                    <h3 class="text-lg font-semibold text-white mb-4">"Active invites"</h3>
                    <Show
                        when=move || invites_resource.get().is_some()
                        fallback=move || view! {
                            <div class="rounded-xl border border-slate-800 bg-slate-900/40 px-4 py-8 text-center">
                                <p class="text-slate-400">"Loading invites..."</p>
                            </div>
                        }
                    >
                        {move || -> AnyView { match invites_resource.get() {
                            Some(Ok(invites)) => {
                                if invites.is_empty() {
                                    view! {
                                        <div class="rounded-xl border border-slate-800 bg-slate-900/40 px-4 py-8 text-center">
                                            <p class="text-slate-400">"No invites created yet"</p>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="space-y-2">
                                            {invites.into_iter().map(|invite| {
                                                let full_url = format!("{}/invite/{}", window().location().origin().unwrap_or_default(), invite.token);
                                                view! {
                                                    <div class="rounded-xl border border-slate-800 bg-slate-900/40 px-4 py-3 hover:bg-slate-900/60 transition-colors">
                                                        <div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                                                            <div class="flex-1 min-w-0">
                                                                <div class="font-mono text-sm text-slate-100 truncate">{full_url.clone()}</div>
                                                                <div class="text-xs text-slate-400 mt-1">
                                                                    {format!("{} days · {} uses", invite.duration_days, invite.uses)}
                                                                    {if invite.is_reusable { " · reusable" } else { " · single-use" }}
                                                                </div>
                                                            </div>
                                                            <button
                                                                class="text-xs px-3 py-1 rounded-lg border border-slate-700 text-slate-300 hover:border-slate-500 hover:text-white transition-colors"
                                                                on:click=move |_| {
                                                                    #[cfg(target_arch = "wasm32")]
                                                                    {
                                                                        if let Some(window) = web_sys::window() {
                                                                            let navigator = window.navigator();
                                                                            let clipboard = navigator.clipboard();
                                                                            let _ = clipboard.write_text(&full_url);
                                                                        }
                                                                    }
                                                                }
                                                            >
                                                                "Copy"
                                                            </button>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }
                            },
                            Some(Err(e)) => view! {
                                <div class="rounded-xl border border-red-900/50 bg-red-950/20 px-4 py-3">
                                    <p class="text-red-400 text-sm">{format!("Error loading invites: {}", e)}</p>
                                </div>
                            }.into_any(),
                            None => ().into_any(),
                        }}}
                    </Show>
                </div>
            </div>
        </AdminLayout>
    }
}
