
#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use flashy::{app::*, db::init_db, features::auth::utils::ensure_admin_user};
    use time::Duration;
    use tower_sessions::{Expiry, SessionManagerLayer};
    use tower_sessions_sqlx_store::SqliteStore;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Note: .env file not loaded: {}", e);
    }

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "flashy=debug,axum=info,sqlx=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Flashy application");

    let pool = init_db()
        .await
        .expect("FATAL: Failed to initialize database - check DATABASE_URL and migrations");

    ensure_admin_user(&pool)
        .await
        .expect("FATAL: Failed to ensure admin user");

    let session_store = SqliteStore::new(pool.clone());
    session_store
        .migrate()
        .await
        .expect("FATAL: Failed to migrate session store");

    let same_site = std::env::var("SESSION_SAME_SITE")
        .unwrap_or_else(|_| "lax".to_string())
        .to_lowercase();
    let same_site = match same_site.as_str() {
        "strict" => tower_sessions::cookie::SameSite::Strict,
        "none" => tower_sessions::cookie::SameSite::None,
        _ => tower_sessions::cookie::SameSite::Lax,
    };

    let secure = std::env::var("SESSION_SECURE")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        != "false";

    let session_layer = SessionManagerLayer::new(session_store)
        .with_expiry(Expiry::OnInactivity(Duration::weeks(1)))
        .with_same_site(same_site)
        .with_secure(secure);

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(App);

    let app = Router::new()
        .leptos_routes_with_context(
            &leptos_options,
            routes,
            {
                let leptos_options = leptos_options.clone();
                let pool = pool.clone();
                move || {
                    provide_context(leptos_options.clone());
                    provide_context(pool.clone());
                }
            },
            {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler(shell))
        .layer(session_layer)
        .with_state(leptos_options);

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    tracing::info!("Server listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
