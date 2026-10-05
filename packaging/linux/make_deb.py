"""Write a .deb without dpkg: an ar archive holding debian-binary, control.tar.gz and data.tar.gz.

Usage: make_deb.py <staged root> <control template> <version> <deb arch> <output .deb>

Every file is owned by root:root with one modification time (SOURCE_DATE_EPOCH, else 0), so the package
carries nothing of the machine that built it.
"""

import io
import os
import sys
import tarfile

MTIME = int(os.environ.get("SOURCE_DATE_EPOCH", "0"))


def tar_gz(members):
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz", format=tarfile.GNU_FORMAT) as tar:
        for name, data, mode, kind in members:
            info = tarfile.TarInfo(name)
            info.uid = info.gid = 0
            info.uname = info.gname = "root"
            info.mtime = MTIME
            info.mode = mode
            if kind == "dir":
                info.type = tarfile.DIRTYPE
                tar.addfile(info)
            else:
                info.size = len(data)
                tar.addfile(info, io.BytesIO(data))
    return buf.getvalue()


def data_members(root):
    out = [("./", None, 0o755, "dir")]
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        rel = os.path.relpath(dirpath, root)
        for d in dirnames:
            out.append(("./" + os.path.normpath(os.path.join(rel, d)) + "/", None, 0o755, "dir"))
        for f in sorted(filenames):
            p = os.path.join(dirpath, f)
            mode = 0o755 if os.access(p, os.X_OK) else 0o644
            with open(p, "rb") as fh:
                out.append(("./" + os.path.normpath(os.path.join(rel, f)), fh.read(), mode, "file"))
    return out


def installed_kb(root):
    total = 0
    for dirpath, _, filenames in os.walk(root):
        for f in filenames:
            total += os.path.getsize(os.path.join(dirpath, f))
    return (total + 1023) // 1024


def ar_member(name, data):
    header = f"{name:<16}{MTIME:<12}{0:<6}{0:<6}{'100644':<8}{len(data):<10}`\n".encode()
    body = data + (b"\n" if len(data) % 2 else b"")
    return header + body


def main():
    root, template, version, arch, out = sys.argv[1:6]
    with open(template) as fh:
        control = fh.read()
    control = control.replace("@VERSION@", version).replace("@ARCH@", arch)
    control = control.replace("@SIZE@", str(installed_kb(root)))
    control_tgz = tar_gz([("./", None, 0o755, "dir"), ("./control", control.encode(), 0o644, "file")])
    data_tgz = tar_gz(data_members(root))
    with open(out, "wb") as fh:
        fh.write(b"!<arch>\n")
        fh.write(ar_member("debian-binary", b"2.0\n"))
        fh.write(ar_member("control.tar.gz", control_tgz))
        fh.write(ar_member("data.tar.gz", data_tgz))


if __name__ == "__main__":
    main()
