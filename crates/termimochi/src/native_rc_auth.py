"""App-owned control policy, copied into one private session root.
Not imported theme code. TTY requests and arbitrary config/code paths are denied.
"""
import os


def is_cmd_allowed(pcmd, window, from_socket, extra_data):
    del window, extra_data
    payload = pcmd.get('payload') or {}
    return bool(from_socket and pcmd.get('cmd') == 'load-config'
                and payload.get('paths') == [os.path.join(os.path.dirname(__file__), 'kitty.conf')]
                and not payload.get('ignore_overrides')
                and not payload.get('override'))
