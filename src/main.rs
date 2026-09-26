#![recursion_limit = "1024"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use axum::extract::DefaultBodyLimit;
    use axum::routing::{get, post};
    use flashy::session_store::SqliteStore;
    use flashy::{
        app::*,
        app_state::AppState,
        config::Config,
        db::init_db,
        features::{
            auth::utils::ensure_admin_user,
            flashcards::handlers::anki_export::download_deck_as_anki,
            oauth::{OAuthConfig, OAuthService},
            projects::handlers::{get_project_pdf, get_project_segment_pdf, upload_project_file},
            projects::storage::{MinioSettings, build_minio_client},
            summaries::handlers::pdf_export::download_summary_pdf,
        },
    };
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use time::Duration;
    use tower_sessions::{Expiry, SessionManagerLayer};
    use tower_sessions_core::session_store::ExpiredDeletion;
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

    // Load configuration from environment
    let config = Config::global();
    let oauth_config = OAuthConfig::from_env().expect("FATAL: Invalid MCP/OAuth configuration");
    if oauth_config.enabled
        && std::env::var("ADMIN_PASSWORD").map_or(true, |value| value == "admin123")
    {
        panic!("FATAL: MCP_ENABLED requires a non-default ADMIN_PASSWORD");
    }
    tracing::info!("Configuration loaded:");
    tracing::info!(
        "  Max upload size: {} MB",
        config.max_upload_bytes / 1024 / 1024
    );
    tracing::info!("  Max PDF size: {} MB", config.max_pdf_bytes / 1024 / 1024);
    tracing::info!("  Max context words: {}", config.max_context_words);

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
    let session_cleanup_store = session_store.clone();

    let same_site = std::env::var("SESSION_SAME_SITE")
        .unwrap_or_else(|_| "lax".to_string())
        .to_lowercase();
    let same_site = match same_site.as_str() {
        "strict" => tower_sessions::cookie::SameSite::Strict,
        "none" => tower_sessions::cookie::SameSite::None,
        _ => tower_sessions::cookie::SameSite::Lax,
    };

    let secure = oauth_config.secure_cookie;

    let session_layer = SessionManagerLayer::new(session_store)
        .with_expiry(Expiry::OnInactivity(Duration::weeks(1)))
        .with_same_site(same_site)
        .with_secure(secure);

    let minio_settings = MinioSettings::from_env();
    let minio_client = build_minio_client(&minio_settings)
        .await
        .expect("FATAL: Failed to initialize MinIO bucket client");

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(App);

    let app_state = AppState {
        leptos_options: leptos_options.clone(),
        db_pool: pool.clone(),
        minio_client,
        bucket_name: minio_settings.bucket.clone(),
        object_key_prefix: minio_settings.key_prefix.clone(),
        oauth: OAuthService::new(pool.clone(), oauth_config),
    };

    let (oauth_router, mcp_router) = if app_state.oauth.config().enabled {
        (
            flashy::features::oauth::router(),
            flashy::mcp::router(&app_state),
        )
    } else {
        (Router::new(), Router::new())
    };

    let app = Router::new()
        .route(
            "/api/projects/{project_id}/upload",
            post(upload_project_file),
        )
        .route(
            "/api/projects/{project_id}/files/{file_id}/pdf",
            get(get_project_pdf),
        )
        .route(
            "/api/projects/{project_id}/files/{file_id}/segment-pdf",
            get(get_project_segment_pdf),
        )
        .route(
            "/api/decks/{deck_id}/download/anki",
            get(download_deck_as_anki),
        )
        .route(
            "/api/summaries/{summary_id}/download/pdf",
            get(download_summary_pdf),
        )
        .layer(DefaultBodyLimit::max(config.max_upload_bytes))
        .leptos_routes_with_context(
            &app_state,
            routes,
            {
                let app_state = app_state.clone();
                move || {
                    provide_context(app_state.leptos_options.clone());
                    provide_context(app_state.db_pool.clone());
                    provide_context(app_state.clone());
                }
            },
            {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
        .merge(oauth_router)
        .merge(mcp_router)
        .layer(session_layer)
        .with_state(app_state.clone());

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    tracing::info!("Server listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    let shutdown = tokio_util::sync::CancellationToken::new();
    let cleanup_shutdown = shutdown.clone();
    let oauth = app_state.oauth.clone();
    let cleanup = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(900));
        loop {
            tokio::select! {
                _ = cleanup_shutdown.cancelled() => break,
                _ = interval.tick() => {
                    if let Err(error) = oauth.cleanup().await {
                        tracing::warn!(?error, "OAuth cleanup failed");
                    }
                    if let Err(error) = session_cleanup_store.delete_expired().await {
                        tracing::warn!(?error, "Session cleanup failed");
                    }
                },
            }
        }
    });
    let signal_shutdown = shutdown.clone();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal().await;
        signal_shutdown.cancel();
    })
    .await
    .unwrap();
    shutdown.cancel();
    let _ = cleanup.await;
}

#[cfg(feature = "ssr")]
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("FATAL: Failed to install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
