# Repository Guidelines

## Project Structure & Module Organization
- `src/` contains the Leptos app, SSR entrypoint, and feature modules.
- `src/app.rs` holds routes and UI views; feature code lives under `src/features/`.
- `src/db/` manages the SQLite pool and migrations.
- `migrations/` contains SQL migrations executed at startup.
- `style/` holds Tailwind input (`style/main.css`).
- `public/` contains static assets copied to the site root.
- `end2end/` hosts Playwright tests.
- `splitify-rs/` is a reference project with similar stack choices.

## Build, Test, and Development Commands
- `cargo leptos watch` — run the SSR server with live reload for local development.
- `cargo leptos build` — build server + WASM + assets for local verification.
- `cargo leptos build --release` — production build (outputs to `target/`).
- `cargo leptos end-to-end` — run Playwright tests in `end2end/`.
- `npm install` — install Tailwind tooling for CSS builds.

## Coding Style & Naming Conventions
- Rust: follow `rustfmt` defaults; use `snake_case` for functions/modules and `CamelCase` for types.
- Leptos components use `#[component]` with `PascalCase` names.
- SQL migrations are numbered (`0001_*.sql`, `0002_*.sql`).
- Keep UI classes in Tailwind utility form; use `style/main.css` only for base rules.

## Testing Guidelines
- End-to-end tests run via Playwright (`end2end/`).
- Prefer descriptive test names that match user flows (e.g., `login.spec.ts`).
- Run `cargo leptos end-to-end` before major UI or auth changes.

## Commit & Pull Request Guidelines
- Commit history uses Conventional Commit-style prefixes (e.g., `feat: invites`).
- Keep commits focused and scoped to a single change.
- PRs should include: summary of changes, relevant commands run, and screenshots for UI changes.

## Security & Configuration Tips
- Auth uses SQLite and sessions; admin is bootstrapped via `.env` (`ADMIN_USERNAME`, `ADMIN_PASSWORD`, `ADMIN_EMAIL`).
- Copy `.env.example` to `.env` for local setup.
- Invite-only registration is enforced via `/invite/:token` -> `/register/:token`.
