const fs = require('fs');
const path = require('path');
const out = path.join(__dirname, 'src-tauri', 'icons', 'icon.ico');
const width = 32;
const height = 32;
const pixelBytes = width * height * 4;
const maskBytes = Math.ceil(width / 32) * 4 * height;
const dibSize = 40;
const imageSize = dibSize + pixelBytes + maskBytes;
const buffer = Buffer.alloc(22 + imageSize);
let offset = 0;
buffer.writeUInt16LE(0, offset); offset += 2;
buffer.writeUInt16LE(1, offset); offset += 2;
buffer.writeUInt16LE(1, offset); offset += 2;
buffer.writeUInt8(width, offset++);
buffer.writeUInt8(height, offset++);
buffer.writeUInt8(0, offset++);
buffer.writeUInt8(0, offset++);
buffer.writeUInt16LE(1, offset); offset += 2;
buffer.writeUInt16LE(32, offset); offset += 2;
buffer.writeUInt32LE(imageSize, offset); offset += 4;
buffer.writeUInt32LE(22, offset); offset += 4;
buffer.writeUInt32LE(dibSize, offset); offset += 4;
buffer.writeInt32LE(width, offset); offset += 4;
buffer.writeInt32LE(height * 2, offset); offset += 4;
buffer.writeUInt16LE(1, offset); offset += 2;
buffer.writeUInt16LE(32, offset); offset += 2;
buffer.writeUInt32LE(0, offset); offset += 4;
buffer.writeUInt32LE(pixelBytes, offset); offset += 4;
buffer.writeInt32LE(0, offset); offset += 4;
buffer.writeInt32LE(0, offset); offset += 4;
buffer.writeUInt32LE(0, offset); offset += 4;
buffer.writeUInt32LE(0, offset); offset += 4;
for (let y = height - 1; y >= 0; y--) {
  for (let x = 0; x < width; x++) {
    const rounded = Math.hypot(x - 15.5, y - 15.5) <= 14.5;
    const letter = x >= 9 && x <= 22 && y >= 8 && y <= 23 && (y <= 11 || x <= 12 || (x >= 13 && x <= 20 && y >= 14) || x >= 20);
    buffer[offset++] = letter ? 121 : 38;
    buffer[offset++] = letter ? 178 : 59;
    buffer[offset++] = letter ? 242 : 59;
    buffer[offset++] = rounded ? 255 : 0;
  }
}
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, buffer);
console.log(out);
