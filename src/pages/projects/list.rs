use leptos::prelude::*;

use crate::features::{
    auth::models::UserSession,
    projects::handlers::{list_projects, CreateProject},
};

#[component]
pub fn ProjectsPage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());

    let create_action = ServerAction::<CreateProject>::new();
    let projects_resource = LocalResource::new(move || async move { list_projects().await });
    let show_create_modal = RwSignal::new(false);

    Effect::new(move |_| {
        if let Some(Ok(_)) = create_action.value().get() {
            projects_resource.refetch();
            show_create_modal.set(false);
        }
    });

    view! {
        <>
        <section class="mx-auto flex min-h-screen max-w-5xl flex-col gap-10 px-6 py-16">
            <div class="space-y-3">
                <p class="text-sm uppercase tracking-[0.3em] text-slate-400">"Workspace"</p>
                <h1 class="text-4xl font-semibold text-white">"Study projects"</h1>
                <p class="text-slate-300">"Upload lecture PDFs and turn them into study-ready material."</p>
            </div>
            <a class="text-sm text-slate-400 hover:text-white" href="/">"← Back to home"</a>

            <Show
                when=move || user.get().is_some()
                fallback=move || view! {
                    <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-6 text-slate-300">
                        "Please log in to create and manage projects."
                    </div>
                }
            >
                <div class="mx-auto w-full max-w-3xl">
                    <div class="mb-6 flex items-center justify-between">
                        <h2 class="text-lg font-semibold text-white">"Your projects"</h2>
                        <button
                            class="inline-flex items-center rounded-full border border-slate-700 bg-slate-900/50 px-4 py-2 text-sm text-slate-300 hover:border-slate-400 hover:bg-slate-900"
                            on:click=move |_| show_create_modal.set(true)
                        >
                            "+ New project"
                        </button>
                    </div>

                    <Show
                        when=move || projects_resource.get().is_some()
                        fallback=move || view! { <p class="text-sm text-slate-400">"Loading projects..."</p> }
                    >
                        {move || -> AnyView { match projects_resource.get() {
                            Some(Ok(projects)) if projects.is_empty() => view! {
                                <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-12 text-center">
                                    <p class="text-sm text-slate-400">"No projects yet."</p>
                                    <p class="mt-2 text-xs text-slate-500">"Click the '+ New project' button to get started."</p>
                                </div>
                            }.into_any(),
                            Some(Ok(projects)) => view! {
                                <ul class="space-y-3">
                                    {projects.into_iter().map(|project| view! {
                                        <li class="rounded-xl border border-slate-800 bg-slate-900/40 p-5 transition hover:border-slate-700 hover:bg-slate-900/70">
                                            <div class="flex items-start justify-between gap-3">
                                                <div class="flex-1">
                                                    <a class="text-base font-semibold text-white hover:text-white/80" href=format!("/projects/{}", project.id)>
                                                        {project.name}
                                                    </a>
                                                    {project.description.as_ref().map(|desc| view! {
                                                        <p class="mt-1 text-sm text-slate-400">{desc.clone()}</p>
                                                    })}
                                                    <p class="mt-2 text-xs text-slate-500">
                                                        {format!("Created {}", project.created_at)}
                                                    </p>
                                                </div>
                                                <span class="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-300">
                                                    {format!("{} files", project.file_count)}
                                                </span>
                                            </div>
                                        </li>
                                    }).collect_view()}
                                </ul>
                            }.into_any(),
                            Some(Err(err)) => view! {
                                <p class="text-sm text-rose-300">{err.to_string()}</p>
                            }.into_any(),
                            None => view! { <> </> }.into_any(),
                        }}}
                    </Show>
                </div>
            </Show>
        </section>

        <Show when=move || show_create_modal.get()>
            <div
                class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
                on:click=move |_| show_create_modal.set(false)
            >
                <div
                    class="w-full max-w-md rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl"
                    on:click=move |ev| ev.stop_propagation()
                >
                    <div class="flex items-center justify-between border-b border-slate-800 px-6 py-4">
                        <h3 class="text-lg font-semibold text-white">"Create new project"</h3>
                        <button
                            class="rounded-full border border-slate-700 px-4 py-1 text-xs text-slate-300 hover:border-slate-400"
                            on:click=move |_| show_create_modal.set(false)
                        >
                            "Close"
                        </button>
                    </div>
                    <div class="px-6 py-5">
                        <p class="mb-5 text-sm text-slate-400">"Keep one project per class or semester."</p>
                        <ActionForm action=create_action>
                            <div class="space-y-4">
                                <label class="flex flex-col gap-2 text-sm text-slate-300">
                                    "Project name"
                                    <input
                                        class="rounded-xl border border-slate-700 bg-slate-900 px-4 py-2 text-slate-100 focus:border-slate-500 focus:outline-none"
                                        type="text"
                                        name="name"
                                        required
                                        minlength="3"
                                        placeholder="e.g., Computer Science 101"
                                    />
                                </label>
                                <label class="flex flex-col gap-2 text-sm text-slate-300">
                                    "Description (optional)"
                                    <textarea
                                        class="min-h-[96px] rounded-xl border border-slate-700 bg-slate-900 px-4 py-2 text-slate-100 focus:border-slate-500 focus:outline-none"
                                        name="description"
                                        placeholder="Optional notes about this project..."
                                    ></textarea>
                                </label>
                                {move || create_action
                                    .value()
                                    .get()
                                    .and_then(|value| value.err())
                                    .map(|err| view! {
                                        <p class="text-sm text-rose-300">{err.to_string()}</p>
                                    })}
                                <div class="flex justify-end gap-3 pt-2">
                                    <button
                                        class="inline-flex items-center rounded-full border border-slate-700 px-5 py-2 text-sm text-slate-300 hover:border-slate-400"
                                        type="button"
                                        on:click=move |_| show_create_modal.set(false)
                                    >
                                        "Cancel"
                                    </button>
                                    <button
                                        class="inline-flex items-center rounded-full bg-white px-5 py-2 text-sm font-semibold text-slate-950 hover:bg-white/90"
                                        type="submit"
                                    >
                                        "Create project"
                                    </button>
                                </div>
                            </div>
                        </ActionForm>
                    </div>
                </div>
            </div>
        </Show>
        </>
    }
}
