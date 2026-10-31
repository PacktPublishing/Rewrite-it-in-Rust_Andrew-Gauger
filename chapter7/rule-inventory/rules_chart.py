"""Figure 7.4: the rules, the rules the AI's notes state, and the rules with a
right AI test, in three groups. Reads the "H5's numbers" table in rules.md.

    python3 chapter7/rule-inventory/rules_chart.py

Needs matplotlib. Writes chapter7/fig-rule-count.png, greyscale for print,
5.5 x 2.0 inches at 192 DPI (the docx builder reads pixels at 96 DPI and caps
the width at the 5.5-inch text column, so it prints at exactly this size).
"""
import re
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "fig-rule-count.png"

table = (HERE / "rules.md").read_text().split("## H5's numbers", 1)[1]
rows = []
for line in table.splitlines():
    cells = [c.strip(" *") for c in line.strip().strip("|").split("|")]
    if len(cells) == 4 and re.fullmatch(r"\d+", cells[1]):
        rows.append((cells[0], *map(int, cells[1:])))
rows = [r for r in rows if r[0] != "All"]
assert len(rows) == 3, rows

labels = {
    "Keep": "Keep as is",
    "Flag, Fix or Ask: needs a decision": "Needs a decision",
    "Only people hold": "Only people hold",
}
groups = [labels[r[0]] for r in rows]
series = [
    ("Rules", [r[1] for r in rows], "#d0d0d0"),
    ("Stated in the AI's notes", [r[2] for r in rows], "#8c8c8c"),
    ("With a right AI test", [r[3] for r in rows], "#333333"),
]

plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 8})
fig, ax = plt.subplots(figsize=(5.5, 2.0), dpi=192)
width, gap = 0.26, 0.02
for i, (name, values, color) in enumerate(series):
    xs = [g + (i - 1) * (width + gap) for g in range(len(groups))]
    bars = ax.bar(xs, values, width, color=color, label=name,
                  edgecolor="#000000", linewidth=0.4)
    for x, v in zip(xs, values):
        ax.text(x, v + 0.6, str(v), ha="center", va="bottom", fontsize=8,
                color="#000000")

ax.set_xticks(range(len(groups)), groups)
ax.set_ylabel("Rules")
ax.set_ylim(0, 33)
ax.yaxis.grid(True, color="#e0e0e0", linewidth=0.5)
ax.set_axisbelow(True)
for side in ("top", "right"):
    ax.spines[side].set_visible(False)
ax.tick_params(length=0)
ax.legend(frameon=False, loc="upper right", fontsize=8)
fig.tight_layout()
fig.savefig(OUT, facecolor="white")
print(f"Wrote {OUT}")
