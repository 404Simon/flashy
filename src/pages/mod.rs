pub mod admin;
pub mod auth;
pub mod flashcards;
pub mod home;
pub mod invites;
pub mod projects;
pub mod settings;

pub use admin::{AdminInvitesPage, AdminUsersPage};
pub use auth::{LoginPage, RegisterPage};
pub use flashcards::{DeckDetailPage, DeckViewerPage, DecksPage};
pub use home::HomePage;
pub use invites::InvitePage;
pub use projects::{ProjectDetailPage, ProjectsPage};
pub use settings::SettingsPage;
