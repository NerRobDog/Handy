# Handy.satoru Identity and Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the fork as a separate app, Handy.satoru, that installs next to official Handy, never pulls upstream updates, credits the original, and is released as unsigned macOS Apple Silicon draft releases.

**Architecture:** Fork-specific facts live in one small module, `src-tauri/src/fork.rs` (product name, build-time updater switch), so upstream syncs touch as few shared lines as possible. Identity is configuration (`tauri.conf.json`); the updater switch is a compile-time env var set in `src-tauri/.cargo/config.toml`; releases come from a new `fork-release.yml` that reuses upstream's `build.yml`.

**Tech Stack:** Rust (Tauri 2), React + i18next, GitHub Actions (`tauri-apps/tauri-action` via `build.yml`).

**Spec:** Design approved in chat on 2026-09-24 (beads epic `handy-hud`); decisions copied verbatim into Global Constraints below.

## Global Constraints

- Work in worktree `/Users/nik/dev/Handy-satoru`, branch `feat/satoru-identity` (from `satoru`, the fork's default branch). `main` mirrors upstream and is never committed to.
- Before any `cargo`/`bun` command: `. ~/.local/opt/handy-dev-env.sh` and `export CARGO_TARGET_DIR=/Users/nik/dev/Handy/src-tauri/target` (reuses compiled dependencies). `cargo` runs in `src-tauri/`, `bun` in the worktree root (run `bun install` once first).
- Product name: `Handy.satoru`. Bundle identifier: `io.github.nerrobdog.handy-satoru`. Fork URL: `https://github.com/NerRobDog/Handy`. Upstream URL stays `https://github.com/cjpais/Handy`.
- Only the outside name changes (Dock, menu bar, window title, tray tooltip, macOS permission prompts). In-app texts that say "Handy" and the icons stay.
- About page adds a fork row crediting CJ Pais with a link to the fork; the upstream "Source Code" row stays.
- The self-updater is off in every fork build (its endpoint serves upstream releases that would overwrite Handy.satoru). Fork auto-updates are backlog `handy-4ix`.
- Releases: manual `workflow_dispatch`, macOS Apple Silicon only, unsigned (ad-hoc), created as a **draft**; publishing is the user's action. App version stays upstream's (`tauri.conf.json` `version`); the release tag is `v<version>-satoru.<N>` with `N` a workflow input.
- Settings are copied once by hand before the first launch (not in code).
- Conventional commits explaining why; every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## File Map

| File                                              | Change | Responsibility                                                |
| ------------------------------------------------- | ------ | ------------------------------------------------------------- |
| `src-tauri/src/fork.rs`                           | create | `PRODUCT_NAME`, `updater_disabled_at_build()`, identity tests |
| `src-tauri/src/lib.rs`                            | modify | `mod fork;`, window title                                     |
| `src-tauri/src/tray.rs`                           | modify | tooltip name                                                  |
| `src-tauri/src/settings.rs`                       | modify | `update_checks_forced_disabled` honors the build switch       |
| `src-tauri/.cargo/config.toml`                    | create | `[env] HANDY_DISABLE_UPDATER = "1"`                           |
| `src-tauri/tauri.conf.json`                       | modify | name, identifier, no updater artifacts                        |
| `src/components/settings/about/AboutSettings.tsx` | modify | fork row                                                      |
| `src/i18n/locales/*/translation.json`             | modify | 3 strings × 26 locales                                        |
| `.github/workflows/fork-release.yml`              | create | draft release, macOS arm64                                    |
| `FORK.md`                                         | create | what the fork is, how to install and update it                |

---

### Task H1: Identity and build-time updater switch

**Files:**

- Create: `src-tauri/src/fork.rs`, `src-tauri/.cargo/config.toml`
- Modify: `src-tauri/tauri.conf.json` (lines 3, 5, `bundle.createUpdaterArtifacts` at ~28), `src-tauri/src/lib.rs` (module list; `.title("Handy")` at ~945), `src-tauri/src/tray.rs` (`version_label` ~446), `src-tauri/src/settings.rs` (`update_checks_forced_disabled` ~1195)

**Interfaces:**

- Produces: `crate::fork::PRODUCT_NAME: &str`, `crate::fork::updater_disabled_at_build() -> bool`.

- [ ] **Step 1: Write the failing tests** — create `src-tauri/src/fork.rs` with only:

```rust
//! Facts that make this build Handy.satoru rather than upstream Handy. Kept in
//! one module so syncing with upstream touches as few shared lines as possible.

#[cfg(test)]
mod tests {
    use super::*;

    fn tauri_conf() -> serde_json::Value {
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json parses")
    }

    #[test]
    fn tauri_conf_carries_the_fork_identity() {
        let conf = tauri_conf();
        assert_eq!(conf["productName"], PRODUCT_NAME);
        assert_eq!(conf["identifier"], "io.github.nerrobdog.handy-satoru");
        assert_eq!(conf["bundle"]["createUpdaterArtifacts"], false);
    }

    #[test]
    fn fork_builds_disable_the_updater() {
        assert!(updater_disabled_at_build());
        assert!(crate::settings::update_checks_forced_disabled());
    }
}
```

and add `mod fork;` to `src-tauri/src/lib.rs` after `mod commands;`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test fork::`
Expected: compile error `cannot find value 'PRODUCT_NAME'`.

- [ ] **Step 3: Implement**

`fork.rs`, above the tests:

```rust
/// Name shown in the Dock, menu bar, window title and tray tooltip. Must
/// match `productName` in `tauri.conf.json`.
pub const PRODUCT_NAME: &str = "Handy.satoru";

/// Whether this binary was built with the self-updater switched off. Fork
/// builds set `HANDY_DISABLE_UPDATER` in `src-tauri/.cargo/config.toml`: the
/// updater endpoint serves upstream releases, which would overwrite
/// Handy.satoru. Accepts the same values as the runtime env flag.
pub fn updater_disabled_at_build() -> bool {
    option_env!("HANDY_DISABLE_UPDATER").is_some_and(|value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        )
    })
}
```

`src-tauri/.cargo/config.toml`:

```toml
# Handy.satoru: build every binary with the self-updater off. Its endpoint
# serves upstream Handy releases, which would overwrite the fork.
[env]
HANDY_DISABLE_UPDATER = "1"
```

`tauri.conf.json`: `"productName": "Handy.satoru"`, `"identifier": "io.github.nerrobdog.handy-satoru"`, `"createUpdaterArtifacts": false`.

`settings.rs` — `update_checks_forced_disabled` becomes:

```rust
pub fn update_checks_forced_disabled() -> bool {
    use std::sync::OnceLock;
    static IS_UPDATER_DISABLED: OnceLock<bool> = OnceLock::new();
    *IS_UPDATER_DISABLED.get_or_init(|| {
        crate::fork::updater_disabled_at_build()
            || utils::env_flag_enabled("HANDY_DISABLE_UPDATER")
    })
}
```

(Update its doc comment: "forced off by `HANDY_DISABLE_UPDATER`, either set at build time (fork builds) or in the environment at runtime (the Nix package)".)

`lib.rs`: `.title("Handy")` → `.title(fork::PRODUCT_NAME)`.

`tray.rs` `version_label`:

```rust
fn version_label() -> String {
    let name = crate::fork::PRODUCT_NAME;
    if cfg!(debug_assertions) {
        format!("{name} v{} (Dev)", env!("CARGO_PKG_VERSION"))
    } else {
        format!("{name} v{}", env!("CARGO_PKG_VERSION"))
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test fork::` then `cargo test`
Expected: both new tests PASS; full suite green. If `fork_builds_disable_the_updater` fails, check that cargo was run from `src-tauri/` (the `[env]` table is only read from there).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/fork.rs src-tauri/.cargo/config.toml src-tauri/tauri.conf.json src-tauri/src/lib.rs src-tauri/src/tray.rs src-tauri/src/settings.rs
git commit -m "feat: give the fork its own identity as Handy.satoru

A separate bundle identifier lets the fork run next to official Handy with
its own settings and permissions; the build-time updater switch keeps the
upstream updater from replacing it with an official release.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task H2: Fork row on the About page

**Files:**

- Modify: `src/components/settings/about/AboutSettings.tsx` (after the "Source Code" `SettingContainer`, ~66–78)
- Modify: `src/i18n/locales/*/translation.json` (26 files, `settings.about.fork`)

- [ ] **Step 1: Confirm the translation check is green before the change**

Run: `bun run check:translations`
Expected: PASS (baseline).

- [ ] **Step 2: Add the strings** — `en` → `settings.about.fork`:

```json
"fork": {
  "title": "Handy.satoru",
  "description": "A fork of Handy by CJ Pais with quick model and translation shortcuts",
  "button": "View Fork"
}
```

`ru` → `settings.about.fork`:

```json
"fork": {
  "title": "Handy.satoru",
  "description": "Форк Handy от CJ Pais с быстрыми горячими клавишами для моделей и перевода",
  "button": "Открыть форк"
}
```

For the other 24 locales translate `description` and `button` (keep `title` as `Handy.satoru`, keep "CJ Pais" and "Handy" untranslated, reuse each locale's existing wording for "shortcuts" from `settings.general.shortcut`). Merge with the same scratch script pattern as the shortcuts plan's Task 8 (a JSON table → `merge_i18n.py`), then `bun run format:frontend`.

- [ ] **Step 3: Add the row** — in `AboutSettings.tsx`, right after the "Source Code" `SettingContainer`:

```tsx
<SettingContainer
  title={t("settings.about.fork.title")}
  description={t("settings.about.fork.description")}
  grouped={true}
>
  <Button
    variant="secondary"
    size="md"
    onClick={() => openUrl("https://github.com/NerRobDog/Handy")}
  >
    {t("settings.about.fork.button")}
  </Button>
</SettingContainer>
```

- [ ] **Step 4: Verify**

Run: `bun run check:translations && bun run lint && bunx tsc --noEmit && bun run format:check`
Expected: all exit 0.

- [ ] **Step 5: Commit**

```bash
git add src/components/settings/about/AboutSettings.tsx src/i18n/locales
git commit -m "feat(about): credit upstream Handy and link the fork

MIT requires keeping the original attribution; the About page now says
this build is a fork of Handy by CJ Pais and links to where it lives.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task H3: Draft-release workflow and FORK.md

**Files:**

- Create: `.github/workflows/fork-release.yml`, `FORK.md`

- [ ] **Step 1: Write `fork-release.yml`**

```yaml
name: "Handy.satoru Release"

# Builds an unsigned macOS Apple Silicon bundle and attaches it to a DRAFT
# release. Publishing the draft is a manual step on GitHub.
on:
  workflow_dispatch:
    inputs:
      fork-revision:
        description: "Fork revision N for the tag v<app version>-satoru.<N>"
        required: true
        default: "1"

jobs:
  create-release:
    permissions:
      contents: write
    runs-on: ubuntu-latest
    outputs:
      release-id: ${{ steps.create-release.outputs.result }}
    steps:
      - name: Checkout repository
        uses: actions/checkout@v5

      - name: Get version from tauri.conf.json
        id: get-version
        shell: bash
        run: |
          VERSION=$(grep -o '"version": "[^"]*"' src-tauri/tauri.conf.json | cut -d'"' -f4)
          echo "version=$VERSION" >> "$GITHUB_OUTPUT"

      - name: Create draft release
        id: create-release
        uses: actions/github-script@v9
        with:
          script: |
            const tag = `v${{ steps.get-version.outputs.version }}-satoru.${{ inputs.fork-revision }}`
            const { data } = await github.rest.repos.createRelease({
              owner: context.repo.owner,
              repo: context.repo.repo,
              tag_name: tag,
              target_commitish: context.sha,
              name: `Handy.satoru ${tag.slice(1)}`,
              body: "Unsigned build for Apple Silicon Macs. See FORK.md for install steps.",
              draft: true,
              prerelease: false,
            })
            return data.id

  build-macos-arm64:
    permissions:
      contents: write
    needs: create-release
    uses: ./.github/workflows/build.yml
    with:
      platform: "macos-26"
      target: "aarch64-apple-darwin"
      build-args: "--target aarch64-apple-darwin"
      release-id: ${{ needs.create-release.outputs.release-id }}
      asset-prefix: "handy-satoru"
      sign-binaries: false
    secrets: inherit
```

- [ ] **Step 2: Write `FORK.md`**

```markdown
# Handy.satoru

Handy.satoru is a fork of [Handy](https://github.com/cjpais/Handy) by CJ Pais. It installs as a separate app (`io.github.nerrobdog.handy-satoru`), so it can sit next to official Handy with its own settings and permissions. Downloaded speech models are shared through the Hugging Face cache.

## What the fork adds

- **Translation shortcut** (⌃⌥T): turns translation to English on and switches to the last model used for translation; pressing it again turns translation off and returns to the previous model.
- **Next model shortcut** (⌃⌥M): cycles through the models starred on the Models page, in the order they were starred; with fewer than two starred, it cycles through all downloaded models.
- A "Translate to English" item in the tray menu and a short overlay notice after each switch.

## Install

Builds are unsigned and made for Apple Silicon Macs only.

1. Download the `.dmg` from [Releases](https://github.com/NerRobDog/Handy/releases) and drag Handy.satoru into Applications.
2. The first launch is blocked by Gatekeeper. Open System Settings → Privacy & Security and click "Open Anyway", or run `xattr -dr com.apple.quarantine /Applications/Handy.satoru.app`.
3. Grant Accessibility and Microphone access when asked.

## Updates

The built-in updater is switched off in this fork: its feed serves official Handy, which would replace Handy.satoru. Install new releases by hand from the Releases page.

## Branches

`satoru` is the fork's main branch. `main` mirrors upstream Handy and is merged into `satoru` to pick up upstream changes.
```

- [ ] **Step 3: Validate the workflow syntax**

Run: `python3 -c "import yaml,sys;d=yaml.safe_load(open('.github/workflows/fork-release.yml'));assert 'workflow_dispatch' in d[True];print('ok', list(d['jobs']))"`
Expected: `ok ['create-release', 'build-macos-arm64']`. (PyYAML parses the `on:` key as `True`.) A real run happens only after this branch is merged into `satoru` — the workflow must be on the default branch to be dispatchable — and only with the user's go-ahead, since it creates a draft release.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/fork-release.yml FORK.md
git commit -m "ci: add Handy.satoru draft-release workflow and fork notes

Upstream's release workflows need Apple signing secrets the fork does not
have; this one builds an unsigned Apple Silicon bundle into a draft
release, and FORK.md explains what the fork is and how to install it.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task H4: Verification

- [ ] **Step 1:** `cargo fmt -- --check && cargo test && cargo clippy --all-targets 2>&1 | grep -E "fork.rs|tray.rs|settings.rs" -A3` — tests green, no new clippy findings in touched files.
- [ ] **Step 2:** `bun run format:check && bun run lint && bun run build && bun run check:translations` — all exit 0.
- [ ] **Step 3:** Code review via `superpowers:requesting-code-review` over `git diff satoru...feat/satoru-identity`.
- [ ] **Step 4 (with the user):** merge into `satoru` via `superpowers:finishing-a-development-branch`; then, with explicit approval, run `gh workflow run "Handy.satoru Release" -R NerRobDog/Handy -f fork-revision=1` and check the draft release carries a `.dmg`.
