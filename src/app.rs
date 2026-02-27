use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Link, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::{use_navigate, use_params_map},
    path, StaticSegment,
};

use crate::features::{
    auth::{
        handlers::{get_user, LoginUser, LogoutUser, RegisterUser},
        models::UserSession,
    },
    invites::handlers::{list_invites, CreateInvite},
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
