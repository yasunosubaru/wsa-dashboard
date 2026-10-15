# One-shot tool: rewrite the OURS array in src/i18n/mod.rs from the derivation.
#
# `derive-ours-keys.py` prints the list; this one puts it in the file, so the
# list and the file cannot drift because of a typo in either direction.
import re
import sys

TARGET = "src/i18n/mod.rs"
DERIVER = "tools/derive-ours-keys.py"


def derive():
    """Run the deriver's logic rather than its printing."""
    sys.path.insert(0, "tools")
    import importlib.util

    spec = importlib.util.spec_from_file_location("derive", DERIVER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    ours = module.ours_english()
    theirs = module.reference_english()
    changed = []
    for key, literal in sorted(ours.items()):
        upstream = theirs.get(key)
        if upstream is not None and upstream != module.unescape(literal):
            changed.append(key)
    return changed


def main():
    keys = derive()
    block = "pub static OURS: &[&str] = &[\n"
    block += "".join('    "%s",\n' % key for key in keys)
    block += "];"

    src = open(TARGET, encoding="utf-8").read()
    replaced, count = re.subn(
        r"pub static OURS: &\[&str\] = &\[.*?\n\];", block, src, flags=re.S
    )
    if count != 1:
        print("could not find exactly one OURS block (found %d)" % count)
        return 1
    with open(TARGET, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(replaced)
    print("OURS now lists %d keys" % len(keys))
    return 0


if __name__ == "__main__":
    sys.exit(main())