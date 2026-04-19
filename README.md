# Flashy

AI-powered flashcard generator for study materials. Upload PDFs, generate flashcards automatically and export to Anki.

## Features

- AI-generated flashcards from PDF documents
- Markdown formatting with LaTeX math support (MathJax)
- Anki export (.apkg format)
- Invite-only user registration
- SQLite database with migrations

## Setup

1. Install dependencies:
```bash
cargo install cargo-leptos --locked
rustup target add wasm32-unknown-unknown
npm install
```

2. Configure environment:
```bash
cp .env.example .env
# Edit .env with your admin credentials and API keys
```

3. Run development server:
```bash
cargo leptos watch
```

## LLM Configuration

Flashcard generation is provider-agnostic. Set these environment variables:

| Variable | Description |
|---|---|
| `LLM_PROVIDER` | Backend name (required) |
| `LLM_API_KEY` | API key for the provider (required) |
| `LLM_MODEL` | Model identifier (required) |

**Supported providers:** `openai`, `anthropic`, `deepseek`, `ollama`, `xai`, `google`, `groq`, `mistral`, `openrouter`, `cohere`, `phind`, `huggingface`, `aws_bedrock`, `azure_openai`

### Example: DeepSeek

```env
LLM_PROVIDER=deepseek
LLM_API_KEY=sk-your-key
LLM_MODEL=deepseek-chat
```

## Stack

- Leptos + Axum
- SQLite
- llm crate (provider-agnostic)
- Tailwind
