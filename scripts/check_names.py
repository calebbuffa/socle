"""Find all property names that aren't valid Rust identifiers."""

import json
import re


def main():
    d = json.load(open("generated/i3s_spec.json"))
    for mod_name, mod in d["modules"].items():
        for t in mod["types"]:
            for p in t["properties"]:
                n = p["name"]
                # Check if it would be a problem as a Rust field name
                if (
                    n.startswith("(")
                    or n.startswith("$")
                    or not re.match(r"^[a-zA-Z_][a-zA-Z0-9_]*$", n)
                ):
                    print(f"  {mod_name}/{t['rust_name']}.{n}")


if __name__ == "__main__":
    main()
