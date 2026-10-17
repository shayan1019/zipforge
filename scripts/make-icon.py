"""Generates the zipforge app icon (1024x1024 PNG) with Pillow.

Usage:  python scripts/make-icon.py [out.png]
Then:   npx tauri icon <out.png>
"""
import sys
from PIL import Image, ImageDraw

S = 1024
C1 = (99, 102, 241)    # indigo-500
C3 = (139, 92, 246)    # violet-500
C2 = (217, 70, 239)    # fuchsia-500

img = Image.new("RGBA", (S, S), (0, 0, 0, 0))

grad = Image.new("RGB", (S, S))
px = grad.load()
last = S - 1
for y in range(S):
    for x in range(S):
        t = (x + y) / (2 * last)
        if t < 0.5:
            u = t * 2
            col = tuple(round(C1[i] + (C3[i] - C1[i]) * u) for i in range(3))
        else:
            u = (t - 0.5) * 2
            col = tuple(round(C3[i] + (C2[i] - C3[i]) * u) for i in range(3))
        px[x, y] = col

mask = Image.new("L", (S, S), 0)
md = ImageDraw.Draw(mask)
md.rounded_rectangle([24, 24, S - 24, S - 24], radius=225, fill=255)
img.paste(grad, (0, 0), mask)

d = ImageDraw.Draw(img)
white = (255, 255, 255, 255)
cx = S // 2

# zipper seam
d.rounded_rectangle([cx - 20, 306, cx + 20, 868], radius=20, fill=white)

# alternating teeth
y = 348
i = 0
while y < 840:
    if i % 2 == 0:
        d.rounded_rectangle([cx - 122, y, cx - 26, y + 54], radius=18, fill=white)
    else:
        d.rounded_rectangle([cx + 26, y, cx + 122, y + 54], radius=18, fill=white)
    y += 94
    i += 1

# slider ring + pull tab
d.rounded_rectangle([cx - 94, 172, cx + 94, 300], radius=56, outline=white, width=34)
d.rounded_rectangle([cx - 26, 96, cx + 26, 196], radius=24, fill=white)

out = sys.argv[1] if len(sys.argv) > 1 else "icon.png"
img.save(out)
print(f"saved {out}")
