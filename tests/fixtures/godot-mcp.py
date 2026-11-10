"""Offline MCP fixture: never starts Godot, authenticates or contacts a service."""
import base64
import json
import os
import sys
import struct
import zlib
from pathlib import Path

root = Path.cwd()
connection_attempts = 0
def png():
    def chunk(name, data):
        return struct.pack('>I', len(data)) + name + data + struct.pack('>I', zlib.crc32(name + data))
    return base64.b64encode(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b'\0\xff\xff\xff\xff')) + chunk(b'IEND', b'')).decode()
for line in sys.stdin:
    request = json.loads(line)
    if 'id' not in request:
        continue
    method = request.get('method')
    result = {}
    if method == 'initialize':
        result = {'protocolVersion': '2025-06-18', 'capabilities': {'tools': {}}, 'serverInfo': {'name': 'Offline Godot fixture', 'version': '1'}}
    elif method == 'tools/list':
        result = {'tools': [{'name': name, 'inputSchema': {'type': 'object', 'properties': {'mode': {'type': 'string'}} if name == 'play_scene' else {}, 'required': []}} for name in ['get_project_info', 'play_scene', 'stop_scene', 'reload_project', 'get_game_screenshot']]}
    elif method == 'tools/call':
        name = request['params']['name']
        if name == 'get_project_info':
            connection_attempts += 1
            if connection_attempts == 1:
                result = {'isError': True, 'content': [{'type': 'text', 'text': 'Editor reconnect pending'}]}
            else:
                result = {'content': [{'type': 'text', 'text': json.dumps({'project_path': os.environ.get('LUMEN_TEST_EDITOR_ROOT', str(root))})}]}
        else:
            with (root / 'visual-action-log.txt').open('a', encoding='utf8') as stream:
                stream.write(name + '\n')
                if name == 'play_scene':
                    stream.write(json.dumps(request['params']['arguments']) + '\n')
            if name == 'get_game_screenshot':
                result = {'content': [{'type': 'image', 'mimeType': 'image/png', 'data': png()}]}
            else:
                result = {'content': [{'type': 'text', 'text': 'Offline fixture action completed'}]}
    print(json.dumps({'jsonrpc': '2.0', 'id': request['id'], 'result': result}), flush=True)
