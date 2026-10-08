// Generate only the small selection lens; SVG must use sRGB filter channels.
export function createLensMap(width: number, height: number, radius: number): string {
  width = Math.max(1, Math.round(width));
  height = Math.max(1, Math.round(height));
  radius = Math.max(0, Math.min(radius, width / 2, height / 2));

  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext('2d', { colorSpace: 'srgb' });
  if (!context) throw new Error('Canvas 2D is unavailable for the glass lens');

  const image = context.createImageData(width, height);
  const pixels = image.data;
  const halfWidth = width / 2;
  const halfHeight = height / 2;
  const edgeWidth = Math.min(8, radius);

  function setPixel(x: number, y: number, red: number, green: number): void {
    const index = (y * width + x) * 4;
    pixels[index] = red;
    pixels[index + 1] = green;
    pixels[index + 2] = 128;
    pixels[index + 3] = 255;
  }

  // Pixel centers in one quadrant give the rounded-rectangle SDF and normal.
  for (let y = 0; y < Math.ceil(halfHeight); y++) {
    const qy = halfHeight - y - 0.5 - (halfHeight - radius);
    for (let x = 0; x < Math.ceil(halfWidth); x++) {
      const qx = halfWidth - x - 0.5 - (halfWidth - radius);
      const ox = Math.max(qx, 0);
      const oy = Math.max(qy, 0);
      const length = Math.hypot(ox, oy);
      const distance = length + Math.min(Math.max(qx, qy), 0) - radius;
      let dx = 0;
      let dy = 0;

      if (edgeWidth > 0 && distance < 0 && distance > -edgeWidth) {
        const nx = length > 0 ? ox / length : qx > qy ? 1 : 0;
        const ny = length > 0 ? oy / length : qx > qy ? 0 : 1;
        // The edge envelope meets both the outside and flat center smoothly.
        // 96 channel steps yield at most ~3px displacement with SVG scale=8.
        const bend = 96 * Math.sin(Math.PI * -distance / edgeWidth) ** 2;
        dx = Math.round(nx * bend);
        dy = Math.round(ny * bend);
      }

      // 128 is the nearest 8-bit neutral to 0.5; mirror both vector signs.
      const mirroredX = width - x - 1;
      const mirroredY = height - y - 1;
      setPixel(x, y, 128 - dx, 128 - dy);
      setPixel(mirroredX, y, 128 + dx, 128 - dy);
      setPixel(x, mirroredY, 128 - dx, 128 + dy);
      setPixel(mirroredX, mirroredY, 128 + dx, 128 + dy);
    }
  }

  context.putImageData(image, 0, 0);
  return canvas.toDataURL('image/png');
}
