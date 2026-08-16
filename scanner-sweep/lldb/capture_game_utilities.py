"""Capture SC2's GameUtilities request and response plaintext in live LLDB.

The request hook records the bytes immediately before TLS.  The response hook
copies the bytes consumed by the generated ``ClientResponse`` protobuf parser
after TLS.  Together they record both halves of the Front -> Sunken bootstrap
without intercepting TLS or invoking code inside the stopped process.
Import after attaching Intel LLDB to SC2 97563.  The output is private because
attributes contain account/session material.
"""

from __future__ import annotations

import datetime
import json
import os
import signal
import threading

import lldb


OUTPUT_PATH = os.environ.get(
    "SC2_GAME_UTILITIES_LOG", "/tmp/sc2-game-utilities.jsonl"
)
EXPECTED_UUID = "45E86456-546D-388B-B8B3-00E7F2B24655"
CLIENT_REQUEST_SERIALIZE_TO_ARRAY = 0x00000001031CCDE0
CLIENT_RESPONSE_MERGE_PARTIAL_FROM_CODED_STREAM = 0x00000001031CD9C0
MAX_MESSAGE_SIZE = 1024 * 1024
STOP_AFTER_CAPTURE = os.environ.get(
    "SC2_GAME_UTILITIES_STOP_AFTER_CAPTURE", "0"
) == "1"
STOP_DELAY_SECONDS = float(os.environ.get(
    "SC2_GAME_UTILITIES_STOP_DELAY_SECONDS", "0"
))
CAPTURE_REQUEST = os.environ.get(
    "SC2_GAME_UTILITIES_CAPTURE_REQUEST", "1"
) == "1"
HARDWARE_RESPONSE_BREAKPOINT = os.environ.get(
    "SC2_GAME_UTILITIES_HARDWARE_RESPONSE_BREAKPOINT", "0"
) == "1"
HARDWARE_RETURN_BREAKPOINT = os.environ.get(
    "SC2_GAME_UTILITIES_HARDWARE_RETURN_BREAKPOINT", "0"
) == "1"

_lock = threading.Lock()
_pending: dict[int, dict[str, int]] = {}
_response_pending: dict[int, dict[str, int]] = {}
_sequence = 0
_captured_request = False
_captured_response = False
_debugger = None


def _append(record: dict) -> None:
    global _sequence
    with _lock:
        _sequence += 1
        line = json.dumps(
            {
                "sequence": _sequence,
                "timestamp": datetime.datetime.now(
                    datetime.timezone.utc
                ).isoformat(),
                **record,
            },
            separators=(",", ":"),
        ) + "\n"
        descriptor = os.open(
            OUTPUT_PATH, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600
        )
        try:
            os.write(descriptor, line.encode("utf-8"))
        finally:
            os.close(descriptor)


def _register(frame, name: str) -> int:
    value = frame.FindRegister(name)
    if not value.IsValid():
        raise RuntimeError(f"register {name} is unavailable")
    return value.GetValueAsUnsigned()


def _read_memory(process, address: int, length: int) -> bytes:
    if not address or length <= 0 or length > MAX_MESSAGE_SIZE:
        raise RuntimeError(f"invalid serialized message length {length}")
    error = lldb.SBError()
    data = process.ReadMemory(address, length, error)
    if not error.Success():
        raise RuntimeError(error.GetCString() or "ReadMemory failed")
    return bytes(data)


def _read_pointer(process, address: int) -> int:
    error = lldb.SBError()
    value = process.ReadPointerFromMemory(address, error)
    if not error.Success():
        raise RuntimeError(error.GetCString() or "ReadPointerFromMemory failed")
    return value


def serializer_entry(frame, _bp_loc, _internal_dict):
    try:
        thread = frame.GetThread()
        process = thread.GetProcess()
        target = process.GetTarget()
        stack_pointer = _register(frame, "rsp")
        error = lldb.SBError()
        return_address = process.ReadPointerFromMemory(stack_pointer, error)
        if not error.Success():
            raise RuntimeError(error.GetCString() or "could not read return address")

        breakpoint = _return_breakpoint(target, return_address)
        breakpoint.SetOneShot(True)
        breakpoint.SetThreadID(thread.GetThreadID())
        breakpoint.SetScriptCallbackFunction(__name__ + ".serializer_return")
        _pending[breakpoint.GetID()] = {
            "start": _register(frame, "rsi"),
        }
    except Exception as error:
        print(f"[sc2-game-utilities] serializer entry: {error}")
    return False


def serializer_return(frame, bp_loc, _internal_dict):
    global _captured_request
    try:
        pending = _pending.pop(bp_loc.GetBreakpoint().GetID(), None)
        if pending is None:
            return False
        end = _register(frame, "rax")
        length = end - pending["start"]
        process = frame.GetThread().GetProcess()
        data = _read_memory(process, pending["start"], length)
        _append(
            {
                "type": "game_utilities_client_request",
                "pid": process.GetProcessID(),
                "thread_id": frame.GetThread().GetThreadID(),
                "length": length,
                "hex": data.hex(),
            }
        )
        _captured_request = True
        print(
            "[sc2-game-utilities] captured ClientRequest "
            f"({length} bytes; payload written only to private log)"
        )
    except Exception as error:
        print(f"[sc2-game-utilities] serializer return: {error}")
    return False


def client_response_parser_entry(frame, bp_loc, _internal_dict):
    entry_breakpoint = bp_loc.GetBreakpoint()
    entry_disabled = False
    try:
        if _captured_response:
            return False
        thread = frame.GetThread()
        process = thread.GetProcess()
        target = process.GetTarget()
        stream_address = _register(frame, "rsi")
        start_address = _read_pointer(process, stream_address + 0x8)
        stack_pointer = _register(frame, "rsp")
        return_address = _read_pointer(process, stack_pointer)
        if HARDWARE_RETURN_BREAKPOINT:
            entry_breakpoint.SetEnabled(False)
            entry_disabled = True
        breakpoint = _return_breakpoint(target, return_address)
        breakpoint.SetOneShot(True)
        breakpoint.SetThreadID(thread.GetThreadID())
        breakpoint.SetScriptCallbackFunction(
            __name__ + ".client_response_parser_return"
        )
        _response_pending[breakpoint.GetID()] = {
            "stream": stream_address,
            "start": start_address,
        }
    except Exception as error:
        if entry_disabled:
            entry_breakpoint.SetEnabled(True)
        print(f"[sc2-game-utilities] ClientResponse parser entry: {error}")
    return False


def client_response_parser_return(frame, bp_loc, _internal_dict):
    global _captured_response
    process = frame.GetThread().GetProcess()
    try:
        pending = _response_pending.pop(
            bp_loc.GetBreakpoint().GetID(), None
        )
        if pending is None:
            return False
        end_address = _read_pointer(process, pending["stream"] + 0x8)
        length = end_address - pending["start"]
        data = _read_memory(process, pending["start"], length)
        _append(
            {
                "type": "game_utilities_client_response",
                "pid": process.GetProcessID(),
                "thread_id": frame.GetThread().GetThreadID(),
                "length": length,
                "hex": data.hex(),
            }
        )
        _captured_response = True
        print(
            "[sc2-game-utilities] captured ClientResponse "
            f"({length} bytes; payload written only to private log)"
        )
        if STOP_AFTER_CAPTURE:
            return True
        if STOP_DELAY_SECONDS > 0:
            timer = threading.Timer(
                STOP_DELAY_SECONDS,
                lambda: os.kill(process.GetProcessID(), signal.SIGSTOP),
            )
            timer.daemon = True
            timer.start()
            print(
                "[sc2-game-utilities] safe detach timer: "
                f"{STOP_DELAY_SECONDS:g} seconds after ClientResponse"
            )
    except Exception as error:
        print(f"[sc2-game-utilities] ClientResponse parser return: {error}")
    return False


def _loaded_address(target, module, file_address: int) -> int:
    address = module.ResolveFileAddress(file_address).GetLoadAddress(target)
    if address == lldb.LLDB_INVALID_ADDRESS:
        raise RuntimeError(f"could not resolve file address 0x{file_address:x}")
    return address


def _hardware_breakpoint(debugger, target, address: int):
    previous_count = target.GetNumBreakpoints()
    result = lldb.SBCommandReturnObject()
    debugger.GetCommandInterpreter().HandleCommand(
        f"breakpoint set --hardware --address 0x{address:x}", result
    )
    if not result.Succeeded() or target.GetNumBreakpoints() != previous_count + 1:
        message = result.GetError() or result.GetOutput() or "unknown LLDB error"
        raise RuntimeError(
            f"could not create hardware breakpoint at 0x{address:x}: "
            f"{message.strip()}"
        )
    breakpoint = target.GetBreakpointAtIndex(previous_count)
    if not breakpoint.IsHardware():
        raise RuntimeError(f"breakpoint at 0x{address:x} is not hardware-backed")
    return breakpoint


def _return_breakpoint(target, address: int):
    if not HARDWARE_RETURN_BREAKPOINT:
        return target.BreakpointCreateByAddress(address)
    if _debugger is None:
        raise RuntimeError("LLDB debugger is unavailable")
    return _hardware_breakpoint(_debugger, target, address)


def __lldb_init_module(debugger, _internal_dict):
    global _debugger
    _debugger = debugger
    target = debugger.GetSelectedTarget()
    process = target.GetProcess() if target.IsValid() else None
    if not target.IsValid() or process is None or not process.IsValid():
        print("[sc2-game-utilities] attach to SC2 before importing this module")
        return
    if not target.GetTriple().startswith("x86_64"):
        print(f"[sc2-game-utilities] unsupported architecture: {target.GetTriple()}")
        return

    module = target.GetModuleAtIndex(0)
    module_uuid = (module.GetUUIDString() or "").upper()
    if module_uuid != EXPECTED_UUID:
        print(
            "[sc2-game-utilities] unsupported SC2 UUID "
            f"{module_uuid or '(missing)'}; expected {EXPECTED_UUID}"
        )
        return

    address = None
    breakpoint = None
    if CAPTURE_REQUEST:
        address = _loaded_address(
            target, module, CLIENT_REQUEST_SERIALIZE_TO_ARRAY
        )
        breakpoint = target.BreakpointCreateByAddress(address)
        breakpoint.SetScriptCallbackFunction(__name__ + ".serializer_entry")
    response_callback_address = _loaded_address(
        target, module, CLIENT_RESPONSE_MERGE_PARTIAL_FROM_CODED_STREAM
    )
    response_breakpoint = (
        _hardware_breakpoint(debugger, target, response_callback_address)
        if HARDWARE_RESPONSE_BREAKPOINT
        else target.BreakpointCreateByAddress(response_callback_address)
    )
    response_breakpoint.SetScriptCallbackFunction(
        __name__ + ".client_response_parser_entry"
    )
    _append(
        {
            "type": "capture_start",
            "pid": process.GetProcessID(),
            "sc2_uuid": module_uuid,
            "hook_address": f"0x{address:x}" if address is not None else None,
            "response_hook_address": f"0x{response_callback_address:x}",
            "hardware_response_breakpoint": HARDWARE_RESPONSE_BREAKPOINT,
            "hardware_return_breakpoint": HARDWARE_RETURN_BREAKPOINT,
        }
    )
    print(f"[sc2-game-utilities] output: {OUTPUT_PATH}")
    if breakpoint is not None:
        print(
            "[sc2-game-utilities] ClientRequest serializer breakpoint: "
            f"{breakpoint.GetNumLocations()}"
        )
    else:
        print("[sc2-game-utilities] ClientRequest capture: disabled")
    print(
        "[sc2-game-utilities] ClientResponse parser breakpoint: "
        f"{response_breakpoint.GetNumLocations()}"
    )
