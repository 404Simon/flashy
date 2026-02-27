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

    Effect::new(move |_| {
        if let Some(Ok(_)) = create_action.value().get() {
            projects_resource.refetch();
        }
    });

    view! {
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
                <div class="grid gap-6 lg:grid-cols-[1.2fr_1fr]">
                    <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                        <h2 class="text-lg font-semibold text-white">"New project"</h2>
                        <p class="mt-1 text-sm text-slate-400">"Keep one project per class or semester."</p>
                        <div class="mt-5 space-y-4">
                            <ActionForm action=create_action>
                                <div class="space-y-4">
                                    <label class="flex flex-col gap-2 text-sm text-slate-300">
                                        "Project name"
                                        <input class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100" type="text" name="name" required minlength="3"/>
                                    </label>
                                    <label class="flex flex-col gap-2 text-sm text-slate-300">
                                        "Description (optional)"
                                        <textarea class="min-h-[96px] rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100" name="description"></textarea>
                                    </label>
                                    <div class="pt-1">
                                        <button class="inline-flex items-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950" type="submit">"Create project"</button>
                                    </div>
                                </div>
                            </ActionForm>
                            {move || create_action
                                .value()
                                .get()
                                .and_then(|value| value.err())
                                .map(|err| view! {
                                    <p class="text-sm text-rose-300">{err.to_string()}</p>
                                })}
                        </div>
                    </div>

                    <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                        <h2 class="text-lg font-semibold text-white">"Project library"</h2>
                        <Show
                            when=move || projects_resource.get().is_some()
                            fallback=move || view! { <p class="mt-4 text-sm text-slate-400">"Loading projects..."</p> }
                        >
                            {move || -> AnyView { match projects_resource.get() {
                                Some(Ok(projects)) if projects.is_empty() => view! {
                                    <> <p class="mt-4 text-sm text-slate-400">"No projects yet."</p> </>
                                }.into_any(),
                                Some(Ok(projects)) => view! {
                                    <>
                                        <ul class="mt-4 space-y-3">
                                            {projects.into_iter().map(|project| view! {
                                                <li class="rounded-xl border border-slate-800 bg-slate-900/40 p-4">
                                                    <div class="flex items-start justify-between gap-3">
                                                        <div>
                                                            <a class="text-base font-semibold text-white hover:text-white/80" href=format!("/projects/{}", project.id)>
                                                                {project.name}
                                                            </a>
                                                            {project.description.as_ref().map(|desc| view! {
                                                                <p class="mt-1 text-sm text-slate-400">{desc.clone()}</p>
                                                            })}
                                                        </div>
                                                        <span class="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-300">
                                                            {format!("{} files", project.file_count)}
                                                        </span>
                                                    </div>
                                                    <p class="mt-3 text-xs text-slate-500">
                                                        {format!("Created {}", project.created_at)}
                                                    </p>
                                                </li>
                                            }).collect_view()}
                                        </ul>
                                    </>
                                }.into_any(),
                                Some(Err(err)) => view! {
                                    <> <p class="mt-4 text-sm text-rose-300">{err.to_string()}</p> </>
                                }.into_any(),
                                None => view! { <> </> }.into_any(),
                            }}}
                        </Show>
                    </div>
                </div>
            </Show>
        </section>
    }
}
