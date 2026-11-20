"""A8: list every dependency license of fm-spike, flag copyleft, report rust-version
needs and the number of crates. Reads `cargo metadata` JSON on stdin."""
import collections
import json
import sys

meta = json.load(sys.stdin)
pkgs = meta["packages"]
copyleft = ("GPL", "AGPL", "LGPL")
by_license = collections.defaultdict(list)
for p in pkgs:
    by_license[p.get("license") or "UNKNOWN"].append(p["name"])
print(f"crates in the dependency graph (cargo metadata, all targets): {len(pkgs)}")
print("--- licenses (count: expression) ---")
for lic, names in sorted(by_license.items(), key=lambda kv: -len(kv[1])):
    print(f"{len(names):4d}: {lic}")
print("--- flagged (GPL/AGPL/LGPL appears in the expression) ---")
flagged = [(p["name"], p.get("license")) for p in pkgs if any(c in (p.get("license") or "") for c in copyleft)]
for name, lic in flagged:
    print(f"  {name}: {lic}")
if not flagged:
    print("  none")
print("--- rust-version / edition of GPUI crates ---")
for p in pkgs:
    if p["name"].startswith(("gpui", "fm-spike")):
        print(f"  {p['name']} {p['version']}: rust-version={p.get('rust_version')} edition={p['edition']}")
print("--- unknown licenses ---")
for name in by_license.get("UNKNOWN", []):
    print("  ", name)
sys.exit(0)
