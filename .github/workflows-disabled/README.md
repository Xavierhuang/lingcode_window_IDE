# Disabled workflows

These are the CI workflows inherited from upstream Zed. They are **intentionally
disabled** in the LingCode fork because they depend on infrastructure this fork
does not have:

- **Self-hosted / Namespace runners** (`namespace-profile-*`, `self-32vcpu-windows-2022`)
  that don't exist on a fork — e.g. `run_tests.yml`, `release.yml`, `run_bundling.yml`.
- **Private secrets** — Factory.ai (`FACTORY_API_KEY` in `docs_suggestions.yml`),
  Azure code-signing, Cloudflare/collab deploy keys, Slack tokens, etc.
- **Zed-only bots** — community labelers, contributor congrats, reviewer assignment.

GitHub only runs workflows located directly in `.github/workflows/`, so moving them
here switches them off without deleting anything.

The only active workflow is **`.github/workflows/lingcode-release.yml`**, which builds
the Windows installer on a standard GitHub-hosted runner.

## Re-enabling one

Move it back:

```bash
git mv .github/workflows-disabled/<name>.yml .github/workflows/<name>.yml
```

Then supply whatever runners/secrets it needs (or edit it to use GitHub-hosted runners).
