#!/usr/bin/env python3
"""Temporary local PTY WebSocket server for Warp wasm remote_tty builds.

This implements the minimal protocol used by app/src/terminal/remote_tty:

  ws://127.0.0.1:3030/create?num_rows=24&num_cols=80

Binary WebSocket messages from the browser are written to the PTY. Bytes read
from the PTY are sent back as binary WebSocket messages. Text messages are
treated as resize JSON in the shape {"width": cols, "height": rows}.

This is intentionally local-only and unauthenticated for development.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import contextlib
import errno
import fcntl
import hashlib
import json
import os
import pty
import select
import shutil
import signal
import struct
import sys
import termios
from dataclasses import dataclass
from typing import Optional
from urllib.parse import parse_qs, urlparse


OP_CONTINUATION = 0x0
OP_TEXT = 0x1
OP_BINARY = 0x2
OP_CLOSE = 0x8
OP_PING = 0x9
OP_PONG = 0xA

WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


@dataclass
class HttpUpgrade:
    path: str
    query: dict[str, list[str]]
    headers: dict[str, str]


@dataclass
class Frame:
    opcode: int
    payload: bytes


def choose_shell(shell_arg: Optional[str]) -> str:
    if shell_arg:
        return shell_arg
    if shutil.which("bash"):
        return shutil.which("bash") or "/bin/bash"
    return os.environ.get("SHELL") or "/bin/sh"


def parse_positive_int(values: Optional[list[str]], default: int) -> int:
    if not values:
        return default
    try:
        value = int(values[0])
    except ValueError:
        return default
    return value if value > 0 else default


def set_pty_size(fd: int, rows: int, cols: int) -> None:
    rows = max(1, int(rows))
    cols = max(1, int(cols))
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)


def spawn_shell(shell: str, rows: int, cols: int) -> tuple[int, int]:
    pid, master_fd = pty.fork()
    if pid == 0:
        env = os.environ.copy()
        env.setdefault("TERM", "xterm-256color")
        env.setdefault("COLORTERM", "truecolor")
        env["WARP_IS_LOCAL_WASM_TTY"] = "1"
        try:
            os.execvpe(shell, [shell], env)
        except Exception as exc:
            print(f"failed to exec shell {shell}: {exc}", file=sys.stderr)
            os._exit(127)

    set_pty_size(master_fd, rows, cols)
    return pid, master_fd


async def read_http_upgrade(reader: asyncio.StreamReader) -> Optional[HttpUpgrade]:
    try:
        raw = await reader.readuntil(b"\r\n\r\n")
    except (asyncio.IncompleteReadError, asyncio.LimitOverrunError):
        return None

    try:
        text = raw.decode("utf-8", errors="replace")
        lines = text.splitlines()
        method, target, _version = lines[0].split(maxsplit=2)
    except Exception:
        return None

    if method.upper() != "GET":
        return None

    headers: dict[str, str] = {}
    for line in lines[1:]:
        if not line or ":" not in line:
            continue
        name, value = line.split(":", 1)
        headers[name.strip().lower()] = value.strip()

    parsed = urlparse(target)
    return HttpUpgrade(parsed.path, parse_qs(parsed.query), headers)


async def write_http_response(
    writer: asyncio.StreamWriter, status: str, body: bytes
) -> None:
    writer.write(
        b"HTTP/1.1 "
        + status.encode()
        + b"\r\n"
        + f"Content-Length: {len(body)}\r\n".encode()
        + b"Content-Type: text/plain\r\n"
        + b"Connection: close\r\n\r\n"
        + body
    )
    await writer.drain()
    writer.close()
    with contextlib.suppress(Exception):
        await writer.wait_closed()


async def accept_websocket(
    reader: asyncio.StreamReader, writer: asyncio.StreamWriter
) -> Optional[HttpUpgrade]:
    request = await read_http_upgrade(reader)
    if request is None:
        await write_http_response(writer, "400 Bad Request", b"bad websocket upgrade\n")
        return None

    if request.path != "/create":
        await write_http_response(writer, "404 Not Found", b"expected /create websocket path\n")
        return None

    key = request.headers.get("sec-websocket-key")
    if not key:
        await write_http_response(writer, "400 Bad Request", b"missing Sec-WebSocket-Key\n")
        return None

    accept = base64.b64encode(hashlib.sha1((key + WS_GUID).encode()).digest()).decode()
    writer.write(
        b"HTTP/1.1 101 Switching Protocols\r\n"
        b"Upgrade: websocket\r\n"
        b"Connection: Upgrade\r\n"
        + f"Sec-WebSocket-Accept: {accept}\r\n".encode()
        + b"\r\n"
    )
    await writer.drain()
    return request


async def read_frame(reader: asyncio.StreamReader) -> Optional[Frame]:
    try:
        header = await reader.readexactly(2)
    except asyncio.IncompleteReadError:
        return None

    first, second = header
    opcode = first & 0x0F
    masked = bool(second & 0x80)
    length = second & 0x7F

    if length == 126:
        length = struct.unpack("!H", await reader.readexactly(2))[0]
    elif length == 127:
        length = struct.unpack("!Q", await reader.readexactly(8))[0]

    mask = await reader.readexactly(4) if masked else b""
    payload = await reader.readexactly(length) if length else b""

    if masked:
        payload = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))

    return Frame(opcode, payload)


async def send_frame(
    writer: asyncio.StreamWriter,
    send_lock: asyncio.Lock,
    opcode: int,
    payload: bytes = b"",
) -> None:
    if len(payload) < 126:
        header = struct.pack("!BB", 0x80 | opcode, len(payload))
    elif len(payload) <= 0xFFFF:
        header = struct.pack("!BBH", 0x80 | opcode, 126, len(payload))
    else:
        header = struct.pack("!BBQ", 0x80 | opcode, 127, len(payload))

    async with send_lock:
        writer.write(header + payload)
        await writer.drain()


async def send_close(
    writer: asyncio.StreamWriter,
    send_lock: asyncio.Lock,
    code: int = 1000,
    reason: bytes = b"",
) -> None:
    payload = struct.pack("!H", code) + reason[:120]
    with contextlib.suppress(Exception):
        await send_frame(writer, send_lock, OP_CLOSE, payload)


async def pump_pty_to_ws(
    master_fd: int,
    writer: asyncio.StreamWriter,
    send_lock: asyncio.Lock,
) -> None:
    loop = asyncio.get_running_loop()
    while True:
        readable, _, _ = await loop.run_in_executor(
            None, select.select, [master_fd], [], [], 0.25
        )
        if not readable:
            continue

        try:
            data = os.read(master_fd, 8192)
        except OSError as exc:
            if exc.errno in (errno.EIO, errno.EBADF):
                break
            raise

        if not data:
            break

        await send_frame(writer, send_lock, OP_BINARY, data)


def write_all(fd: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        written = os.write(fd, view)
        view = view[written:]


async def pump_ws_to_pty(
    reader: asyncio.StreamReader,
    writer: asyncio.StreamWriter,
    send_lock: asyncio.Lock,
    master_fd: int,
) -> None:
    while True:
        frame = await read_frame(reader)
        if frame is None:
            break

        if frame.opcode == OP_CLOSE:
            break
        if frame.opcode == OP_PING:
            await send_frame(writer, send_lock, OP_PONG, frame.payload)
            continue
        if frame.opcode == OP_BINARY or frame.opcode == OP_CONTINUATION:
            if frame.payload:
                await asyncio.to_thread(write_all, master_fd, frame.payload)
            continue
        if frame.opcode == OP_TEXT:
            try:
                resize = json.loads(frame.payload.decode("utf-8"))
                cols = int(resize.get("width"))
                rows = int(resize.get("height"))
                set_pty_size(master_fd, rows, cols)
            except Exception as exc:
                print(f"ignoring bad resize message: {exc}", file=sys.stderr)


async def handle_connection(
    reader: asyncio.StreamReader,
    writer: asyncio.StreamWriter,
    shell: str,
) -> None:
    peer = writer.get_extra_info("peername")
    request = await accept_websocket(reader, writer)
    if request is None:
        return

    rows = parse_positive_int(request.query.get("num_rows"), 24)
    cols = parse_positive_int(request.query.get("num_cols"), 80)
    pid, master_fd = spawn_shell(shell, rows, cols)
    send_lock = asyncio.Lock()

    print(f"created local tty pid={pid} peer={peer} size={rows}x{cols} shell={shell}")

    pty_task = asyncio.create_task(pump_pty_to_ws(master_fd, writer, send_lock))
    ws_task = asyncio.create_task(pump_ws_to_pty(reader, writer, send_lock, master_fd))

    try:
        done, pending = await asyncio.wait(
            {pty_task, ws_task}, return_when=asyncio.FIRST_COMPLETED
        )
        for task in done:
            task.result()
        for task in pending:
            task.cancel()
    finally:
        await send_close(writer, send_lock)
        writer.close()
        with contextlib.suppress(Exception):
            await writer.wait_closed()
        with contextlib.suppress(OSError):
            os.close(master_fd)
        with contextlib.suppress(ProcessLookupError):
            os.kill(pid, signal.SIGHUP)
        with contextlib.suppress(ChildProcessError, OSError):
            os.waitpid(pid, os.WNOHANG)
        print(f"closed local tty pid={pid}")


async def amain() -> None:
    parser = argparse.ArgumentParser(
        description="Temporary local PTY WebSocket server for Warp wasm remote_tty."
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3030)
    parser.add_argument("--shell", default=None)
    args = parser.parse_args()

    if os.name != "posix":
        raise SystemExit("local-tty-server.py only supports POSIX systems")

    shell = choose_shell(args.shell)
    server = await asyncio.start_server(
        lambda r, w: handle_connection(r, w, shell),
        host=args.host,
        port=args.port,
    )

    addrs = ", ".join(str(sock.getsockname()) for sock in server.sockets or [])
    print(f"Serving local PTY websocket on {addrs}")
    print("Warp wasm remote_tty expects ws://127.0.0.1:3030/create")

    async with server:
        await server.serve_forever()


def main() -> None:
    try:
        asyncio.run(amain())
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
