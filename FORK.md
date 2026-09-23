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
4. Optional: to start with your official Handy settings, quit both apps before the first launch of Handy.satoru and copy `~/Library/Application Support/com.pais.handy/settings_store.json` into `~/Library/Application Support/io.github.nerrobdog.handy-satoru/` (create the folder if needed).

## Updates

The built-in updater is switched off in this fork: its feed serves official Handy, which would replace Handy.satoru. Install new releases by hand from the Releases page.

## Branches

`satoru` is the fork's main branch. `main` mirrors upstream Handy and is merged into `satoru` to pick up upstream changes.
