import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const cssPath = path.join(__dirname, 'src/app/globals.css');
const cssContent = fs.readFileSync(cssPath, 'utf8');

function assertToken(name, expectedValue) {
  const regex = new RegExp(`--${name}\\s*:\\s*([^;]+);`);
  const match = cssContent.match(regex);
  if (!match) {
    console.error(`❌ Token --${name} not found`);
    process.exit(1);
  }
  
  const actualValue = match[1].trim();
  if (actualValue !== expectedValue) {
    console.error(`❌ Token --${name} mismatch. Expected: ${expectedValue}, Actual: ${actualValue}`);
    process.exit(1);
  }
  console.log(`✅ Token --${name} = ${actualValue}`);
}

// Accent
assertToken('accent', 'var(--rust-500)');
assertToken('accent-hover', 'var(--rust-400)');
assertToken('accent-press', 'var(--rust-600)');
assertToken('accent-soft', 'color-mix(in srgb, var(--rust-500) 16%, transparent)');
assertToken('focus-ring', 'var(--rust-400)');

console.log("All tests passed!");

// Surfaces
assertToken('bg', 'var(--ink-850)');
assertToken('bg-deep', 'var(--ink-950)');
assertToken('surface', 'var(--ink-700)');
assertToken('surface-raised', 'var(--ink-600)');
assertToken('surface-sunken', 'var(--ink-900)');
assertToken('screen', 'var(--dmg-darkest)');

// Borders
assertToken('border', 'var(--ink-500)');
assertToken('border-strong', 'var(--ink-400)');
assertToken('border-screen', '#2a2f3a');

// Text
assertToken('text', 'var(--paper)');
assertToken('text-strong', 'var(--white)');
assertToken('text-muted', 'var(--slate-300)');
assertToken('text-faint', 'var(--ink-300)');
assertToken('text-on-accent', 'var(--white)');
assertToken('text-screen', 'var(--dmg-light)');

// CGB sparkles
assertToken('cgb-purple', '#8b5cf6');
assertToken('cgb-teal', '#19c3b1');
assertToken('cgb-green', '#8bd450');
assertToken('cgb-amber', '#f5b342');

// Status
assertToken('success', 'var(--signal-pass)');
assertToken('warning', 'var(--signal-warn)');
assertToken('danger', 'var(--signal-fail)');
assertToken('info', 'var(--signal-info)');

// Typography
assertToken('font-pixel', '"Pixelify Sans", "Press Start 2P", ui-monospace, monospace');
assertToken('font-mono', '"IBM Plex Mono", ui-monospace, "SFMono-Regular", Menlo, monospace');
assertToken('font-sans', '"IBM Plex Sans", ui-sans-serif, system-ui, -apple-system, sans-serif');

assertToken('weight-regular', '400');
assertToken('weight-medium', '500');
assertToken('weight-semibold', '600');
assertToken('weight-bold', '700');

assertToken('text-2xs', '0.6875rem');
assertToken('text-xs', '0.75rem');
assertToken('text-sm', '0.875rem');
assertToken('text-md', '1rem');
assertToken('text-lg', '1.125rem');
assertToken('text-xl', '1.375rem');
assertToken('text-2xl', '1.75rem');
assertToken('text-3xl', '2.25rem');
assertToken('text-4xl', '3rem');
assertToken('text-5xl', '4rem');

assertToken('leading-tight', '1.1');
assertToken('leading-snug', '1.3');
assertToken('leading-normal', '1.55');
assertToken('leading-relaxed', '1.7');

assertToken('tracking-tight', '-0.01em');
assertToken('tracking-normal', '0');
assertToken('tracking-wide', '0.04em');
assertToken('tracking-caps', '0.12em');

// Spacing
assertToken('space-0', '0');
assertToken('space-1', '0.25rem');
assertToken('space-2', '0.5rem');
assertToken('space-3', '0.75rem');
assertToken('space-4', '1rem');
assertToken('space-5', '1.5rem');
assertToken('space-6', '2rem');
assertToken('space-7', '3rem');
assertToken('space-8', '4rem');
assertToken('space-9', '6rem');

// Radii
assertToken('radius-0', '0');
assertToken('radius-sm', '3px');
assertToken('radius', '5px');
assertToken('radius-md', '8px');
assertToken('radius-lg', '12px');
assertToken('radius-screen', '6px');

// Borders
assertToken('border-width', '1px');
assertToken('border-width-2', '2px');
assertToken('border-screen-width', '2px');

// Shadows
assertToken('shadow-sm', '0 1px 0 rgba(0,0,0,0.4)');
assertToken('shadow', '0 2px 0 rgba(0,0,0,0.45), 0 4px 12px rgba(0,0,0,0.35)');
assertToken('shadow-lg', '0 4px 0 rgba(0,0,0,0.45), 0 12px 32px rgba(0,0,0,0.5)');
assertToken('shadow-inset', 'inset 0 1px 0 rgba(255,255,255,0.04)');
assertToken('shadow-focus', '0 0 0 3px var(--accent-soft)');
assertToken('press-offset', '3px');
assertToken('glow-screen', '0 0 0 1px rgba(136,192,112,0.25), 0 0 24px rgba(136,192,112,0.18)');

// Motion
assertToken('ease', 'cubic-bezier(0.2, 0, 0.1, 1)');
assertToken('ease-step', 'steps(4, end)');
assertToken('dur-fast', '90ms');
assertToken('dur', '140ms');
assertToken('dur-slow', '240ms');

// Containers
assertToken('container', '1120px');
assertToken('container-wide', '1320px');
