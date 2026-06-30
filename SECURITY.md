# Security Policy

## Supported versions

Notefix is distributed as a desktop application. Only the latest released
version receives security fixes. Please make sure you are on the most recent
release before reporting an issue.

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, use GitHub's private vulnerability reporting:

1. Go to the [**Security** tab](https://github.com/NoiXdev/notefix/security/advisories) of this repository.
2. Click **Report a vulnerability** and fill in the details.

If you are unable to use private reporting, you may email the maintainer at
**d.elskamp@3b.de** instead.

Please include, where possible:

- A description of the vulnerability and its impact.
- Steps to reproduce or a proof of concept.
- The affected version and your operating system.

## What to expect

- We aim to acknowledge new reports within **5 business days**.
- We will keep you informed as we investigate and work on a fix.
- Once a fix is released, we are happy to credit you in the release notes
  unless you prefer to remain anonymous.

## Scope

Notefix stores all notes in a local SQLite database and never transmits data to
any first-party server by default. Optional server sync uses OAuth 2.0 with PKCE
and stores access/refresh tokens in the operating-system keychain (via the
`keyring` crate) — never in plain text. Reports concerning credential handling,
the OAuth/PKCE flow, the optional Model Context Protocol (MCP) server's
authentication and write-permission gating, deep-link (`notefix://`) handling, or
the auto-update/release pipeline are especially welcome.
