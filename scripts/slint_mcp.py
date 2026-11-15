"""Streamable-HTTP client for the pinned Slint developer MCP; no runtime dependency."""
import json
import urllib.request

class McpClient:
    def __init__(self, url):
        self.url = url
        self.session = None
        self.version = None
        self.sequence = 0

    def call(self, method, params=None, notification=False):
        self.sequence += 1
        data = {'jsonrpc': '2.0', 'method': method}
        if not notification:
            data['id'] = self.sequence
        if params is not None:
            data['params'] = params
        headers = {'Content-Type': 'application/json', 'Accept': 'application/json, text/event-stream',
                   'User-Agent': 'OmenHubPlusPlus-Slint-Preparation/0.0.0'}
        if self.session:
            headers['Mcp-Session-Id'] = self.session
        if self.version:
            headers['MCP-Protocol-Version'] = self.version
        request = urllib.request.Request(self.url, data=json.dumps(data).encode(), headers=headers)
        with urllib.request.urlopen(request, timeout=20) as response:
            if response.headers.get('Mcp-Session-Id'):
                self.session = response.headers['Mcp-Session-Id']
            body = response.read().decode()
        if not body.strip():
            return None
        if body.lstrip().startswith('{'):
            message = json.loads(body)
        else:
            messages = [json.loads(line[5:].strip()) for line in body.splitlines() if line.startswith('data:')]
            message = next(item for item in messages if item.get('id') == self.sequence)
        if 'error' in message:
            raise RuntimeError(message['error'])
        return message.get('result')

    def initialize(self):
        result = self.call('initialize', {
            'protocolVersion': '2025-03-26', 'capabilities': {},
            'clientInfo': {'name': 'omen-slint-preparation', 'version': '0.0.0'},
        })
        self.version = result['protocolVersion']
        self.call('notifications/initialized', notification=True)
        return result

    def tool(self, name, arguments):
        result = self.call('tools/call', {'name': name, 'arguments': arguments})
        if result.get('isError'):
            raise RuntimeError(result)
        return result


def unpack(result):
    if 'structuredContent' in result:
        return result['structuredContent']
    text = '\n'.join(part['text'] for part in result.get('content', []) if part['type'] == 'text')
    try:
        return json.loads(text)
    except ValueError:
        return text
