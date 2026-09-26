# Flashy MCP

Flashy's read-only MCP server lets AI clients access the projects, documents,
summaries, decks, and flashcards in your account. It cannot change your data.

## Connect with Codex

Add the hosted Flashy server and start the login flow:

```bash
codex mcp add flashy --url https://<your-domain>/mcp
codex mcp login flashy
```

The login command opens Flashy in your browser. Sign in with your existing
account, review the requested access, and choose **Allow**.

## Connect with OpenCode

Let OpenCode add the server, then start the login flow:

```bash
opencode mcp add flashy --url https://<your-domain>/mcp
opencode mcp auth flashy
```

Alternatively, add the server to your OpenCode configuration:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "flashy": {
      "type": "remote",
      "url": "https://<your-domain>/mcp"
    }
  }
}
```

Then run `opencode mcp auth flashy`. The auth command opens the same browser
login and consent flow.

To revoke access later, open **Settings → Connected applications** in Flashy
and choose **Disconnect**. Changing your password also revokes all connected
applications; logging out in the browser does not.

## Self-hosting

Enable MCP and set the public URL for your Flashy instance:

```dotenv
MCP_ENABLED=true
FLASHY_PUBLIC_ORIGIN=https://<your-domain>
SESSION_SECURE=true
```

Then replace the hosted URL in the Codex or OpenCode setup with
`https://<your-domain>/mcp`. Public instances must use HTTPS. Changing
`FLASHY_PUBLIC_ORIGIN` requires connected clients to log in again.

Run the database migrations before exposing the endpoint, and add a
reverse-proxy rate limit for internet-facing installations.

## Limits

List tools return 25 records by default and at most 100. Document and summary
reads return 8,000 Unicode characters by default and at most 20,000. Follow the
returned cursor to read additional records or content.
