jcodex is a small Codex fork with command monitors enabled by default.

- Background command output wakes the main agent without model polling.
- `/monitor` lists active watches; select one to stop it without a model call.
- Watches are bounded, time out, and stop on session shutdown. Subagents cannot create them.
- `jcodex` coexists with `codex` and uses the same configuration, login and session storage.

Install on macOS with `brew install julien-blanchon/tap/jcodex`, or on macOS/Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/julien-blanchon/jcodex/main/install.sh | bash
```

Windows: extract the matching ZIP and add its `bin` directory to PATH. Keep the complete package together.

Update through Homebrew or rerun the installer. Upstream automatic updates and shared daemon management are disabled in this fork. Packages are not notarized or Authenticode signed.
