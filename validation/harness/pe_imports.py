"""List the DLLs a Windows executable imports and reject ones a clean machine lacks."""
import os
import struct
from pathlib import Path

MSVC_ADVICE = (
    "rebuild with the MSVC toolchain (rustup stable-x86_64-pc-windows-msvc, "
    "static CRT) so the runtime imports only Windows system DLLs"
)


def imported_dlls(path: Path) -> list[str]:
    data = Path(path).read_bytes()
    if data[:2] != b"MZ":
        raise ValueError(f"{path} is not a PE image")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe : pe + 4] != b"PE\0\0":
        raise ValueError(f"{path} is not a PE image")
    sections = struct.unpack_from("<H", data, pe + 6)[0]
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    directories = optional + (112 if struct.unpack_from("<H", data, optional)[0] == 0x20B else 96)
    import_rva = struct.unpack_from("<I", data, directories + 8)[0]
    table = [
        struct.unpack_from("<8sIIII", data, optional + optional_size + index * 40)
        for index in range(sections)
    ]

    def offset(rva: int) -> int:
        for _, virtual_size, virtual_address, raw_size, raw_pointer in table:
            if virtual_address <= rva < virtual_address + max(virtual_size, raw_size):
                return rva - virtual_address + raw_pointer
        raise ValueError(f"{path} has an import table outside its sections")

    names = []
    if import_rva:
        cursor = offset(import_rva)
        while True:
            name_rva = struct.unpack_from("<I", data, cursor + 12)[0]
            if name_rva == 0:
                break
            start = offset(name_rva)
            names.append(data[start : data.index(b"\0", start)].decode("ascii"))
            cursor += 20
    return names


def require_system_only(path: Path) -> None:
    system32 = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"
    missing = [
        name
        for name in imported_dlls(path)
        if not name.lower().startswith("api-ms-win-") and not (system32 / name).exists()
    ]
    if missing:
        raise ValueError(f"{path} imports {', '.join(missing)}, which a clean Windows install lacks: {MSVC_ADVICE}")
