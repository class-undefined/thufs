---
phase: 03-share-workflow
status: complete
created: 2026-04-21
---

# Phase 3 Research

## Seafile Share Link API

Upstream Seafile v2.1 documents share-link creation as:

- Method: `POST`
- Path: `/api/v2.1/share-links/`
- Auth: token authorization
- Form fields: `repo_id`, `path`, optional `password`, optional `expire_days`
- Response includes share-link data such as `link`, `token`, `path`, and related metadata.

## Implementation Guidance

- Reuse `RemoteRef::parse` and repository resolution.
- Keep CLI handlers thin; put business validation in `ShareService`.
- Treat expiration as positive integer days.
- Avoid printing password or token-like internals in human output.
- Use JSON output for structured metadata.

