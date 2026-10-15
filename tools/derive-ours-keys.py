# One-shot tool: compare our English table with the reference's en.toml.
#
# The difference is what `OURS` in src/i18n/mod.rs has to be: the keys whose
# wording we deliberately changed (Q12 keeps the key names, not the sentences),
# and which a translated pack must therefore not overwrite.
#
# Run from the repo root:  python tools/derive-ours-keys.py
import re
import sys
import tomllib

OURS_TABLE = "src/i18n/mod.rs"
REFERENCE_EN = "docs/reference/assets/i18n/en.toml"


def ours_english():
    """Pull `("key", "value"),` pairs out of the ENGLISH array."""
    src = open(OURS_TABLE, encoding="utf-8").read()
    body = src[src.index("pub static ENGLISH"):]
    pairs = re.findall(r'\(\s*"([a-z0-9_.]+)"\s*,\s*("(?:[^"\\]|\\.)*")', body)
    return {key: value for key, value in pairs}


def unescape(literal):
    """A Rust string literal, minus its quotes."""
    body = literal[1:-1]
    return (
        body.replace('\\\\"', '"')
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\\", "\\")
    )


def reference_english():
    """Flatten the reference's `[section] key = "..."` into `section.key`."""
    with open(REFERENCE_EN, "rb") as handle:
        table = tomllib.load(handle)
    flat = {}

    def walk(node, prefix):
        for key, value in node.items():
            full = key if not prefix else prefix + "." + key
            if isinstance(value, dict):
                walk(value, full)
            elif isinstance(value, str):
                flat[full] = value

    walk(table, "")
    return flat


def main():
    ours = ours_english()
    reference = reference_english()
    print("our table keys      :", len(ours))
    print("reference en.toml   :", len(reference))

    changed, identical, new = [], [], []
    for key, literal in sorted(ours.items()):
        if key not in reference:
            new.append(key)
        elif reference[key] == unescape(literal):
            identical.append(key)
        else:
            changed.append(key)

    print("identical wording   :", len(identical))
    print("changed wording     :", len(changed))
    for key in changed:
        print("    OURS", key)
    print("key absent upstream :", len(new))
    for key in new:
        print("    NEW ", key)

    # The list the test will assert, one per line, ready to paste.
    print()
    print("// paste into OURS")
    for key in changed:
        print('    "%s",' % key)
    return 0


if __name__ == "__main__":
    sys.exit(main())