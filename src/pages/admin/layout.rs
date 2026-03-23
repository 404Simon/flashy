use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::features::auth::models::UserSession;

#[component]
pub fn AdminLayout(children: ChildrenFn) -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());
    let location = use_location();
    let current_path = move || location.pathname.get();

    view! {
        <section class="mx-auto min-h-screen max-w-6xl px-6 py-16">
            <div class="space-y-2 mb-8">
                <p class="text-sm uppercase tracking-[0.3em] text-slate-400">"Admin"</p>
                <h1 class="text-4xl font-semibold text-white">"Administration"</h1>
            </div>

            <Show when=move || user.get().map(|u| u.is_admin).unwrap_or(false) fallback=move || {
                view! {
                    <div class="space-y-4">
                        <p class="text-slate-300">"You are not authorized to view this page."</p>
                        <a class="inline-flex items-center rounded-full border border-slate-700 px-5 py-2 text-sm hover:border-slate-400" href="/">"← Back to home"</a>
                    </div>
                }
            }>
                <a class="inline-flex items-center text-sm text-slate-400 hover:text-white mb-6 transition-colors" href="/">"← Back to home"</a>

                <nav class="mb-8 flex flex-wrap gap-2 border-b border-slate-800 pb-4">
                    <a
                        href="/admin/invites"
                        class=move || {
                            let base = "px-4 py-2 text-sm rounded-lg transition-colors";
                            if current_path() == "/admin/invites" {
                                format!("{} bg-slate-800 text-white", base)
                            } else {
                                format!("{} text-slate-400 hover:text-white hover:bg-slate-900/50", base)
                            }
                        }
                    >
                        "Invites"
                    </a>
                    <a
                        href="/admin/users"
                        class=move || {
                            let base = "px-4 py-2 text-sm rounded-lg transition-colors";
                            if current_path() == "/admin/users" {
                                format!("{} bg-slate-800 text-white", base)
                            } else {
                                format!("{} text-slate-400 hover:text-white hover:bg-slate-900/50", base)
                            }
                        }
                    >
                        "Users"
                    </a>
                </nav>

                <div>{children()}</div>
            </Show>
        </section>
    }
}
