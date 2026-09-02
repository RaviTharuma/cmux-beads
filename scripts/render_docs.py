#!/usr/bin/env python3
"""Render lab-data docs screenshots with the host Ghostty / cmux ANSI palette.

Issue titles are synthetic lab copy only. No usernames, emails, paths, or PII.
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

# Ghostty default 16-color palette (host TERM). Not an app theme.
BG = (29, 31, 33)
FG = (197, 200, 198)
PANE_BG = (24, 26, 27)
CHROME = (45, 48, 51)
RULE = (80, 84, 88)

ANSI = {
    "reset": FG,
    "black": (29, 31, 33),
    "red": (204, 52, 43),
    "green": (25, 136, 68),
    "yellow": (251, 169, 34),
    "blue": (57, 113, 237),
    "magenta": (163, 106, 199),
    "cyan": (57, 173, 199),
    "darkgray": (150, 152, 150),
    "white": (197, 200, 198),
}

CELL_W = 11
CELL_H = 22
PAD = 18

LAB = [
    # status, pri, id, title, assigned
    ("open", 2, "lab-1", "Ship onboarding", False),
    ("open", 3, "lab-3", "Document the API", False),
    ("in_progress", 1, "lab-2", "Fix login timeout", True),
    ("blocked", 0, "lab-4", "Triage checkout flake", False),
    ("deferred", 4, "lab-5", "Polish empty states", False),
]


def font(size: int = 16) -> ImageFont.FreeTypeFont:
    candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
    ]
    for path in candidates:
        if Path(path).exists():
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def status_color(status: str) -> tuple[int, int, int]:
    return {
        "open": ANSI["green"],
        "in_progress": ANSI["cyan"],
        "blocked": ANSI["red"],
        "deferred": ANSI["darkgray"],
        "closed": ANSI["darkgray"],
        "pinned": ANSI["yellow"],
        "hooked": ANSI["magenta"],
    }.get(status, FG)


class Term:
    def __init__(self, cols: int, rows: int, bg: tuple[int, int, int] = BG) -> None:
        self.cols = cols
        self.rows = rows
        self.bg = bg
        self.cells: list[list[tuple[str, tuple[int, int, int], tuple[int, int, int], bool]]] = [
            [(" ", FG, bg, False) for _ in range(cols)] for _ in range(rows)
        ]

    def put(
        self,
        x: int,
        y: int,
        text: str,
        fg: tuple[int, int, int] = FG,
        bg: tuple[int, int, int] | None = None,
        reverse: bool = False,
    ) -> None:
        fill = self.bg if bg is None else bg
        for i, ch in enumerate(text):
            if 0 <= x + i < self.cols and 0 <= y < self.rows:
                cell_fg, cell_bg = (fill, fg) if reverse else (fg, fill)
                self.cells[y][x + i] = (ch, cell_fg, cell_bg, reverse)

    def to_image(self) -> Image.Image:
        img = Image.new("RGB", (self.cols * CELL_W, self.rows * CELL_H), self.bg)
        draw = ImageDraw.Draw(img)
        face = font(16)
        for y, row in enumerate(self.cells):
            for x, (ch, fg, bg, _rev) in enumerate(row):
                left = x * CELL_W
                top = y * CELL_H
                draw.rectangle([left, top, left + CELL_W, top + CELL_H], fill=bg)
                draw.text((left, top + 2), ch, font=face, fill=fg)
        return img


def draw_list(term: Term, selected: str = "lab-2") -> None:
    term.put(0, 0, "cmux-beads ", ANSI["cyan"])
    term.put(11, 0, "cmux", ANSI["green"])
    term.put(16, 0, " List repo all", ANSI["darkgray"])
    term.put(31, 0, " 1ws 2pn", ANSI["darkgray"])
    term.put(0, 1, "pane lab", ANSI["darkgray"])
    y = 3
    last = ""
    for status, pri, ident, title, assigned in LAB:
        if status != last:
            term.put(1, y, status.replace("_", " "), status_color(status))
            y += 1
            last = status
        rev = ident == selected
        flag = "*" if assigned else " "
        marker = "▸" if rev else " "
        if rev:
            term.put(0, y, f"{marker}P{pri} {ident}{flag} {title}"[: term.cols], FG, reverse=True)
        else:
            term.put(0, y, marker)
            term.put(1, y, f"P{pri}", ANSI["yellow"])
            term.put(3, y, f" {ident}", ANSI["cyan"])
            term.put(4 + len(ident), y, flag, ANSI["green"] if assigned else FG)
            term.put(5 + len(ident), y, f" {title}")
        y += 1
    term.put(0, term.rows - 1, "5/5  K views  A assign  ? help", ANSI["darkgray"])


def draw_table(term: Term, selected: str = "lab-2") -> None:
    term.put(0, 0, "cmux-beads ", ANSI["cyan"])
    term.put(11, 0, "cmux", ANSI["green"])
    term.put(16, 0, " Table repo all", ANSI["darkgray"])
    term.put(32, 0, " 1ws 2pn", ANSI["darkgray"])
    term.put(0, 1, "pane lab", ANSI["darkgray"])
    term.put(0, 3, " P  id         status     title", ANSI["darkgray"])
    y = 4
    for status, pri, ident, title, assigned in LAB:
        rev = ident == selected
        flag = "*" if assigned else " "
        marker = "▸" if rev else " "
        row = f"{marker}P{pri} {ident:<6}{flag} {title}"
        if rev:
            term.put(0, y, row[: term.cols], FG, reverse=True)
        else:
            term.put(0, y, marker)
            term.put(1, y, f"P{pri}", ANSI["yellow"])
            term.put(3, y, f" {ident:<6}", ANSI["cyan"])
            term.put(10, y, flag, ANSI["green"] if assigned else FG)
            term.put(11, y, f" {title}")
        y += 1
    term.put(0, term.rows - 1, "5/5  K views  A assign  ? help", ANSI["darkgray"])


def draw_kanban(term: Term) -> None:
    term.put(0, 0, "cmux-beads ", ANSI["cyan"])
    term.put(11, 0, "cmux", ANSI["green"])
    term.put(16, 0, " Kanban repo all", ANSI["darkgray"])
    term.put(33, 0, " 1ws 2pn", ANSI["darkgray"])
    term.put(0, 1, "pane lab", ANSI["darkgray"])
    term.put(0, 3, "← in progress →  1 cards", ANSI["cyan"])
    term.put(0, 5, "▸P1 lab-2* Fix login timeout", FG, reverse=True)
    term.put(0, term.rows - 1, "5/5  h/l column  v move  ? help", ANSI["darkgray"])


def frame_window(inner: Image.Image, title: str) -> Image.Image:
    """Minimal cmux chrome. No traffic-light dots, no user paths."""
    w, h = inner.size
    top = 36
    out = Image.new("RGB", (w + PAD * 2, h + top + PAD), CHROME)
    draw = ImageDraw.Draw(out)
    draw.rectangle([0, 0, out.size[0], top], fill=(38, 41, 44))
    draw.line([(0, top - 1), (out.size[0], top - 1)], fill=RULE)
    face = font(14)
    draw.text((PAD, 10), title, font=face, fill=ANSI["darkgray"])
    out.paste(inner, (PAD, top))
    return out


def compose_hero() -> Image.Image:
    pane = Term(52, 16, bg=PANE_BG)
    pane.put(0, 0, "$ bd list --json", ANSI["darkgray"])
    rows = [
        ("lab-1", "open", "Ship onboarding"),
        ("lab-2", "in_progress", "Fix login timeout"),
        ("lab-3", "open", "Document the API"),
        ("lab-4", "blocked", "Triage checkout flake"),
        ("lab-5", "deferred", "Polish empty states"),
    ]
    for i, (ident, status, title) in enumerate(rows):
        y = 2 + i
        pane.put(0, y, ident, ANSI["cyan"])
        pane.put(7, y, f"{status:<13}", status_color(status))
        pane.put(21, y, title)
    pane.put(0, 8, "$", ANSI["green"])
    pane_img = pane.to_image()

    side = Term(42, 16)
    draw_list(side)
    side_img = side.to_image()

    gap = 2
    combo = Image.new("RGB", (pane_img.width + gap + side_img.width, pane_img.height), RULE)
    combo.paste(pane_img, (0, 0))
    combo.paste(side_img, (pane_img.width + gap, 0))
    return frame_window(combo, "cmux")


def main() -> None:
    docs = Path(__file__).resolve().parents[1] / "docs"
    docs.mkdir(exist_ok=True)

    compose_hero().save(docs / "hero.png")

    for name, painter in (
        ("list.png", draw_list),
        ("table.png", draw_table),
        ("kanban.png", draw_kanban),
    ):
        term = Term(42, 16)
        painter(term)
        frame_window(term.to_image(), "cmux-beads").save(docs / name)

    for path in docs.glob("*.png"):
        print(f"wrote {path} ({path.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
