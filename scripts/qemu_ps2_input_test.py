#!/usr/bin/env python3
"""Drive real emulated PS/2 keyboard and mouse devices through QEMU QMP."""
import argparse
import json
import os
import socket
import sys
import time


def read_message(stream):
    while True:
        line = stream.readline()
        if not line:
            raise RuntimeError("QMP connection closed")
        message = json.loads(line)
        if "event" not in message:
            return message


def command(stream, name, arguments=None):
    payload = {"execute": name}
    if arguments is not None:
        payload["arguments"] = arguments
    stream.write((json.dumps(payload) + "\r\n").encode())
    stream.flush()
    response = read_message(stream)
    if "error" in response:
        raise RuntimeError(f"QMP {name} failed: {response['error']}")
    return response.get("return")


def wait_for_text(path, expected, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with open(path, encoding="utf-8", errors="replace") as log:
                content = log.read()
        except FileNotFoundError:
            content = ""
        if expected in content:
            return
        if "SYSTEMES_KERNEL_FAIL:" in content:
            raise RuntimeError("Kernel reported failure before the expected QEMU marker")
        time.sleep(0.1)
    raise TimeoutError(f"Timed out waiting for kernel marker: {expected}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--socket", required=True)
    parser.add_argument("--debug-log", required=True)
    args = parser.parse_args()

    deadline = time.monotonic() + 15
    connection = None
    while time.monotonic() < deadline:
        try:
            connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            connection.connect(args.socket)
            break
        except OSError:
            if connection:
                connection.close()
            connection = None
            time.sleep(0.1)
    if connection is None:
        raise TimeoutError("QEMU QMP socket did not become available")

    with connection:
        stream = connection.makefile("rwb", buffering=0)
        greeting = read_message(stream)
        if "QMP" not in greeting:
            raise RuntimeError(f"Unexpected QMP greeting: {greeting}")
        command(stream, "qmp_capabilities")
        wait_for_text(args.debug_log, "SYSTEMES_TEST: PS2_INPUT_READY", 15)

        # Send explicit key-down/key-up events through QEMU's input layer.
        # This exercises the emulated keyboard device rather than a host-side
        # terminal shortcut, and gives the kernel a complete make/break pair.
        command(stream, "input-send-event", {"events": [
            {"type": "key", "data": {
                "down": True, "key": {"type": "qcode", "data": "a"},
            }},
        ]})
        time.sleep(0.12)
        command(stream, "input-send-event", {"events": [
            {"type": "key", "data": {
                "down": False, "key": {"type": "qcode", "data": "a"},
            }},
        ]})

        # Move the PS/2 mouse while pressing left, then release it in a second packet.
        command(stream, "input-send-event", {"events": [
            {"type": "rel", "data": {"axis": "x", "value": 23}},
            {"type": "rel", "data": {"axis": "y", "value": 11}},
            {"type": "btn", "data": {"down": True, "button": "left"}},
        ]})
        time.sleep(0.15)
        command(stream, "input-send-event", {"events": [
            {"type": "btn", "data": {"down": False, "button": "left"}},
        ]})

        wait_for_text(args.debug_log,
                      "SYSTEMES_TEST: PS/2 IRQ input end-to-end verified", 12)
        with open(args.debug_log, encoding="utf-8", errors="replace") as log:
            content = log.read()
        required = [
            "SYSTEMES_TEST: PS/2 keyboard make/break consumed",
            "SYSTEMES_TEST: PS/2 mouse motion and button release consumed",
            "SYSTEMES_TEST: PS/2 IRQ input end-to-end verified",
        ]
        missing = [marker for marker in required if marker not in content]
        if missing:
            raise RuntimeError(f"Missing kernel evidence: {missing}")
        print("QEMU PS/2 keyboard and mouse events reached the kernel and were consumed.")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"PS/2 QEMU integration test failed: {error}", file=sys.stderr)
        sys.exit(1)
