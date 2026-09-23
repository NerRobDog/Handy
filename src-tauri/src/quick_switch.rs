//! Quick switching between transcription models from global shortcuts and
//! the tray: cycling through starred models and toggling translation mode.

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
        assert_eq!(
            ids(&cycle_list(&downloaded(), &favs(&["large", "gone"]))),
            expected
        );
    }

    #[test]
    fn next_in_cycle_advances_and_wraps() {
        let list = cycle_list(&downloaded(), &favs(&["large", "parakeet", "turbo"]));
        assert_eq!(
            next_in_cycle(&list, "large").map(|m| m.id.as_str()),
            Some("parakeet")
        );
        assert_eq!(
            next_in_cycle(&list, "turbo").map(|m| m.id.as_str()),
            Some("large")
        );
    }

    #[test]
    fn next_in_cycle_starts_at_first_when_current_is_outside() {
        let list = cycle_list(&downloaded(), &favs(&["large", "parakeet"]));
        assert_eq!(
            next_in_cycle(&list, "turbo").map(|m| m.id.as_str()),
            Some("large")
        );
        assert_eq!(
            next_in_cycle(&list, "").map(|m| m.id.as_str()),
            Some("large")
        );
    }

    #[test]
    fn next_in_cycle_has_nothing_to_do_for_empty_or_single_current() {
        assert_eq!(next_in_cycle(&[], "large"), None);
        let single = vec![model("large", "Whisper Large v3", true)];
        assert_eq!(next_in_cycle(&single, "large"), None);
        assert_eq!(
            next_in_cycle(&single, "parakeet").map(|m| m.id.as_str()),
            Some("large")
        );
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
            translation_target(
                &models,
                &favs(&["parakeet", "medium", "large"]),
                None,
                "parakeet"
            )
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
        assert_eq!(
            translation_target(&models, &[], Some("large"), "parakeet"),
            None
        );
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
