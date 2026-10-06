# Security Policy

## Reporting a vulnerability

Please report suspected security vulnerabilities **privately** using
**GitHub Private Vulnerability Reporting** on this repository
(**Security → Advisories → Report a vulnerability**).

Do not open a public issue, pull request, or discussion for
vulnerability details.

Please include:

- Affected repository, version, branch, or commit
- Description of the issue and its potential impact
- Steps to reproduce and a minimal proof of concept (if available)
- Relevant configuration or logs, with credentials and personal data removed

Do **not** include passwords, API keys, tokens, or customer data. Use redacted
or synthetic examples. Keep vulnerability details private until maintainers
coordinate disclosure.

## Response

We aim to acknowledge valid reports within **72 hours**. Fix timelines depend
on severity and scope. There is no bug bounty program for this repository
unless separately advertised.

## Supported versions

Report the affected version or commit. Historical releases are assessed
case-by-case; this policy does not guarantee support for every past release.

## Secrets

Never commit plaintext secrets. Prefer schema-first secret tooling (e.g. Varlock)
and GitHub encrypted secrets / environment protection where applicable.
