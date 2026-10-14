import { createI18n } from "vue-i18n";

import en from "./shared/locales/en.json";
import zh from "./shared/locales/zh.json";
import zhHk from "./shared/locales/zh-hk.json";
import zhTw from "./shared/locales/zh-tw.json";
import zhSs from "./shared/locales/zh-ss.json";
import cat from "./shared/locales/cat.json";
import lzh from "./shared/locales/lzh.json";

const getSystemLocale = (): AppLocale => {
  return navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
};

const LOCALES = ["en", "zh", "cat", "zh-hk", "zh-tw", "zh-ss", "lzh"] as const;
export type AppLocale = (typeof LOCALES)[number];

export const isLocale = (value: string | null): value is AppLocale =>
  (LOCALES as readonly (string | null)[]).includes(value);

/** Maps the stored language setting ("system" or a locale) to a locale. */
export const resolveLocale = (saved: string | null): AppLocale =>
  isLocale(saved) ? saved : getSystemLocale();

export const i18n = createI18n({
  legacy: false,
  locale: resolveLocale(localStorage.getItem("micyou_language")),
  fallbackLocale: "en",
  messages: { en, zh, "zh-hk": zhHk, "zh-tw": zhTw, "zh-ss": zhSs, cat, lzh },
});

export function setLocale(setting: string) {
  i18n.global.locale.value = resolveLocale(setting) as typeof i18n.global.locale.value;
}

// Windows share localStorage; follow a language change made in another window.
window.addEventListener("storage", (event) => {
  if (event.key === "micyou_language") setLocale(event.newValue ?? "system");
});
