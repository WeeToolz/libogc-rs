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

GENERATED_MARKER = (
    "The caller must uphold the validity requirements of every raw pointer"
)


def find_matching_paren(source: str, opening: int) -> int:
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

    raise RuntimeError("Unbalanced function signature")


def split_parameters(parameters: str) -> list[str]:
    result: list[str] = []
    start = 0
    depth = 0

    for index, char in enumerate(parameters):
        if char in "([{<":
            depth += 1
        elif char in ")]}>":
            depth = max(depth - 1, 0)
        elif char == "," and depth == 0:
            parameter = parameters[start:index].strip()
            if parameter:
                result.append(parameter)
            start = index + 1

    final = parameters[start:].strip()
    if final:
        result.append(final)

    return result


def raw_pointers(parameters: str) -> list[tuple[str, str, str]]:
    pointers: list[tuple[str, str, str]] = []

    for parameter in split_parameters(parameters):
        match = re.search(
            r"\b([A-Za-z_][A-Za-z0-9_]*)\s*:\s*"
            r"\*(const|mut)\s+(.+)$",
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


def preceding_doc_range(
    source: str,
    function_start: int,
) -> tuple[int, int, str] | None:
    line_start = source.rfind("\n", 0, function_start) + 1
    cursor = line_start
    collected: list[tuple[int, int, str]] = []

    while cursor > 0:
        previous_end = cursor - 1
        previous_start = source.rfind("\n", 0, previous_end) + 1
        line = source[previous_start:previous_end]
        stripped = line.strip()

        if stripped.startswith("///") or stripped.startswith("#[") or not stripped:
            collected.append((previous_start, cursor, line))
            cursor = previous_start
            continue

        break

    if not collected:
        return None

    start = min(item[0] for item in collected)
    text = source[start:line_start]

    if "/// # Safety" not in text or GENERATED_MARKER not in text:
        return None

    # Preserve attributes immediately above the function.
    lines = text.splitlines(keepends=True)
    doc_lines: list[str] = []
    offset = start
    doc_start: int | None = None
    doc_end: int | None = None

    for line in lines:
        stripped = line.strip()

        if stripped.startswith("///") or (
            doc_start is not None and not stripped
        ):
            if doc_start is None:
                doc_start = offset
            doc_end = offset + len(line)
            doc_lines.append(line)
        elif doc_start is not None:
            break

        offset += len(line)

    if doc_start is None or doc_end is None:
        return None

    return doc_start, doc_end, "".join(doc_lines)


def is_c_string_pointer(name: str, pointee: str) -> bool:
    lower_name = name.lower()
    lower_type = pointee.lower()

    if "c_char" not in lower_type and lower_type not in {"i8", "u8"}:
        return False

    string_words = (
        "name",
        "path",
        "url",
        "host",
        "addr",
        "filename",
        "gamecode",
        "company",
        "interface",
    )
    return any(word in lower_name for word in string_words)


def make_docs(
    indent: str,
    function_name: str,
    pointers: list[tuple[str, str, str]],
) -> str:
    lines = [
        f"{indent}/// # Safety",
        f"{indent}///",
    ]

    descriptions: list[str] = []

    for name, mutability, pointee in pointers:
        quoted = f"`{name}`"

        if is_c_string_pointer(name, pointee) and mutability == "const":
            descriptions.append(
                f"{quoted} must be null or point to a readable "
                "NUL-terminated C string."
            )
        elif is_c_string_pointer(name, pointee) and mutability == "mut":
            descriptions.append(
                f"{quoted} must be null or point to writable storage "
                "large enough for the resulting C string and NUL byte."
            )
        elif mutability == "const":
            descriptions.append(
                f"{quoted} must be null where permitted, or properly "
                "aligned and valid for all required reads."
            )
        else:
            descriptions.append(
                f"{quoted} must be null where permitted, or properly "
                "aligned and valid for all required reads and writes."
            )

    for index, description in enumerate(descriptions):
        prefix = "" if index == 0 else "Additionally, "
        lines.append(f"{indent}/// {prefix}{description}")

    lines.extend(
        [
            f"{indent}/// Buffer extents must cover the supplied lengths, counts, and offsets,",
            f"{indent}/// and the referenced memory must obey Rust's aliasing requirements.",
        ]
    )

    if "async" in function_name.lower():
        lines.extend(
            [
                f"{indent}/// All referenced objects must remain valid and unmoved until the",
                f"{indent}/// asynchronous operation and its callback have completed.",
            ]
        )
    else:
        lines.append(
            f"{indent}/// The pointers must remain valid for the duration of the call."
        )

    return "\n".join(lines) + "\n"


def process(path: Path) -> tuple[int, int]:
    source = path.read_text()
    replacements: list[tuple[int, int, str]] = []
    old_lines = 0
    new_lines = 0

    for match in FUNCTION_RE.finditer(source):
        doc = preceding_doc_range(source, match.start())
        if doc is None:
            continue

        doc_start, doc_end, old_doc = doc

        opening = source.find("(", match.start(), match.end() + 1)
        if opening == -1:
            raise RuntimeError(
                f"Could not locate parameters for {match.group('name')} in {path}"
            )

        closing = find_matching_paren(source, opening)
        parameters = source[opening + 1:closing]
        pointers = raw_pointers(parameters)

        if not pointers:
            continue

        new_doc = make_docs(
            match.group("indent"),
            match.group("name"),
            pointers,
        )

        replacements.append((doc_start, doc_end, new_doc))
        old_lines += old_doc.count("\n")
        new_lines += new_doc.count("\n")

    for start, end, replacement in reversed(replacements):
        source = source[:start] + replacement + source[end:]

    if replacements:
        path.write_text(source)

    print(
        f"{path}: compacted {len(replacements)} block(s), "
        f"{old_lines} -> {new_lines} documentation lines"
    )

    return old_lines, new_lines


def main() -> None:
    total_old = 0
    total_new = 0

    for path in FILES:
        old, new = process(path)
        total_old += old
        total_new += new

    print(
        f"Total generated documentation: "
        f"{total_old} -> {total_new} lines "
        f"({total_old - total_new} removed)"
    )


if __name__ == "__main__":
    main()
