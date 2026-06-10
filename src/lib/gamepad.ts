/** Game-controller navigation.
 *
 * Polls connected gamepads and translates d-pad / left-stick / face buttons
 * into the same synthetic keyboard events the app already handles, so mouse,
 * keyboard and controller share one navigation model:
 *
 *   d-pad / stick → Arrow keys     A → Enter      B → Escape
 *   Y → "f" (toggle favorite)      Start → "l" (launch)
 */

const REPEAT_DELAY_MS = 350;
const REPEAT_INTERVAL_MS = 120;
const STICK_THRESHOLD = 0.55;

type ButtonAction = { key: string };

const BUTTON_MAP: Record<number, ButtonAction> = {
  0: { key: "Enter" }, // A / Cross
  1: { key: "Escape" }, // B / Circle
  3: { key: "f" }, // Y / Triangle → favorite
  9: { key: "l" }, // Start → launch
  12: { key: "ArrowUp" },
  13: { key: "ArrowDown" },
  14: { key: "ArrowLeft" },
  15: { key: "ArrowRight" },
};

function dispatchKey(key: string) {
  const target = document.activeElement ?? document.body;
  target.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
}

interface ButtonState {
  pressedAt: number;
  lastRepeat: number;
}

export function startGamepadNavigation(): () => void {
  const held = new Map<string, ButtonState>();
  let raf = 0;

  const axisKey = (axis: number, dir: 1 | -1) =>
    axis === 0 ? (dir === 1 ? "ArrowRight" : "ArrowLeft") : dir === 1 ? "ArrowDown" : "ArrowUp";

  function handlePress(id: string, key: string, repeats: boolean, now: number) {
    const state = held.get(id);
    if (!state) {
      held.set(id, { pressedAt: now, lastRepeat: now });
      dispatchKey(key);
      return;
    }
    if (!repeats) return;
    const sincePress = now - state.pressedAt;
    const sinceRepeat = now - state.lastRepeat;
    if (sincePress > REPEAT_DELAY_MS && sinceRepeat > REPEAT_INTERVAL_MS) {
      state.lastRepeat = now;
      dispatchKey(key);
    }
  }

  function poll() {
    const now = performance.now();
    const pads = navigator.getGamepads?.() ?? [];
    const active = new Set<string>();

    for (const pad of pads) {
      if (!pad) continue;
      for (const [indexRaw, action] of Object.entries(BUTTON_MAP)) {
        const index = Number(indexRaw);
        const button = pad.buttons[index];
        if (!button?.pressed) continue;
        const id = `${pad.index}:b${index}`;
        active.add(id);
        const repeats = action.key.startsWith("Arrow");
        handlePress(id, action.key, repeats, now);
      }
      // Left stick → arrows with repeat.
      for (const axis of [0, 1] as const) {
        const value = pad.axes[axis] ?? 0;
        for (const dir of [1, -1] as const) {
          if (dir * value > STICK_THRESHOLD) {
            const id = `${pad.index}:a${axis}:${dir}`;
            active.add(id);
            handlePress(id, axisKey(axis, dir), true, now);
          }
        }
      }
    }

    for (const id of [...held.keys()]) {
      if (!active.has(id)) held.delete(id);
    }
    raf = requestAnimationFrame(poll);
  }

  raf = requestAnimationFrame(poll);
  return () => cancelAnimationFrame(raf);
}
