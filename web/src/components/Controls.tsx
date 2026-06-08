"use client";

import { useEffect } from "react";
import { emulator, BTN } from "@/lib/emulator";
import { Gamepad, OverlayControls } from "./Gamepad";
import { useEmulator } from "@/lib/store";

export function Controls() {
  const { settings } = useEmulator();

  useEffect(() => {
    const KEY_MAP: Record<string, number> = {
      ArrowRight: BTN.RIGHT,
      ArrowLeft: BTN.LEFT,
      ArrowUp: BTN.UP,
      ArrowDown: BTN.DOWN,
      KeyX: BTN.A,
      KeyZ: BTN.B,
      Enter: BTN.START,
      ShiftRight: BTN.SELECT,
      Backspace: BTN.SELECT,
    };

    const onKeyDown = (e: KeyboardEvent) => {
      const code = KEY_MAP[e.code];
      if (code !== undefined) {
        emulator.setButton(code, true);
        e.preventDefault();
      }
    };

    const onKeyUp = (e: KeyboardEvent) => {
      const code = KEY_MAP[e.code];
      if (code !== undefined) {
        emulator.setButton(code, false);
        e.preventDefault();
      }
    };

    window.addEventListener("keydown", onKeyDown, { passive: false });
    window.addEventListener("keyup", onKeyUp, { passive: false });
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, []);

  if (settings.controls === "overlay") {
    return <OverlayControls />;
  }

  return (
    <div className="flex flex-col gap-8 p-6 w-full max-w-md mx-auto select-none touch-none">
      <Gamepad />
    </div>
  );
}
