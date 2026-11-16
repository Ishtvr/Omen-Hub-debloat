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
import tempfile
import time
import tomllib

from slint_mcp import McpClient, unpack
from PIL import Image  # Developer-only screenshot assertions; never a runtime dependency.

REPO = Path(__file__).resolve().parent.parent


def run(args):
    # Prevent Windows from virtualizing this harness's resize coordinates.
    # Slint's scale override and the desktop's DPI are independent quantities.
    dpi_user = c.WinDLL('user32', use_last_error=True)
    dpi_user.SetProcessDpiAwarenessContext.argtypes = [wt.HANDLE]
    dpi_user.SetProcessDpiAwarenessContext.restype = wt.BOOL
    assert dpi_user.SetProcessDpiAwarenessContext(wt.HANDLE(-4)), c.get_last_error()
    output = REPO / '.local' / ('slint-ui-qa-' + time.strftime('%Y%m%d-%H%M%S') + '-' + args.language + '-' + args.backend)
    output.mkdir(parents=True, exist_ok=False)
    texts = json.loads((REPO / 'crates/app/lang' / (args.language + '.json')).read_text(encoding='utf-8'))
    with socket.socket() as port:
        port.bind(('127.0.0.1', 9315))
    environment = os.environ.copy()
    environment.update(SLINT_MCP_PORT='9315', SLINT_BACKEND=args.backend)
    fixture_dir = Path(tempfile.mkdtemp(prefix='omen-ui-settings-'))
    (fixture_dir / 'ui-test-only.marker').write_text('Omen Hub++ isolated UI settings\n', encoding='utf-8')
    environment['OMEN_UI_TEST_DIR'] = str(fixture_dir)
    expected_version = tomllib.loads((REPO / 'Cargo.toml').read_text(encoding='utf-8'))['workspace']['package']['version']
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
                try:
                    element=find(label,'Button')
                    try:
                        view=by_id('AppWindow::curve-editor');y=element['absolutePosition'].get('y',0);top=view['absolutePosition'].get('y',0)
                        if y<top or y+element['size']['height']>top+view['size']['height']: element=curve_to(label)
                    except StopIteration: pass
                except StopIteration:
                    element=curve_to(label)
                click(element)

            def key(text, event='PressAndRelease'):
                client.tool('dispatch_key_event', {'windowHandle': window, 'text': text, 'eventType': event})

            def set_value(element, value):
                ids = [p.get('id') for p in element['typeNamesAndIds']]
                if any(p in ids for p in ['AppWindow::color-background','AppWindow::color-foreground','AppWindow::color-accent','AppWindow::lighting-color']):
                    click(element)
                    box = picker_hex()
                    client.tool('set_element_value', {'elementHandle': box['handle'], 'value': value})
                    wait_for(lambda: find(texts['color.use'], 'Button').get('accessibleEnabled'))
                    label_click(texts['color.use'])
                    return
                client.tool('set_element_value', {'elementHandle': element['handle'], 'value': value})

            def picker_hex():
                wait_for(lambda: by_id('ColorPicker::panel'))
                for _ in range(12):
                    try:
                        box = by_id('ColorPicker::hex-field')
                        view = by_id('ColorPicker::body')
                        y=box['absolutePosition'].get('y',0); top=view['absolutePosition'].get('y',0)
                        if top <= y and y+box['size']['height'] <= top+view['size']['height']: return box
                    except StopIteration: pass
                    view=by_id('ColorPicker::body')
                    client.tool('dispatch_pointer_scroll',{'windowHandle':window,'deltaY':-80,
                        'position':{'x':view['absolutePosition'].get('x',0)+5,'y':view['absolutePosition'].get('y',0)+view['size']['height']/2}})
                    time.sleep(.03)
                raise AssertionError('Picker HEX field not reachable')

            def shot(name):
                content = client.tool('take_screenshot', {'windowHandle': window})['content']
                (output / (name + '.png')).write_bytes(base64.b64decode(next(p for p in content if p['type'] == 'image')['data']))

            def assert_curve_pixels(name):
                """Catch unsupported rotation: diagonals must actually be painted."""
                shot(name)
                picture = Image.open(output / (name + '.png')).convert('RGB')
                nodes = [e for e in tree() if any(p.get('id') == 'CurveCanvas::draft-node' for p in e['typeNamesAndIds'])]
                scale = properties['scaleFactor']
                checked = 0
                for a, b in zip(nodes, nodes[1:]):
                    ax = a['absolutePosition'].get('x', 0) + a['size']['width'] / 2
                    ay = a['absolutePosition'].get('y', 0) + a['size']['height'] / 2
                    bx = b['absolutePosition'].get('x', 0) + b['size']['width'] / 2
                    by = b['absolutePosition'].get('y', 0) + b['size']['height'] / 2
                    if abs(by - ay) < 5 or abs(bx - ax) < 5:
                        continue
                    x, y = round((ax + bx) / 2 * scale), round((ay + by) / 2 * scale)
                    if not (2 <= x < picture.width - 2 and 2 <= y < picture.height - 2):
                        continue
                    # Default Slate accent: the expected connected line must
                    # cross a midpoint far away from either point marker.
                    pixels = [picture.getpixel((x + dx, y + dy)) for dx in range(-2, 3) for dy in range(-2, 3)]
                    assert any(abs(r - 124) + abs(g - 135) + abs(b - 248) < 75 for r, g, b in pixels), (name, x, y, pixels)
                    checked += 1
                assert checked >= 2, (name, 'Too few visible diagonal segments')

            def scroll(delta):
                view = by_id('AppWindow::preset-list')
                client.tool('dispatch_pointer_scroll', {'windowHandle': window, 'deltaY': delta,
                    'position': {'x': view['absolutePosition'].get('x', 0) + view['size']['width'] / 2,
                                 'y': view['absolutePosition'].get('y', 0) + view['size']['height'] / 2}})

            def navigate(key_name, id_name):
                label_click(texts[key_name])
                wait_for(lambda: by_id('AppWindow::' + id_name))

            def scroll_to(label, role='Button'):
                view = by_id('AppWindow::preset-list')
                top = view['absolutePosition'].get('y', 0)
                bottom = top + view['size']['height']
                for e in tree():
                    y = e['absolutePosition'].get('y', 0)
                    if e.get('accessibleLabel') == label and e.get('accessibleRole') == role and top <= y and y + e['size']['height'] <= bottom:
                        return e
                scroll(10000)
                for _ in range(48):
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

            def name_field():
                return scroll_to(texts['ui.preset_name'], 'TextInput')

            def curve_to(label,role='Button'):
                view=by_id('AppWindow::curve-editor')
                def wheel(delta):
                    client.tool('dispatch_pointer_scroll',{'windowHandle':window,'deltaY':delta,'position':{
                        'x':view['absolutePosition'].get('x',0)+5,'y':view['absolutePosition'].get('y',0)+view['size']['height']/2}})
                for direction in [0,10000]:
                    if direction: wheel(direction)
                    for _ in range(20):
                        for e in tree():
                            y=e['absolutePosition'].get('y',0);top=view['absolutePosition'].get('y',0)
                            if e.get('accessibleLabel')==label and e.get('accessibleRole')==role and top<=y and y+e['size']['height']<=top+view['size']['height']: return e
                        wheel(-80)
                raise AssertionError('Curve control not reachable: '+label)

            def lighting_to(name):
                def wheel(delta):
                    view = by_id('AppWindow::lighting-scroll')
                    client.tool('dispatch_pointer_scroll', {'windowHandle': window, 'deltaY': delta,
                        'position': {'x': view['absolutePosition'].get('x', 0) + 5,
                                     'y': view['absolutePosition'].get('y', 0) + view['size']['height'] / 2}})
                wheel(10000)
                for _ in range(25):
                    try:
                        e = by_id('AppWindow::' + name); view = by_id('AppWindow::lighting-scroll')
                        y = e['absolutePosition'].get('y', 0); top = view['absolutePosition'].get('y', 0)
                        if top <= y and y + e['size']['height'] <= top + view['size']['height']: return e
                    except StopIteration: pass
                    wheel(-80)
                raise AssertionError('Lighting control not reachable: ' + name)

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
            wait_for(lambda: name_field())
            assert abs(name_field()['size']['width'] - 276) < .1
            assert abs(by_id('AppWindow::preset-selector')['size']['width'] - 276) < .1
            navigate('ui.overview', 'overview')
            scroll(10000)
            wait_for(lambda: any(e.get('accessibleLabel') == 'GPU · ' + texts['ui.test_data'] for e in tree()))
            tabs = [e for e in tree() if any(i.get('id') == 'SegmentedTabs::tab' for i in e['typeNamesAndIds'])]
            assert len(tabs) == 3
            assert max(e['size']['width'] for e in tabs) - min(e['size']['width'] for e in tabs) < .01
            navigate('ui.overview', 'overview')
            name = 'Scroll-Test Größe Ω'
            set_value(name_field(), name)
            click(name_field())
            key('x')
            wait_for(lambda: name_field().get('accessibleValue') == name + 'x' and name_field().get('accessibleEnabled'))
            key('\x11', 'Press'); key('z'); key('\x11', 'Release')
            wait_for(lambda: name_field().get('accessibleValue') == name)
            key('\x11', 'Press'); key('y'); key('\x11', 'Release')
            wait_for(lambda: name_field().get('accessibleValue') == name + 'x')
            click(scroll_to(texts['preset.new']))
            wait_for(lambda: find(texts['ui.cancel'], 'Button'))
            key('\x1b')
            wait_for(lambda: name_field().get('accessibleValue') == name + 'x' and name_field().get('accessibleEnabled'))
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
            def matches_size(width, height):
                actual = unpack(client.tool('get_window_properties', {'windowHandle': window}))['size']
                return abs(actual['width'] - round(width * scale)) <= 2 and abs(actual['height'] - round(height * scale)) <= 2
            for width, height in [(760, 540), (1140, 740), (720, 500), (960, 660), (760, 540)] * 3:
                assert user.SetWindowPos(hwnd, None, 0, 0, round(width * scale) + frame_w, round(height * scale) + frame_h, 0x2 | 0x4 | 0x10)
                wait_for(lambda: matches_size(width, height))
                assert name_field()['accessibleValue'] == name + 'x'
                if width == 720:
                    shot('compact-overview')
            for _ in range(3):
                user.ShowWindow(hwnd, 6); time.sleep(.1); user.ShowWindow(hwnd, 9); time.sleep(.1)
            assert name_field()['accessibleValue'] == name + 'x'
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
                wait_for(lambda: name_field()['accessibleValue'] == name + 'x')
                handles.clear(); user.EnumWindows(enum, 0)
                assert len(handles) == 1
                hwnd = handles[0]
            for key_name, id_name in [('ui.overview', 'overview'), ('ui.cooling', 'cooling'), ('ui.lighting', 'lighting'), ('settings.title', 'settings'), ('ui.overview', 'overview')]:
                navigate(key_name, id_name)
                shot(id_name)
            assert name_field()['accessibleValue'] == name + 'x'

            navigate('ui.lighting', 'lighting')
            assert lighting_to('lighting-color')['accessibleValue'] == '#6EAF96'  # actual WASD slot 3
            set_value(lighting_to('lighting-color'), '#FF0000')
            set_value(lighting_to('lighting-brightness'), '50')
            assert by_id('AppWindow::lighting-color')['accessibleValue'] == '#800000'
            assert float(by_id('AppWindow::lighting-slider')['accessibleValue']) == 50
            set_value(lighting_to('lighting-brightness'), '101')
            assert not by_id('AppWindow::lighting-apply').get('accessibleEnabled', False)
            set_value(lighting_to('lighting-brightness'), '50')
            label_click(texts['lighting.rest'])
            assert lighting_to('lighting-color').get('accessibleValue', '') == ''  # differing original left/middle
            set_value(lighting_to('lighting-color'), '#0000FF')
            label_click(texts['lighting.numpad_right'])
            assert lighting_to('lighting-color')['accessibleValue'] == '#73A0C8'  # right slot 0, not WASD
            click(by_id('AppWindow::lighting-reset'))
            assert not by_id('AppWindow::lighting-apply').get('accessibleEnabled', False)
            label_click('WASD')
            assert lighting_to('lighting-color')['accessibleValue'] == '#6EAF96'
            click(lighting_to('lighting-slider')); key('\uf729')
            assert by_id('AppWindow::lighting-brightness')['accessibleValue'] == '0'
            assert by_id('AppWindow::lighting-color')['accessibleValue'] == '#000000'
            key('\uf72b')
            assert by_id('AppWindow::lighting-brightness')['accessibleValue'] == '100'
            click(by_id('AppWindow::lighting-reset'))
            set_value(lighting_to('lighting-color'), '#FF0000')
            set_value(lighting_to('lighting-brightness'), '50')
            click(by_id('AppWindow::lighting-apply'))  # fixture accepts only in memory
            assert float(by_id('AppWindow::lighting-slider')['accessibleValue']) == 50
            assert not by_id('AppWindow::lighting-apply').get('accessibleEnabled', False)
            shot('lighting-controls-compact')
            assert user.SetWindowPos(hwnd, None, 0, 0, round(1000 * scale) + frame_w, round(720 * scale) + frame_h, 0x2 | 0x4 | 0x10)
            time.sleep(.2)
            lighting_to('keyboard-preview')
            shot('lighting-keyboard')
            assert user.SetWindowPos(hwnd, None, 0, 0, round(760 * scale) + frame_w, round(540 * scale) + frame_h, 0x2 | 0x4 | 0x10)
            navigate('ui.overview', 'overview')

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
            assert scroll_to(slow, 'TextInput')['accessibleValue'] == expected
            # A real preset switch must refresh reused editors, not just its name.
            name_field()
            click(scroll_to(texts['preset.new']))
            wait_for(lambda: name_field().get('accessibleValue', '') == '')
            assert scroll_to(slow, 'TextInput')['accessibleValue'] == '55'
            assert not find(slow, 'Checkbox').get('accessibleChecked', False)
            name_field()
            click(scroll_to(texts['preset.selector_label'],'Combobox'))
            key('\uf701')
            key('\n')
            wait_for(lambda: name_field()['accessibleValue'] == name + 'x')
            key('\x1b')
            assert scroll_to(slow, 'TextInput')['accessibleValue'] == expected
            assert find(slow, 'Checkbox').get('accessibleChecked')

            # Curve history and exact anchors use the same core editor as production.
            click(scroll_to(texts['group.gpu']))
            click(scroll_to(texts['ui.edit_curve']))
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
                return [e['absolutePosition'].get('x', 0) + e['size']['width'] / 2 for e in tree() if any(p.get('id') == 'CurveCanvas::draft-node' for p in e['typeNamesAndIds'])]
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
                expected_x = [canvas['absolutePosition']['x'] + ((450 + 10 * i - 630) / 512) * canvas['size']['width'] for i in range(80)]
                assert all(any(abs(x - expected) < .002 for expected in expected_x) for x in anchors()), 'GPU voltage anchors changed during a frequency gesture'
                edited = by_id('AppWindow::point-y')['accessibleValue']
                key('\x11', 'Press'); key('z'); key('\x11', 'Release')
                time.sleep(.08)
                key('\x11', 'Press'); key('y'); key('\x11', 'Release')
                wait_for(lambda: by_id('AppWindow::point-y')['accessibleValue'] == edited)
            final_y = by_id('AppWindow::point-y')['accessibleValue']
            shot('gpu-curve')
            assert user.SetWindowPos(hwnd,None,0,0,round(720*scale)+frame_w,round(500*scale)+frame_h,0x2|0x4|0x10)
            curve_to(texts['ui.use_curve'])
            chart=by_id('AppWindow::chart')
            assert chart['absolutePosition'].get('x',0)+chart['size']['width']<=720+.5
            primary=find(texts['ui.use_curve'],'Button')
            assert primary['absolutePosition'].get('x',0)+primary['size']['width']<=720+.5
            shot('gpu-curve-minimum')
            assert user.SetWindowPos(hwnd,None,0,0,round(760*scale)+frame_w,round(540*scale)+frame_h,0x2|0x4|0x10)
            label_click(texts['ui.back'])
            click(scroll_to(texts['ui.edit_curve']))
            wait_for(lambda: by_id('AppWindow::point-y')['accessibleValue'] == final_y)
            label_click(texts['ui.use_curve'])
            click(by_id('AppWindow::save-preset'))
            wait_for(lambda: any(e.get('accessibleLabel', '').startswith(texts['ui.test_saved']) for e in tree()))
            navigate('ui.cooling', 'cooling')
            click(by_id('AppWindow::edit-fan'))
            wait_for(lambda: by_id('AppWindow::curve-editor'))
            wait_for(lambda: any(e.get('accessibleLabel') == texts['ui.ready'] for e in tree()))
            assert_curve_pixels('fan-curve-connected')
            assert user.SetWindowPos(hwnd,None,0,0,round(720*scale)+frame_w,round(500*scale)+frame_h,0x2|0x4|0x10)
            curve_to(texts['ui.save_curve'])
            chart=by_id('AppWindow::chart')
            assert chart['absolutePosition'].get('x',0)+chart['size']['width']<=720+.5
            shot('fan-curve-minimum')
            assert user.SetWindowPos(hwnd,None,0,0,round(760*scale)+frame_w,round(540*scale)+frame_h,0x2|0x4|0x10)
            label_click(texts['ui.back'])
            navigate('settings.title', 'settings')
            shot('settings-final')
            def settings_scroll(delta):
                view = by_id('AppWindow::settings')
                client.tool('dispatch_pointer_scroll', {'windowHandle': window, 'deltaY': delta,
                    'position': {'x': view['absolutePosition'].get('x', 0) + 5,
                                 'y': view['absolutePosition'].get('y', 0) + view['size']['height'] / 2}})
                time.sleep(.08)
            def settings_to(name):
                settings_scroll(10000)
                for _ in range(35):
                    try:
                        e = by_id('AppWindow::' + name)
                        view = by_id('AppWindow::settings')
                        y = e['absolutePosition'].get('y', 0)
                        top = view['absolutePosition'].get('y', 0)
                        if top <= y and y + e['size']['height'] <= top + view['size']['height']:
                            return e
                    except StopIteration:
                        pass
                    settings_scroll(-100)
                raise AssertionError('Setting not reachable: ' + name)
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
            assert any(expected_version in e.get('accessibleLabel', '') for e in about_tree)
            client.tool('click_element', {'elementHandle': close['handle']})
            time.sleep(.1)
            settings_to('palette-selector')
            label_click(texts['settings.light'])
            wait_for(lambda: by_id('AppWindow::color-background')['accessibleValue'] == '#F5F6FA')
            shot('appearance-light')
            label_click('Slate')
            wait_for(lambda: by_id('AppWindow::color-background')['accessibleValue'] == '#202127')
            click(settings_to('color-background'))
            box=picker_hex()
            client.tool('set_element_value', {'elementHandle': box['handle'], 'value': '#'})
            assert not find(texts['color.use'],'Button').get('accessibleEnabled',False)
            key('\x1b')
            assert by_id('AppWindow::color-background')['accessibleValue'] == '#202127'
            # Cancel returns keyboard focus to the invoking color field.
            assert user.SetWindowPos(hwnd, None, 0, 0, round(1000 * scale)+frame_w, round(720 * scale)+frame_h, 0x2 | 0x4 | 0x10)
            key(' ')
            assert user.SetWindowPos(hwnd,None,0,0,round(720*scale)+frame_w,round(500*scale)+frame_h,0x2|0x4|0x10)
            wait_for(lambda: by_id('ColorPicker::panel'))
            key('\t');key('\t');key('\t')
            focused=by_id('ColorPicker::hex-field');body=by_id('ColorPicker::body')
            assert body['absolutePosition'].get('y',0)<=focused['absolutePosition'].get('y',0)
            assert focused['absolutePosition'].get('y',0)+focused['size']['height']<=body['absolutePosition'].get('y',0)+body['size']['height']+.5
            shot('color-picker-minimum-focus')
            assert user.SetWindowPos(hwnd,None,0,0,round(1000*scale)+frame_w,round(720*scale)+frame_h,0x2|0x4|0x10)
            picker_hex()
            shot('color-picker')
            view = by_id('ColorPicker::body')
            client.tool('dispatch_pointer_scroll', {'windowHandle':window,'deltaY':10000,
                'position':{'x':view['absolutePosition'].get('x',0)+5,'y':view['absolutePosition'].get('y',0)+20}})
            plane=by_id('ColorPicker::pointer')
            client.tool('drag_element',{'elementHandle':plane['handle'],'target':{
                'x':plane['absolutePosition'].get('x',0)+plane['size']['width']*.75,
                'y':plane['absolutePosition'].get('y',0)+plane['size']['height']*.25}})
            before_hue=picker_hex()['accessibleValue']
            assert len(before_hue)==7 and before_hue!='#202127'
            hue=by_id('ColorPicker::hue')
            click(hue);key('\uf729')
            assert picker_hex()['accessibleValue']!=before_hue
            label_click(texts['color.swatch']+' 1')
            assert picker_hex()['accessibleValue']=='#FFFFFF'
            for _ in range(6): key('\t')
            assert not find('Slate','Button').get('accessibleEnabled',False)
            key('\x1b')
            assert user.SetWindowPos(hwnd, None, 0, 0, round(760 * scale)+frame_w, round(540 * scale)+frame_h, 0x2 | 0x4 | 0x10)
            set_value(settings_to('color-background'), '#16191E')
            set_value(settings_to('color-accent'), '#91A2FF')
            shot('appearance-custom')
            click(settings_to('tray-start'))
            settings_scroll(-10000)
            wait_for(lambda: by_id('AppWindow::settings-save').get('accessibleEnabled'))
            click(by_id('AppWindow::settings-save'))
            wait_for(lambda: not by_id('AppWindow::settings-save').get('accessibleEnabled', False))
            persisted = json.loads((fixture_dir / 'app-settings.json').read_text(encoding='utf-8'))
            assert 'appearance' not in persisted
            appearance = json.loads((fixture_dir / 'ui-settings.json').read_text(encoding='utf-8'))['appearance']
            assert appearance['palette'] == 'custom'
            assert appearance['custom']['background'] == '#16191E'
            assert appearance['custom']['accent'] == '#91A2FF'
            assert persisted['start_in_tray']
            navigate('ui.overview', 'overview')
            assert not by_id('AppWindow::startup').get('accessibleChecked', False), 'App startup must not grant preset startup'
            # A real process restart reads the same atomic settings file, in an
            # explicitly marked temp fixture; it never opens the live data store.
            user.PostMessageW(owners[0], 0x0010, 0, 0)
            process.wait(timeout=10)
            environment['OMEN_UI_TEST_LONG_CHOICES'] = '1'
            process = subprocess.Popen([str(args.exe.resolve()), '--lang', args.language, '--ui-test'],
                                       env=environment, cwd=REPO, stdout=stdout, stderr=stderr,
                                       creationflags=subprocess.CREATE_NO_WINDOW)
            deadline = time.monotonic() + 15
            while True:
                try:
                    client = McpClient('http://127.0.0.1:9315/mcp'); client.initialize()
                    window = unpack(client.tool('list_windows', {}))['windowHandles'][0]
                    root = unpack(client.tool('get_window_properties', {'windowHandle': window}))['rootElementHandle']
                    break
                except (OSError, IndexError):
                    if time.monotonic() > deadline: raise
                    time.sleep(.1)
            navigate('settings.title', 'settings')
            wait_for(lambda: by_id('AppWindow::color-background')['accessibleValue'] == '#16191E')
            assert by_id('AppWindow::color-accent')['accessibleValue'] == '#91A2FF'
            assert settings_to('tray-start').get('accessibleChecked')
            assert not settings_to('settings-save').get('accessibleEnabled', False)
            assert not (fixture_dir / 'startup.json').exists()
            navigate('ui.overview', 'overview')
            name_field()
            click(scroll_to(texts['preset.selector_label'],'Combobox'))

            def visible_option():
                for menu_window in unpack(client.tool('list_windows', {}))['windowHandles']:
                    menu_root = unpack(client.tool('get_window_properties', {'windowHandle': menu_window}))['rootElementHandle']
                    elements = unpack(client.tool('get_element_tree', {'elementHandle': menu_root, 'maxElements': 1500}))['elements']
                    lists = [e for e in elements if any(i.get('id') == 'ComboBox::list' for i in e['typeNamesAndIds'])]
                    selected = [e for e in elements if e.get('accessibleRole') == 'ListItem' and e.get('accessibleItemSelected')]
                    for item in selected:
                        for view in lists:
                            y = item['absolutePosition'].get('y', 0); top = view['absolutePosition'].get('y', 0)
                            if top - .5 <= y and y + item['size']['height'] <= top + view['size']['height'] + .5:
                                return menu_window, item, view
                return None

            key('\uf72b')  # End traverses many variable-height, wrapped rows.
            menu_window, option, menu = wait_for(visible_option)
            late_name = texts['ui.test_long_preset'].replace('{0}', '24')
            assert option['accessibleLabel'] == late_name
            assert option['size']['height'] > 36
            shot('long-dropdown-late')
            for _ in range(18):
                key('\uf700'); wait_for(visible_option)
            key('\uf72b'); wait_for(visible_option)
            # Manual wheel scrolling must not detach keyboard ensure-visible.
            client.tool('dispatch_pointer_scroll', {'windowHandle': menu_window, 'deltaY': 10000,
                'position': {'x': menu['absolutePosition'].get('x', 0) + 10, 'y': menu['absolutePosition'].get('y', 0) + 20}})
            key('\uf700'); key('\uf701'); wait_for(visible_option)
            key('\n')
            wait_for(lambda: name_field()['accessibleValue'] == late_name)
            click(scroll_to(texts['preset.selector_label'],'Combobox'))  # Opening a late selection also reveals it.
            _, option, _ = wait_for(visible_option)
            assert option['accessibleLabel'] == late_name
            shot('long-dropdown-reopened')
            key('\x1b')
            (output / 'result.json').write_text(json.dumps({'passed': True, 'language': args.language, 'backend': args.backend,
                'binarySha256': hashlib.sha256(args.exe.read_bytes()).hexdigest(),
                'window': properties, 'wheelEvents': 100, 'resizes': 15, 'minimizeRestoreCycles': 3, 'trayRestoreCycles': 3,
                'nameAndUndo': True, 'actualTabTransitions': True, 'equalNavigationGeometry': True, 'exactMilliwatts': 55125,
                'curveNumericUndoRedoAndReopen': True, 'curveMouseAndModifierGestures': 4,
                'aboutLanguageAndVersion': True, 'settingsDraftSaveAndSeparateStartup': True,
                'diagonalCurvePixels': True, 'customAppearanceAndProcessRestart': True,
                'lightingZoneMappingBrightnessResetAndDraftApply': True,
                'sharedColorPickerHexCancelFocusAndPalette': True, 'uniformFormFieldWidth':276,
                'wrappedDropdownOpeningKeysAndWheel': True,
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
