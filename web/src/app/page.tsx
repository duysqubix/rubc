"use client";

import { useState, useEffect } from "react";
import { Emulator } from "@/components/Emulator";
import { RomLoader } from "@/components/RomLoader";
import { emulator } from "@/lib/emulator";

export default function Home() {
  const [romBytes, setRomBytes] = useState<Uint8Array | null>(null);
  const [wasmReady, setWasmReady] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    emulator.onReady = () => setWasmReady(true);
    emulator.onError = (err) => setError(err.message);
    emulator.init();
  }, []);

  if (error) {
    return (
      <main className="flex min-h-screen flex-col items-center justify-center bg-zinc-950 text-red-400 p-4 text-center">
        <h2 className="text-xl font-bold mb-2">Failed to load emulator core</h2>
        <p className="text-sm opacity-80">{error}</p>
      </main>
    );
  }

  if (!wasmReady) {
    return (
      <main className="flex min-h-screen flex-col items-center justify-center bg-zinc-950 text-zinc-500">
        <div className="w-8 h-8 border-2 border-zinc-800 border-t-zinc-400 rounded-full animate-spin mb-4" />
        <p className="text-sm font-medium tracking-widest uppercase">Initializing</p>
      </main>
    );
  }

  return (
    <main className="flex min-h-[100dvh] flex-col items-center bg-zinc-950 text-zinc-50 overflow-hidden">
      {romBytes ? (
        <Emulator romBytes={romBytes} onExit={() => setRomBytes(null)} />
      ) : (
        <RomLoader onRomLoaded={setRomBytes} />
      )}
    </main>
  );
}
