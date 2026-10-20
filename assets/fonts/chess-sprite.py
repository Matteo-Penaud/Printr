"""Planche des pièces d'échecs (src/blocks/chess-pieces.png), d'après chess-pieces.ttf : les 12 glyphes
♔♕♖♗♘♙♚♛♜♝♞♟ de Noto Sans Symbols 2 (licence OFL, voir OFL.txt), extraits par

    uvx --from fonttools pyftsubset NotoSansSymbols2-Regular.ttf --unicodes=U+2654-265F \
        --output-file=assets/fonts/chess-pieces.ttf --no-hinting --desubroutinize

Une case par pièce, dans l'ordre roi, dame, tour, fou, cavalier, pion, blancs puis noirs. Trois
niveaux : noir pour l'encre, blanc pour l'intérieur de la pièce (il cache le hachurage des cases
foncées), gris (128) pour le dehors, transparent.

    uv run --with pillow python assets/fonts/chess-sprite.py
"""

from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFont

CELL = 56
HERE = Path(__file__).resolve().parent
OUT = HERE.parent.parent / "src" / "blocks" / "chess-pieces.png"


def glyph(font, char):
    """Le glyphe en 1 bit, centré dans sa case (noir = encre)."""
    img = Image.new("L", (CELL, CELL), 255)
    draw = ImageDraw.Draw(img)
    left, top, right, bottom = draw.textbbox((0, 0), char, font=font)
    x = (CELL - (right - left)) / 2 - left
    y = (CELL - (bottom - top)) / 2 - top
    draw.text((x, y), char, font=font, fill=0)
    return img.point(lambda v: 0 if v < 128 else 255)


def silhouette(ink):
    """Tout ce que la pièce couvre : l'encre et ce qu'elle entoure (remplissage depuis le bord)."""
    outside = ink.copy()
    ImageDraw.floodfill(outside, (0, 0), 128)
    return outside.point(lambda v: 255 if v == 128 else 0)  # blanc dehors, noir dedans


def main():
    font = ImageFont.truetype(str(HERE / "chess-pieces.ttf"), 48)
    sheet = Image.new("L", (CELL * 12, CELL), 128)
    for k, code in enumerate(range(0x2654, 0x2660)):
        ink = glyph(font, chr(code))
        # La silhouette d'une pièce blanche vient de sa jumelle noire (pleine), même contour.
        solid = glyph(font, chr(code + 6 if code < 0x265A else code))
        inside = ImageChops.darker(silhouette(solid), silhouette(ink))
        cell = Image.new("L", (CELL, CELL), 128)
        cell.paste(255, mask=inside.point(lambda v: 255 if v == 0 else 0))
        cell.paste(0, mask=ink.point(lambda v: 255 if v == 0 else 0))
        sheet.paste(cell, (k * CELL, 0))
    sheet.save(OUT, optimize=True)
    print(OUT)


main()
