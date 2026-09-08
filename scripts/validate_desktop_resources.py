#!/usr/bin/env python3
"""Validate desktop files, shared copies, and the actual embedded resource bytes."""

from pathlib import Path
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parent.parent
APP_ID = "io.github.miiikuuu.termimochi"
PREFIX = "/io/github/miiikuuu/termimochi"
MANIFEST = Path(f"crates/termimochi/resources/{APP_ID}.gresource.xml")
ACTION_ICONS = ("open", "save", "layout", "prompt")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def inventory(directory):
    return {
        path.relative_to(directory): path
        for path in directory.rglob("*")
        if path.is_file()
    }


def same_paths(actual, expected, label):
    missing = sorted(str(path) for path in expected.keys() - actual.keys())
    extra = sorted(str(path) for path in actual.keys() - expected.keys())
    require(not missing and not extra, f"{label}: missing={missing}, unexpected={extra}")


def expected_resources(root):
    crate = root / "crates/termimochi"
    installed = inventory(root / "data/icons/hicolor")
    embedded = inventory(crate / "resources/icons")
    # Action icons are private application resources, not desktop-theme copies.
    shared = {
        path: source for path, source in embedded.items()
        if path.parts[:2] != ("symbolic", "actions")
    }
    require(installed, "Desktop icon inventory is empty")
    same_paths(shared, installed, "Shared desktop icons")
    for path, source in installed.items():
        require(
            source.read_bytes() == shared[path].read_bytes(),
            f"Desktop icon copy differs: {path}",
        )
    expected = {}
    for path, source in embedded.items():
        alias = path
        if path.parts[:2] == ("symbolic", "actions"):
            alias = Path("scalable/actions") / path.name
        expected[f"{PREFIX}/icons/{alias.as_posix()}"] = source
    for action in ACTION_ICONS:
        key = f"{PREFIX}/icons/scalable/actions/termimochi-{action}-symbolic.svg"
        require(key in expected, f"Missing application action icon: {key}")
    for alias, name in [
        ("Fastfetch-MIT.txt", "LICENSE"),
        ("Fastfetch-assets.md", "README.md"),
    ]:
        source = crate / "resources/fastfetch" / name
        require(source.is_file(), f"Missing bundled license: {source}")
        expected[f"{PREFIX}/licenses/{alias}"] = source
    return expected


def validate_manifest(root, expected):
    declared = {}
    for group in ET.parse(root / MANIFEST).getroot().findall("gresource"):
        for entry in group.findall("file"):
            source = (entry.text or "").strip()
            require(source, "Empty resource source in manifest")
            alias = entry.get("alias", source)
            key = f"{group.get('prefix', '').rstrip('/')}/{alias}"
            require(key not in declared, f"Duplicate resource alias: {key}")
            declared[key] = root / "crates/termimochi" / source
    same_paths(declared, expected, "Resource manifest")
    for key, source in declared.items():
        require(source == expected[key], f"Wrong source for {key}: {source}")


def compile_bundle(root, target):
    subprocess.run(
        [
            "glib-compile-resources", str(root / MANIFEST),
            f"--sourcedir={root / 'crates/termimochi'}", f"--target={target}",
        ],
        check=True,
    )


def validate_bundle(bundle, expected):
    listed = subprocess.check_output(
        ["gresource", "list", str(bundle)], text=True,
    ).splitlines()
    require(len(listed) == len(set(listed)), "Duplicate paths in compiled resource bundle")
    same_paths(dict.fromkeys(listed), expected, "Compiled resources")
    for key, source in expected.items():
        content = subprocess.check_output(["gresource", "extract", str(bundle), key])
        require(content == source.read_bytes(), f"Embedded resource content differs: {key}")


def validate_packaged_palettes(root):
    for name, destination in [
        ("fog-paper.palette", "crates/termimochi-core/fixtures"),
        ("fog-paper-codex.palette", "crates/termimochi-core/fixtures"),
        ("fog-paper-codex.palette", "crates/termimochi-cli/fixtures"),
        ("fog-paper.palette", "crates/termimochi/resources/themes"),
    ]:
        copy = root / destination / name
        require(
            (root / "themes" / name).read_bytes() == copy.read_bytes(),
            f"Packaged palette differs: {copy}",
        )


def main():
    desktop = ROOT / "data" / f"{APP_ID}.desktop"
    metadata = ROOT / "data" / f"{APP_ID}.metainfo.xml"
    subprocess.run(["desktop-file-validate", str(desktop)], check=True)
    require(
        "Exec=termimochi %f" in desktop.read_text(encoding="utf-8").splitlines(),
        "Unexpected desktop Exec command",
    )
    subprocess.run(["appstreamcli", "validate", "--pedantic", str(metadata)], check=True)
    subprocess.run(["xmllint", "--noout", str(metadata), str(ROOT / MANIFEST)], check=True)
    expected = expected_resources(ROOT)
    validate_manifest(ROOT, expected)
    with tempfile.TemporaryDirectory(prefix="termimochi-resource-check-") as directory:
        bundle = Path(directory) / "termimochi.gresource"
        compile_bundle(ROOT, bundle)
        validate_bundle(bundle, expected)
    validate_packaged_palettes(ROOT)
    print(
        f"Desktop metadata, {len(expected)} embedded resources (paths and bytes), "
        "and packaged copies passed."
    )


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, ET.ParseError, subprocess.CalledProcessError) as error:
        print(f"Desktop/resource validation failed: {error}", file=sys.stderr)
        sys.exit(1)
