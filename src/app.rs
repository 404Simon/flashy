use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Link, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    path, StaticSegment,
};

use crate::{features::auth::handlers::get_user, pages::*};

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
                <script>
                    "MathJax = {
                        tex: {
                            inlineMath: [['\\\\(', '\\\\)']],
                            displayMath: [['\\\\[', '\\\\]']]
                        },
                        svg: {
                            fontCache: 'global'
                        }
                    };"
                </script>
                <script src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js" id="MathJax-script"></script>
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
                    <Route path=StaticSegment("settings") view=SettingsPage/>
                    <Route path=StaticSegment("projects") view=ProjectsPage/>
                    <Route path=path!("/projects/:id") view=ProjectDetailPage/>
                    <Route path=path!("/projects/:id/decks") view=DecksPage/>
                    <Route path=path!("/decks/:deck_id") view=DeckDetailPage/>
                    <Route path=path!("/decks/:deck_id/study") view=DeckViewerPage/>
                    <Route path=path!("/admin/invites") view=AdminInvitesPage/>
                    <Route path=path!("/admin/users") view=AdminUsersPage/>
                </Routes>
            </main>
        </Router>
    }
}
