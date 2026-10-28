#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path

FILES = [
    Path("crates/services/ogc-usbgecko/src/lib.rs"),
    Path("crates/services/ogc-network-sys/src/lib.rs"),
    Path("crates/services/ogc-card-sys/src/lib.rs"),
]

FUNCTION_RE = re.compile(
    r"(?m)^(?P<indent>[ \t]*)pub\s+unsafe\s+fn\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\("
)


def find_closing_paren(source: str, opening: int) -> int:
    depth = 0
    index = opening

    while index < len(source):
        char = source[index]

        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0:
                return index
        elif char == '"':
            index += 1
            while index < len(source):
                if source[index] == "\\":
                    index += 2
                    continue
                if source[index] == '"':
                    break
                index += 1

        index += 1

    raise RuntimeError("Unbalanced function parameter list")


def split_parameters(parameters: str) -> list[str]:
    result: list[str] = []
    start = 0
    depth = 0

    for index, char in enumerate(parameters):
        if char in "([{<":
            depth += 1
        elif char in ")]}>":
            depth = max(0, depth - 1)
        elif char == "," and depth == 0:
            value = parameters[start:index].strip()
            if value:
                result.append(value)
            start = index + 1

    final = parameters[start:].strip()
    if final:
        result.append(final)

    return result


def raw_pointer_parameters(parameters: str) -> list[tuple[str, str, str]]:
    pointers: list[tuple[str, str, str]] = []

    for parameter in split_parameters(parameters):
        match = re.search(
            r"\b([A-Za-z_][A-Za-z0-9_]*)\s*:\s*"
            r"\*(const|mut)\s+([^,]+)",
            parameter,
        )
        if match:
            pointers.append(
                (
                    match.group(1),
                    match.group(2),
                    match.group(3).strip(),
                )
            )

    return pointers


def already_has_safety_docs(source: str, function_start: int) -> bool:
    prefix = source[:function_start]
    lines = prefix.splitlines()

    relevant: list[str] = []

    for line in reversed(lines):
        stripped = line.strip()

        if stripped.startswith("///"):
            relevant.append(stripped)
            continue

        if stripped.startswith("#["):
            continue

        if not stripped:
            if relevant:
                break
            continue

        break

    return any("# Safety" in line for line in relevant)


def make_docs(
    indent: str,
    name: str,
    pointers: list[tuple[str, str, str]],
) -> str:
    lines = [
        f"{indent}/// # Safety",
        f"{indent}///",
        f"{indent}/// The caller must uphold the validity requirements of every raw pointer",
        f"{indent}/// passed to this function and of the underlying libogc operation.",
    ]

    for parameter, kind, pointee in pointers:
        quoted = f"`{parameter}`"

        if "c_char" in pointee or "i8" in pointee:
            if kind == "const":
                requirement = (
                    f"{quoted}, when non-null, must point to readable memory. "
                    "When interpreted as a C string, it must be terminated by a "
                    "NUL byte before the end of the readable allocation."
                )
            else:
                requirement = (
                    f"{quoted}, when non-null, must point to writable memory "
                    "with sufficient capacity for the operation."
                )
        elif kind == "const":
            requirement = (
                f"{quoted}, when non-null, must be properly aligned and valid "
                "for every read performed by this function."
            )
        else:
            requirement = (
                f"{quoted}, when non-null, must be properly aligned and valid "
                "for every read or write performed by this function."
            )

        lines.append(f"{indent}///")
        lines.append(f"{indent}/// {requirement}")

    lines.extend(
        [
            f"{indent}///",
            f"{indent}/// Each referenced allocation must be large enough for the sizes,",
            f"{indent}/// counts, offsets, and structures supplied through the other arguments.",
            f"{indent}/// The memory must remain valid for the entire period in which the",
            f"{indent}/// underlying operation can access it, and all Rust aliasing rules must",
            f"{indent}/// be respected.",
        ]
    )

    if "async" in name.lower():
        lines.extend(
            [
                f"{indent}///",
                f"{indent}/// For this asynchronous operation, all referenced objects and buffers",
                f"{indent}/// must remain alive and must not be moved, freed, or incompatibly",
                f"{indent}/// accessed until the operation has completed and its callback has",
                f"{indent}/// finished using them.",
            ]
        )

    return "\n".join(lines) + "\n"


def process(path: Path) -> int:
    if not path.is_file():
        raise FileNotFoundError(path)

    source = path.read_text()
    insertions: list[tuple[int, str]] = []

    for match in FUNCTION_RE.finditer(source):
        function_start = match.start()
        name = match.group("name")
        indent = match.group("indent")

        if already_has_safety_docs(source, function_start):
            continue

        opening = source.find("(", match.start(), match.end() + 1)
        if opening == -1:
            raise RuntimeError(f"Could not find parameter list for {name} in {path}")

        closing = find_closing_paren(source, opening)
        parameters = source[opening + 1 : closing]
        pointers = raw_pointer_parameters(parameters)

        if not pointers:
            continue

        insertions.append(
            (
                function_start,
                make_docs(indent, name, pointers),
            )
        )

    for position, documentation in reversed(insertions):
        source = source[:position] + documentation + source[position:]

    if insertions:
        path.write_text(source)

    return len(insertions)


def main() -> None:
    total = 0

    for path in FILES:
        count = process(path)
        total += count
        print(f"{path}: added {count} Safety section(s)")

    print(f"Total added: {total}")


if __name__ == "__main__":
    main()
