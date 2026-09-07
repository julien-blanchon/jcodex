# jcodex

A small [OpenAI Codex](https://github.com/openai/codex) fork adding Claude Code–style **Monitor** support. The command is `jcodex`; it shares Codex’s config, login and saved sessions in `~/.codex` (or `CODEX_HOME`), so both can be installed together.

Install on macOS:

```bash
brew install julien-blanchon/tap/jcodex
```

Or on macOS/Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/julien-blanchon/jcodex/main/install.sh | bash
```

The installer verifies release checksums and links `~/.local/bin/jcodex`. [GitHub releases](https://github.com/julien-blanchon/jcodex/releases) include macOS, Linux and Windows packages for ARM64 and x86-64. On Windows, extract the ZIP and add its `bin` directory to PATH; keep the complete package together.

Ask the agent to watch a log, build or deployment. It starts a background command; output wakes the same conversation, with no model polling while silent. **`/monitor` lists watches and lets you stop them without a model call.** Monitors are always enabled in jcodex, even when disabled in the shared config. Up to four main-agent watches run at once; output is bounded and batched. They end on exit, timeout (30 minutes by default), cancellation or session shutdown, and are not restored on resume. Subagents cannot create watches.

Update with `brew upgrade julien-blanchon/tap/jcodex` or rerun the installer. Upstream auto-updates and shared daemon management are disabled. Packages are not notarized or Authenticode signed.

Internal names and configuration remain upstream-compatible. `monitor-only` contains the feature without distribution changes; `main` adds the jcodex packaging. Merge upstream into `monitor-only`, then merge that branch into `main`; keep changes as Git commits rather than maintaining a separate patch or doing global renames. See [upstream README](README.codex.md) for Codex documentation.
