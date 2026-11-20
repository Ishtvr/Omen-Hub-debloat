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
from PIL import Image, ImageChops  # Developer-only screenshot assertions; never a runtime dependency.

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
    if args.pending_switch_only:
        environment['OMEN_UI_TEST_PENDING'] = '1'
    else:
        environment.pop('OMEN_UI_TEST_PENDING',None)
    if args.new_profile_curve_only:
        environment['OMEN_UI_TEST_CURVE_APPLIED']='1'
    else:
        environment.pop('OMEN_UI_TEST_CURVE_APPLIED',None)
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

            def new_profile_back():
                wait_for(lambda: by_id('AppWindow::curve-editor'))
                assert not curve_to(texts['ui.undo']).get('accessibleEnabled',False)
                label_click(texts['ui.back'])
                wait_for(lambda: name_field().get('accessibleValue','')=='')

            def dismiss_notice():
                for e in tree():
                    if e.get('accessibleLabel')==texts['notice.dismiss'] and e.get('accessibleRole')=='Button':
                        click(e)
                        break

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

            def pane_to(view_id,label,role='Button'):
                for direction in [0,10000]:
                    for step in range(35):
                        view=by_id('AppWindow::'+view_id);top=view['absolutePosition'].get('y',0)
                        for e in tree():
                            y=e['absolutePosition'].get('y',0)
                            if e.get('accessibleLabel')==label and e.get('accessibleRole')==role and top<=y and y+e['size']['height']<=top+view['size']['height']:
                                return e
                        delta=direction if step==0 and direction else -90
                        client.tool('dispatch_pointer_scroll',{'windowHandle':window,'deltaY':delta,'position':{
                            'x':view['absolutePosition'].get('x',0)+5,'y':top+view['size']['height']/2}})
                raise AssertionError('Pane control not reachable: '+label)

            # Use only the native window belonging to our --ui-test subprocess.
            user = c.WinDLL('user32', use_last_error=True)
            user.GetWindowThreadProcessId.argtypes = [wt.HWND, c.POINTER(wt.DWORD)]
            user.GetWindowTextW.argtypes = [wt.HWND, wt.LPWSTR, c.c_int]
            user.IsWindowVisible.argtypes = [wt.HWND]
            user.SetWindowPos.argtypes = [wt.HWND, wt.HWND, c.c_int, c.c_int, c.c_int, c.c_int, wt.UINT]
            user.ShowWindow.argtypes = [wt.HWND, c.c_int]
            user.GetClientRect.argtypes = [wt.HWND, c.POINTER(wt.RECT)]
            user.GetDoubleClickTime.restype = wt.UINT
            user.GetForegroundWindow.restype = wt.HWND
            user.SetForegroundWindow.argtypes = [wt.HWND]
            user.WindowFromPoint.argtypes = [wt.POINT]
            user.WindowFromPoint.restype = wt.HWND
            user.GetDC.argtypes = [wt.HWND]
            user.GetDC.restype = wt.HDC
            user.ReleaseDC.argtypes = [wt.HWND, wt.HDC]
            gdi = c.WinDLL('gdi32', use_last_error=True)
            gdi.GetPixel.argtypes = [wt.HDC, c.c_int, c.c_int]
            gdi.GetPixel.restype = wt.DWORD
            gdi.CreateCompatibleDC.argtypes=[wt.HDC];gdi.CreateCompatibleDC.restype=wt.HDC
            gdi.CreateCompatibleBitmap.argtypes=[wt.HDC,c.c_int,c.c_int];gdi.CreateCompatibleBitmap.restype=wt.HANDLE
            gdi.SelectObject.argtypes=[wt.HDC,wt.HANDLE];gdi.SelectObject.restype=wt.HANDLE
            gdi.DeleteObject.argtypes=[wt.HANDLE];gdi.DeleteObject.restype=wt.BOOL
            gdi.DeleteDC.argtypes=[wt.HDC];gdi.DeleteDC.restype=wt.BOOL
            gdi.BitBlt.argtypes=[wt.HDC,c.c_int,c.c_int,c.c_int,c.c_int,wt.HDC,c.c_int,c.c_int,wt.DWORD]
            gdi.BitBlt.restype=wt.BOOL
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
            presented_checks = []

            def presented_reference():
                # Slint's MCP snapshot resets partial-render caches itself.
                # Use it only BEFORE a lifecycle transition, then read actual
                # desktop pixels strictly inside our own foreground client.
                shot('before-native-restore')
                picture = Image.open(output / 'before-native-restore.png').convert('RGB')
                scale = properties['scaleFactor']
                grounds = {picture.getpixel((round(x*scale),round(55*scale))) for x in [5,165]}
                samples = []
                # Caption hover/focus can legitimately change on close/restore.
                for y in range(round(55 * properties['scaleFactor']), picture.height - 22, 47):
                    for x in range(22, picture.width - 22, 53):
                        patch = picture.crop((x-4, y-4, x+5, y+5))
                        if all(lo == hi for lo, hi in patch.getextrema()) and picture.getpixel((x,y)) in grounds:
                            r, g, b = picture.getpixel((x, y))
                            samples.append((x, y, r | g << 8 | b << 16))
                assert len(samples) > 40, len(samples)
                return samples

            def assert_presented(samples, label):
                # Only our own window is raised; focus stealing is unnecessary
                # and can be refused by Windows while the user is in Codex.
                assert user.SetWindowPos(hwnd, wt.HWND(-1), 0, 0, 0, 0, 0x1 | 0x2 | 0x10)
                origin = wt.POINT(0, 0)
                assert user.ClientToScreen(hwnd, c.byref(origin))
                deadline = time.monotonic() + 2
                try:
                    while True:
                        for x, y, _ in samples:
                            owner = user.WindowFromPoint(wt.POINT(origin.x+x, origin.y+y))
                            pid = wt.DWORD()
                            user.GetWindowThreadProcessId(owner, c.byref(pid))
                            assert pid.value == process.pid, ('Sample is occluded', label, x, y)
                        dc = user.GetDC(None)
                        assert dc
                        memory=gdi.CreateCompatibleDC(dc);assert memory
                        width=max(x for x,_,_ in samples)+1;height=max(y for _,y,_ in samples)+1
                        bitmap=gdi.CreateCompatibleBitmap(dc,width,height);assert bitmap
                        previous=gdi.SelectObject(memory,bitmap)
                        try:
                            # Copy the shown desktop once. Hundreds of live
                            # GetPixel calls can straddle compositor frames and
                            # consume the entire retry deadline themselves.
                            # This does not request a Slint snapshot/cache reset.
                            assert gdi.BitBlt(memory,0,0,width,height,dc,origin.x,origin.y,0x00CC0020)
                            bad=[]
                            for x,y,expected in samples:
                                actual=gdi.GetPixel(memory,x,y)
                                if actual!=expected:bad.append((x,y,expected,actual))
                        finally:
                            gdi.SelectObject(memory,previous);gdi.DeleteObject(bitmap);gdi.DeleteDC(memory)
                            user.ReleaseDC(None, dc)
                        if not bad:
                            presented_checks.append({'transition': label, 'screenPixels': len(samples)})
                            return
                        if time.monotonic() > deadline:
                            raise AssertionError(('Presented client differs after ' + label, bad[:12], len(bad)))
                        time.sleep(.04)
                finally:
                    user.SetWindowPos(hwnd, wt.HWND(-2), 0, 0, 0, 0, 0x1 | 0x2 | 0x10)
            user.GetWindowLongPtrW.argtypes=[wt.HWND,c.c_int]
            user.GetWindowLongPtrW.restype=c.c_ssize_t
            user.IsZoomed.argtypes=[wt.HWND]
            # Winit retains caption style bits for Windows snapping. Verify the
            # actual client inset rather than equating those bits with paint.
            user.ClientToScreen.argtypes=[wt.HWND,c.POINTER(wt.POINT)]
            user.GetWindowRect.argtypes=[wt.HWND,c.POINTER(wt.RECT)]
            bounds=wt.RECT();origin=wt.POINT(0,0)
            user.GetWindowRect(hwnd,c.byref(bounds));user.ClientToScreen(hwnd,c.byref(origin))
            assert abs((origin.y-bounds.top)-(origin.x-bounds.left))<=2, 'Native caption inset remains'
            wait_for(lambda: name_field())
            assert abs(name_field()['size']['width'] - 276) < .1
            assert abs(by_id('AppWindow::preset-selector')['size']['width'] - 276) < .1
            navigate('ui.overview', 'overview')
            scroll(10000)
            wait_for(lambda: any(e.get('accessibleLabel') == 'GPU · ' + texts['ui.test_data'] for e in tree()))
            tabs = [by_id('AppWindow::navigation-'+name) for name in ['overview','cooling','lighting']]
            assert len(tabs) == 3
            assert max(e['size']['width'] for e in tabs) - min(e['size']['width'] for e in tabs) < .01
            if args.editor_extensions_only:
                user.GetClientRect.argtypes=[wt.HWND,c.POINTER(wt.RECT)]
                scale=properties['scaleFactor']
                def resize(w,h):
                    outer=wt.RECT();inner=wt.RECT();user.GetWindowRect(hwnd,c.byref(outer));user.GetClientRect(hwnd,c.byref(inner))
                    assert user.SetWindowPos(hwnd,None,0,0,round(w*scale)+outer.right-outer.left-inner.right,round(h*scale)+outer.bottom-outer.top-inner.bottom,0x2|0x4|0x10)
                    time.sleep(.1)
                def cooling_to(label,role='Button'):return pane_to('cooling',label,role)
                def field(label):return scroll_to(label,'TextInput')
                navigate('ui.cooling','cooling')
                wait_for(lambda: find(texts['fan_profile.name'],'TextInput').get('accessibleEnabled'))
                set_value(cooling_to(texts['fan_profile.name'],'TextInput'),'Fan one')
                click(cooling_to(texts['preset.save']))
                wait_for(lambda: find(texts['fan_profile.preset'],'Combobox').get('accessibleValue')=='Fan one')
                dismiss_notice()
                shot('named-fan-library')
                navigate('ui.overview','overview')
                set_value(name_field(),'Saved baseline')
                temp=texts['field.CpuTemperatureLimitC']
                click(scroll_to(temp,'Checkbox'));set_value(field(temp),'85')
                click(scroll_to(texts['group.curve_optimizer_per_core']))
                click(scroll_to(texts['field.CpuCurveOptimizerPerCore'],'Checkbox'))
                set_value(field('CCD0 · 0'),'-10')
                fan=scroll_to(texts['fan_profile.link'],'Combobox');click(fan);key('\uf72b');key('\n')
                wait_for(lambda: find(texts['fan_profile.link'],'Combobox').get('accessibleValue')=='Fan one')
                click(scroll_to(texts['ui.edit_curve']))
                wait_for(lambda: by_id('AppWindow::point-y'))
                set_value(by_id('AppWindow::point-y'),'1915');label_click(texts['ui.set_point']);label_click(texts['ui.use_curve'])
                click(by_id('AppWindow::save-preset'))
                wait_for(lambda: scroll_to(texts['preset.selector_label'],'Combobox').get('accessibleValue')=='Saved baseline')
                dismiss_notice()
                set_value(name_field(),'Edited name');set_value(field(temp),'95');set_value(field('CCD0 · 0'),'-20')
                click(scroll_to(texts['fan_profile.link'],'Combobox'));key('\uf729');key('\n')
                click(by_id('AppWindow::startup'))
                click(scroll_to(texts['fan_profile.link'],'Combobox'));key('\uf72b');key('\n')
                click(by_id('AppWindow::save-preset'))
                wait_for(lambda: any(e.get('accessibleLabel')==texts['fan_profile.err.startup'] for e in tree()))
                dismiss_notice()
                click(scroll_to(texts['fan_profile.link'],'Combobox'));key('\uf729');key('\n')
                click(scroll_to(texts['ui.edit_curve']))
                wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')=='1915')
                set_value(by_id('AppWindow::point-y'),'1930');label_click(texts['ui.set_point'])
                label_click(texts['preset.cancel_edit'])
                wait_for(lambda: name_field().get('accessibleValue')=='Saved baseline')
                assert field(temp)['accessibleValue']=='85'
                assert field('CCD0 · 0')['accessibleValue']=='-10'
                assert scroll_to(texts['fan_profile.link'],'Combobox').get('accessibleValue')=='Fan one'
                assert not by_id('AppWindow::startup').get('accessibleChecked',False)
                shot('cancel-to-saved-preset')
                dismiss_notice()
                click(scroll_to(texts['ui.edit_curve']))
                wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')=='1915')
                assert not find(texts['ui.undo'],'Button').get('accessibleEnabled',False)
                set_value(curve_to(texts['curve.range_from'],'TextInput'),'950')
                set_value(curve_to(texts['curve.range_to'],'TextInput'),'1000')
                label_click(texts['curve.select_range'])
                set_value(by_id('AppWindow::point-y'),'1900');label_click(texts['curve.equalize'])
                def nodes():return [e for e in tree() if any(i.get('id')=='CurveCanvas::draft-node' for i in e['typeNamesAndIds'])]
                selected=[e for e in nodes() if e['size']['width']==8]
                assert len(selected)==6,len(selected)
                assert max(e['absolutePosition'].get('y',0) for e in selected)-min(e['absolutePosition'].get('y',0) for e in selected)<.1
                shot('selected-equalized-range')
                label_click(texts['ui.undo']);wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')=='1915')
                set_value(curve_to(texts['curve.range_from'],'TextInput'),'951');set_value(curve_to(texts['curve.range_to'],'TextInput'),'959')
                label_click(texts['curve.select_range'])
                wait_for(lambda: any(e.get('accessibleLabel')==texts['curve.err.empty_selection'] for e in tree()))
                assert not find(texts['curve.flatten_right'],'Button').get('accessibleEnabled',False)
                assert by_id('AppWindow::point-y')['accessibleValue']=='1915'
                dismiss_notice()
                canvas=by_id('CurveCanvas::canvas-input')
                client.tool('drag_element',{'elementHandle':canvas['handle'],'target':{'x':canvas['absolutePosition']['x']+canvas['size']['width']*.8,'y':canvas['absolutePosition']['y']+canvas['size']['height']*.3}})
                assert len([e for e in nodes() if e['size']['width']==8])>5
                shot('pointer-selected-voltage-band')
                label_click(texts['preset.cancel_edit']);dismiss_notice()
                resize(1600,1000)
                overview=by_id('AppWindow::preset-list');cx=overview['absolutePosition'].get('x',0)+overview['size']['width']/2
                assert abs(cx-(160+(1600-160)/2))<16,(cx,overview)
                scroll(10000)
                wait_for(lambda: any(e.get('accessibleLabel')=='GPU · '+texts['ui.test_data'] for e in tree()))
                shot('centered-wide-overview')
                navigate('ui.lighting','lighting')
                preview=wait_for(lambda: by_id('AppWindow::keyboard-preview'))
                assert preview['size']['width']>1000,preview
                for label,expected in [('lighting.zone_left','#AA8C64'),('lighting.zone_middle','#A578AF'),('lighting.numpad_right','#73A0C8'),('lighting.wasd','#6EAF96')]:
                    label_click(texts[label]);assert lighting_to('lighting-color')['accessibleValue']==expected
                label_click(texts['lighting.zone_left']);set_value(lighting_to('lighting-color'),'#FF0000')
                label_click(texts['lighting.zone_middle']);assert lighting_to('lighting-color')['accessibleValue']=='#A578AF'
                label_click(texts['lighting.zone_left']);assert lighting_to('lighting-color')['accessibleValue']=='#FF0000'
                shot('large-four-zone-keyboard')
                navigate('settings.title','settings')
                action=pane_to('settings',texts['ui.hp_restore'])
                assert action['size']['width']==276,action
                subtree=unpack(client.tool('get_element_tree',{'elementHandle':action['handle'],'maxElements':40}))['elements']
                label=next(e for e in subtree if any(i.get('id')=='ActionButton::label' for i in e['typeNamesAndIds']))
                assert label['size']['width']<=action['size']['width']-15
                assert label['absolutePosition'].get('x',0)>=action['absolutePosition'].get('x',0)
                shot('centered-settings-hp-action')
                resize(820,500);pane_to('settings',texts['ui.hp_restore'])
                assert find(texts['ui.about'],'Button')['size']['width']==276
                shot('compact-hp-action')
                (output/'result.json').write_text(json.dumps({'passed':True,'language':args.language,'backend':args.backend,
                    'binarySha256':hashlib.sha256(args.exe.read_bytes()).hexdigest(),
                    'rangeDragNumericEqualizeUndoAndEmptyGuard':True,'cancelRestoresCurrentPresetAndFanLink':True,
                    'centeredColumnsAndBoundedHpLabel':True,'largeIndependentFourZoneKeyboard':True,
                    'scope':'Isolated UI fixture; no real tuning or service action'},indent=2),encoding='utf-8')
                print(output)
                return
            if args.new_profile_curve_only:
                # Startup remains on Overview. The fake driver carries a live
                # +60 MHz offset; its factory target at 950 mV is 1900 MHz.
                assert not any(any(i.get('id')=='AppWindow::curve-editor' for i in e['typeNamesAndIds']) for e in tree())
                set_value(name_field(),'Saved curve')
                click(scroll_to(texts['ui.edit_curve']))
                wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')=='1900')
                set_value(by_id('AppWindow::point-y'),'1915');label_click(texts['ui.set_point'])
                label_click(texts['ui.use_curve'])
                click(by_id('AppWindow::save-preset'))
                wait_for(lambda: scroll_to(texts['preset.selector_label'],'Combobox').get('accessibleValue')=='Saved curve')
                dismiss_notice()

                def factory_open(capture):
                    wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')=='1900')
                    assert not curve_to(texts['ui.undo']).get('accessibleEnabled',False)
                    assert not curve_to(texts['ui.redo']).get('accessibleEnabled',False)
                    shot(capture)
                    key('\uf700')  # New editor receives keyboard focus immediately.
                    wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')=='1901')
                    key('\x11','Press');key('z');key('\x11','Release')
                    wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')=='1900')
                def saved_curve():
                    click(scroll_to(texts['preset.selector_label'],'Combobox'));key('\uf729');key('\uf701');key('\n')
                    wait_for(lambda: name_field().get('accessibleValue')=='Saved curve')
                    click(scroll_to(texts['ui.edit_curve']))
                    wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')=='1915')

                click(scroll_to(texts['preset.new']))
                factory_open('new-button-factory')
                label_click(texts['ui.back'])
                saved_curve()  # Saved target is retained, unlike the live offset.
                set_value(by_id('AppWindow::point-y'),'1930');label_click(texts['ui.set_point'])
                label_click(texts['ui.back'])
                click(scroll_to(texts['preset.new']))
                wait_for(lambda: by_id('ConfirmPrompt::save-button'));key('\x1b')
                click(scroll_to(texts['ui.edit_curve']))
                wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')=='1930')
                label_click(texts['ui.back'])
                click(scroll_to(texts['preset.new']))
                wait_for(lambda: by_id('ConfirmPrompt::confirm-button'))
                click(by_id('ConfirmPrompt::confirm-button'))
                factory_open('discard-to-new-factory')
                label_click(texts['ui.back'])
                saved_curve()
                label_click(texts['ui.back'])
                click(scroll_to(texts['preset.selector_label'],'Combobox'));key('\uf729');key('\n')
                factory_open('dropdown-new-factory')
                label_click(texts['ui.back'])
                click(scroll_to(texts['preset.new']))
                factory_open('repeated-new-factory')
                label_click(texts['ui.use_curve'])
                set_value(name_field(),'Save then new')
                click(scroll_to(texts['preset.new']))
                wait_for(lambda: by_id('ConfirmPrompt::save-button'));click(by_id('ConfirmPrompt::save-button'))
                factory_open('save-then-new-factory')
                (output/'result.json').write_text(json.dumps({'passed':True,'language':args.language,'backend':args.backend,
                    'binarySha256':hashlib.sha256(args.exe.read_bytes()).hexdigest(),
                    'newButtonAndDropdownOpenFactory':True,'liveOffsetAndCachedEditNotInherited':True,
                    'cancelRetainsDraftDiscardResetsSaveThenNewWorks':True,'savedCurveRetained':True,
                    'scope':'Isolated UI fixture with applied driver offset, no hardware writes'},indent=2),encoding='utf-8')
                print(output)
                return
            if args.gpu_curve_enable_only:
                draft_name='GPU curve opening'
                set_value(name_field(),draft_name)
                curve_label=texts['field.GpuVfCurve']
                include=scroll_to(curve_label,'Checkbox')
                assert not include.get('accessibleChecked',False)
                click(include)
                assert find(curve_label,'Checkbox').get('accessibleChecked')
                click(scroll_to(texts['ui.edit_curve']))
                wait_for(lambda: by_id('AppWindow::curve-editor'))
                original=by_id('AppWindow::point-y')['accessibleValue']
                set_value(by_id('AppWindow::point-y'),str(int(original)+15))
                label_click(texts['ui.set_point'])
                label_click(texts['ui.back'])
                click(scroll_to(texts['ui.edit_curve']))
                wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')==str(int(original)+15))
                label_click(texts['ui.undo'])
                wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')==original)
                label_click(texts['ui.redo'])
                wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')==str(int(original)+15))
                shot('gpu-curve-enabled-before-opening')
                label_click(texts['ui.use_curve'])
                wait_for(lambda: name_field().get('accessibleValue')==draft_name)
                assert scroll_to(curve_label,'Checkbox').get('accessibleChecked')
                click(scroll_to(texts['ui.edit_curve']))
                wait_for(lambda: by_id('AppWindow::point-y').get('accessibleValue')==str(int(original)+15))
                shot('gpu-curve-reopened-with-draft')
                (output/'result.json').write_text(json.dumps({'passed':True,'language':args.language,'backend':args.backend,
                    'binarySha256':hashlib.sha256(args.exe.read_bytes()).hexdigest(),
                    'enableEmptyCurveBeforeOpening':True,'draftNameAndCurveHistoryRetained':True,
                    'scope':'Isolated UI fixture, no profile persistence or hardware access'},indent=2),encoding='utf-8')
                print(output)
                return
            mode = scroll_to(texts['ui.performance'], 'Combobox')
            assert abs(mode['size']['width'] - 276) < .1
            click(mode)
            choices = []
            for menu_window in unpack(client.tool('list_windows', {}))['windowHandles']:
                menu_root = unpack(client.tool('get_window_properties', {'windowHandle': menu_window}))['rootElementHandle']
                choices.extend(e.get('accessibleLabel') for e in unpack(client.tool('get_element_tree',
                    {'elementHandle': menu_root, 'maxElements': 1000}))['elements'] if e.get('accessibleRole') == 'ListItem')
            for mode_key in ['mode.quiet', 'mode.balanced', 'mode.performance', 'mode.unleashed']:
                assert texts['mode.default_entry'].replace('{0}',texts[mode_key]) in choices, (mode_key,choices)
            key('\uf700');key('\n')
            click(scroll_to(texts['preset.apply_button']))
            wait_for(lambda: by_id('AppWindow::notification'))
            assert any(e.get('accessibleLabel') == texts['ui.test_no_write'] for e in tree())
            popup = by_id('AppWindow::notification')
            for label in [texts['ui.details'], texts['notice.dismiss']]:
                control = find(label, 'Button')
                assert control['absolutePosition'].get('y',0) + control['size']['height'] <= popup['absolutePosition'].get('y',0) + popup['size']['height'], label
            shot('error-popup')
            label_click(texts['ui.details'])
            wait_for(lambda: any(e.get('accessibleRole')=='TextInput' and e.get('accessibleValue')==texts['ui.test_no_write'] for e in tree()))
            shot('error-details-before-selection')
            key('\t')
            key('\x11','Press');key('a');key('\x11','Release')
            shot('error-details')
            details = find(texts['notice.details_text'],'TextInput')
            s = properties['scaleFactor']
            left = round((details['absolutePosition'].get('x',0)+10)*s)
            top = round((details['absolutePosition'].get('y',0)+10)*s)
            box = (left,top,left+round((details['size']['width']-20)*s),top+round((details['size']['height']-20)*s))
            before = Image.open(output/'error-details-before-selection.png').convert('RGB').crop(box)
            after = Image.open(output/'error-details.png').convert('RGB').crop(box)
            changed = sum(1 for pixel in ImageChops.difference(before,after).get_flattened_data() if pixel != (0,0,0))
            assert changed > 200, ('Keyboard selection did not reach the message',changed)
            key('\t');key('\t');key('\x10','Press');key('\t');key('\x10','Release');key('\n')
            wait_for(lambda: not any(e.get('accessibleLabel')==texts['notice.details_text'] for e in tree()))
            wait_for(lambda: by_id('AppWindow::notification'))
            key('\n')  # Return focus to the same Details button and reopen.
            wait_for(lambda: find(texts['notice.details_text'],'TextInput'))
            key('\x1b')
            label_click(texts['notice.dismiss'])
            if args.pending_switch_only:
                first = 'Pending-Switch'
                set_value(name_field(),first)
                click(scroll_to(texts['preset.new']))
                wait_for(lambda: by_id('ConfirmPrompt::save-button'))
                click(by_id('ConfirmPrompt::save-button'))
                new_profile_back()
                wait_for(lambda: name_field().get('accessibleValue','')=='')
                shot('pending-switch-saved')
                click(scroll_to(texts['preset.selector_label'],'Combobox'));key('\uf701');key('\n')
                wait_for(lambda: name_field().get('accessibleValue')==first)
                set_value(name_field(),'')
                click(scroll_to(texts['preset.new']))
                wait_for(lambda: by_id('ConfirmPrompt::save-button'))
                click(by_id('ConfirmPrompt::save-button'))
                wait_for(lambda: any(e.get('accessibleLabel')==texts['preset.name_required'] for e in tree()))
                assert scroll_to(texts['preset.selector_label'],'Combobox').get('accessibleValue')==first
                shot('pending-switch-save-failed')
                label_click(texts['notice.dismiss'])
                set_value(name_field(),'Repaired')
                click(by_id('AppWindow::save-preset'))
                wait_for(lambda: name_field().get('accessibleValue')=='Repaired')
                assert scroll_to(texts['preset.selector_label'],'Combobox').get('accessibleValue')=='Repaired'
                shot('pending-switch-retry')
                (output/'result.json').write_text(json.dumps({'passed':True,'language':args.language,'backend':args.backend,
                    'binarySha256':hashlib.sha256(args.exe.read_bytes()).hexdigest(),
                    'saveSwitchWithPendingRecoveryWarning':True,'failedSaveRetainsDraftAndClearsSwitchIntent':True,
                    'scope':'Isolated UI metadata, no live recovery journal or hardware access'},indent=2),encoding='utf-8')
                print(output)
                return
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
            click(by_id('AppWindow::window-maximize'))
            wait_for(lambda: user.IsZoomed(hwnd))
            shot('frameless-maximized')
            click(by_id('AppWindow::window-maximize'))
            wait_for(lambda: not user.IsZoomed(hwnd))
            def caption_layout_ready():
                client_rect=wt.RECT();assert user.GetClientRect(hwnd,c.byref(client_rect))
                return abs(by_id('AppWindow::titlebar-move')['size']['width']+150-client_rect.right/properties['scaleFactor'])<1
            wait_for(caption_layout_ready)
            # Separate gestures by the system click interval. MCP computes each
            # press/release at the element's current center; a resize mid-chain
            # otherwise mixes the preceding click series with new coordinates.
            time.sleep(max(.5,user.GetDoubleClickTime()/1000)+.05)
            client.tool('click_element',{'elementHandle':by_id('AppWindow::titlebar-move')['handle'],'action':'DoubleClick'})
            wait_for(lambda: user.IsZoomed(hwnd))
            wait_for(caption_layout_ready)
            time.sleep(max(.5,user.GetDoubleClickTime()/1000)+.05)
            client.tool('click_element',{'elementHandle':by_id('AppWindow::titlebar-move')['handle'],'action':'DoubleClick'})
            wait_for(lambda: not user.IsZoomed(hwnd))
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
            for width, height in [(860, 540), (1140, 740), (820, 500), (960, 660), (860, 540)] * 3:
                assert user.SetWindowPos(hwnd, None, 0, 0, round(width * scale) + frame_w, round(height * scale) + frame_h, 0x2 | 0x4 | 0x10)
                wait_for(lambda: matches_size(width, height))
                assert name_field()['accessibleValue'] == name + 'x'
                if width == 820:
                    shot('compact-overview')
            # Keep the owned 200% fixture on screen before reading desktop DC
            # pixels. Restoring its default offset can leave the final sample
            # rows beyond the monitor, where GetPixel returns CLR_INVALID.
            assert user.SetWindowPos(hwnd,None,0,0,0,0,0x1|0x4|0x10)
            samples = presented_reference()
            assert_presented(samples, 'baseline')
            click(by_id('AppWindow::window-minimize'))
            time.sleep(.1);user.ShowWindow(hwnd,9);time.sleep(.1)
            assert_presented(samples, 'caption-minimize-restore')
            for cycle in range(3):
                user.ShowWindow(hwnd, 6); time.sleep(.1); user.ShowWindow(hwnd, 9); time.sleep(.1)
                assert_presented(samples, 'native-minimize-restore-' + str(cycle))
            assert name_field()['accessibleValue'] == name + 'x'
            # Save from the switch popup preserves the draft before selection.
            click(scroll_to(texts['preset.new']))
            wait_for(lambda: by_id('ConfirmPrompt::save-button'))
            shot('save-switch-popup')
            click(by_id('ConfirmPrompt::save-button'))
            # The saved notice is a transient toast (6 s): check it as soon as
            # it appears, before the fresh-preset curve editor is closed.
            wait_for(lambda: any(e.get('accessibleLabel')==texts['notice.saved'] for e in tree()))
            shot('saved-popup')
            new_profile_back()
            wait_for(lambda: name_field().get('accessibleValue','')=='')
            click(scroll_to(texts['preset.selector_label'],'Combobox'));key('\uf701');key('\n')
            wait_for(lambda: name_field().get('accessibleValue')==name+'x')
            # The toast may already have timed out on slower renderers.
            dismiss_notice()
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
            samples = presented_reference()
            assert_presented(samples, 'before-tray')
            for cycle in range(3):
                if cycle==0: click(by_id('AppWindow::window-close'))
                else: user.PostMessageW(hwnd, 0x0010, 0, 0)  # Same close policy for OS requests.
                wait_for(lambda: not user.IsWindowVisible(hwnd))
                assert process.poll() is None
                user.PostMessageW(owners[0], activate, 0, 0)
                wait_for(lambda: unpack(client.tool('list_windows', {}))['windowHandles'])
                window = unpack(client.tool('list_windows', {}))['windowHandles'][0]
                root = unpack(client.tool('get_window_properties', {'windowHandle': window}))['rootElementHandle']
                wait_for(lambda: name_field()['accessibleValue'] == name + 'x')
                assert_presented(samples, 'tray-restore-' + str(cycle))
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
            label_click(texts['lighting.zone_left'])
            assert lighting_to('lighting-color').get('accessibleValue', '') == '#AA8C64'
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
            assert user.SetWindowPos(hwnd, None, 0, 0, round(860 * scale) + frame_w, round(540 * scale) + frame_h, 0x2 | 0x4 | 0x10)
            navigate('ui.overview', 'overview')

            # Hub-style CPU power slider in the Dynamic Boost group (beta.10):
            # 15-160 W without Dynamic Boost, 45-80 W with it, value beside it.
            def set_checked(label, state):
                for _ in range(3):
                    box = scroll_to(label, 'Checkbox')
                    if bool(box.get('accessibleChecked', False)) == state:
                        return
                    click(box)
                    try:
                        wait_for(lambda: bool(find(label, 'Checkbox').get('accessibleChecked', False)) == state)
                        return
                    except AssertionError:
                        pass
                raise AssertionError('Checkbox did not reach its state: ' + label)
            cpu_power = texts['field.CpuSlowPptLimitMw']
            boost_flag = texts['field.GpuUnleashed']
            set_checked(boost_flag, False)
            set_checked(cpu_power, True)
            power_slider = find(cpu_power, 'Slider')
            assert float(power_slider['accessibleValueMinimum']) == 15 and float(power_slider['accessibleValueMaximum']) == 160
            set_value(power_slider, '60')
            wait_for(lambda: float(find(cpu_power, 'Slider')['accessibleValue']) == 60)
            set_checked(boost_flag, True)
            wait_for(lambda: float(find(cpu_power, 'Slider')['accessibleValueMaximum']) == 80)
            assert float(find(cpu_power, 'Slider')['accessibleValueMinimum']) == 45
            set_checked(boost_flag, False)
            set_checked(cpu_power, False)
            # Expand/collapse and exact decimal-unit input survive virtualization.
            click(scroll_to(texts['group.gpu']))
            slow = texts['field.CpuPptLimitMw']
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
            new_profile_back()
            wait_for(lambda: name_field().get('accessibleValue', '') == '')
            assert scroll_to(slow, 'TextInput')['accessibleValue'] == '80'
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
            assert user.SetWindowPos(hwnd,None,0,0,round(820*scale)+frame_w,round(500*scale)+frame_h,0x2|0x4|0x10)
            curve_to(texts['ui.use_curve'])
            chart=by_id('AppWindow::chart')
            assert chart['absolutePosition'].get('x',0)+chart['size']['width']<=820+.5
            primary=find(texts['ui.use_curve'],'Button')
            assert primary['absolutePosition'].get('x',0)+primary['size']['width']<=820+.5
            shot('gpu-curve-minimum')
            assert user.SetWindowPos(hwnd,None,0,0,round(860*scale)+frame_w,round(540*scale)+frame_h,0x2|0x4|0x10)
            label_click(texts['ui.back'])
            click(scroll_to(texts['ui.edit_curve']))
            wait_for(lambda: by_id('AppWindow::point-y')['accessibleValue'] == final_y)
            label_click(texts['ui.use_curve'])
            click(by_id('AppWindow::save-preset'))
            wait_for(lambda: any(e.get('accessibleLabel', '').startswith(texts['ui.test_saved']) for e in tree()))
            navigate('ui.cooling', 'cooling')
            click(pane_to('cooling',texts['ui.edit_curve']))
            wait_for(lambda: by_id('AppWindow::curve-editor'))
            assert not any(e.get('accessibleLabel') == texts['control.working'] for e in tree())
            assert_curve_pixels('fan-curve-connected')
            assert user.SetWindowPos(hwnd,None,0,0,round(820*scale)+frame_w,round(500*scale)+frame_h,0x2|0x4|0x10)
            curve_to(texts['ui.use_curve'])
            chart=by_id('AppWindow::chart')
            assert chart['absolutePosition'].get('x',0)+chart['size']['width']<=820+.5
            shot('fan-curve-minimum')
            assert user.SetWindowPos(hwnd,None,0,0,round(860*scale)+frame_w,round(540*scale)+frame_h,0x2|0x4|0x10)
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
            assert user.SetWindowPos(hwnd,None,0,0,round(820*scale)+frame_w,round(500*scale)+frame_h,0x2|0x4|0x10)
            wait_for(lambda: by_id('ColorPicker::panel'))
            key('\t');key('\t');key('\t')
            focused=by_id('ColorPicker::hex-field');body=by_id('ColorPicker::body')
            assert body['absolutePosition'].get('y',0)<=focused['absolutePosition'].get('y',0)
            assert focused['absolutePosition'].get('y',0)+focused['size']['height']<=body['absolutePosition'].get('y',0)+body['size']['height']+.5
            shot('color-picker-minimum-focus')
            assert user.SetWindowPos(hwnd,None,0,0,round(1000*scale)+frame_w,round(820*scale)+frame_h,0x2|0x4|0x10)
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
            assert user.SetWindowPos(hwnd, None, 0, 0, round(860 * scale)+frame_w, round(540 * scale)+frame_h, 0x2 | 0x4 | 0x10)
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
                'window': properties, 'wheelEvents': 100, 'resizes': 15, 'minimizeRestoreCycles': 4, 'trayRestoreCycles': 3,
                'nameAndUndo': True, 'actualTabTransitions': True, 'equalNavigationGeometry': True, 'exactMilliwatts': 55125,
                'curveNumericUndoRedoAndReopen': True, 'curveMouseAndModifierGestures': 4,
                'aboutLanguageAndVersion': True, 'settingsDraftSaveAndSeparateStartup': True,
                'diagonalCurvePixels': True, 'customAppearanceAndProcessRestart': True,
                'lightingZoneMappingBrightnessResetAndDraftApply': True,
                'sharedColorPickerHexCancelFocusAndPalette': True, 'uniformFormFieldWidth':276,
                'wrappedDropdownOpeningKeysAndWheel': True,
                'framelessNativeStyleCaptionActionsAndDoubleClick': True,
                'hpModesOnOverviewAndPopupDetails': True, 'saveBeforePresetSwitch': True,
                'detailsKeyboardSelectionTabTrapAndReturnFocus': True,
                'presentedPixelsAfterRestore': presented_checks,
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
    parser.add_argument('--pending-switch-only',action='store_true')
    parser.add_argument('--gpu-curve-enable-only',action='store_true')
    parser.add_argument('--new-profile-curve-only',action='store_true')
    parser.add_argument('--editor-extensions-only',action='store_true')
    run(parser.parse_args())
