# One-shot tool: lay out assets/i18n from the reference's packs.
#
# Copies the reference's translation packs verbatim and writes an `en.toml`
# generated from *our* ENGLISH table, not the reference's. That direction
# matters: our English is the authority (Q12 keeps the key names, not the
# sentences), and a pack translated from the reference's sentences has to fall
# back to ours rather than overwrite it -- see `OURS` in src/i18n/mod.rs.
#
# Run from the repo root:  python tools/install-language-packs.py
import os
import re
import shutil
import sys

REFERENCE_PACKS = "docs/reference/assets/i18n"
TARGET = "assets/i18n"
OURS_TABLE = "src/i18n/mod.rs"


def ours_english():
    """Pull `("key", "value"),` pairs out of the ENGLISH array."""
    src = open(OURS_TABLE, encoding="utf-8").read()
    body = src[src.index("pub static ENGLISH"):]
    pairs = re.findall(r'\(\s*"([a-z0-9_.]+)"\s*,\s*("(?:[^"\\]|\\.)*")', body)
    return dict(pairs)


def toml_escape(literal):
    """Reuse the Rust literal as-is: TOML's basic strings share its escapes."""
    return literal


def main():
    if not os.path.isdir(REFERENCE_PACKS):
        print("missing", REFERENCE_PACKS)
        return 1
    os.makedirs(TARGET, exist_ok=True)

    copied = 0
    for name in sorted(os.listdir(REFERENCE_PACKS)):
        if not name.endswith(".toml") or name == "en.toml":
            continue
        shutil.copyfile(
            os.path.join(REFERENCE_PACKS, name), os.path.join(TARGET, name)
        )
        copied += 1

    # Our own English, grouped by the first dotted segment so the file reads like
    # the reference's (`[network]`, `[sidebar]`) rather than like a flat list.
    table = ours_english()
    sections = {}
    for key, literal in table.items():
        head, _, rest = key.partition(".")
        # Keys with no dot are the ones with no section; the reference has none
        # and neither do we, but putting them under `[main]` would invent a name.
        sections.setdefault(head, {})[rest or key] = toml_escape(literal)

    lines = [
        "# English strings for WSA Dashboard.",
        "#",
        "# Generated from `ENGLISH` in src/i18n/mod.rs by",
        "# tools/install-language-packs.py -- edit the table, not this file.",
        "# A test asserts the two agree, so hand-editing this is caught.",
        "#",
        "# The wording here is ours wherever WSA has no counterpart to what the",
        "# reference labels; see docs/MAPPING.md. The other 49 packs in this",
        "# folder are the reference's own translations.",
    ]
    for section in sorted(sections):
        lines.append("")
        lines.append("[%s]" % section)
        for key in sorted(sections[section]):
            lines.append('%s = %s' % (key, sections[section][key]))

    with open(os.path.join(TARGET, "en.toml"), "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(lines) + "\n")

    total = len(os.listdir(TARGET))
    print("copied %d packs, wrote en.toml with %d keys, %d files total"
          % (copied, len(table), total))
    return 0


if __name__ == "__main__":
    sys.exit(main())