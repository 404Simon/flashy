pub mod pool;

#[cfg(feature = "ssr")]
pub use pool::init_db;
