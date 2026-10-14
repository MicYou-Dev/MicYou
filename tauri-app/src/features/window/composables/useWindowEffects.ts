import { onMounted, onUnmounted, watch, type Ref } from 'vue';
import { appWindow, command, type BlurRect } from '@/platform';
import { useListeners } from '@/shared/lib/listeners';

interface WindowEffectsOptions {
  /** Blur the desktop behind the `data-blur-region` elements. */
  blur: Ref<boolean>;
  /** Corner radius of a native drop shadow around the whole window, or null. */
  shadowRadius: Ref<number | null>;
}

const REGION_SELECTOR = '[data-blur-region]';
/**
 * Compositor effects for the frameless, transparent windows. Blur follows the
 * elements marked `data-blur-region` so it matches rounded panels instead of
 * the window rectangle. Only KDE Plasma on Wayland supports blur and shadow;
 * elsewhere the backend reports false once and the CSS fallback stays.
 * Also marks the window inactive so glass styles can thicken when unfocused.
 */
export function useWindowEffects({ blur, shadowRadius }: WindowEffectsOptions) {
  const root = document.documentElement;
  let blurUnsupported = false;
  let shadowUnsupported = false;
  let lastBlur = '[]';
  let shadowPending = false;
  let suspended = false;
  let frame = 0;
  const resizeObserver = new ResizeObserver(() => scheduleBlur());
  const mutationObserver = new MutationObserver(() => scheduleBlur());

  function collectRegions(): BlurRect[] {
    resizeObserver.disconnect();
    return Array.from(document.querySelectorAll<HTMLElement>(REGION_SELECTOR)).flatMap((el) => {
      resizeObserver.observe(el);
      const rect = el.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return [];
      return [{
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height,
        radius: parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0,
      }];
    });
  }

  async function syncBlur() {
    frame = 0;
    if (blurUnsupported || suspended) return;
    const regions = blur.value ? collectRegions() : [];
    const payload = JSON.stringify(regions);
    if (payload === lastBlur) return;
    lastBlur = payload;
    try {
      const active = await command('set_window_blur', { regions });
      if (!active && regions.length > 0) {
        if (await isHidden()) {
          // Stored and applied by the backend once the window is shown;
          // resend then so the result reflects real support.
          lastBlur = '';
          return;
        }
        blurUnsupported = true;
      }
      root.classList.toggle('native-blur', active);
    } catch (e) {
      blurUnsupported = true;
      root.classList.remove('native-blur');
      console.warn('Window blur is unavailable:', e);
    }
  }

  function scheduleBlur() {
    if (blurUnsupported || frame) return;
    frame = requestAnimationFrame(() => void syncBlur());
  }

  async function syncShadow() {
    if (shadowUnsupported || suspended) return;
    const radius = shadowRadius.value;
    shadowPending = false;
    try {
      const active = await command('set_window_shadow', { radius });
      if (!active && radius !== null) {
        if (await isHidden()) {
          shadowPending = true;
          return;
        }
        shadowUnsupported = true;
      }
    } catch (e) {
      shadowUnsupported = true;
      console.warn('Window shadow is unavailable:', e);
    }
  }

  /**
   * Removes blur and shadow while the window content animates out, so the
   * compositor does not keep drawing empty glass until the window is gone.
   */
  async function suspend() {
    suspended = true;
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    const requests: Promise<unknown>[] = [];
    if (!blurUnsupported && lastBlur !== '[]') {
      lastBlur = '[]';
      requests.push(command('set_window_blur', { regions: [] }));
    }
    if (!shadowUnsupported && shadowRadius.value !== null) {
      requests.push(command('set_window_shadow', { radius: null }));
    }
    await Promise.allSettled(requests);
  }

  /**
   * Sends blur and shadow right away instead of on the next frame. Called
   * before a hidden window is shown: the backend applies stored effects the
   * moment the window maps, so they appear together with it. Also undoes
   * `suspend`.
   */
  async function apply() {
    suspended = false;
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    await Promise.all([syncBlur(), syncShadow()]);
  }

  // A hidden window has no surface, so the backend reports false without
  // telling us anything about compositor support.
  async function isHidden() {
    return !(await appWindow.isVisible());
  }

  // Window focus, not document focus: plugin panels are iframes and taking
  // focus into one must not count as the window going inactive.
  const track = useListeners();
  const setInactive = (inactive: boolean) => root.classList.toggle('window-inactive', inactive);

  watch(blur, scheduleBlur);
  watch(shadowRadius, () => void syncShadow());

  onMounted(async () => {
    // Panels are added and removed with v-if; sizes are tracked per element.
    mutationObserver.observe(document.body, { childList: true, subtree: true });
    window.addEventListener('resize', scheduleBlur);
    scheduleBlur();
    void syncShadow();
    setInactive(!(await appWindow.isFocused()));
    await track(appWindow.onFocusChanged((focused) => {
      setInactive(!focused);
      if (!focused) return;
      // Shown again (e.g. from the tray): retry what was deferred while hidden.
      scheduleBlur();
      if (shadowPending) void syncShadow();
    }));
  });

  onUnmounted(() => {
    mutationObserver.disconnect();
    resizeObserver.disconnect();
    window.removeEventListener('resize', scheduleBlur);
    if (frame) cancelAnimationFrame(frame);
  });

  return { suspend, apply };
}
