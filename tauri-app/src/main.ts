import { createApp, type Component } from "vue";
import "./shared/assets/index.css";
import { i18n } from "./i18n";

// Every webview loads only the root component of its own window.
const hash = window.location.hash;
const loadRoot: () => Promise<{ default: Component }> = hash.startsWith('#/plugin/')
  ? () => import("./features/plugins/components/PluginPanelWindow.vue")
  : hash === '#/settings'
    ? () => import("./features/settings/components/SettingsWindow.vue")
    : () => import("./App.vue");

void loadRoot().then(({ default: RootComponent }) => {
  const app = createApp(RootComponent);
  app.use(i18n);
  app.mount("#app");
});
