"use client";

import { useEffect, useRef, useState } from "react";
import { emulator } from "@/lib/emulator";
import { Controls } from "./Controls";

export function Emulator({ romBytes, onExit }: { romBytes: Uint8Array, onExit: () => void }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [isMuted, setIsMuted] = useState(true);

  useEffect(() => {
    if (!canvasRef.current) return;
    
    emulator.loadRom(romBytes, canvasRef.current).catch(console.error);

    const handleVisibility = () => {
      if (document.visibilityState === "hidden") emulator.flushSave();
    };
    const handlePageHide = () => emulator.flushSave();

    document.addEventListener("visibilitychange", handleVisibility);
    window.addEventListener("pagehide", handlePageHide);

    return () => {
      document.removeEventListener("visibilitychange", handleVisibility);
      window.removeEventListener("pagehide", handlePageHide);
      emulator.destroy();
    };
  }, [romBytes]);

  const toggleMute = async () => {
    if (isMuted) {
      await emulator.resumeAudio();
      setIsMuted(false);
    } else {
      if (emulator.audioCtx) {
        await emulator.audioCtx.suspend();
      }
      setIsMuted(true);
    }
  };

  return (
    <div className="flex flex-col items-center w-full max-w-2xl mx-auto h-full justify-between py-4">
      <div className="w-full flex justify-between px-4 mb-4">
        <button onClick={onExit} className="text-zinc-400 hover:text-white text-sm font-medium">
          ← Back
        </button>
        <button onClick={toggleMute} className="text-zinc-400 hover:text-white text-sm font-medium">
          {isMuted ? "🔇 Unmute" : "🔊 Mute"}
        </button>
      </div>
      
      <div className="relative bg-black p-4 rounded-t-3xl rounded-b-xl shadow-2xl border-4 border-zinc-800 w-full max-w-[100vw] aspect-[10/9] flex items-center justify-center overflow-hidden">
        <canvas
          ref={canvasRef}
          width={160}
          height={144}
          className="w-full h-full object-contain"
          style={{ imageRendering: "pixelated" }}
        />
        <div className="absolute top-2 left-4 flex gap-1">
          <div className="w-2 h-2 rounded-full bg-red-500/50" />
          <div className="text-[8px] text-zinc-600 font-bold tracking-widest">BATTERY</div>
        </div>
      </div>

      <div className="flex-1 w-full flex items-center">
        <Controls />
      </div>
    </div>
  );
}
