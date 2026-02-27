use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Link, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::{use_navigate, use_params_map, use_query_map},
    path, StaticSegment,
};

use crate::features::{
    auth::{
        handlers::{get_user, LoginUser, LogoutUser, RegisterUser},
        models::UserSession,
    },
    invites::handlers::{list_invites, CreateInvite},
    projects::handlers::{
        get_project, get_project_file_text, list_project_files, list_projects, CreateProject,
    },
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body class="min-h-screen bg-slate-950 text-slate-100 bg-grid">
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    let user_resource = LocalResource::new(|| async move { get_user().await });
    provide_context(user_resource);

    view! {
        <Link rel="preconnect" href="https://fonts.bunny.net"/>
        <Link href="https://fonts.bunny.net/css?family=manrope:400,500,600,700&display=swap" rel="stylesheet"/>

        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/flashy.css"/>

        // sets the document title
        <Title text="Flashy - AI Flashcards from Slides"/>

        // content for this welcome page
        <Router>
            <main class="min-h-screen">
                <Routes fallback=|| view! {
                    <div class="min-h-screen flex items-center justify-center px-6">
                        <div class="text-center space-y-4">
                            <h1 class="text-4xl font-semibold">"404"</h1>
                            <p class="text-slate-300">"Page not found"</p>
                            <a class="inline-flex items-center rounded-full border border-slate-700 px-5 py-2 text-sm hover:border-slate-400" href="/">"Go home"</a>
                        </div>
                    </div>
                }.into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                    <Route path=StaticSegment("login") view=LoginPage/>
                    <Route path=path!("/register/:token") view=RegisterPage/>
                    <Route path=path!("/invite/:token") view=InvitePage/>
                    <Route path=StaticSegment("projects") view=ProjectsPage/>
                    <Route path=path!("/projects/:id") view=ProjectDetailPage/>
                    <Route path=path!("/admin/invites") view=AdminInvitesPage/>
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
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

#[component]
fn LoginPage() -> impl IntoView {
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
fn RegisterPage() -> impl IntoView {
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

#[component]
fn InvitePage() -> impl IntoView {
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
fn AdminInvitesPage() -> impl IntoView {
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

#[component]
fn ProjectsPage() -> impl IntoView {
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

#[component]
fn ProjectDetailPage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());
    let params = use_params_map();
    let query = use_query_map();
    let project_id = move || params.with(|p| p.get("id").and_then(|id| id.parse::<i64>().ok()));
    let project_id_signal = Signal::derive(move || project_id());

    let project_resource = LocalResource::new(move || {
        let id = project_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid project"))?;
            get_project(id).await
        }
    });

    let files_resource = LocalResource::new(move || {
        let id = project_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid project"))?;
            list_project_files(id).await
        }
    });

    let uploaded = move || query.with(|q| q.get("uploaded").is_some());
    let selected_file_id = RwSignal::new(None::<i64>);
    let pdf_modal_url = RwSignal::new(None::<String>);

    let text_resource = LocalResource::new(move || {
        let file_id = selected_file_id.get();
        async move {
            match file_id {
                Some(id) => get_project_file_text(id).await,
                None => Ok(String::new()),
            }
        }
    });

    let open_text_modal = move |file_id: i64| {
        pdf_modal_url.set(None);
        selected_file_id.set(Some(file_id));
    };

    let open_pdf_modal = move |project_id: i64, file_id: i64| {
        selected_file_id.set(None);
        pdf_modal_url.set(Some(format!(
            "/api/projects/{project_id}/files/{file_id}/pdf"
        )));
    };

    let close_modals = move || {
        selected_file_id.set(None);
        pdf_modal_url.set(None);
    };

    view! {
        <>
        <section class="mx-auto flex min-h-screen max-w-5xl flex-col gap-8 px-6 py-16">
            <Show
                when=move || user.get().is_some()
                fallback=move || view! {
                    <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-6 text-slate-300">
                        "Please log in to view this project."
                    </div>
                }
            >
                <Show
                    when=move || project_resource.get().is_some()
                    fallback=move || view! { <p class="text-sm text-slate-400">"Loading project..."</p> }
                >
                    {move || -> AnyView { match project_resource.get() {
                        Some(Ok(project)) => view! {
                            <div class="space-y-2">
                                <a class="text-sm text-slate-400 hover:text-white" href="/projects">"← Back to projects"</a>
                                <h1 class="text-4xl font-semibold text-white">{project.name}</h1>
                                {project.description.as_ref().map(|desc| view! {
                                    <p class="text-slate-300">{desc.clone()}</p>
                                })}
                            </div>

                            <div class="grid gap-6 lg:grid-cols-[1.1fr_0.9fr]">
                                <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                                    <div class="flex items-center justify-between">
                                        <h2 class="text-lg font-semibold text-white">"Lecture uploads"</h2>
                                        <span class="text-xs text-slate-500">{format!("Created {}", project.created_at)}</span>
                                    </div>
                                    <p class="mt-2 text-sm text-slate-400">
                                        "Upload PDF slides only."
                                    </p>
                                    {move || -> AnyView { if uploaded() {
                                        view! {
                                            <div class="mt-4 rounded-xl border border-emerald-500/40 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-200">
                                                "Upload complete. Text extraction finished."
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div class="hidden"></div> }.into_any()
                                    }}}
                                    <form
                                        class="mt-6 space-y-4"
                                        method="post"
                                        enctype="multipart/form-data"
                                        action=move || project_id_signal
                                            .get()
                                            .map(|id| format!("/api/projects/{id}/upload"))
                                            .unwrap_or_default()
                                    >
                                        <label class="flex flex-col gap-2 text-sm text-slate-300">
                                            "PDF file"
                                            <input
                                                class="rounded-xl border border-slate-700 bg-slate-950 px-4 py-2 text-slate-100"
                                                type="file"
                                                name="file"
                                                accept="application/pdf,.pdf"
                                                required
                                            />
                                        </label>
                                        <div class="pt-1">
                                            <button class="inline-flex items-center rounded-full bg-white px-6 py-2 text-sm font-semibold text-slate-950" type="submit">"Upload slides"</button>
                                        </div>
                                    </form>
                                </div>

                                <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                                    <h2 class="text-lg font-semibold text-white">"Text extraction"</h2>
                                    <p class="mt-2 text-sm text-slate-400">
                                        "We’ll summarize the upload here for future study sessions."
                                    </p>
                                    <Show
                                        when=move || files_resource.get().is_some()
                                        fallback=move || view! { <p class="mt-4 text-sm text-slate-400">"Loading files..."</p> }
                                    >
                                        {move || -> AnyView { match files_resource.get() {
                                            Some(Ok(files)) if files.is_empty() => view! {
                                                <> <p class="mt-4 text-sm text-slate-400">"No PDFs uploaded yet."</p> </>
                                            }.into_any(),
                                            Some(Ok(files)) => view! {
                                                <>
                                                    <ul class="mt-4 space-y-3">
                                                        {files.into_iter().map(|file| {
                                                            let preview = file.text_preview.clone().unwrap_or_default();
                                                            let has_preview = !preview.trim().is_empty();
                                                            view! {
                                                                <li
                                                                    class="rounded-xl border border-slate-800 bg-slate-900/40 p-4 transition hover:border-slate-700 hover:bg-slate-900/70"
                                                                    on:click=move |_| {
                                                                        open_text_modal(file.id);
                                                                    }
                                                                >
                                                                    <div class="flex items-start justify-between gap-3">
                                                                        <div>
                                                                            <p class="text-sm font-semibold text-white">{file.original_filename}</p>
                                                                            <p class="text-xs text-slate-500">{format!("{} • {}", format_bytes(file.file_size), file.created_at)}</p>
                                                                        </div>
                                                                        <button
                                                                            class="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-300 hover:border-slate-400"
                                                                            on:click=move |ev| {
                                                                                ev.stop_propagation();
                                                                                if let Some(project_id) = project_id_signal.get() {
                                                                                    open_pdf_modal(project_id, file.id);
                                                                                }
                                                                            }
                                                                        >
                                                                            "PDF"
                                                                        </button>
                                                                    </div>
                                                                    {if has_preview {
                                                                        view! {
                                                                            <p class="mt-3 max-h-32 overflow-hidden text-xs text-slate-400 whitespace-pre-line">{preview}</p>
                                                                        }
                                                                    } else {
                                                                        view! {
                                                                            <p class="mt-3 text-xs text-slate-500">"No extractable text found."</p>
                                                                        }
                                                                    }}
                                                                </li>
                                                            }
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
                        }.into_any(),
                        Some(Err(err)) => view! { <> <p class="text-sm text-rose-300">{err.to_string()}</p> </> }.into_any(),
                        None => view! { <> </> }.into_any(),
                    }}}
                </Show>
            </Show>
        </section>

        <Show when=move || pdf_modal_url.get().is_some()>
            {move || match pdf_modal_url.get() {
                Some(url) => view! {
                    <div
                        class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
                        on:click=move |_| close_modals()
                    >
                        <div
                            class="w-full max-w-5xl rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl"
                            on:click=move |ev| ev.stop_propagation()
                        >
                            <div class="flex items-center justify-between border-b border-slate-800 px-6 py-4">
                                <p class="text-sm font-semibold text-white">"PDF Preview"</p>
                                <button
                                    class="rounded-full border border-slate-700 px-4 py-1 text-xs text-slate-300 hover:border-slate-400"
                                    on:click=move |_| close_modals()
                                >
                                    "Close"
                                </button>
                            </div>
                            <div class="h-[70vh] w-full bg-slate-900">
                                <iframe class="h-full w-full" src=url title="PDF preview"></iframe>
                            </div>
                        </div>
                    </div>
                }.into_any(),
                None => view! { <span></span> }.into_any(),
            }}
        </Show>

        <Show when=move || selected_file_id.get().is_some()>
            <div
                class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
                on:click=move |_| close_modals()
            >
                <div
                    class="w-full max-w-3xl rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl"
                    on:click=move |ev| ev.stop_propagation()
                >
                    <div class="flex items-center justify-between border-b border-slate-800 px-6 py-4">
                        <p class="text-sm font-semibold text-white">"Extracted text"</p>
                        <button
                            class="rounded-full border border-slate-700 px-4 py-1 text-xs text-slate-300 hover:border-slate-400"
                            on:click=move |_| close_modals()
                        >
                            "Close"
                        </button>
                    </div>
                    <div class="max-h-[70vh] overflow-auto px-6 py-4">
                        <Show
                            when=move || text_resource.get().is_some()
                            fallback=move || view! { <p class="text-sm text-slate-400">"Loading text..."</p> }
                        >
                            {move || match text_resource.get() {
                                Some(Ok(text)) if text.trim().is_empty() => view! {
                                    <p class="text-sm text-slate-400">"No extractable text found."</p>
                                }.into_any(),
                                Some(Ok(text)) => view! {
                                    <pre class="whitespace-pre-wrap text-sm text-slate-200">{text}</pre>
                                }.into_any(),
                                Some(Err(err)) => view! {
                                    <p class="text-sm text-rose-300">{err.to_string()}</p>
                                }.into_any(),
                                None => view! { <span></span> }.into_any(),
                            }}
                        </Show>
                    </div>
                </div>
            </div>
        </Show>
        </>
    }
}

fn format_bytes(bytes: i64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut index = 0usize;

    while size >= 1024.0 && index < units.len() - 1 {
        size /= 1024.0;
        index += 1;
    }

    if index == 0 {
        format!("{bytes} {}", units[index])
    } else {
        format!("{size:.1} {}", units[index])
    }
}
