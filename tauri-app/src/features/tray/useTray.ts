import { onMounted, watch, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { command, onEvent, type TrayMenuStrings } from "@/platform";
import { useListeners } from "@/shared/lib/listeners";

export interface TrayCallbacks {
  onShow: () => void | Promise<void>;
  onToggleStream: () => void | Promise<void>;
  onExit: () => void | Promise<void>;
  onSwitchCli: () => void | Promise<void>;
  onSwitchTui: () => void | Promise<void>;
}

export function trayStringsFromI18n(
  t: (key: string) => string,
): TrayMenuStrings {
  return {
    tooltip: t("tray.tooltip"),
    show: t("tray.show"),
    hide: t("tray.hide"),
    start: t("tray.start"),
    stop: t("tray.stop"),
    exit: t("tray.exit"),
    switchCli: t("tray.switchCli"),
    switchTui: t("tray.switchTui"),
  };
}

export function useTray(
  callbacks: TrayCallbacks,
  visibility: Ref<boolean>,
  streaming: Ref<boolean>,
) {
  const { t, locale } = useI18n();
  const track = useListeners();
  let lastPushedStrings: string | null = null;

  async function pushStrings() {
    const strings = trayStringsFromI18n(t);
    const key = JSON.stringify(strings);
    if (key === lastPushedStrings) return;
    lastPushedStrings = key;
    try {
      await command("set_tray_strings", { strings });
    } catch (e) {
      console.error("set_tray_strings failed:", e);
    }
  }

  async function pushState() {
    try {
      await command("set_tray_state", {
        state: {
          windowVisible: visibility.value,
          isStreaming: streaming.value,
        },
      });
    } catch (e) {
      console.error("set_tray_state failed:", e);
    }
  }

  onMounted(async () => {
    await track(onEvent("tray-action", (payload) => {
      const id = payload;
      switch (id) {
        case "show":
          void callbacks.onShow();
          break;
        case "toggle_stream":
          void callbacks.onToggleStream();
          break;
        case "exit":
          void callbacks.onExit();
          break;
        case "switch_cli":
          void callbacks.onSwitchCli();
          break;
        case "switch_tui":
          void callbacks.onSwitchTui();
          break;
        default:
          console.warn("Unknown tray-action id:", id);
      }
    }));

    await pushStrings();
    await pushState();
  });

  watch(locale, () => {
    void pushStrings();
  });

  watch([visibility, streaming], () => {
    void pushState();
  });

  return {
    pushStrings,
    pushState,
  };
}
