import { onScopeDispose } from 'vue';
import type { UnlistenFn } from '@/platform';

/**
 * Ties backend and window listeners to the calling component's lifetime.
 * Call it synchronously during setup; the returned `track` takes the pending
 * registration of any listener. Everything tracked is removed when the scope
 * is disposed, and a registration that resolves after that is removed at
 * once, so unmounting mid-setup cannot leak a listener.
 */
export function useListeners() {
  const unlisteners: UnlistenFn[] = [];
  let disposed = false;

  onScopeDispose(() => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  });

  return async function track(pending: Promise<UnlistenFn>): Promise<void> {
    const unlisten = await pending;
    if (disposed) unlisten();
    else unlisteners.push(unlisten);
  };
}
