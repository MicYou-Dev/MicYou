import { ref, onMounted } from 'vue';
import { useStorage } from '@vueuse/core';
import { command, onEvent, type AudioMetrics } from '@/platform';
import { useListeners } from '@/shared/lib/listeners';

/**
 * Composable for managing audio status, mute state, audio level, metrics and warning dialogs
 */
export function useAudio() {
  // Real-time audio input volume level (0-100)
  const audioLevel = ref(0);
  
  // Audio mute status
  const isMuted = ref(false);
  
  // Real-time audio performance and network statistics
  const audioMetrics = ref<AudioMetrics | null>(null);
  
  // Real-time audio earback / monitoring status (listening to own voice via speaker)
  const isMonitoringEnabled = ref(false);
  
  // Flag showing/hiding the monitoring panel
  const showMonitoringPanel = ref(false);
  
  // Flag indicating if the UDP port fallback warning should be displayed
  const showUdpWarning = ref(false);

  // Flag and storage for earback / monitoring warning dialog
  const showMonitoringWarning = ref(false);
  const dontShowMonitoringWarning = useStorage('micyou_dont_show_monitoring_warning', false);

  const track = useListeners();

  /**
   * Toggles the mute state of the server-side audio engine
   */
  async function toggleMute() {
    const newVal = !isMuted.value;
    isMuted.value = newVal;
    try {
      await command('set_mute_state', { isMuted: newVal });
    } catch (e) {
      console.error('set_mute_state failed:', e);
      isMuted.value = !newVal;
    }
  }

  /**
   * Direct execution of monitoring state change
   */
  async function executeSetMonitoring(enabled: boolean) {
    isMonitoringEnabled.value = enabled;
    try {
      await command('set_monitoring', { enabled });
    } catch (e) {
      console.error('set_monitoring failed:', e);
      isMonitoringEnabled.value = !enabled;
    }
  }

  /**
   * Toggles real-time audio monitoring / earback with warning prompt on enable
   */
  function toggleMonitoringEnabled() {
    if (isMonitoringEnabled.value) {
      void executeSetMonitoring(false);
    } else {
      if (dontShowMonitoringWarning.value) {
        void executeSetMonitoring(true);
      } else {
        showMonitoringWarning.value = true;
      }
    }
  }

  function handleMonitoringWarningConfirm(dontAskAgain: boolean) {
    if (dontAskAgain) {
      dontShowMonitoringWarning.value = true;
    }
    showMonitoringWarning.value = false;
    void executeSetMonitoring(true);
  }

  function handleMonitoringWarningCancel() {
    showMonitoringWarning.value = false;
  }

  /**
   * Toggles the visibility of the monitoring panel
   */
  function toggleMonitoring() {
    showMonitoringPanel.value = !showMonitoringPanel.value;
  }

  onMounted(() => {
    const hideUdpWarning = () => {
      showUdpWarning.value = false;
    };
    void Promise.all([
      track(onEvent('audio-level', (payload) => {
        audioLevel.value = payload;
        if (payload > 0) hideUdpWarning();
      })),
      track(onEvent('audio-metrics', (payload) => {
        audioMetrics.value = payload;
      })),
      // Mute and monitoring changes made from other surfaces (tray, CLI, phone)
      track(onEvent('mute-state-changed', (payload) => {
        isMuted.value = payload;
      })),
      track(onEvent('monitoring-enabled-changed', (payload) => {
        isMonitoringEnabled.value = payload;
      })),
      // The firewall likely blocks the UDP audio port
      track(onEvent('udp_audio_warning', () => {
        if (!isMuted.value) showUdpWarning.value = true;
      })),
      track(onEvent('device-disconnected', hideUdpWarning)),
      track(onEvent('server-stopped', hideUdpWarning)),
    ]).catch((e) => console.error('Failed to listen for audio events:', e));
  });

  return {
    audioLevel,
    isMuted,
    isMonitoringEnabled,
    audioMetrics,
    showMonitoringPanel,
    showUdpWarning,
    showMonitoringWarning,
    toggleMute,
    toggleMonitoringEnabled,
    handleMonitoringWarningConfirm,
    handleMonitoringWarningCancel,
    toggleMonitoring,
  };
}

