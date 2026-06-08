"use client";

import { useState, useRef, useEffect } from "react";
import { getRomKey, exportSave, importSave, loadRomFile } from "@/lib/emulator";

interface RecentRom {
  key: string;
  name: string;
  size: number;
  lastPlayed: number;
}

export function RomLoader({ onRomLoaded }: { onRomLoaded: (bytes: Uint8Array) => void }) {
  const [isDragging, setIsDragging] = useState(false);
  const [recentRoms, setRecentRoms] = useState<RecentRom[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const saveInputRef = useRef<HTMLInputElement>(null);
  const [selectedSaveKey, setSelectedSaveKey] = useState<string | null>(null);

  useEffect(() => {
    const stored = localStorage.getItem("rubc-recent-roms");
    if (stored) {
      try {
        setRecentRoms(JSON.parse(stored));
      } catch (e) {}
    }
  }, []);

  const handleFile = async (file: File) => {
    let rom;
    try {
      rom = await loadRomFile(file);
    } catch (err) {
      alert(err instanceof Error ? err.message : "Could not load that file.");
      return;
    }
    const { bytes, name } = rom;
    const key = getRomKey(bytes);

    const newRecent = {
      key,
      name,
      size: bytes.length,
      lastPlayed: Date.now()
    };

    const updated = [newRecent, ...recentRoms.filter(r => r.key !== key)].slice(0, 5);
    setRecentRoms(updated);
    localStorage.setItem("rubc-recent-roms", JSON.stringify(updated));

    onRomLoaded(bytes);
  };

  const handleSaveImport = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file || !selectedSaveKey) return;
    await importSave(selectedSaveKey, file);
    alert("Save imported successfully!");
    setSelectedSaveKey(null);
    if (saveInputRef.current) saveInputRef.current.value = '';
  };

  return (
    <div className="flex flex-col items-center justify-center w-full max-w-md mx-auto min-h-[80vh] p-6">
      <div className="text-center mb-12 flex flex-col items-center">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img
          src="/logo.png"
          alt="rubc"
          width={144}
          height={144}
          className="w-36 h-36 object-contain mb-2 select-none drop-shadow-[0_0_24px_rgba(255,140,40,0.25)]"
          draggable={false}
        />
        <p className="text-zinc-400 text-sm font-medium tracking-wide">GAMEBOY EMULATOR</p>
      </div>

      <div 
        className={`w-full p-8 rounded-2xl border-2 border-dashed transition-all duration-200 flex flex-col items-center justify-center gap-4 cursor-pointer
          ${isDragging ? 'border-blue-500 bg-blue-500/10' : 'border-zinc-700 bg-zinc-900/50 hover:border-zinc-500 hover:bg-zinc-800/50'}`}
        onDragOver={(e) => { e.preventDefault(); setIsDragging(true); }}
        onDragLeave={() => setIsDragging(false)}
        onDrop={(e) => {
          e.preventDefault();
          setIsDragging(false);
          if (e.dataTransfer.files[0]) handleFile(e.dataTransfer.files[0]);
        }}
        onClick={() => fileInputRef.current?.click()}
      >
        <div className="w-16 h-16 rounded-full bg-zinc-800 flex items-center justify-center mb-2">
          <svg className="w-8 h-8 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
          </svg>
        </div>
        <div className="text-center">
          <p className="text-zinc-200 font-medium">Drop ROM or .zip here</p>
          <p className="text-zinc-500 text-sm mt-1">or click to browse (.gb, .gbc, .zip)</p>
        </div>
        <input 
          type="file" 
          ref={fileInputRef} 
          className="hidden" 
          accept=".gb,.gbc,.zip" 
          onChange={(e) => e.target.files?.[0] && handleFile(e.target.files[0])} 
        />
      </div>

      {recentRoms.length > 0 && (
        <div className="w-full mt-12">
          <h3 className="text-xs font-bold text-zinc-500 uppercase tracking-wider mb-4">Recent Games</h3>
          <div className="flex flex-col gap-2">
            {recentRoms.map(rom => (
              <div key={rom.key} className="flex items-center justify-between p-3 rounded-xl bg-zinc-900 border border-zinc-800">
                <div className="flex flex-col">
                  <span className="text-zinc-200 font-medium text-sm">{rom.name}</span>
                  <span className="text-zinc-500 text-xs">{(rom.size / 1024).toFixed(1)} KB</span>
                </div>
                <div className="flex gap-2">
                  <button 
                    onClick={() => exportSave(rom.key)}
                    className="p-2 text-zinc-400 hover:text-white hover:bg-zinc-800 rounded-lg transition-colors"
                    title="Export Save"
                  >
                    ↓
                  </button>
                  <button 
                    onClick={() => {
                      setSelectedSaveKey(rom.key);
                      saveInputRef.current?.click();
                    }}
                    className="p-2 text-zinc-400 hover:text-white hover:bg-zinc-800 rounded-lg transition-colors"
                    title="Import Save"
                  >
                    ↑
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
      
      <input 
        type="file" 
        ref={saveInputRef} 
        className="hidden" 
        accept=".sav" 
        onChange={handleSaveImport} 
      />
    </div>
  );
}
