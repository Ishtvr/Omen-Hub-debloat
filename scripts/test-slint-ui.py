"""Real Slint interaction regression test, Windows / Python standard library.

Build with SLINT_EMIT_DEBUG_INFO=1 and --features slint/mcp. This script starts
only --ui-test: no real controllers, profile/settings writes or startup tasks.
It stops only that subprocess. Artifacts stay in ignored .local/.
"""
import argparse
import base64
import ctypes as c
from ctypes import wintypes as wt
import json
import hashlib
import os
from pathlib import Path
import socket
import subprocess
import time

from slint_mcp import McpClient, unpack

REPO = Path(__file__).resolve().parent.parent


def run(args):
    output = REPO / '.local' / ('slint-ui-qa-' + time.strftime('%Y%m%d-%H%M%S') + '-' + args.language + '-' + args.backend)
    output.mkdir(parents=True, exist_ok=False)
    texts = json.loads((REPO / 'crates/app/lang' / (args.language + '.json')).read_text(encoding='utf-8'))
    with socket.socket() as port:
        port.bind(('127.0.0.1', 9315))
    environment = os.environ.copy()
    environment.update(SLINT_MCP_PORT='9315', SLINT_BACKEND=args.backend)
    if args.scale:
        environment['SLINT_SCALE_FACTOR'] = args.scale
    with (output / 'stdout.log').open('w', encoding='utf-8') as stdout, (output / 'stderr.log').open('w', encoding='utf-8') as stderr:
        process = subprocess.Popen([str(args.exe.resolve()), '--lang', args.language, '--ui-test'],
                                   env=environment, cwd=REPO, stdout=stdout, stderr=stderr,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            deadline = time.monotonic() + 20
            while True:
                if process.poll() is not None:
                    raise RuntimeError('The isolated app exited; inspect stderr.log')
                try:
                    client = McpClient('http://127.0.0.1:9315/mcp')
                    client.initialize()
                    break
                except OSError:
                    if time.monotonic() > deadline:
                        raise
                    time.sleep(.1)
            window = unpack(client.tool('list_windows', {}))['windowHandles'][0]
            properties = unpack(client.tool('get_window_properties', {'windowHandle': window}))
            root = properties['rootElementHandle']

            def tree():
                return unpack(client.tool('get_element_tree', {'elementHandle': root, 'maxElements': 1000}))['elements']

            def by_id(name):
                return next(e for e in tree() if any(i.get('id') == name for i in e['typeNamesAndIds']))

            def find(label, role):
                return next(e for e in tree() if e.get('accessibleLabel') == label and e.get('accessibleRole') == role)

            def wait_for(test):
                deadline = time.monotonic() + 3
                while True:
                    try:
                        value = test()
                        if value:
                            return value
                    except (StopIteration, KeyError):
                        pass
                    if time.monotonic() > deadline:
                        raise AssertionError('The UI did not reach its expected state')
                    time.sleep(.03)

            def click(element):
                client.tool('click_element', {'elementHandle': element['handle']})
                time.sleep(.06)

            def label_click(label):
                click(find(label, 'Button'))

            def key(text, event='PressAndRelease'):
                client.tool('dispatch_key_event', {'windowHandle': window, 'text': text, 'eventType': event})

            def set_value(element, value):
                client.tool('set_element_value', {'elementHandle': element['handle'], 'value': value})

            def shot(name):
                content = client.tool('take_screenshot', {'windowHandle': window})['content']
                (output / (name + '.png')).write_bytes(base64.b64decode(next(p for p in content if p['type'] == 'image')['data']))

            def scroll(delta):
                view = by_id('AppWindow::preset-list')
                client.tool('dispatch_pointer_scroll', {'windowHandle': window, 'deltaY': delta,
                    'position': {'x': view['absolutePosition'].get('x', 0) + view['size']['width'] / 2,
                                 'y': view['absolutePosition'].get('y', 0) + view['size']['height'] / 2}})

            def navigate(key_name, id_name):
                label_click(texts[key_name])
                wait_for(lambda: by_id('AppWindow::' + id_name))

            def scroll_to(label, role='Button'):
                scroll(10000)
                for _ in range(16):
                    view = by_id('AppWindow::preset-list')
                    top = view['absolutePosition'].get('y', 0)
                    bottom = top + view['size']['height']
                    for e in tree():
                        y = e['absolutePosition'].get('y', 0)
                        if e.get('accessibleLabel') == label and e.get('accessibleRole') == role and top <= y and y + e['size']['height'] <= bottom:
                            return e
                    scroll(-100)
                    time.sleep(.02)
                raise AssertionError('Element not reachable by scrolling: ' + label)

            # Use only the native window belonging to our --ui-test subprocess.
            user = c.WinDLL('user32', use_last_error=True)
            user.GetWindowThreadProcessId.argtypes = [wt.HWND, c.POINTER(wt.DWORD)]
            user.GetWindowTextW.argtypes = [wt.HWND, wt.LPWSTR, c.c_int]
            user.IsWindowVisible.argtypes = [wt.HWND]
            user.SetWindowPos.argtypes = [wt.HWND, wt.HWND, c.c_int, c.c_int, c.c_int, c.c_int, wt.UINT]
            user.ShowWindow.argtypes = [wt.HWND, c.c_int]
            handles = []

            @c.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
            def enum(hwnd, _):
                pid = wt.DWORD()
                title = c.create_unicode_buffer(256)
                user.GetWindowThreadProcessId(hwnd, c.byref(pid))
                user.GetWindowTextW(hwnd, title, 256)
                if pid.value == process.pid and user.IsWindowVisible(hwnd) and title.value == 'Omen Hub++':
                    handles.append(hwnd)
                return True

            user.EnumWindows(enum, 0)
            assert len(handles) == 1, handles
            hwnd = handles[0]
            wait_for(lambda: by_id('AppWindow::preset-name'))
            navigate('ui.overview', 'overview')
            wait_for(lambda: any(e.get('accessibleLabel') == 'GPU · ' + texts['ui.test_data'] for e in tree()))
            navigate('ui.presets', 'presets')
            name = 'Scroll-Test Größe Ω'
            set_value(by_id('AppWindow::preset-name'), name)
            click(by_id('AppWindow::preset-name'))
            key('x')
            wait_for(lambda: by_id('AppWindow::preset-name').get('accessibleValue') == name + 'x' and by_id('AppWindow::preset-name').get('accessibleEnabled'))
            key('\x11', 'Press'); key('z'); key('\x11', 'Release')
            wait_for(lambda: by_id('AppWindow::preset-name').get('accessibleValue') == name)
            key('\x11', 'Press'); key('y'); key('\x11', 'Release')
            wait_for(lambda: by_id('AppWindow::preset-name').get('accessibleValue') == name + 'x')
            click(by_id('AppWindow::new-preset'))
            wait_for(lambda: find(texts['ui.cancel'], 'Button'))
            key('\x1b')
            wait_for(lambda: by_id('AppWindow::preset-name').get('accessibleValue') == name + 'x' and by_id('AppWindow::preset-name').get('accessibleEnabled'))
            for i in range(100):
                scroll(-650 if i % 2 == 0 else 650)
            scroll(10000)
            shot('presets-after-scroll')
            user.GetWindowRect.argtypes = [wt.HWND, c.POINTER(wt.RECT)]
            user.GetClientRect.argtypes = [wt.HWND, c.POINTER(wt.RECT)]
            outer = wt.RECT(); inner = wt.RECT()
            user.GetWindowRect(hwnd, c.byref(outer)); user.GetClientRect(hwnd, c.byref(inner))
            frame_w = outer.right - outer.left - inner.right
            frame_h = outer.bottom - outer.top - inner.bottom
            scale = properties['scaleFactor']
            for width, height in [(760, 540), (1140, 740), (720, 500), (960, 660), (760, 540)] * 3:
                assert user.SetWindowPos(hwnd, None, 0, 0, round(width * scale) + frame_w, round(height * scale) + frame_h, 0x2 | 0x4 | 0x10)
                assert by_id('AppWindow::preset-name')['accessibleValue'] == name + 'x'
            for _ in range(3):
                user.ShowWindow(hwnd, 6); time.sleep(.1); user.ShowWindow(hwnd, 9); time.sleep(.1)
            assert by_id('AppWindow::preset-name')['accessibleValue'] == name + 'x'
            user.GetClassNameW.argtypes = [wt.HWND, wt.LPWSTR, c.c_int]
            user.RegisterWindowMessageW.argtypes = [wt.LPCWSTR]
            user.RegisterWindowMessageW.restype = wt.UINT
            user.PostMessageW.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]
            owners = []

            @c.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
            def owner_enum(native, _):
                pid = wt.DWORD(); cls = c.create_unicode_buffer(128)
                user.GetWindowThreadProcessId(native, c.byref(pid)); user.GetClassNameW(native, cls, 128)
                if pid.value == process.pid and cls.value == 'OmenHubSlintTestEvents':
                    owners.append(native)
                return True

            user.EnumWindows(owner_enum, 0)
            assert len(owners) == 1
            activate = user.RegisterWindowMessageW('OmenHubPP.Activate')
            for _ in range(3):
                user.PostMessageW(hwnd, 0x0010, 0, 0)  # WM_CLOSE, respects stored tray behavior
                wait_for(lambda: not user.IsWindowVisible(hwnd))
                assert process.poll() is None
                user.PostMessageW(owners[0], activate, 0, 0)
                wait_for(lambda: unpack(client.tool('list_windows', {}))['windowHandles'])
                window = unpack(client.tool('list_windows', {}))['windowHandles'][0]
                root = unpack(client.tool('get_window_properties', {'windowHandle': window}))['rootElementHandle']
                wait_for(lambda: by_id('AppWindow::preset-name')['accessibleValue'] == name + 'x')
                handles.clear(); user.EnumWindows(enum, 0)
                assert len(handles) == 1
                hwnd = handles[0]
            for key_name, id_name in [('ui.overview', 'overview'), ('ui.cooling', 'cooling'), ('ui.lighting', 'lighting'), ('settings.title', 'settings'), ('ui.presets', 'presets')]:
                navigate(key_name, id_name)
                shot(id_name)
            assert by_id('AppWindow::preset-name')['accessibleValue'] == name + 'x'

            # Expand/collapse and exact decimal-unit input survive virtualization.
            click(scroll_to(texts['group.gpu']))
            slow = texts['field.CpuSlowPptLimitMw']
            click(scroll_to(slow, 'Checkbox'))
            wait_for(lambda: find(slow, 'Checkbox').get('accessibleChecked'))
            set_value(find(slow, 'TextInput'), '55,125' if args.language == 'de' else '55.125')
            click(find(slow, 'TextInput'))
            key('0')
            # Collapsing the bounded form must preserve each edit widget's Undo.
            click(scroll_to(texts['group.limits']))
            click(scroll_to(texts['group.limits']))
            click(scroll_to(slow, 'TextInput'))
            key('\x11', 'Press'); key('z'); key('\x11', 'Release')
            wait_for(lambda: find(slow, 'TextInput')['accessibleValue'] == ('55,125' if args.language == 'de' else '55.125'))
            click(scroll_to(texts['group.processor']))
            boost = texts['field.WindowsBoostMode']
            click(scroll_to(boost, 'Checkbox'))
            combo = find(boost, 'Combobox')
            client.tool('invoke_accessibility_action', {'elementHandle': combo['handle'], 'action': 'Expand'})
            time.sleep(.1)
            shot('windows-boost-dropdown')
            for _ in range(6):
                key('\uf701')
            key('\n')
            wait_for(lambda: find(boost, 'Combobox').get('accessibleValue') == texts['choice.EfficientAggressiveAtGuaranteed'])
            scroll(10000)
            click(by_id('AppWindow::save-preset'))
            wait_for(lambda: any(e.get('accessibleLabel', '').startswith(texts['ui.test_saved']) for e in tree()))
            expected = '55,125' if args.language == 'de' else '55.125'
            assert find(slow, 'TextInput')['accessibleValue'] == expected
            # A real preset switch must refresh reused editors, not just its name.
            click(by_id('AppWindow::new-preset'))
            wait_for(lambda: by_id('AppWindow::preset-name').get('accessibleValue', '') == '')
            assert find(slow, 'TextInput')['accessibleValue'] == '55'
            assert not find(slow, 'Checkbox').get('accessibleChecked', False)
            click(by_id('AppWindow::preset-selector'))
            key('\uf701')
            wait_for(lambda: by_id('AppWindow::preset-name')['accessibleValue'] == name + 'x')
            key('\x1b')
            assert find(slow, 'TextInput')['accessibleValue'] == expected
            assert find(slow, 'Checkbox').get('accessibleChecked')

            # Curve history and exact anchors use the same core editor as production.
            click(scroll_to(texts['group.gpu']))
            label_click(texts['ui.edit_curve'])
            wait_for(lambda: by_id('AppWindow::curve-editor'))
            original = by_id('AppWindow::point-y')['accessibleValue']
            set_value(by_id('AppWindow::point-y'), str(int(original) + 15))
            label_click(texts['ui.set_point'])
            wait_for(lambda: by_id('AppWindow::point-y')['accessibleValue'] == str(int(original) + 15))
            label_click(texts['ui.undo'])
            wait_for(lambda: by_id('AppWindow::point-y')['accessibleValue'] == original)
            label_click(texts['ui.redo'])
            wait_for(lambda: by_id('AppWindow::point-y')['accessibleValue'] == str(int(original) + 15))
            def anchors():
                return [e['absolutePosition'].get('x', 0) for e in tree() if any(p.get('id') == 'CurveCanvas::draft-node' for p in e['typeNamesAndIds'])]
            for modifiers in [{}, {'shift': True}, {'alt': True}, {'control': True}]:
                canvas = by_id('CurveCanvas::canvas-input')
                before = anchors()
                # Introspection omits nodes outside the clipped viewport.
                assert len(before) >= 2
                target = {'x': canvas['absolutePosition']['x'] + canvas['size']['width'] / 2,
                          'y': canvas['absolutePosition']['y'] + canvas['size']['height'] / 2 - 18}
                client.tool('drag_element', {'elementHandle': canvas['handle'], 'target': target, 'modifiers': modifiers})
                time.sleep(.08)
                # Frequency shifts may clip additional nodes vertically. Their
                # x coordinates must still match this fixture's immutable anchors.
                expected_x = [canvas['absolutePosition']['x'] + ((450 + 10 * i - 630) / 512) * canvas['size']['width'] - 3 for i in range(80)]
                assert all(any(abs(x - expected) < .002 for expected in expected_x) for x in anchors()), 'GPU voltage anchors changed during a frequency gesture'
                edited = by_id('AppWindow::point-y')['accessibleValue']
                key('\x11', 'Press'); key('z'); key('\x11', 'Release')
                time.sleep(.08)
                key('\x11', 'Press'); key('y'); key('\x11', 'Release')
                wait_for(lambda: by_id('AppWindow::point-y')['accessibleValue'] == edited)
            final_y = by_id('AppWindow::point-y')['accessibleValue']
            shot('gpu-curve')
            label_click(texts['ui.back'])
            label_click(texts['ui.edit_curve'])
            wait_for(lambda: by_id('AppWindow::point-y')['accessibleValue'] == final_y)
            label_click(texts['ui.use_curve'])
            click(by_id('AppWindow::save-preset'))
            wait_for(lambda: any(e.get('accessibleLabel', '').startswith(texts['ui.test_saved']) for e in tree()))
            navigate('settings.title', 'settings')
            shot('settings-final')
            def settings_scroll(delta):
                view = by_id('AppWindow::settings')
                client.tool('dispatch_pointer_scroll', {'windowHandle': window, 'deltaY': delta,
                    'position': {'x': view['absolutePosition'].get('x', 0) + 5,
                                 'y': view['absolutePosition'].get('y', 0) + view['size']['height'] / 2}})
                time.sleep(.08)
            settings_scroll(-10000)
            label_click(texts['ui.about'])
            def about_elements():
                for other in unpack(client.tool('list_windows', {}))['windowHandles']:
                    if other == window:
                        continue
                    other_root = unpack(client.tool('get_window_properties', {'windowHandle': other}))['rootElementHandle']
                    elements = unpack(client.tool('get_element_tree', {'elementHandle': other_root, 'maxElements': 200}))['elements']
                    if any(i.get('id') == 'AboutWindow::root' for e in elements for i in e['typeNamesAndIds']):
                        if any(e.get('accessibleRole') == 'Button' and e.get('accessibleLabel') == texts['settings.close'] for e in elements):
                            return elements
                return None
            about_tree = wait_for(about_elements)
            close = next(e for e in about_tree if e.get('accessibleRole') == 'Button' and e.get('accessibleLabel') == texts['settings.close'])
            assert any('0.10.0-beta.1' in e.get('accessibleLabel', '') for e in about_tree)
            client.tool('click_element', {'elementHandle': close['handle']})
            time.sleep(.1)
            settings_scroll(10000)
            click(by_id('AppWindow::tray-start'))
            settings_scroll(-10000)
            wait_for(lambda: by_id('AppWindow::settings-save').get('accessibleEnabled'))
            click(by_id('AppWindow::settings-save'))
            wait_for(lambda: not by_id('AppWindow::settings-save').get('accessibleEnabled', False))
            navigate('ui.presets', 'presets')
            assert not by_id('AppWindow::startup').get('accessibleChecked', False), 'App startup must not grant preset startup'
            (output / 'result.json').write_text(json.dumps({'passed': True, 'language': args.language, 'backend': args.backend,
                'binarySha256': hashlib.sha256(args.exe.read_bytes()).hexdigest(),
                'window': properties, 'wheelEvents': 100, 'resizes': 15, 'minimizeRestoreCycles': 3, 'trayRestoreCycles': 3,
                'nameAndUndo': True, 'actualTabTransitions': True, 'exactMilliwatts': 55125,
                'curveNumericUndoRedoAndReopen': True, 'curveMouseAndModifierGestures': 4,
                'aboutLanguageAndVersion': True, 'settingsDraftSaveAndSeparateStartup': True,
                'scope': 'Isolated UI fixture, no hardware qualification'}, indent=2), encoding='utf-8')
            print(str(output))
        except BaseException:
            if 'shot' in locals():
                try:
                    shot('failure')
                    (output / 'failure-tree.json').write_text(json.dumps(tree(), indent=2, ensure_ascii=False), encoding='utf-8')
                except Exception:
                    pass
            print('QA artifacts: ' + str(output))
            raise
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, default=REPO / 'target/debug/omen-hub-pp.exe')
    parser.add_argument('--language', choices=['de', 'en'], default='de')
    parser.add_argument('--backend', choices=['winit-software', 'winit-femtovg'], default='winit-software')
    parser.add_argument('--scale')
    run(parser.parse_args())
