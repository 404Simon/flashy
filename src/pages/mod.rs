pub mod auth;
pub mod flashcards;
pub mod home;
pub mod invites;
pub mod projects;

pub use auth::{LoginPage, RegisterPage};
pub use flashcards::{DeckDetailPage, DecksPage};
pub use home::HomePage;
pub use invites::{AdminInvitesPage, InvitePage};
pub use projects::{ProjectDetailPage, ProjectsPage};
