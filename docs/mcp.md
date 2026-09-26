# Flashy MCP setup

Flashy exposes a read-only MCP server at `/mcp`. Authorization uses an existing
Flashy account and an explicit browser consent screen. The `flashy:read` scope
includes project metadata, filenames, extracted document text, decks,
flashcards, and summaries belonging to that account. It cannot modify study
data or access another account.

The public registration and authorization endpoints use durable per-peer and
global rate limits. Deploy an additional reverse-proxy rate limit for
internet-facing installations; Flashy deliberately keys its internal limit to
the direct TCP peer and does not trust spoofable forwarding headers.

## Server configuration

Set these environment variables in production:

```dotenv
MCP_ENABLED=true
FLASHY_PUBLIC_ORIGIN=https://flashy.example
SESSION_SECURE=true
ADMIN_PASSWORD=a-strong-non-default-password
```

`FLASHY_PUBLIC_ORIGIN` is the canonical issuer. It must be HTTPS except for an
explicit loopback development origin. Changing it changes the OAuth audience,
so connected clients must authorize again. `MCP_CIMD_TRUSTED_HOSTS` can add a
comma-separated set of HTTPS hosts whose client metadata Flashy may retrieve;
`chatgpt.com` is trusted by default.

Run the database migrations before exposing the endpoint. Flashy never stores
authorization codes, access tokens, or refresh tokens in plaintext.

## Clients

Use this server URL in Codex or OpenCode v1:

```text
https://flashy.example/mcp
```

The client discovers OAuth metadata and opens a browser. Log in with an
existing Flashy account, review the requested access, and choose **Allow**.
OpenCode v1 can use dynamic client registration. Codex can use its published
client metadata; forced DCR is also supported. OpenCode v2 has not been
acceptance-tested.

To disconnect a client, open **Settings → Connected applications** and choose
**Disconnect**. Changing the account password also revokes every OAuth grant.
Ordinary browser logout does not disconnect CLI clients.

## Limits

Lists default to 25 records and accept at most 100. Document and summary reads
default to 8,000 Unicode characters and accept at most 20,000. Cursors are
opaque and tied to their collection or filter. If document content changes,
restart chunked reading without the old cursor.
