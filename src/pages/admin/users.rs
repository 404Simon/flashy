use leptos::prelude::*;

use crate::{
    features::auth::admin_handlers::{list_users, DeleteUser, ToggleUserAdmin},
    pages::admin::layout::AdminLayout,
};

#[component]
pub fn AdminUsersPage() -> impl IntoView {
    let users_resource = LocalResource::new(move || async move { list_users().await });
    let toggle_admin_action = ServerAction::<ToggleUserAdmin>::new();
    let delete_user_action = ServerAction::<DeleteUser>::new();

    // Refetch users when actions complete
    Effect::new(move |_| {
        if toggle_admin_action.value().get().is_some() {
            users_resource.refetch();
        }
    });

    Effect::new(move |_| {
        if delete_user_action.value().get().is_some() {
            users_resource.refetch();
        }
    });

    view! {
        <AdminLayout>
            <div class="space-y-6">
                <div>
                    <h2 class="text-2xl font-semibold text-white mb-2">"Manage Users"</h2>
                    <p class="text-slate-400 text-sm">"View and manage user accounts and permissions"</p>
                </div>

                <Show
                    when=move || users_resource.get().is_some()
                    fallback=move || view! {
                        <div class="rounded-xl border border-slate-800 bg-slate-900/40 px-4 py-8 text-center">
                            <p class="text-slate-400">"Loading users..."</p>
                        </div>
                    }
                >
                    {move || -> AnyView { match users_resource.get() {
                        Some(Ok(users)) => {
                            if users.is_empty() {
                                view! {
                                    <div class="rounded-xl border border-slate-800 bg-slate-900/40 px-4 py-8 text-center">
                                        <p class="text-slate-400">"No users found"</p>
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="space-y-3">
                                        {users.into_iter().map(|user| {
                                            let user_id = user.id;
                                            let is_admin = user.is_admin;
                                            let username = user.username.clone();

                                            view! {
                                                <div class="rounded-xl border border-slate-800 bg-slate-900/40 p-4 hover:bg-slate-900/60 transition-colors">
                                                    <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                                                        <div class="flex-1 min-w-0">
                                                            <div class="flex items-center gap-2 mb-1">
                                                                <span class="font-medium text-slate-100">{user.username}</span>
                                                                {is_admin.then(|| view! {
                                                                    <span class="px-2 py-0.5 bg-indigo-900/30 text-indigo-300 text-xs font-medium rounded border border-indigo-800">"Admin"</span>
                                                                })}
                                                            </div>
                                                            <div class="text-xs text-slate-400 space-y-0.5">
                                                                {user.email.as_ref().map(|e| view! {
                                                                    <div>{e.clone()}</div>
                                                                })}
                                                                <div>{format!("Joined: {}", user.created_at)}</div>
                                                            </div>
                                                        </div>
                                                        <div class="flex gap-2">
                                                            <ActionForm action=toggle_admin_action>
                                                                <input type="hidden" name="user_id" value=user_id/>
                                                                <button
                                                                    type="submit"
                                                                    class="text-xs px-3 py-1.5 rounded-lg border border-slate-700 text-slate-300 hover:border-indigo-600 hover:text-indigo-400 transition-colors"
                                                                >
                                                                    {if is_admin { "Remove admin" } else { "Make admin" }}
                                                                </button>
                                                            </ActionForm>
                                                            <ActionForm action=delete_user_action>
                                                                <input type="hidden" name="user_id" value=user_id/>
                                                                <button
                                                                    type="submit"
                                                                    class="text-xs px-3 py-1.5 rounded-lg border border-slate-700 text-slate-300 hover:border-red-600 hover:text-red-400 transition-colors"
                                                                    on:click=move |e| {
                                                                        if !window().confirm_with_message(&format!("Are you sure you want to delete user '{}'?", username)).unwrap_or(false) {
                                                                            e.prevent_default();
                                                                        }
                                                                    }
                                                                >
                                                                    "Delete"
                                                                </button>
                                                            </ActionForm>
                                                        </div>
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
                                <p class="text-red-400 text-sm">{format!("Error loading users: {}", e)}</p>
                            </div>
                        }.into_any(),
                        None => ().into_any(),
                    }}}
                </Show>

                <Show when=move || {
                    toggle_admin_action.value().get().and_then(|r| r.err()).is_some()
                }>
                    {move || {
                        toggle_admin_action.value().get().and_then(|r| r.err()).map(|e| view! {
                            <div class="rounded-xl border border-red-900/50 bg-red-950/20 px-4 py-3">
                                <p class="text-red-400 text-sm">{e.to_string()}</p>
                            </div>
                        })
                    }}
                </Show>

                <Show when=move || {
                    delete_user_action.value().get().and_then(|r| r.err()).is_some()
                }>
                    {move || {
                        delete_user_action.value().get().and_then(|r| r.err()).map(|e| view! {
                            <div class="rounded-xl border border-red-900/50 bg-red-950/20 px-4 py-3">
                                <p class="text-red-400 text-sm">{e.to_string()}</p>
                            </div>
                        })
                    }}
                </Show>
            </div>
        </AdminLayout>
    }
}
