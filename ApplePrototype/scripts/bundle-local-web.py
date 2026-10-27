"""Bundle the locally compiled SolidCraft engine without changing saved app documents."""
import hashlib
import json
from pathlib import Path
import os
import shutil
import subprocess

prototype = Path(__file__).resolve().parents[1]
upstream = prototype.parent / "upstream-solidcraft"
source = upstream / "dist/web"
destination = prototype / "SolidCraftTest/Web"
backup = prototype / "prebuilt-web-backup"
if not (source / "index.html").is_file() or not list(source.glob("*.wasm")):
    raise SystemExit("Compile the local web target before bundling it.")
if not backup.exists():
    shutil.copytree(destination, backup)
shutil.rmtree(destination)
shutil.copytree(source, destination, ignore=shutil.ignore_patterns(".stage"))
for name in ["LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "THIRD-PARTY-LICENSES.txt", "ATTRIBUTION.md"]:
    shutil.copy2(upstream / name, destination / name)
files = [{"file": str(p.relative_to(destination)), "sha256": hashlib.sha256(p.read_bytes()).hexdigest(), "bytes": p.stat().st_size}
         for p in sorted(destination.rglob("*")) if p.is_file()]
revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=upstream, text=True).strip()
(destination / "asset-manifest.json").write_text(json.dumps({"origin": "local source build", "base_revision": revision,
    "modified_source": True, "profile": os.environ.get("SOLIDCRAFT_BUNDLE_PROFILE", "release, Rust optimization enabled, wasm-opt disabled for prototype iteration"), "files": files}, indent=2))
print(f"Bundled {len(files)} files from the local source build.")
