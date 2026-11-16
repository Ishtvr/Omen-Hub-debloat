"""Bounded native wheel benchmark of an owned --ui-test process only.

Normal optimized binaries work; no MCP or hardware controllers are required.
Slint's opt-in console metrics report rendered frames. WM_NULL roundtrip samples
measure event-thread responsiveness, not presentation latency. Never use on the
live GUI: this script enumerates only its own child and closes only that child.
"""
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

REPO = Path(__file__).resolve().parent.parent
user = c.WinDLL('user32', use_last_error=True)
user.SetProcessDpiAwarenessContext.argtypes = [w.HANDLE]
user.SetProcessDpiAwarenessContext(w.HANDLE(-4))
user.GetWindowThreadProcessId.argtypes = [w.HWND, c.POINTER(w.DWORD)]
user.GetWindowTextW.argtypes = [w.HWND, w.LPWSTR, c.c_int]
user.GetWindowRect.argtypes = [w.HWND, c.POINTER(w.RECT)]
user.GetClientRect.argtypes = [w.HWND, c.POINTER(w.RECT)]
user.ClientToScreen.argtypes = [w.HWND, c.POINTER(w.POINT)]
user.SetWindowPos.argtypes = [w.HWND, w.HWND, c.c_int, c.c_int, c.c_int, c.c_int, w.UINT]
user.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user.SendMessageTimeoutW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM, w.UINT, w.UINT, c.POINTER(c.c_size_t)]
kernel = c.WinDLL('kernel32', use_last_error=True)
kernel.GetProcessTimes.argtypes = [w.HANDLE, c.POINTER(w.FILETIME), c.POINTER(w.FILETIME), c.POINTER(w.FILETIME), c.POINTER(w.FILETIME)]


def cpu_ms(process):
    values = [w.FILETIME() for _ in range(4)]
    assert kernel.GetProcessTimes(w.HANDLE(int(process._handle)), *(c.byref(v) for v in values))
    return sum((v.dwHighDateTime << 32) | v.dwLowDateTime for v in values[2:]) / 10000


def run(args):
    output = REPO / '.local' / ('scroll-benchmark-' + time.strftime('%Y%m%d-%H%M%S') + '-' + args.backend)
    output.mkdir(parents=True)
    env = os.environ.copy()
    env.update(SLINT_BACKEND=args.backend, SLINT_SCALE_FACTOR='1.25', SLINT_DEBUG_PERFORMANCE='refresh_lazy,console')
    with (output / 'render.log').open('w', encoding='utf-8') as log:
        process = subprocess.Popen([str(args.exe.resolve()), '--lang', 'de', '--ui-test'], cwd=REPO, env=env,
                                   stdout=log, stderr=log, creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            handles = []

            @c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
            def enum(hwnd, _):
                pid = w.DWORD(); title = c.create_unicode_buffer(128)
                user.GetWindowThreadProcessId(hwnd, c.byref(pid)); user.GetWindowTextW(hwnd, title, 128)
                if pid.value == process.pid and title.value == 'Omen Hub++' and user.IsWindowVisible(hwnd): handles.append(hwnd)
                return True

            deadline = time.monotonic() + 15
            while not handles:
                if process.poll() is not None: raise RuntimeError('Fixture exited')
                user.EnumWindows(enum, 0)
                if time.monotonic() > deadline: raise TimeoutError('Fixture window missing')
                time.sleep(.05)
            hwnd = handles[0]
            outer = w.RECT(); client = w.RECT()
            user.GetWindowRect(hwnd, c.byref(outer)); user.GetClientRect(hwnd, c.byref(client))
            fw, fh = outer.right - outer.left - client.right, outer.bottom - outer.top - client.bottom
            runs = []
            for width, height in [(1280, 800), (1920, 1080)]:
                assert user.SetWindowPos(hwnd, None, 40, 40, width + fw, height + fh, 0x4 | 0x10)
                time.sleep(.4)
                point = w.POINT(width // 2, height // 2)
                user.ClientToScreen(hwnd, c.byref(point))
                packed = (point.y << 16) | (point.x & 0xffff)
                user.PostMessageW(hwnd, 0x0200, 0, ((height // 2) << 16) | (width // 2))
                started = time.perf_counter(); before = cpu_ms(process); samples = []; count = 0
                while time.perf_counter() - started < args.seconds:
                    now = time.perf_counter() - started
                    delta = -120 if int(now / .65) % 2 == 0 else 120
                    user.PostMessageW(hwnd, 0x020A, (delta & 0xffff) << 16, packed)
                    if count % 10 == 0:
                        ack = c.c_size_t(); t = time.perf_counter()
                        assert user.SendMessageTimeoutW(hwnd, 0, 0, 0, 2, 1000, c.byref(ack)), 'UI thread did not respond'
                        samples.append((time.perf_counter() - t) * 1000)
                    count += 1
                    time.sleep(max(0, (started + count / 60) - time.perf_counter()))
                elapsed = time.perf_counter() - started
                runs.append({'clientPixels': [width, height], 'wheelEvents': count, 'seconds': elapsed,
                             'cpuMs': cpu_ms(process) - before, 'eventThreadMedianMs': statistics.median(samples),
                             'eventThreadP95Ms': sorted(samples)[int((len(samples) - 1) * .95)], 'eventThreadMaxMs': max(samples)})
            result = {'backend': args.backend, 'binarySha256': hashlib.sha256(args.exe.read_bytes()).hexdigest(),
                      'runs': runs, 'scope': 'Owned optimized UI fixture; no sensors/controllers. WM_NULL is event-thread response, not presentation latency.'}
            log.flush()
            result['renderedFramesPerSecond'] = [int(v) for v in re.findall(r'average frames per second: (\d+)', (output / 'render.log').read_text(encoding='utf-8'))]
            (output / 'result.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
            print(str(output)); print(json.dumps(result, indent=2))
        finally:
            if process.poll() is None: process.terminate(); process.wait(timeout=10)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, default=REPO / 'target/release/omen-hub-pp.exe')
    parser.add_argument('--backend', choices=['winit-software', 'winit-femtovg'], default='winit-software')
    parser.add_argument('--seconds', type=float, default=5)
    run(parser.parse_args())
