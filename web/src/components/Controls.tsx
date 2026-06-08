"use client";

import { useEffect } from "react";
import { emulator, BTN } from "@/lib/emulator";

export function Controls() {
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

  const handlePointer = (btn: number, pressed: boolean) => (e: React.PointerEvent) => {
    e.preventDefault();
    if (pressed) {
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
      if (navigator.vibrate) navigator.vibrate(10);
    } else {
      (e.target as HTMLElement).releasePointerCapture(e.pointerId);
    }
    emulator.setButton(btn, pressed);
  };

  return (
    <div className="flex flex-col gap-8 p-6 w-full max-w-md mx-auto select-none touch-none">
      <div className="flex justify-between items-end">
        {/* D-Pad */}
        <div className="relative w-32 h-32 bg-zinc-800 rounded-full shadow-inner">
          <div className="absolute top-0 left-1/2 -translate-x-1/2 w-10 h-12 bg-zinc-700 rounded-t-lg active:bg-zinc-600"
               onPointerDown={handlePointer(BTN.UP, true)} onPointerUp={handlePointer(BTN.UP, false)} onPointerCancel={handlePointer(BTN.UP, false)} />
          <div className="absolute bottom-0 left-1/2 -translate-x-1/2 w-10 h-12 bg-zinc-700 rounded-b-lg active:bg-zinc-600"
               onPointerDown={handlePointer(BTN.DOWN, true)} onPointerUp={handlePointer(BTN.DOWN, false)} onPointerCancel={handlePointer(BTN.DOWN, false)} />
          <div className="absolute left-0 top-1/2 -translate-y-1/2 w-12 h-10 bg-zinc-700 rounded-l-lg active:bg-zinc-600"
               onPointerDown={handlePointer(BTN.LEFT, true)} onPointerUp={handlePointer(BTN.LEFT, false)} onPointerCancel={handlePointer(BTN.LEFT, false)} />
          <div className="absolute right-0 top-1/2 -translate-y-1/2 w-12 h-10 bg-zinc-700 rounded-r-lg active:bg-zinc-600"
               onPointerDown={handlePointer(BTN.RIGHT, true)} onPointerUp={handlePointer(BTN.RIGHT, false)} onPointerCancel={handlePointer(BTN.RIGHT, false)} />
          <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-10 h-10 bg-zinc-700" />
        </div>

        {/* A/B Buttons */}
        <div className="flex gap-4 -rotate-12">
          <div className="flex flex-col items-center gap-2 mt-8">
            <button className="w-14 h-14 rounded-full bg-red-600 shadow-[0_4px_0_rgb(153,27,27)] active:shadow-none active:translate-y-1 transition-all"
                    onPointerDown={handlePointer(BTN.B, true)} onPointerUp={handlePointer(BTN.B, false)} onPointerCancel={handlePointer(BTN.B, false)} />
            <span className="text-zinc-500 font-bold text-sm">B</span>
          </div>
          <div className="flex flex-col items-center gap-2">
            <button className="w-14 h-14 rounded-full bg-red-600 shadow-[0_4px_0_rgb(153,27,27)] active:shadow-none active:translate-y-1 transition-all"
                    onPointerDown={handlePointer(BTN.A, true)} onPointerUp={handlePointer(BTN.A, false)} onPointerCancel={handlePointer(BTN.A, false)} />
            <span className="text-zinc-500 font-bold text-sm">A</span>
          </div>
        </div>
      </div>

      {/* Start/Select */}
      <div className="flex justify-center gap-8 mt-4">
        <div className="flex flex-col items-center gap-2">
          <button className="w-16 h-4 rounded-full bg-zinc-700 shadow-inner active:bg-zinc-600 -rotate-12"
                  onPointerDown={handlePointer(BTN.SELECT, true)} onPointerUp={handlePointer(BTN.SELECT, false)} onPointerCancel={handlePointer(BTN.SELECT, false)} />
          <span className="text-zinc-500 font-bold text-xs uppercase tracking-widest">Select</span>
        </div>
        <div className="flex flex-col items-center gap-2">
          <button className="w-16 h-4 rounded-full bg-zinc-700 shadow-inner active:bg-zinc-600 -rotate-12"
                  onPointerDown={handlePointer(BTN.START, true)} onPointerUp={handlePointer(BTN.START, false)} onPointerCancel={handlePointer(BTN.START, false)} />
          <span className="text-zinc-500 font-bold text-xs uppercase tracking-widest">Start</span>
        </div>
      </div>
    </div>
  );
}
