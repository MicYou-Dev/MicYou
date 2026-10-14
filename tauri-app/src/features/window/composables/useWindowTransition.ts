import type { Ref } from 'vue';

interface WindowTransitionOptions {
  /** Drop compositor blur and shadow first, so no empty glass is left behind. */
  suspendEffects?: () => Promise<void>;
}

// Same curves as --ease-* in index.css.
const STANDARD = 'cubic-bezier(0.2, 0, 0, 1)';
const DECELERATE = 'cubic-bezier(0.05, 0.7, 0.1, 1)';
const ACCELERATE = 'cubic-bezier(0.3, 0, 0.8, 0.15)';

function reducedMotion() {
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/**
 * Animates the content of a frameless, transparent window in and out, since
 * hiding, closing and showing it are otherwise instant. Minimizing is left to
 * the compositor, which animates the window itself.
 *
 * Compositor blur cannot fade, so callers set it before the window is shown
 * and `enter` brings the content in quickly over it.
 */
export function useWindowTransition(target: Ref<HTMLElement | null>, options: WindowTransitionOptions = {}) {
  let running: Animation[] = [];

  function play(steps: [Keyframe[], KeyframeAnimationOptions][], keep: boolean): Promise<void> {
    running.forEach((animation) => animation.cancel());
    const el = target.value;
    if (!el) {
      running = [];
      return Promise.resolve();
    }
    const instant = reducedMotion();
    const animations = steps.map(([keyframes, timing]) =>
      el.animate(keyframes, { ...timing, duration: instant ? 0 : timing.duration, fill: 'forwards' }),
    );
    running = animations;
    return Promise.all(animations.map((animation) => animation.finished)).then(
      () => {
        // A transform left on the root would turn it into the containing
        // block of every `position: fixed` dialog, so only the hidden state
        // is kept.
        if (!keep) animations.forEach((animation) => animation.cancel());
      },
      () => {},
    );
  }

  /** Fade the content out before the window is hidden or closed. */
  async function exit() {
    await Promise.all([
      options.suspendEffects?.(),
      play([[[{ opacity: 1, transform: 'scale(1)' }, { opacity: 0, transform: 'scale(0.97)' }], { duration: 150, easing: ACCELERATE }]], true),
    ]);
  }

  /** Bring the content in right after the window was shown. */
  function enter(): Promise<void> {
    return play(
      [
        [[{ opacity: 0 }, { opacity: 1 }], { duration: 120, easing: STANDARD }],
        [[{ transform: 'scale(0.97)' }, { transform: 'scale(1)' }], { duration: 260, easing: DECELERATE }],
      ],
      false,
    );
  }

  return { enter, exit };
}
