#!/usr/bin/env python3
"""Package every Native32 .smf game with its dependent files into one ZIP.

Native32 SMF files can reference scenes and videos that live elsewhere in the
game tree, for example:

    /NA32SSL /ENGLISH /BBLADE  /BBSTART.SSL
    /NA32SSL /ENGLISH /LOGO    /NALOGO.MPG

The player has to keep that directory layout intact or the game cannot find
its scenes. This script walks those references (including references found in
the referenced SSL scenes) and writes a self-contained archive per game:

    dist/EACT/EBBLADE.zip
      EACT/EBBLADE.smf
      EACT/EBBLADE.dat
      NA32SSL/ENGLISH/BBLADE/*.SSL
      NA32SSL/ENGLISH/LOGO/NALOGO.MPG

Paths inside the archive are relative to the game tree root, so the original
relative layout between the SMF and its resources is preserved.

The emulator resolves resource paths by searching from the current SMF's
directory upward, so an archive like the above can be launched directly.
"""

from __future__ import annotations

import argparse
import re
import sys
import zipfile
from pathlib import Path

# File types that can reference other content.
SCAN_SUFFIXES = {".smf", ".sgm", ".ssl", ".dat"}
# File types that count as game resources.
REFERENCE_SUFFIXES = {".smf", ".sgm", ".ssl", ".dat", ".mpg", ".nes"}
# Save files and shell metadata are not part of a package.
EXCLUDED_NAMES = {"thumbs.db"}
EXCLUDED_SUFFIXES = {".ssl_sav"}

ASCII_RUN = re.compile(rb"[\x20-\x7e]{3,}")
COMPONENT_RE = re.compile(r"^[A-Za-z0-9_ .-]+$")
INVALID_PATH_CHARS = set('\\:*?"<>|')
MENU_SMFS = {"fhui.smf", "na32ui.smf"}

# Resource references are a property of the file, so packages that share an
# asset folder (EACT and EPOP both use NA32SSL/ENGLISH/BBLADE, for example)
# can reuse the scan result.
REFERENCE_CACHE: dict[Path, frozenset[str]] = {}


def references_for(path: Path) -> frozenset[str]:
    cached = REFERENCE_CACHE.get(path)
    if cached is None:
        cached = frozenset(extract_references(path.read_bytes()))
        REFERENCE_CACHE[path] = cached
    return cached


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def default_game_root() -> Path:
    return repo_root() / "tmp" / "native32_game"


def is_asset_name(name: str) -> bool:
    return Path(name).suffix.lower() in REFERENCE_SUFFIXES


def clean_components(ref: str) -> list[str]:
    """Split a Native32 path, trimming the padding spaces on every component."""
    components: list[str] = []
    for raw in ref.split("/"):
        component = raw.strip()
        if component in ("", "."):
            continue
        if component == "..":
            if components:
                components.pop()
            continue
        components.append(component)
    return components


def extract_references(data: bytes) -> set[str]:
    """Collect path-like strings from a Native32 container.

    Native32 files are not fully encrypted, so their resource paths appear as
    plain NUL-terminated ASCII strings. Several paths can share one string,
    separated by '+'.
    """
    references: set[str] = set()

    for match in ASCII_RUN.finditer(data):
        text = match.group().decode("ascii", errors="ignore")
        for part in text.split("+"):
            candidate = part.strip()
            if not candidate or any(ch in INVALID_PATH_CHARS for ch in candidate):
                continue

            absolute = candidate.startswith("/")
            if not absolute and not is_asset_name(candidate):
                continue

            components = clean_components(candidate)
            if not components:
                continue
            if any(not COMPONENT_RE.match(component) for component in components):
                continue
            # A lone "/word" is usually just UI text or binary noise; a real
            # reference either has an asset extension or several components.
            if not is_asset_name(components[-1]) and len(components) < 2:
                continue

            references.add("/".join(components) if not absolute else "/" + "/".join(components))

    return references


class Package:
    def __init__(self, root: Path, entry: Path, verbose: bool) -> None:
        self.root = root
        self.entry = entry
        self.verbose = verbose
        self.include_dat = True
        self.files: dict[Path, None] = {}
        self.directories: set[Path] = set()
        self.pending: list[Path] = []
        self.visited_refs: set[tuple[Path, str]] = set()
        self.unresolved: set[str] = set()
        self._listdir_cache: dict[Path, list[Path]] = {}

    def add(self, path: Path) -> None:
        path = path.resolve()
        if path in self.files or not path.is_file():
            return
        if not self.is_packable(path):
            return
        self.files[path] = None
        if self.is_scannable(path):
            self.pending.append(path)

    @staticmethod
    def is_packable(path: Path) -> bool:
        if path.name.casefold() in EXCLUDED_NAMES:
            return False
        return path.suffix.casefold() not in EXCLUDED_SUFFIXES

    @staticmethod
    def is_scannable(path: Path) -> bool:
        return path.suffix.casefold() in SCAN_SUFFIXES

    def entries(self, directory: Path) -> list[Path]:
        if directory not in self._listdir_cache:
            try:
                self._listdir_cache[directory] = sorted(directory.iterdir())
            except OSError:
                self._listdir_cache[directory] = []
        return self._listdir_cache[directory]

    def match_entry(self, directory: Path, name: str) -> Path | None:
        target = name.casefold()
        exact = None
        for entry in self.entries(directory):
            folded = entry.name.casefold()
            if folded == target:
                return entry
            if exact is None and folded.strip() == target:
                exact = entry
        return exact

    def resolve(self, base: Path, components: list[str]) -> Path | None:
        current = base
        for component in components:
            match = self.match_entry(current, component)
            if match is None:
                return None
            current = match
        return current

    def include_file(self, path: Path) -> None:
        """Include a referenced file and the folder that holds its siblings.

        Whole folders are included because Native32 games also build file names
        at runtime (for example ``/MSPLAY`` + ``10.SSL``), which a static scan
        cannot see.
        """
        parent = path.parent
        if parent == self.root:
            self.add(path)
        else:
            self.include_directory(parent)

    def include_directory(self, directory: Path) -> None:
        if directory in self.directories:
            return
        self.directories.add(directory)
        for path in self.entries(directory):
            if path.is_dir():
                self.include_directory(path)
            else:
                self.add(path)

    def collect_reference(self, reference: str, source: Path) -> None:
        key = (source.parent, reference)
        if key in self.visited_refs:
            return
        self.visited_refs.add(key)

        absolute = reference.startswith("/")
        components = clean_components(reference)
        if not components:
            return
        if absolute and self.match_entry(self.root, components[0]) is None:
            # Compressed image/movie data can contain path-like noise; a real
            # root-relative reference always starts with a top-level folder.
            return

        bases = [self.root] if absolute else self.search_bases(source.parent)

        # Prefer an exact match, searching the same way the emulator does:
        # from the current file's folder upward to the content root.
        for base in bases:
            resolved = self.resolve(base, components)
            if resolved is not None:
                if resolved.is_dir():
                    self.include_directory(resolved)
                else:
                    self.include_file(resolved)
                return

        # No exact match: the final component may be a runtime prefix used to
        # build file names (e.g. "/MSPLAY" -> MSPLAY10.SSL). Include the folder
        # that contains such siblings.
        has_extension = is_asset_name(components[-1])
        for base in bases:
            parent = self.resolve(base, components[:-1])
            if parent is None or not parent.is_dir():
                continue
            last = components[-1].casefold()
            has_prefixed_entries = any(
                entry.name.casefold().startswith(last) for entry in self.entries(parent)
            )
            if has_prefixed_entries or not has_extension:
                self.include_directory(parent)
                return

        self.unresolved.add(reference)

    def search_bases(self, start: Path) -> list[Path]:
        bases: list[Path] = []
        current = start
        while True:
            bases.append(current)
            if current == self.root or current.parent == current or self.root not in current.parents:
                break
            current = current.parent
        return bases

    def build(self) -> list[Path]:
        self.add(self.entry)

        if self.include_dat:
            dat = self.match_entry(self.entry.parent, self.entry.stem + ".dat")
            if dat is not None and dat.is_file():
                self.add(dat)

        while self.pending:
            source = self.pending.pop()
            try:
                references = references_for(source)
            except OSError as exc:
                print(f"  warning: cannot read {source}: {exc}", file=sys.stderr)
                continue
            for reference in references:
                self.collect_reference(reference, source)

        return sorted(self.files)


def discover_games(root: Path, output: Path, patterns: list[str]) -> list[Path]:
    games: list[Path] = []
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.suffix.casefold() != ".smf":
            continue
        try:
            path.relative_to(output)
            continue  # never package our own output
        except ValueError:
            pass
        if path.parent == root and path.name.casefold() in MENU_SMFS:
            continue  # the front-end menu is not a single game
        rel = path.relative_to(root).as_posix()
        if patterns and not any(Path(rel).match(pattern) for pattern in patterns):
            continue
        games.append(path)
    return games


def write_package(package: Package, output: Path, root: Path, dry_run: bool) -> tuple[int, int]:
    files = package.build()
    relative_entry = package.entry.relative_to(root)
    archive = output / relative_entry.parent / f"{package.entry.stem}.zip"

    if package.unresolved and package.verbose:
        for reference in sorted(package.unresolved):
            print(f"  unresolved: {reference}")

    total_bytes = sum(path.stat().st_size for path in files)
    if dry_run:
        print(f"{archive.relative_to(output).as_posix()}  ({len(files)} files, {total_bytes / 1e6:.1f} MB)")
        for path in files:
            print(f"    {path.relative_to(root).as_posix()}")
        return 0, total_bytes

    archive.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for path in files:
            zf.write(path, path.relative_to(root).as_posix())

    print(
        f"{archive.relative_to(output).as_posix()}  "
        f"({len(files)} files, {total_bytes / 1e6:.1f} MB -> {archive.stat().st_size / 1e6:.1f} MB)"
    )
    return archive.stat().st_size, total_bytes


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Package each Native32 .smf game and its dependencies into a ZIP."
    )
    parser.add_argument(
        "root",
        nargs="?",
        type=Path,
        default=None,
        help="game tree root (default: tmp/native32_game in this repository)",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=None,
        help="output folder (default: <root>/dist)",
    )
    parser.add_argument(
        "--only",
        action="append",
        default=[],
        metavar="GLOB",
        help="only package SMFs whose root-relative path matches GLOB (repeatable)",
    )
    parser.add_argument(
        "--no-dat",
        action="store_true",
        help="do not include the companion .dat metadata file",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="list the package contents without writing archives",
    )
    parser.add_argument("-v", "--verbose", action="store_true", help="show unresolved references")
    args = parser.parse_args()

    root = (args.root or default_game_root()).resolve()
    if not root.is_dir():
        parser.error(f"game root does not exist: {root}")

    output = (args.output or (root / "dist")).resolve()
    games = discover_games(root, output, args.only)
    if not games:
        parser.error(f"no .smf games found under {root}")

    print(f"Packaging {len(games)} game(s) from {root} -> {output}")
    total_zip = 0
    total_input = 0
    for entry in games:
        package = Package(root, entry, args.verbose)
        package.include_dat = not args.no_dat
        zip_bytes, input_bytes = write_package(package, output, root, args.dry_run)
        total_zip += zip_bytes
        total_input += input_bytes

    if not args.dry_run:
        print(
            f"Done: {len(games)} package(s), "
            f"{total_input / 1e6:.1f} MB input -> {total_zip / 1e6:.1f} MB zipped"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
