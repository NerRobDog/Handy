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
