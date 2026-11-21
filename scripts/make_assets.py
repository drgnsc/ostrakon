"""Generates the stone textures and the app icon. Output is checked in; rerun only to restyle.

Run with: pnpm assets
"""

from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "src" / "assets"
ICONS = ROOT / "src-tauri" / "icons"

TERRACOTTA = (196, 116, 74)  # brand "ember"
TERRACOTTA_DARK = (142, 79, 46)  # brand "emberInk"
BROKEN_CLAY = (226, 178, 140)
DEEP = (26, 37, 48)  # brand "deep"

# Shard outline and the Phoenician letter mem (the ancestor of M), on a 0-1 grid.
SHARD = [
    (0.15, 0.13), (0.34, 0.07), (0.47, 0.11), (0.60, 0.05), (0.80, 0.11), (0.92, 0.27),
    (0.87, 0.40), (0.95, 0.56), (0.89, 0.76), (0.72, 0.84), (0.62, 0.95), (0.42, 0.90),
    (0.27, 0.96), (0.12, 0.81), (0.16, 0.63), (0.06, 0.46), (0.11, 0.30),
]
MEM = [
    [(0.22, 0.31), (0.36, 0.48), (0.46, 0.28), (0.57, 0.48), (0.73, 0.27)],
    [(0.65, 0.37), (0.60, 0.78)],
]
CHIP_EDGES = [(0, 1), (7, 8), (11, 12), (15, 16)]


def tileable_noise(size, beta, rng):
    """Spectrally shaped white noise: periodic by construction, so it tiles without seams."""
    spectrum = np.fft.fft2(rng.standard_normal((size, size)))
    fx = np.fft.fftfreq(size)[:, None]
    fy = np.fft.fftfreq(size)[None, :]
    radius = np.sqrt(fx * fx + fy * fy)
    radius[0, 0] = 1.0
    field = np.real(np.fft.ifft2(spectrum / radius ** beta))
    return (field - field.mean()) / field.std()


def pits(size, count, max_radius, rng):
    """Small dark pockmarks, wrapped around the edges to keep the tile seamless."""
    canvas = Image.new("L", (size * 3, size * 3), 0)
    draw = ImageDraw.Draw(canvas)
    for _ in range(count):
        x, y = rng.integers(0, size, 2)
        radius = rng.uniform(0.4, max_radius)
        squash = rng.uniform(0.5, 1.0)
        strength = int(rng.uniform(90, 255))
        for dx in (0, size, size * 2):
            for dy in (0, size, size * 2):
                draw.ellipse(
                    (x + dx - radius, y + dy - radius * squash, x + dx + radius, y + dy + radius * squash),
                    fill=strength,
                )
    blurred = canvas.filter(ImageFilter.GaussianBlur(0.6)).crop((size, size, size * 2, size * 2))
    return np.asarray(blurred, dtype=np.float32) / 255.0


def grain(path, size, seed, strength, pit_count, pit_radius):
    """Neutral overlay: black and white specks with alpha, so it works on any theme colour."""
    rng = np.random.default_rng(seed)
    field = (
        0.25 * tileable_noise(size, 0.95, rng)
        + 0.45 * tileable_noise(size, 0.45, rng)
        + 0.30 * rng.standard_normal((size, size))
    )
    field = field / field.std()
    field -= pits(size, pit_count, pit_radius, rng) * 3.2

    alpha = np.clip(np.abs(field) * strength, 0, 1)
    rgba = np.zeros((size, size, 4), dtype=np.uint8)
    rgba[..., :3] = np.where(field[..., None] > 0, 255, 0)
    rgba[..., 3] = (alpha * 255).astype(np.uint8)
    Image.fromarray(rgba).save(path, "WEBP", quality=72, alpha_quality=70, method=6)
    print(path.name, path.stat().st_size // 1024, "KB")


def scaled(points, size):
    return [(x * size, y * size) for x, y in points]


def draw_icon(size, textured):
    """Hand-tuned per size: below 48 px texture turns to mud and strokes must thicken."""
    scale = 4
    big = size * scale
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).polygon(scaled(SHARD, big), fill=255)

    colour = np.zeros((big, big, 3), dtype=np.float32)
    colour[...] = TERRACOTTA
    # Light from the upper left, like everything else in the app.
    ramp = np.linspace(1.14, 0.84, big, dtype=np.float32)
    colour *= ((ramp[:, None] + ramp[None, :]) / 2)[..., None]

    if textured:
        rng = np.random.default_rng(7)
        noise = 0.65 * tileable_noise(big, 1.2, rng) + 0.35 * rng.standard_normal((big, big))
        colour *= (1 + np.clip(noise, -3, 3) * 0.055)[..., None]
        speck = pits(big, int(big * 0.55), big / 150, rng)
        colour *= (1 - speck * 0.42)[..., None]

    image = Image.fromarray(np.clip(colour, 0, 255).astype(np.uint8))
    draw = ImageDraw.Draw(image)

    # Bare clay where the edge broke off.
    edge_width = max(1, round(big * (0.022 if textured else 0.035)))
    outline = scaled(SHARD, big)
    for a, b in CHIP_EDGES:
        draw.line([outline[a], outline[b]], fill=BROKEN_CLAY, width=edge_width * 2)

    # Incised letter: a lit lower lip under the dark cut.
    stroke = max(2, round(big * (0.062 if textured else 0.092 if size > 20 else 0.115)))
    lip = max(1, round(big * 0.012))
    for line in MEM:
        points = scaled(line, big)
        if textured:
            shifted = [(x + lip, y + lip) for x, y in points]
            draw.line(shifted, fill=BROKEN_CLAY, width=stroke, joint="curve")
        draw.line(points, fill=DEEP, width=stroke, joint="curve")

    image.putalpha(mask)
    if textured:
        image = add_outer_shadow(image, big)
    return image.resize((size, size), Image.LANCZOS)


def add_outer_shadow(image, big):
    alpha = image.getchannel("A")
    shadow = Image.new("RGBA", (big, big), (*DEEP, 0))
    shadow.putalpha(alpha.point(lambda v: int(v * 0.55)).filter(ImageFilter.GaussianBlur(big * 0.012)))
    canvas = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    canvas.alpha_composite(shadow, (round(big * 0.008), round(big * 0.014)))
    canvas.alpha_composite(image)
    return canvas


def icons():
    ICONS.mkdir(parents=True, exist_ok=True)
    frames = {size: draw_icon(size, size >= 48) for size in (16, 20, 24, 32, 40, 48, 64, 128, 256, 512)}
    frames[32].save(ICONS / "32x32.png")
    frames[128].save(ICONS / "128x128.png")
    frames[512].save(ICONS / "icon.png")
    order = [256, 128, 64, 48, 40, 32, 24, 20, 16]
    frames[256].save(
        ICONS / "icon.ico",
        format="ICO",
        sizes=[(s, s) for s in order],
        append_images=[frames[s] for s in order[1:]],
    )
    frames[128].save(ASSETS / "mark.png")
    frames[32].save(ASSETS / "mark-small.png")
    # Contact sheet for eyeballing small sizes.
    sheet = Image.new("RGB", (900, 300), (245, 241, 234))
    half = Image.new("RGB", (450, 300), DEEP)
    sheet.paste(half, (450, 0))
    for base in (0, 450):
        x = base + 10
        for size in (16, 24, 32, 48, 128):
            sheet.paste(frames[size], (x, 86), frames[size])
            x += size + 14
    sheet.save(ROOT / "scripts" / "icon-sheet.png")


def main():
    ASSETS.mkdir(parents=True, exist_ok=True)
    grain(ASSETS / "grain-fine.webp", 512, 11, 0.028, 120, 1.2)
    grain(ASSETS / "grain-rough.webp", 512, 23, 0.075, 300, 2.0)
    icons()


if __name__ == "__main__":
    main()
