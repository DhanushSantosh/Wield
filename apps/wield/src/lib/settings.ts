const BLUR_TO_HIDE_KEY = "wield.settings.blurToHide";

export function getBlurToHide(): boolean {
  try {
    const stored = window.localStorage.getItem(BLUR_TO_HIDE_KEY);
    if (stored === null) {
      return true;
    }
    const parsed: unknown = JSON.parse(stored);
    return typeof parsed === "boolean" ? parsed : true;
  } catch {
    return true;
  }
}

export function setBlurToHide(value: boolean): void {
  try {
    window.localStorage.setItem(BLUR_TO_HIDE_KEY, JSON.stringify(value));
  } catch {
    // Palette-behaviour settings are a non-essential enhancement; storage
    // failures must not block anything.
  }
}
