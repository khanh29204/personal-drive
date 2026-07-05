const fs = require('node:fs');
const path = require('node:path');

const rootDir = path.join(__dirname, '..');

const assets = [
  { from: path.join(rootDir, 'src', 'views'), to: path.join(rootDir, 'dist', 'views') },
  { from: path.join(rootDir, 'src', 'public'), to: path.join(rootDir, 'dist', 'public') },
];

for (const { from, to } of assets) {
  fs.cpSync(from, to, { recursive: true });
  console.log(`Copied ${path.relative(rootDir, from)} -> ${path.relative(rootDir, to)}`);
}
