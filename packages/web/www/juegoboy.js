import init, { Emulator, Button } from "./pkg/gameboy_web.js";

export { Button };

const WIDTH = 160;
const HEIGHT = 144;
const MS_PER_FRAME = (70224 / 4194304) * 1000; // ≈ 59.73 fps
const SAVE_INTERVAL_MS = 5000;

/** KeyboardEvent.code → button. Same layout as the native frontend. */
const KEYS = {
  KeyW: Button.Up,
  KeyA: Button.Left,
  KeyS: Button.Down,
  KeyD: Button.Right,
  ArrowUp: Button.A,
  ArrowLeft: Button.B,
  ArrowRight: Button.Start,
  ArrowDown: Button.Select,
};

/**
 * Attach an emulator to a canvas.
 *
 * Options:
 *   romInput  – <input type="file"> to load ROMs from
 *   keyTarget – where to listen for keys (default: the canvas; pass `window` for page-wide)
 */
export async function mount(canvas, { romInput, keyTarget = canvas } = {}) {
  await init();

  canvas.width = WIDTH;
  canvas.height = HEIGHT;
  canvas.style.imageRendering = "pixelated";
  if (keyTarget === canvas) canvas.tabIndex = 0; // canvases only get key events when focusable

  const ctx = canvas.getContext("2d");
  let emulator = null;
  let saveKey = null;
  let paused = false;
  let lastTime = 0;
  let behind = 0;

  function draw() {
    const pixels = new Uint8ClampedArray(emulator.frame());
    ctx.putImageData(new ImageData(pixels, WIDTH, HEIGHT), 0, 0);
  }

  // Persist cartridge RAM to localStorage. Wrapped in try/catch since storage can be
  // unavailable (private browsing) or full; the emulator should keep running either way.
  function save() {
    if (!emulator || !saveKey) return;
    const ram = emulator.get_cartridge_ram();
    if (ram.length === 0) return;
    try {
      localStorage.setItem(saveKey, btoa(String.fromCharCode(...ram)));
    } catch (err) {
      console.warn("Couldn't write save", err);
    }
  }

  function releaseAll() {
    for (const button of Object.values(KEYS)) emulator?.release_button(button);
  }

  // requestAnimationFrame runs based on the display's refresh rate,
  // so run however many frames are due.
  // TODO - port this to the gui
  function loop(now) {
    requestAnimationFrame(loop);
    if (!emulator || paused) return;

    behind = Math.min(behind + now - lastTime, MS_PER_FRAME * 5);
    lastTime = now;
    if (behind < MS_PER_FRAME) return;

    while (behind >= MS_PER_FRAME) {
      emulator.step_frame();
      behind -= MS_PER_FRAME;
    }
    draw();
  }

  function onKey(e) {
    const button = KEYS[e.code];
    if (button === undefined || !emulator) return;
    e.preventDefault();
    if (e.type === "keydown") emulator.press_button(button);
    else emulator.release_button(button);
  }

  keyTarget.addEventListener("keydown", onKey);
  keyTarget.addEventListener("keyup", onKey);
  keyTarget.addEventListener("blur", releaseAll);
  window.addEventListener("blur", releaseAll);

  // Save periodically and whenever the page is hidden or closed
  setInterval(save, SAVE_INTERVAL_MS);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") save();
  });
  window.addEventListener("pagehide", save);

  romInput?.addEventListener("change", async () => {
    const file = romInput.files[0];
    if (!file) return;
    gb.loadRom(await file.arrayBuffer());
    romInput.blur();
    if (keyTarget === canvas) canvas.focus();
  });

  const gb = {
    /** Start a ROM from an ArrayBuffer, replacing any running one. */
    loadRom(bytes) {
      save(); // flush the outgoing game before replacing it
      emulator?.free();

      const rom = new Uint8Array(bytes);
      emulator = new Emulator(rom);

      // Key saves by the header title (0x134-0x143) plus global checksum (0x14E-0x14F)
      // so different games, and different revisions of the same game, don't collide.
      const title = String.fromCharCode(...rom.subarray(0x134, 0x144)).replace(/\0+$/, "");
      const checksum = ((rom[0x14e] << 8) | rom[0x14f]).toString(16).padStart(4, "0");
      saveKey = `juegoboy:sav:${title}:${checksum}`;

      try {
        const stored = localStorage.getItem(saveKey);
        if (stored) {
          emulator.load_cartridge_ram(Uint8Array.from(atob(stored), (c) => c.charCodeAt(0)));
        }
      } catch (err) {
        console.warn("Couldn't load save", err);
      }

      lastTime = performance.now();
      behind = 0;
    },

    /** Hold a button down (e.g. from an on-screen control). No-op before a ROM is loaded. */
    press(button) {
      emulator?.press_button(button);
    },

    /** Let go of a button pressed with `press`. */
    release(button) {
      emulator?.release_button(button);
    },

    get paused() {
      return paused;
    },

    pause() {
      paused = true;
      releaseAll();
    },

    resume() {
      paused = false;
      lastTime = performance.now(); // don't fast-forward through the pause
    },
  };

  requestAnimationFrame(loop);
  return gb;
}
