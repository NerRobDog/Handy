# Translation and Model-Cycle Shortcuts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two global shortcuts to the Handy fork: ⌃⌥T toggles a translation mode (switches to the last translation model and back), ⌃⌥M cycles through starred models; with a star button on model cards, a tray check item and a short overlay notice.

**Architecture:** A new focused backend module `src-tauri/src/quick_switch.rs` holds the pure selection logic (unit-tested) and the app-level operations that reuse the tray's existing `commands::models::switch_active_model`. Shortcut actions and the tray item only delegate to it. The overlay gains a `notice` state driven by a typed tauri-specta event; the frontend adds a star to `ModelCard` and two `ShortcutInput` rows.

**Tech Stack:** Rust (Tauri 2, tauri-specta rc.21, serde), React 18 + TypeScript + Zustand 5 + i18next, lucide-react icons, Bun.

**Spec:** `docs/superpowers/specs/2026-09-23-model-translation-shortcuts-design.md`

## Global Constraints

- Branch `feat/model-shortcuts`; never commit to `main` (it mirrors upstream for `gh repo sync`).
- Before any `cargo`/`bun` command run `. ~/.local/opt/handy-dev-env.sh` (native arm64 rustup + CMake; Homebrew's x86_64 cmake breaks `transcribe-cpp-sys`). `cargo` commands run in `src-tauri/`, `bun` commands in the repo root.
- Binding ids: `toggle_translation`, `cycle_model`. Defaults: macOS `ctrl+option+t` / `ctrl+option+m`; other platforms `ctrl+alt+shift+t` / `ctrl+alt+shift+m`.
- New `AppSettings` fields: `favorite_models: Vec<String>`, `translation_model: Option<String>`, `translation_return_model: Option<String>`.
- Overlay event: `OverlayNoticeEvent { kind: OverlayNoticeKind, model: Option<String> }`; kinds serialize as `translation_on`, `translation_off`, `model`, `model_no_translation`, `no_translation_model`, `switch_failed`.
- Both shortcuts do nothing while recording or transcribing (`tray::is_busy`).
- All user-facing text goes through i18next; new keys land in all 26 locales (`src/i18n/locales/*`), `bun run check:translations` must pass.
- Conventional commits, message explains why; every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Do not run the app or any binary that touches `~/Library/Application Support/com.pais.handy/` without first backing up `settings_store.json` there (the installed Handy 0.9.6 shares that store).

## File Map

| File                                                  | Change              | Responsibility                                                 |
| ----------------------------------------------------- | ------------------- | -------------------------------------------------------------- |
| `src-tauri/src/settings.rs`                           | modify              | new fields, two default bindings, tests                        |
| `src-tauri/src/shortcut/mod.rs`                       | modify (tests only) | default bindings valid in both keyboard implementations        |
| `src-tauri/src/quick_switch.rs`                       | create              | pure selection logic + app operations                          |
| `src-tauri/src/overlay.rs`                            | modify              | `OverlayNoticeEvent`, `show_notice_overlay`                    |
| `src-tauri/src/tray.rs`                               | modify              | `is_busy`, `invalidate_tray_menu`, translate check item        |
| `src-tauri/src/actions.rs`                            | modify              | two actions in `ACTION_MAP`                                    |
| `src-tauri/src/commands/models.rs`                    | modify              | translation-model memory hook, `toggle_favorite_model`         |
| `src-tauri/src/lib.rs`                                | modify              | `mod quick_switch`, command + event registration, tray handler |
| `src/bindings.ts`                                     | regenerated         | tauri-specta output                                            |
| `src/overlay/notice.ts`, `src/overlay/notice.test.ts` | create              | notice text/icon selection + test                              |
| `src/overlay/RecordingOverlay.tsx`, `.css`            | modify              | `notice` state                                                 |
| `src/components/onboarding/ModelCard.tsx`             | modify              | star button                                                    |
| `src/components/settings/models/ModelsSettings.tsx`   | modify              | favorites wiring                                               |
| `src/components/settings/general/GeneralSettings.tsx` | modify              | two shortcut rows                                              |
| `src/i18n/locales/*/translation.json`                 | modify              | new strings                                                    |
| `package.json`                                        | modify              | `test:overlay` script                                          |

---

### Task 1: Settings fields and default bindings

**Files:**

- Modify: `src-tauri/src/settings.rs` (struct `AppSettings` at ~364, `get_default_settings` at ~858, tests module at ~1241)
- Test: `src-tauri/src/settings.rs` tests, `src-tauri/src/shortcut/mod.rs` tests (~1407)

**Interfaces:**

- Produces: `AppSettings.favorite_models: Vec<String>`, `AppSettings.translation_model: Option<String>`, `AppSettings.translation_return_model: Option<String>`; default bindings `toggle_translation`, `cycle_model`.

- [ ] **Step 1: Write the failing tests** — append to `mod tests` in `settings.rs`:

```rust
    #[test]
    fn quick_switch_fields_default_when_missing() {
        let settings: AppSettings = serde_json::from_value(serde_json::json!({}))
            .expect("all AppSettings fields need serde defaults");
        assert!(settings.favorite_models.is_empty());
        assert_eq!(settings.translation_model, None);
        assert_eq!(settings.translation_return_model, None);
    }

    #[test]
    fn default_bindings_include_quick_switch_shortcuts() {
        let defaults = get_default_settings();
        for id in ["toggle_translation", "cycle_model"] {
            let binding = defaults
                .bindings
                .get(id)
                .unwrap_or_else(|| panic!("missing default binding '{id}'"));
            assert_eq!(binding.id, id);
            assert_eq!(binding.current_binding, binding.default_binding);
        }
        #[cfg(target_os = "macos")]
        {
            assert_eq!(
                defaults.bindings["toggle_translation"].default_binding,
                "ctrl+option+t"
            );
            assert_eq!(defaults.bindings["cycle_model"].default_binding, "ctrl+option+m");
        }
    }
```

and append to `mod tests` in `shortcut/mod.rs`:

```rust
    #[test]
    fn every_default_binding_is_valid_for_both_implementations() {
        use crate::settings::{get_default_settings, KeyboardImplementation};
        for (id, binding) in get_default_settings().bindings {
            for implementation in [KeyboardImplementation::Tauri, KeyboardImplementation::HandyKeys] {
                super::validate_shortcut_for_implementation(&binding.default_binding, implementation)
                    .unwrap_or_else(|e| {
                        panic!(
                            "default binding '{id}' ({}) invalid for {implementation:?}: {e}",
                            binding.default_binding
                        )
                    });
            }
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test quick_switch_fields_default_when_missing default_bindings_include_quick_switch_shortcuts`
Expected: compile error `no field 'favorite_models' on type 'AppSettings'`. (`every_default_binding_is_valid_for_both_implementations` may already pass — it guards the new defaults once added. If it fails on an existing binding, stop and report: that is a pre-existing upstream issue, not ours.)

- [ ] **Step 3: Implement** — in `struct AppSettings`, directly after `pub selected_model: String,` add:

```rust
    /// Model ids starred for the next-model shortcut, in the order they were
    /// starred. Ids that are not downloaded are skipped when cycling.
    #[serde(default)]
    pub favorite_models: Vec<String>,
    /// Last model that ran with translation on; the translation shortcut
    /// switches back to it when translation is turned on again.
    #[serde(default)]
    pub translation_model: Option<String>,
    /// Model to restore when the translation shortcut turns translation off.
    #[serde(default)]
    pub translation_return_model: Option<String>,
```

In `get_default_settings`, before `AppSettings {` add:

```rust
    #[cfg(target_os = "macos")]
    let (default_translation_shortcut, default_cycle_model_shortcut) =
        ("ctrl+option+t", "ctrl+option+m");
    // Ctrl+Alt+T opens a terminal on Ubuntu, so other platforms add Shift.
    #[cfg(not(target_os = "macos"))]
    let (default_translation_shortcut, default_cycle_model_shortcut) =
        ("ctrl+alt+shift+t", "ctrl+alt+shift+m");

    bindings.insert(
        "toggle_translation".to_string(),
        ShortcutBinding {
            id: "toggle_translation".to_string(),
            name: "Toggle Translation".to_string(),
            description: "Turns translation to English on or off, switching to the last translation model and back."
                .to_string(),
            default_binding: default_translation_shortcut.to_string(),
            current_binding: default_translation_shortcut.to_string(),
        },
    );
    bindings.insert(
        "cycle_model".to_string(),
        ShortcutBinding {
            id: "cycle_model".to_string(),
            name: "Next Model".to_string(),
            description: "Switches to the next starred transcription model.".to_string(),
            default_binding: default_cycle_model_shortcut.to_string(),
            current_binding: default_cycle_model_shortcut.to_string(),
        },
    );
```

and in the `AppSettings { ... }` literal, after `selected_model: "".to_string(),`:

```rust
        favorite_models: Vec::new(),
        translation_model: None,
        translation_return_model: None,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test quick_switch_fields_default_when_missing default_bindings_include_quick_switch_shortcuts every_default_binding_is_valid_for_both_implementations` then `cargo test`
Expected: all PASS, full suite still 274+ passed, 0 failed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/shortcut/mod.rs
git commit -m "feat(settings): add quick-switch fields and default shortcuts

Favorites and translation memory need to persist across restarts, and the
two new shortcuts must exist in the default set so existing stores pick
them up through the missing-binding merge.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Pure quick-switch selection logic

**Files:**

- Create: `src-tauri/src/quick_switch.rs`
- Modify: `src-tauri/src/lib.rs` (module list, lines 1–26: add `mod quick_switch;` after `mod paste_tx;`)

**Interfaces:**

- Produces (all in `crate::quick_switch`):
  - `pub struct ModelCandidate { pub id: String, pub name: String, pub supports_translation: bool }` (`Clone, Debug, PartialEq, Eq`)
  - `pub fn cycle_list(downloaded: &[ModelCandidate], favorites: &[String]) -> Vec<ModelCandidate>`
  - `pub fn next_in_cycle<'a>(list: &'a [ModelCandidate], current: &str) -> Option<&'a ModelCandidate>`
  - `pub fn translation_target<'a>(downloaded: &'a [ModelCandidate], favorites: &[String], remembered: Option<&str>, current: &str) -> Option<&'a ModelCandidate>`
  - `pub fn return_target<'a>(downloaded: &'a [ModelCandidate], return_model: Option<&str>, current: &str) -> Option<&'a ModelCandidate>`
  - `pub fn toggle_favorite(favorites: &mut Vec<String>, model_id: &str) -> bool`

- [ ] **Step 1: Write the failing tests** — create `src-tauri/src/quick_switch.rs` with only the tests module and add `mod quick_switch;` to `lib.rs`:

```rust
//! Quick switching between transcription models from global shortcuts and
//! the tray: cycling through starred models and toggling translation mode.

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, name: &str, translates: bool) -> ModelCandidate {
        ModelCandidate {
            id: id.to_string(),
            name: name.to_string(),
            supports_translation: translates,
        }
    }

    fn ids(list: &[ModelCandidate]) -> Vec<&str> {
        list.iter().map(|m| m.id.as_str()).collect()
    }

    fn favs(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn downloaded() -> Vec<ModelCandidate> {
        vec![
            model("turbo", "Whisper Large v3 Turbo", false),
            model("parakeet", "Parakeet V3", false),
            model("large", "Whisper Large v3", true),
        ]
    }

    #[test]
    fn cycle_list_uses_favorites_in_star_order() {
        let list = cycle_list(&downloaded(), &favs(&["large", "parakeet"]));
        assert_eq!(ids(&list), ["large", "parakeet"]);
    }

    #[test]
    fn cycle_list_skips_favorites_that_are_not_downloaded() {
        let list = cycle_list(&downloaded(), &favs(&["gone", "turbo", "parakeet"]));
        assert_eq!(ids(&list), ["turbo", "parakeet"]);
    }

    #[test]
    fn cycle_list_falls_back_to_all_downloaded_by_name() {
        let expected = ["parakeet", "large", "turbo"];
        assert_eq!(ids(&cycle_list(&downloaded(), &[])), expected);
        assert_eq!(ids(&cycle_list(&downloaded(), &favs(&["large"]))), expected);
        assert_eq!(ids(&cycle_list(&downloaded(), &favs(&["large", "gone"]))), expected);
    }

    #[test]
    fn next_in_cycle_advances_and_wraps() {
        let list = cycle_list(&downloaded(), &favs(&["large", "parakeet", "turbo"]));
        assert_eq!(next_in_cycle(&list, "large").map(|m| m.id.as_str()), Some("parakeet"));
        assert_eq!(next_in_cycle(&list, "turbo").map(|m| m.id.as_str()), Some("large"));
    }

    #[test]
    fn next_in_cycle_starts_at_first_when_current_is_outside() {
        let list = cycle_list(&downloaded(), &favs(&["large", "parakeet"]));
        assert_eq!(next_in_cycle(&list, "turbo").map(|m| m.id.as_str()), Some("large"));
        assert_eq!(next_in_cycle(&list, "").map(|m| m.id.as_str()), Some("large"));
    }

    #[test]
    fn next_in_cycle_has_nothing_to_do_for_empty_or_single_current() {
        assert_eq!(next_in_cycle(&[], "large"), None);
        let single = vec![model("large", "Whisper Large v3", true)];
        assert_eq!(next_in_cycle(&single, "large"), None);
        assert_eq!(next_in_cycle(&single, "parakeet").map(|m| m.id.as_str()), Some("large"));
    }

    #[test]
    fn translation_target_prefers_remembered_model() {
        let mut models = downloaded();
        models.push(model("medium", "Whisper Medium", true));
        let target = translation_target(&models, &[], Some("medium"), "large");
        assert_eq!(target.map(|m| m.id.as_str()), Some("medium"));
    }

    #[test]
    fn translation_target_ignores_remembered_model_that_is_gone_or_cannot_translate() {
        let models = downloaded();
        assert_eq!(
            translation_target(&models, &[], Some("gone"), "parakeet").map(|m| m.id.as_str()),
            Some("large")
        );
        assert_eq!(
            translation_target(&models, &[], Some("turbo"), "parakeet").map(|m| m.id.as_str()),
            Some("large")
        );
    }

    #[test]
    fn translation_target_falls_back_current_then_favorite_then_name() {
        let mut models = downloaded();
        models.push(model("medium", "Whisper Medium", true));
        // Current model can translate: stay on it.
        assert_eq!(
            translation_target(&models, &[], None, "medium").map(|m| m.id.as_str()),
            Some("medium")
        );
        // First translation-capable favorite wins over name order.
        assert_eq!(
            translation_target(&models, &favs(&["parakeet", "medium", "large"]), None, "parakeet")
                .map(|m| m.id.as_str()),
            Some("medium")
        );
        // Otherwise the first translation-capable model by name.
        assert_eq!(
            translation_target(&models, &[], None, "parakeet").map(|m| m.id.as_str()),
            Some("large")
        );
    }

    #[test]
    fn translation_target_is_none_without_a_capable_model() {
        let models = vec![model("parakeet", "Parakeet V3", false)];
        assert_eq!(translation_target(&models, &[], Some("large"), "parakeet"), None);
    }

    #[test]
    fn return_target_restores_a_downloaded_different_model() {
        let models = downloaded();
        assert_eq!(
            return_target(&models, Some("parakeet"), "large").map(|m| m.id.as_str()),
            Some("parakeet")
        );
        assert_eq!(return_target(&models, None, "large"), None);
        assert_eq!(return_target(&models, Some("large"), "large"), None);
        assert_eq!(return_target(&models, Some("gone"), "large"), None);
    }

    #[test]
    fn toggle_favorite_appends_and_removes() {
        let mut favorites = favs(&["large"]);
        assert!(toggle_favorite(&mut favorites, "parakeet"));
        assert_eq!(favorites, favs(&["large", "parakeet"]));
        assert!(!toggle_favorite(&mut favorites, "large"));
        assert_eq!(favorites, favs(&["parakeet"]));
        assert!(toggle_favorite(&mut favorites, "large"));
        assert_eq!(favorites, favs(&["parakeet", "large"]));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test quick_switch`
Expected: compile errors `cannot find type 'ModelCandidate'` / `cannot find function 'cycle_list'`.

- [ ] **Step 3: Implement** — insert above the tests module:

```rust
/// The facts about a downloaded model that quick switching needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelCandidate {
    pub id: String,
    pub name: String,
    pub supports_translation: bool,
}

fn sorted_by_name(models: &[ModelCandidate]) -> Vec<ModelCandidate> {
    let mut sorted = models.to_vec();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    sorted
}

/// Models the next-model shortcut rotates through: downloaded favorites in
/// the order they were starred, or every downloaded model by name (the tray
/// order) when fewer than two favorites are downloaded.
pub fn cycle_list(downloaded: &[ModelCandidate], favorites: &[String]) -> Vec<ModelCandidate> {
    let starred: Vec<ModelCandidate> = favorites
        .iter()
        .filter_map(|id| downloaded.iter().find(|m| &m.id == id).cloned())
        .collect();
    if starred.len() >= 2 {
        starred
    } else {
        sorted_by_name(downloaded)
    }
}

/// The model after `current` in `list`, wrapping around; the first model
/// when `current` is not in the list. `None` when there is nothing to
/// switch to.
pub fn next_in_cycle<'a>(list: &'a [ModelCandidate], current: &str) -> Option<&'a ModelCandidate> {
    let next = match list.iter().position(|m| m.id == current) {
        Some(index) => &list[(index + 1) % list.len()],
        None => list.first()?,
    };
    (next.id != current).then_some(next)
}

/// Model to use when translation is turned on: the remembered translation
/// model, else the current model, else the first translation-capable
/// favorite, else the first translation-capable model by name. `None` when
/// no downloaded model can translate.
pub fn translation_target<'a>(
    downloaded: &'a [ModelCandidate],
    favorites: &[String],
    remembered: Option<&str>,
    current: &str,
) -> Option<&'a ModelCandidate> {
    let capable = |id: &str| {
        downloaded
            .iter()
            .find(|m| m.id == id && m.supports_translation)
    };
    remembered
        .and_then(capable)
        .or_else(|| capable(current))
        .or_else(|| favorites.iter().find_map(|id| capable(id.as_str())))
        .or_else(|| {
            downloaded
                .iter()
                .filter(|m| m.supports_translation)
                .min_by(|a, b| a.name.cmp(&b.name))
        })
}

/// Model to restore when translation is turned off. `None` means stay on the
/// current model.
pub fn return_target<'a>(
    downloaded: &'a [ModelCandidate],
    return_model: Option<&str>,
    current: &str,
) -> Option<&'a ModelCandidate> {
    let id = return_model.filter(|id| *id != current)?;
    downloaded.iter().find(|m| m.id == id)
}

/// Stars or unstars `model_id`, keeping star order. Returns whether the
/// model is starred afterwards.
pub fn toggle_favorite(favorites: &mut Vec<String>, model_id: &str) -> bool {
    if let Some(index) = favorites.iter().position(|id| id == model_id) {
        favorites.remove(index);
        false
    } else {
        favorites.push(model_id.to_string());
        true
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test quick_switch`
Expected: 12 passed. (`dead_code` warnings for the new pub fns are expected until Task 5.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/quick_switch.rs src-tauri/src/lib.rs
git commit -m "feat: add quick-switch model selection logic

Keeps the cycle order, translation-model choice and return target as pure
functions so every fallback branch is unit-tested independently of the
Tauri runtime.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Overlay notice (backend)

**Files:**

- Modify: `src-tauri/src/overlay.rs` (imports lines 1–6, `overlay_dimensions` ~57, new fn after `show_processing_overlay` ~629, tests module ~770)
- Modify: `src-tauri/src/lib.rs` (`collect_events!` ~768)

**Interfaces:**

- Produces: `crate::overlay::OverlayNoticeKind` (enum, snake_case serde), `crate::overlay::OverlayNoticeEvent { pub kind: OverlayNoticeKind, pub model: Option<String> }` (tauri-specta event, frontend `events.overlayNoticeEvent`), `pub fn show_notice_overlay(app_handle: &AppHandle, notice: OverlayNoticeEvent)`.

- [ ] **Step 1: Write the failing tests** — append inside the existing `mod tests` in `overlay.rs`:

```rust
    #[test]
    fn notice_kinds_serialize_as_snake_case() {
        use super::OverlayNoticeKind;
        let cases = [
            (OverlayNoticeKind::TranslationOn, "translation_on"),
            (OverlayNoticeKind::TranslationOff, "translation_off"),
            (OverlayNoticeKind::Model, "model"),
            (OverlayNoticeKind::ModelNoTranslation, "model_no_translation"),
            (OverlayNoticeKind::NoTranslationModel, "no_translation_model"),
            (OverlayNoticeKind::SwitchFailed, "switch_failed"),
        ];
        for (kind, expected) in cases {
            assert_eq!(serde_json::to_value(kind).unwrap(), serde_json::json!(expected));
        }
    }

    #[test]
    fn notice_uses_the_compact_overlay_size() {
        assert_eq!(
            super::overlay_dimensions("notice"),
            (super::OVERLAY_WIDTH, super::OVERLAY_HEIGHT)
        );
    }
```

(If the existing tests module uses `use super::*;`, drop the `super::` prefixes.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test notice_`
Expected: compile error `cannot find type 'OverlayNoticeKind'`.

- [ ] **Step 3: Implement** — add imports at the top of `overlay.rs`:

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;
```

After `show_processing_overlay` add:

```rust
/// What a transient overlay notice reports. The overlay localizes each kind
/// (see `src/overlay/notice.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OverlayNoticeKind {
    TranslationOn,
    TranslationOff,
    Model,
    ModelNoTranslation,
    NoTranslationModel,
    SwitchFailed,
}

/// Short-lived overlay message shown after a quick-switch shortcut.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct OverlayNoticeEvent {
    pub kind: OverlayNoticeKind,
    /// Display name of the model the notice is about, when there is one.
    pub model: Option<String>,
}

const NOTICE_VISIBLE_MS: u64 = 1500;

/// Shows `notice` in the overlay for a moment, then hides it — unless a newer
/// overlay session (e.g. a recording) took over in the meantime. Respects the
/// overlay being turned off.
pub fn show_notice_overlay(app_handle: &AppHandle, notice: OverlayNoticeEvent) {
    if settings::get_settings(app_handle).overlay_style == OverlayStyle::None {
        return;
    }
    let handle = app_handle.clone();
    let _ = app_handle.run_on_main_thread(move || {
        // Payload first, so the overlay has the text when "show-overlay" lands.
        let _ = notice.emit_to(&handle, "recording_overlay");
        show_overlay_state_on_main(&handle, "notice");
        // Shows are serialized on the main thread, so this is our session.
        let generation = OVERLAY_SHOW_GENERATION.load(Ordering::SeqCst);
        let hide_handle = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(NOTICE_VISIBLE_MS));
            if OVERLAY_SHOW_GENERATION.load(Ordering::SeqCst) == generation {
                hide_recording_overlay(&hide_handle);
            }
        });
    });
}
```

Note: `overlay_dimensions` already returns the compact size for any non-`"streaming"` state; no change needed there. If `use tauri_specta::Event;` clashes with an existing `Event` name in the file, use `use tauri_specta::Event as _;`.

In `lib.rs` `collect_events![...]` add `overlay::OverlayNoticeEvent,`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test notice_` then `cargo test`
Expected: PASS, full suite green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/overlay.rs src-tauri/src/lib.rs
git commit -m "feat(overlay): add a transient notice state

Quick-switch shortcuts need visible confirmation; reusing the overlay
window with its generation guard lets a recording pre-empt the notice
without a stale hide.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Tray busy predicate and translation check item

**Files:**

- Modify: `src-tauri/src/tray.rs` (`MenuInputs` ~57, `set_tray_state` area ~218, `compute_desired` ~312, `build_menu` idle branch ~533–570, tests ~670)
- Modify: `src/i18n/locales/en/translation.json`, `src/i18n/locales/ru/translation.json` (`tray` section)

**Interfaces:**

- Produces: `pub fn tray::is_busy(app: &AppHandle) -> bool`, `pub fn tray::invalidate_tray_menu(app: &AppHandle)`, tray menu item id `"toggle_translation"`, tray string field `strings.translate_to_english` (generated by `build.rs` from key `tray.translateToEnglish`).

- [ ] **Step 1: Write the failing test** — in `tray.rs` tests, add `translate_to_english: false,` to the `inputs()` helper literal and append:

```rust
    #[test]
    fn translation_flag_changes_menu_inputs() {
        let mut translating = inputs(false);
        translating.translate_to_english = true;
        assert_ne!(inputs(false), translating);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test translation_flag_changes_menu_inputs`
Expected: compile error `struct 'MenuInputs' has no field named 'translate_to_english'`.

- [ ] **Step 3: Implement**
  1. Add to `src/i18n/locales/en/translation.json` → `"tray"`: `"translateToEnglish": "Translate to English"`; to `ru` → `"tray"`: `"translateToEnglish": "Переводить на английский"`.
  2. `MenuInputs`: add field `translate_to_english: bool,` (after `update_checks_enabled`).
  3. `compute_desired`: add `translate_to_english: settings.translate_to_english,` to the `MenuInputs` literal.
  4. After `refresh_tray_icon` add:

```rust
/// Whether Handy is recording or transcribing — the state in which the tray
/// swaps the model controls for "Cancel". Quick-switch shortcuts use it to
/// stay out of the way of an active capture.
pub fn is_busy(app: &AppHandle) -> bool {
    app.try_state::<TrayState>()
        .map(|state| state.lock().icon_state.is_busy())
        .unwrap_or(false)
}

/// Forces the next sync to rebuild the menu even if its inputs are unchanged.
/// Needed after a check item was clicked but the action did not change the
/// setting: the native item has already flipped its own check mark.
pub fn invalidate_tray_menu(app: &AppHandle) {
    sync_tray_with(app, |inner| inner.applied_menu = None);
}
```

5. `build_menu`, idle branch: after `unload_model_i` add

```rust
        let translate_i = CheckMenuItem::with_id(
            app,
            "toggle_translation",
            &strings.translate_to_english,
            true,
            inputs.translate_to_english,
            None::<&str>,
        )?;
```

and insert `&translate_i,` into that branch's `Menu::with_items` right after `&unload_model_i,`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test tray` then `cargo test`
Expected: PASS. (`is_busy`/`invalidate_tray_menu` are unused until Task 5 — warnings OK.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tray.rs src/i18n/locales/en/translation.json src/i18n/locales/ru/translation.json
git commit -m "feat(tray): expose busy state and add a translation check item

The shortcuts must ignore presses mid-capture exactly when the tray hides
its model controls, and the tray should show and toggle the translation
mode that is otherwise invisible.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Wire quick switching into the app

**Files:**

- Modify: `src-tauri/src/quick_switch.rs` (app operations)
- Modify: `src-tauri/src/commands/models.rs` (`switch_active_model` ~95–159, new command)
- Modify: `src-tauri/src/actions.rs` (actions before `ACTION_MAP` ~930, tests)
- Modify: `src-tauri/src/lib.rs` (`on_menu_event` ~287–345, `collect_commands!` ~734)
- Regenerate: `src/bindings.ts`

**Interfaces:**

- Consumes: Task 2 functions, `overlay::show_notice_overlay`/`OverlayNoticeEvent`/`OverlayNoticeKind` (Task 3), `tray::is_busy`/`invalidate_tray_menu`/`update_tray_menu` (Task 4), `commands::models::switch_active_model(app: &AppHandle, model_id: &str) -> Result<(), String>`.
- Produces: `pub fn quick_switch::cycle_model(app: &AppHandle)`, `pub fn quick_switch::toggle_translation_mode(app: &AppHandle)`, `pub fn quick_switch::remember_translation_model(settings: &mut AppSettings, model: &ModelCandidate) -> bool`, Tauri command `toggle_favorite_model(model_id: String) -> Result<Vec<String>, String>` (frontend `commands.toggleFavoriteModel`).

- [ ] **Step 1: Write the failing tests** — append to `quick_switch.rs` tests:

```rust
    #[test]
    fn remembers_translation_model_only_while_translating_with_a_capable_model() {
        let mut settings = crate::settings::get_default_settings();
        let large = model("large", "Whisper Large v3", true);
        let parakeet = model("parakeet", "Parakeet V3", false);

        assert!(!remember_translation_model(&mut settings, &large));
        assert_eq!(settings.translation_model, None);

        settings.translate_to_english = true;
        assert!(!remember_translation_model(&mut settings, &parakeet));
        assert!(remember_translation_model(&mut settings, &large));
        assert_eq!(settings.translation_model.as_deref(), Some("large"));
        assert!(!remember_translation_model(&mut settings, &large));
    }
```

and to `actions.rs` tests:

```rust
    #[test]
    fn every_default_binding_has_an_action() {
        for id in crate::settings::get_default_settings().bindings.keys() {
            assert!(
                super::ACTION_MAP.contains_key(id),
                "no action registered for default binding '{id}'"
            );
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test remembers_translation_model every_default_binding_has_an_action`
Expected: compile error `cannot find function 'remember_translation_model'`; after stubbing, `every_default_binding_has_an_action` fails with `no action registered for default binding 'toggle_translation'`.

- [ ] **Step 3: Implement**

  3a. In `quick_switch.rs`, above `ModelCandidate`, add imports:

```rust
use crate::commands::models::switch_active_model;
use crate::managers::model::ModelManager;
use crate::overlay::{show_notice_overlay, OverlayNoticeEvent, OverlayNoticeKind};
use crate::settings::{get_settings, write_settings, AppSettings};
use crate::tray;
use log::{info, warn};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
```

and after `toggle_favorite` add:

```rust
/// Records `model` as the translation model when translation is on and the
/// model can translate. Returns whether `settings` changed.
pub fn remember_translation_model(settings: &mut AppSettings, model: &ModelCandidate) -> bool {
    let remember = settings.translate_to_english
        && model.supports_translation
        && settings.translation_model.as_deref() != Some(model.id.as_str());
    if remember {
        settings.translation_model = Some(model.id.clone());
    }
    remember
}

fn downloaded_candidates(app: &AppHandle) -> Vec<ModelCandidate> {
    app.state::<Arc<ModelManager>>()
        .get_available_models()
        .into_iter()
        .filter(|m| m.is_downloaded)
        .map(|m| ModelCandidate {
            id: m.id,
            name: m.name,
            supports_translation: m.supports_translation,
        })
        .collect()
}

fn notify(app: &AppHandle, kind: OverlayNoticeKind, model: Option<&ModelCandidate>) {
    show_notice_overlay(
        app,
        OverlayNoticeEvent {
            kind,
            model: model.map(|m| m.name.clone()),
        },
    );
}

fn emit_translation_changed(app: &AppHandle, enabled: bool) {
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({ "setting": "translate_to_english", "value": enabled }),
    );
}

/// Next-model shortcut: switch to the next model in the cycle. Model loading
/// blocks, so the work runs on its own thread like the tray's model menu.
pub fn cycle_model(app: &AppHandle) {
    if tray::is_busy(app) {
        info!("Ignoring next-model shortcut while recording or transcribing.");
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let settings = get_settings(&app);
        let list = cycle_list(&downloaded_candidates(&app), &settings.favorite_models);
        let Some(next) = next_in_cycle(&list, &settings.selected_model).cloned() else {
            info!("Next-model shortcut: no other model to switch to.");
            return;
        };
        match switch_active_model(&app, &next.id) {
            Ok(()) => {
                let kind = if settings.translate_to_english && !next.supports_translation {
                    OverlayNoticeKind::ModelNoTranslation
                } else {
                    OverlayNoticeKind::Model
                };
                notify(&app, kind, Some(&next));
            }
            Err(e) => {
                warn!("Next-model shortcut: switching to {} failed: {}", next.id, e);
                notify(&app, OverlayNoticeKind::SwitchFailed, Some(&next));
            }
        }
        tray::update_tray_menu(&app);
    });
}

/// Translation shortcut and tray item: turn translation mode on or off.
pub fn toggle_translation_mode(app: &AppHandle) {
    if tray::is_busy(app) {
        info!("Ignoring translation shortcut while recording or transcribing.");
        tray::invalidate_tray_menu(app);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if get_settings(&app).translate_to_english {
            disable_translation(&app);
        } else {
            enable_translation(&app);
        }
        // The tray check item flips itself on click; rebuild so it always
        // matches the stored setting, including when nothing changed.
        tray::invalidate_tray_menu(&app);
    });
}

fn enable_translation(app: &AppHandle) {
    let settings = get_settings(app);
    let current = settings.selected_model.clone();
    let downloaded = downloaded_candidates(app);
    let Some(target) = translation_target(
        &downloaded,
        &settings.favorite_models,
        settings.translation_model.as_deref(),
        &current,
    )
    .cloned() else {
        notify(app, OverlayNoticeKind::NoTranslationModel, None);
        return;
    };

    if target.id != current {
        if let Err(e) = switch_active_model(app, &target.id) {
            warn!("Translation shortcut: switching to {} failed: {}", target.id, e);
            notify(app, OverlayNoticeKind::SwitchFailed, Some(&target));
            return;
        }
    }

    let mut settings = get_settings(app);
    settings.translate_to_english = true;
    settings.translation_model = Some(target.id.clone());
    settings.translation_return_model = Some(current);
    write_settings(app, settings);
    emit_translation_changed(app, true);
    notify(app, OverlayNoticeKind::TranslationOn, Some(&target));
}

fn disable_translation(app: &AppHandle) {
    let mut settings = get_settings(app);
    let return_model = settings.translation_return_model.take();
    let current = settings.selected_model.clone();
    settings.translate_to_english = false;
    write_settings(app, settings);
    emit_translation_changed(app, false);

    let downloaded = downloaded_candidates(app);
    let final_model = match return_target(&downloaded, return_model.as_deref(), &current) {
        Some(target) => {
            let target = target.clone();
            if let Err(e) = switch_active_model(app, &target.id) {
                warn!("Translation shortcut: returning to {} failed: {}", target.id, e);
                notify(app, OverlayNoticeKind::SwitchFailed, Some(&target));
                return;
            }
            Some(target)
        }
        None => downloaded.into_iter().find(|m| m.id == current),
    };
    notify(app, OverlayNoticeKind::TranslationOff, final_model.as_ref());
}
```

3b. In `commands/models.rs`, rename the existing `pub fn switch_active_model` to `fn switch_active_model_inner` (body unchanged) and add above it:

```rust
/// Validates the model, updates the persisted setting, and loads the model
/// unless the unload timeout is set to "Immediately". On success, remembers
/// the model as the translation model when translation is on.
pub fn switch_active_model(app: &AppHandle, model_id: &str) -> Result<(), String> {
    switch_active_model_inner(app, model_id)?;
    let model_manager = app.state::<Arc<ModelManager>>();
    if let Some(info) = model_manager.get_model_info(model_id) {
        let candidate = crate::quick_switch::ModelCandidate {
            id: info.id,
            name: info.name,
            supports_translation: info.supports_translation,
        };
        let mut settings = get_settings(app);
        if crate::quick_switch::remember_translation_model(&mut settings, &candidate) {
            write_settings(app, settings);
        }
    }
    Ok(())
}
```

(Move the existing doc comment from the old function onto this wrapper; keep a one-line `/// Does the actual switch; see [`switch_active_model`].` on the inner fn.)

and at the end of the file:

```rust
/// Stars or unstars a model for the next-model shortcut. Returns the
/// favorites in star order.
#[tauri::command]
#[specta::specta]
pub fn toggle_favorite_model(app_handle: AppHandle, model_id: String) -> Result<Vec<String>, String> {
    let mut settings = get_settings(&app_handle);
    crate::quick_switch::toggle_favorite(&mut settings.favorite_models, &model_id);
    let favorites = settings.favorite_models.clone();
    write_settings(&app_handle, settings);
    let _ = app_handle.emit(
        "settings-changed",
        serde_json::json!({ "setting": "favorite_models" }),
    );
    Ok(favorites)
}
```

3c. In `actions.rs`, before `// Static Action Map`:

```rust
// Toggle Translation Action
struct ToggleTranslationAction;

impl ShortcutAction for ToggleTranslationAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        crate::quick_switch::toggle_translation_mode(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Acts on press only
    }
}

// Cycle Model Action
struct CycleModelAction;

impl ShortcutAction for CycleModelAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        crate::quick_switch::cycle_model(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Acts on press only
    }
}
```

and inside `ACTION_MAP` before `"test"`:

```rust
    map.insert(
        "toggle_translation".to_string(),
        Arc::new(ToggleTranslationAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "cycle_model".to_string(),
        Arc::new(CycleModelAction) as Arc<dyn ShortcutAction>,
    );
```

3d. In `lib.rs` `on_menu_event`, before `id if id.starts_with("model_select:")`:

```rust
            "toggle_translation" => {
                quick_switch::toggle_translation_mode(app);
            }
```

and in `collect_commands![...]` after `commands::models::set_active_model,` add `commands::models::toggle_favorite_model,`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test` then `cargo clippy --all-targets 2>&1 | grep -E "quick_switch|overlay.rs|tray.rs|actions.rs|models.rs|settings.rs" -A3`
Expected: all tests PASS; no clippy findings in touched files (pre-existing findings elsewhere are out of scope).

- [ ] **Step 5: Regenerate TypeScript bindings** (debug builds export `src/bindings.ts` at startup; `--list-models` runs headless and exits)

```bash
S="$HOME/Library/Application Support/com.pais.handy/settings_store.json"
B=/private/tmp/claude-501/-Users-nik/44b4f835-49af-4b48-a7c6-b746fd6c416a/scratchpad/settings_store.backup.json
cp "$S" "$B"
cargo run -- --list-models
cmp -s "$S" "$B" || { cp "$B" "$S"; echo "restored settings_store.json"; }
git diff --stat ../src/bindings.ts
grep -n "toggleFavoriteModel\|favorite_models\|overlayNoticeEvent\|translation_return_model" ../src/bindings.ts
```

Expected: `bindings.ts` changed; all four names found.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/quick_switch.rs src-tauri/src/commands/models.rs src-tauri/src/actions.rs src-tauri/src/lib.rs src/bindings.ts
git commit -m "feat: wire translation and next-model shortcuts

Both shortcuts and the tray item reuse the tray's switch_active_model path
so loading, reverting and busy handling stay identical; the translation
model is remembered inside that path so every way of switching feeds it.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Overlay notice (frontend)

**Files:**

- Create: `src/overlay/notice.ts`, `src/overlay/notice.test.ts`
- Modify: `src/overlay/RecordingOverlay.tsx`, `src/overlay/RecordingOverlay.css`, `package.json` (scripts)
- Modify: `src/i18n/locales/en/translation.json`, `src/i18n/locales/ru/translation.json` (`overlay` section)

**Interfaces:**

- Consumes: `OverlayNoticeEvent`, `events.overlayNoticeEvent` from `@/bindings` (Task 5 regeneration).
- Produces: `noticeLabel(t, notice): string`, `isTranslationNotice(notice): boolean`.

- [ ] **Step 1: Write the failing test** — `src/overlay/notice.test.ts`:

```ts
import assert from "node:assert/strict";
import type { OverlayNoticeEvent } from "@/bindings";
import { isTranslationNotice, noticeLabel } from "./notice";

const t = (key: string, options?: Record<string, unknown>) =>
  options ? `${key}(${JSON.stringify(options)})` : key;

const notice = (
  kind: OverlayNoticeEvent["kind"],
  model: string | null,
): OverlayNoticeEvent => ({ kind, model });

assert.equal(
  noticeLabel(t, notice("translation_on", "Whisper Large v3")),
  "overlay.notice.translationOn · Whisper Large v3",
);
assert.equal(
  noticeLabel(t, notice("translation_off", null)),
  "overlay.notice.translationOff",
);
assert.equal(noticeLabel(t, notice("model", "Parakeet V3")), "Parakeet V3");
assert.equal(
  noticeLabel(t, notice("model_no_translation", "Parakeet V3")),
  'overlay.notice.modelNoTranslation({"model":"Parakeet V3"})',
);
assert.equal(
  noticeLabel(t, notice("no_translation_model", null)),
  "overlay.notice.noTranslationModel",
);
assert.equal(
  noticeLabel(t, notice("switch_failed", "Whisper Large v3")),
  'overlay.notice.switchFailed({"model":"Whisper Large v3"})',
);
assert.equal(isTranslationNotice(notice("translation_on", null)), true);
assert.equal(isTranslationNotice(notice("no_translation_model", null)), true);
assert.equal(isTranslationNotice(notice("model", "Parakeet V3")), false);

console.log("overlay notice tests passed");
```

Add to `package.json` scripts after `test:keyboard`: `"test:overlay": "bun src/overlay/notice.test.ts",`.

- [ ] **Step 2: Run test to verify it fails**

Run: `bun run test:overlay`
Expected: FAIL — `Cannot find module './notice'`.

- [ ] **Step 3: Implement** — `src/overlay/notice.ts`:

```ts
import type { OverlayNoticeEvent } from "@/bindings";

type Translate = (key: string, options?: Record<string, unknown>) => string;

/** One-line, localized text for a quick-switch notice. */
export const noticeLabel = (
  t: Translate,
  notice: OverlayNoticeEvent,
): string => {
  const withModel = (label: string) =>
    notice.model ? `${label} · ${notice.model}` : label;
  const model = notice.model ?? "";
  switch (notice.kind) {
    case "translation_on":
      return withModel(t("overlay.notice.translationOn"));
    case "translation_off":
      return withModel(t("overlay.notice.translationOff"));
    case "model":
      return model;
    case "model_no_translation":
      return t("overlay.notice.modelNoTranslation", { model });
    case "no_translation_model":
      return t("overlay.notice.noTranslationModel");
    case "switch_failed":
      return t("overlay.notice.switchFailed", { model });
  }
};

/** Translation notices get the languages icon, model notices the waveform. */
export const isTranslationNotice = (notice: OverlayNoticeEvent): boolean =>
  notice.kind === "translation_on" ||
  notice.kind === "translation_off" ||
  notice.kind === "no_translation_model";
```

Strings — `en` `"overlay"` section add:

```json
"notice": {
  "translationOn": "Translation on",
  "translationOff": "Translation off",
  "modelNoTranslation": "{{model}} · doesn't translate",
  "noTranslationModel": "No downloaded model can translate",
  "switchFailed": "Couldn't switch to {{model}}"
}
```

`ru` `"overlay"` section add:

```json
"notice": {
  "translationOn": "Перевод: вкл",
  "translationOff": "Перевод: выкл",
  "modelNoTranslation": "{{model}} · не переводит",
  "noTranslationModel": "Нет скачанной модели с переводом",
  "switchFailed": "Не удалось переключиться на {{model}}"
}
```

`RecordingOverlay.tsx`:

- imports: add `import { AudioLines, Languages } from "lucide-react";`, `import type { OverlayNoticeEvent } from "@/bindings";` (merge into the existing type import), `import { isTranslationNotice, noticeLabel } from "./notice";`
- `type OverlayState = "recording" | "streaming" | "transcribing" | "processing" | "notice";`
- state: `const [notice, setNotice] = useState<OverlayNoticeEvent | null>(null);`
- in `setupEventListeners`, after `unlistenPhase`:

```tsx
const unlistenNotice = await events.overlayNoticeEvent.listen((event) => {
  setNotice(event.payload);
});
```

    and call `unlistenNotice();` in the returned cleanup.

- before the `// ---- Minimal overlay` block:

```tsx
// ---- Notice: one icon + one line, shown briefly after a quick-switch
// shortcut. Same pill and fade as the minimal overlay.
if (state === "notice") {
  const Icon = notice && isTranslationNotice(notice) ? Languages : AudioLines;
  return (
    <div
      dir={direction}
      className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
    >
      <div className="scard compact">
        <div className="sbase">
          <div className="sbase-l">
            <Icon className="snotice-icon" aria-hidden="true" />
          </div>
          <span className="swork-label">
            {notice ? noticeLabel(t, notice) : ""}
          </span>
          <div className="sbase-r" />
        </div>
      </div>
    </div>
  );
}
```

`RecordingOverlay.css` — after `.swork-label { ... }`:

```css
.snotice-icon {
  width: 14px;
  height: 14px;
  color: var(--s-muted);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `bun run test:overlay && bun run lint && bunx tsc --noEmit`
Expected: `overlay notice tests passed`; lint and tsc clean.

- [ ] **Step 5: Commit**

```bash
git add src/overlay/notice.ts src/overlay/notice.test.ts src/overlay/RecordingOverlay.tsx src/overlay/RecordingOverlay.css package.json src/i18n/locales/en/translation.json src/i18n/locales/ru/translation.json
git commit -m "feat(overlay): render quick-switch notices

Shows which model or translation mode a shortcut just selected, localized
in the overlay so the backend only sends the kind and the model name.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Star button and shortcut rows

**Files:**

- Modify: `src/components/onboarding/ModelCard.tsx` (props ~6–34, handlers ~140, bottom row ~277)
- Modify: `src/components/settings/models/ModelsSettings.tsx` (imports, component body, downloaded `ModelCard` at ~410)
- Modify: `src/components/settings/general/GeneralSettings.tsx`
- Modify: `src/i18n/locales/en/translation.json`, `src/i18n/locales/ru/translation.json`

**Interfaces:**

- Consumes: `commands.toggleFavoriteModel(modelId)`, `Settings.favorite_models` (Task 5 bindings).
- Produces: `ModelCard` props `favoriteRank?: number | null`, `onToggleFavorite?: (modelId: string) => void`.

- [ ] **Step 1: Add strings** — `en`:
  - `settings.general.shortcut.bindings.toggle_translation`: `{ "name": "Translation Shortcut", "description": "Turns translation to English on or off. Switches to the last model used for translation, and back when turned off." }`
  - `settings.general.shortcut.bindings.cycle_model`: `{ "name": "Next Model Shortcut", "description": "Switches to the next starred model. With fewer than two starred, cycles through all downloaded models." }`
  - `modelSelector.favorite`: `{ "add": "Star for the Next Model shortcut", "remove": "Unstar" }`

  `ru`:
  - `toggle_translation`: `{ "name": "Горячая клавиша перевода", "description": "Включает и выключает перевод на английский. Переключается на последнюю модель, на которой шёл перевод, а при выключении возвращает прежнюю." }`
  - `cycle_model`: `{ "name": "Горячая клавиша следующей модели", "description": "Переключает на следующую модель со звёздочкой. Если отмечено меньше двух, перебирает все скачанные модели." }`
  - `modelSelector.favorite`: `{ "add": "Отметить для горячей клавиши следующей модели", "remove": "Снять звёздочку" }`

- [ ] **Step 2: `ModelCard.tsx`**
  - add `Star` to the `lucide-react` import;
  - `ModelCardProps`: add

```tsx
  /** 1-based position in the next-model cycle; null when not starred. */
  favoriteRank?: number | null;
  onToggleFavorite?: (modelId: string) => void;
```

- destructure `favoriteRank = null, onToggleFavorite,` in the component params;
- next to `handleDelete`:

```tsx
const handleToggleFavorite = (e: React.MouseEvent) => {
  e.stopPropagation();
  onToggleFavorite?.(model.id);
};
const isFavorite = favoriteRank !== null;
```

- in the bottom row, immediately before the delete `Button`:

```tsx
{
  onToggleFavorite && (status === "available" || status === "active") && (
    <Button
      variant="ghost"
      size="sm"
      onClick={handleToggleFavorite}
      aria-pressed={isFavorite}
      title={
        isFavorite
          ? t("modelSelector.favorite.remove")
          : t("modelSelector.favorite.add")
      }
      className={`flex items-center gap-1 ${showModelSize ? "" : "ms-auto"} ${
        isFavorite
          ? "text-logo-primary"
          : "text-text/50 hover:text-logo-primary"
      } hover:bg-logo-primary/10`}
    >
      <Star
        className="w-3.5 h-3.5"
        fill={isFavorite ? "currentColor" : "none"}
      />
      {isFavorite && (
        <span className="text-xs tabular-nums">{favoriteRank}</span>
      )}
    </Button>
  );
}
```

- [ ] **Step 3: `ModelsSettings.tsx`**
  - imports: `import { commands } from "@/bindings";` and `import { useSettingsStore } from "@/stores/settingsStore";`
  - module level (above the component): `const NO_FAVORITES: string[] = [];` (stable fallback — a fresh `[]` inside a Zustand 5 selector re-renders forever)
  - in the component, after `useModelStore()`:

```tsx
const favoriteModels =
  useSettingsStore((state) => state.settings?.favorite_models) ?? NO_FAVORITES;
const refreshSettings = useSettingsStore((state) => state.refreshSettings);

const favoriteRank = (modelId: string): number | null => {
  const index = favoriteModels.indexOf(modelId);
  return index === -1 ? null : index + 1;
};

const handleToggleFavorite = async (modelId: string) => {
  const result = await commands.toggleFavoriteModel(modelId);
  if (result.status === "error") {
    console.error(`Failed to toggle favorite for ${modelId}:`, result.error);
    return;
  }
  await refreshSettings();
};
```

- on the downloaded-models `ModelCard` (the one with `showRecommended={false}` at ~410) add `favoriteRank={favoriteRank(model.id)}` and `onToggleFavorite={handleToggleFavorite}`. Do not add them to the available-models list.

- [ ] **Step 4: `GeneralSettings.tsx`** — after the cancel row:

```tsx
        <ShortcutInput shortcutId="toggle_translation" grouped={true} />
        <ShortcutInput shortcutId="cycle_model" grouped={true} />
```

- [ ] **Step 5: Verify**

Run: `bun run lint && bunx tsc --noEmit && bun run format:frontend && git diff --stat`
Expected: clean lint/tsc; Prettier only touches files from this task.

- [ ] **Step 6: Commit**

```bash
git add src/components/onboarding/ModelCard.tsx src/components/settings/models/ModelsSettings.tsx src/components/settings/general/GeneralSettings.tsx src/i18n/locales/en/translation.json src/i18n/locales/ru/translation.json
git commit -m "feat(ui): star models for the cycle and show the new shortcuts

The star number mirrors the cycle order so it is visible which model comes
next; the shortcut rows reuse ShortcutInput so rebinding works as for the
existing shortcuts.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Translations for the remaining 24 locales

**Files:**

- Modify: `src/i18n/locales/{ar,bg,ca,cs,da,de,es,fr,he,hi,id,it,ja,ko,ne,nl,pl,pt,sv,tr,uk,vi,zh,zh-TW}/translation.json`

New keys (source = `en`, already added in Tasks 4, 6, 7): `tray.translateToEnglish`, `overlay.notice.{translationOn,translationOff,modelNoTranslation,noTranslationModel,switchFailed}`, `settings.general.shortcut.bindings.toggle_translation.{name,description}`, `settings.general.shortcut.bindings.cycle_model.{name,description}`, `modelSelector.favorite.{add,remove}` — 13 strings per locale.

- [ ] **Step 1: Confirm the check fails**

Run: `bun run check:translations`
Expected: FAIL listing the 13 missing keys for each of the 24 locales.

- [ ] **Step 2: Write translations** — for each locale, translate the 13 `en` strings. Rules: keep `{{model}}` verbatim; reuse the locale's existing term for "shortcut"/"hotkey" from its `settings.general.shortcut.bindings.transcribe.name`, for "model" from `tray.model`, for "translate" from `modelSelector.capabilities.translate`; keep the `·` separator. Put them in a scratch JSON `{ "<locale>": { "<dotted.key>": "<text>" } }` at `$SCRATCH/i18n-quick-switch.json` (`SCRATCH=/private/tmp/claude-501/-Users-nik/44b4f835-49af-4b48-a7c6-b746fd6c416a/scratchpad`).

- [ ] **Step 3: Merge with a script (not committed)** — `$SCRATCH/merge_i18n.py`:

```python
import json, sys, pathlib
root = pathlib.Path("src/i18n/locales")
table = json.load(open(sys.argv[1], encoding="utf-8"))
for locale, entries in table.items():
    path = root / locale / "translation.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    for dotted, text in entries.items():
        node = data
        *parents, leaf = dotted.split(".")
        for key in parents:
            node = node.setdefault(key, {})
        node[leaf] = text
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"{locale}: {len(entries)} keys")
```

Run: `python3 $SCRATCH/merge_i18n.py $SCRATCH/i18n-quick-switch.json && bun run format:frontend`

- [ ] **Step 4: Verify**

Run: `bun run check:translations && git diff --stat -- src/i18n/locales | tail -1`
Expected: check passes; diff touches exactly 24 files, only additions (`git diff -- src/i18n/locales | grep '^-' | grep -v '^---'` prints nothing except possible trailing-comma lines).

- [ ] **Step 5: Commit**

```bash
git add src/i18n/locales
git commit -m "i18n: translate quick-switch strings into all locales

Keeps check:translations green and avoids empty tray labels, since the
tray strings fall back to an empty string when a locale lacks a key.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Full verification

**Files:** none (fixes only if a check fails).

- [ ] **Step 1: Backend**

Run: `cargo fmt -- --check && cargo test && cargo clippy --all-targets 2>&1 | tail -5`
Expected: fmt clean; all tests pass (274 baseline + new); clippy introduces no new findings in touched files.

- [ ] **Step 2: Frontend**

Run: `bun run format:check && bun run lint && bun run build && bun run check:translations && bun run test:keyboard && bun run test:overlay`
Expected: every command exits 0.

- [ ] **Step 3: Code review** — invoke `superpowers:requesting-code-review` over `git diff main...feat/model-shortcuts`, fix confirmed findings with a test first where behavior changes, re-run Steps 1–2.

- [ ] **Step 4: Manual scenarios** (blocked until the Handy.satoru identity decision in epic `handy-hud`: either the fork gets its own identifier, or quit Handy 0.9.6 and back up `settings_store.json` first). Run `CMAKE_POLICY_VERSION_MINIMUM=3.5 bun run tauri dev` and check:
  1. Parakeet active, ⌃⌥T → large-v3 loads, notice «Перевод: вкл · Whisper Large v3», tray check on, Settings translation toggle on.
  2. ⌃⌥T again → back to Parakeet, notice «Перевод: выкл · Parakeet V3», check off.
  3. Star large-v3 then Parakeet (★1, ★2 in that order); ⌃⌥M alternates between them; unstar one → cycle covers all downloaded by name.
  4. Hold the transcribe shortcut and press ⌃⌥M/⌃⌥T mid-recording → nothing happens; no notice over the recording pill.
  5. Overlay style "None" → shortcuts still switch, no notice.
  6. Rebind ⌃⌥T in Settings → new combo works, old one does not.
