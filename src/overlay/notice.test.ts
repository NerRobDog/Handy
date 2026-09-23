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
