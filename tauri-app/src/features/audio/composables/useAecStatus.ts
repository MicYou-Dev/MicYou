import { computed, ref } from 'vue';
import { command, onEvent, type AecStatus } from '@/platform';

// AEC availability as the backend reports it. The backend decides what the
// platform can do (`unsupported_os` on systems without a capture path,
// `permission_denied` when the bundle may not record system audio), so no
// component should guess from the operating system it happens to detect.

/** Reasons that mean AEC can never run here, as opposed to a capture that stopped. */
const CAPABILITY_REASONS = new Set(['unsupported_os', 'permission_denied']);

// Module-level singleton: every component shares one answer and one listener, so
// the UI cannot end up with two disagreeing AEC states.
const aecAvailable = ref(true);
const aecReason = ref<string | null>(null);
let initialised = false;

function apply(status: AecStatus) {
  aecAvailable.value = status.available;
  aecReason.value = status.reason ?? null;
}

async function refresh() {
  try {
    apply(await command('get_aec_status'));
  } catch (error) {
    console.error('get_aec_status failed:', error);
  }
}

/** Whether this system can run AEC at all. */
const aecSupported = computed(() => !CAPABILITY_REASONS.has(aecReason.value ?? ''));

export function useAecStatus() {
  if (!initialised) {
    initialised = true;
    void refresh();
    // The listener lives as long as the app does, so the unsubscribe handle is
    // dropped rather than stored: nothing tears a module-level singleton down.
    void onEvent('aec-status-changed', apply).catch((error) => {
      console.error('aec-status-changed listener failed:', error);
    });
  }

  return { aecAvailable, aecSupported, aecReason };
}
