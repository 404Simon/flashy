#[cfg(feature = "ssr")]
use axum::extract::FromRef;
#[cfg(feature = "ssr")]
use s3::Client;
#[cfg(feature = "ssr")]
use leptos::prelude::LeptosOptions;
#[cfg(feature = "ssr")]
use sqlx::SqlitePool;

#[cfg(feature = "ssr")]
#[derive(Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub db_pool: SqlitePool,
    pub minio_client: Client,
    pub bucket_name: String,
    pub object_key_prefix: String,
}

#[cfg(feature = "ssr")]
impl FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos_options.clone()
    }
}

#[cfg(feature = "ssr")]
impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.db_pool.clone()
    }
}

#[cfg(feature = "ssr")]
impl FromRef<AppState> for Client {
    fn from_ref(state: &AppState) -> Self {
        state.minio_client.clone()
    }
}
