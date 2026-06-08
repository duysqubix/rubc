import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const uiDir = path.join(__dirname, 'src/components/ui');

function assertComponent(name, expectedProps) {
  const filePath = path.join(uiDir, `${name}.tsx`);
  if (!fs.existsSync(filePath)) {
    console.error(`❌ Component ${name}.tsx not found`);
    process.exit(1);
  }
  
  const content = fs.readFileSync(filePath, 'utf8');
  
  // Check export
  if (!content.includes(`export function ${name}`)) {
    console.error(`❌ Component ${name} not exported correctly`);
    process.exit(1);
  }
  
  // Check props
  for (const prop of expectedProps) {
    if (!content.includes(prop)) {
      console.error(`❌ Component ${name} missing expected prop/string: ${prop}`);
      process.exit(1);
    }
  }
  
  console.log(`✅ Component ${name} verified`);
}

assertComponent('Button', ['variant', 'size', 'block', 'translateY(var(--press-offset))']);
assertComponent('Badge', ['tone', 'variant', 'rubc-badge']);
assertComponent('Kbd', ['rubc-kbd', '0 2px 0 0 var(--bg-deep)']);
assertComponent('StatusPill', ['status', 'label', 'detail', 'rubc-status']);
assertComponent('Screen', ['src', 'scale', 'status', 'glow', 'var(--glow-screen)']);
assertComponent('Input', ['label', 'hint', 'prefix', 'invalid']);
assertComponent('Switch', ['checked', 'onChange', 'label', 'disabled']);
assertComponent('Card', ['title', 'eyebrow', 'accent', 'rubc-card']);

// Check index.ts
const indexPath = path.join(uiDir, 'index.ts');
if (!fs.existsSync(indexPath)) {
  console.error(`❌ index.ts not found`);
  process.exit(1);
}
const indexContent = fs.readFileSync(indexPath, 'utf8');
const components = ['Button', 'Badge', 'Kbd', 'StatusPill', 'Screen', 'Input', 'Switch', 'Card'];
for (const comp of components) {
  if (!indexContent.includes(`export * from "./${comp}"`)) {
    console.error(`❌ index.ts missing export for ${comp}`);
    process.exit(1);
  }
}
console.log(`✅ index.ts verified`);

console.log("All component tests passed!");
