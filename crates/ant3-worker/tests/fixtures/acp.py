import json
import os
import sys

label = sys.argv[1]
received = []
active = None


def emit(**fields):
    print(json.dumps({"jsonrpc": "2.0", **fields}), flush=True)


def response(request_id, **result):
    emit(id=request_id, result=result)


def update(kind, **fields):
    emit(method="session/update", params={
        "sessionId": "fixture", "update": {"sessionUpdate": kind, **fields},
    })


for line in sys.stdin:
    message = json.loads(line)
    method = message["method"]
    if method == "initialize":
        assert message["params"]["clientCapabilities"] == {}
        response(message["id"], protocolVersion=1, authMethods=[{"id": "fixture"}])
    elif method == "authenticate":
        assert message["params"]["methodId"] == "fixture"
        response(message["id"])
    elif method == "session/new":
        assert message["params"] == {"cwd": os.getcwd(), "mcpServers": []}
        response(message["id"], sessionId="fixture")
    elif method == "session/prompt":
        assert message["params"]["sessionId"] == "fixture"
        prompt = message["params"]["prompt"][0]["text"]
        if prompt == "disconnect":
            sys.exit(0)
        if prompt == "wait":
            active = message["id"]
            update("plan", entries=[])
            continue
        emit(id="permission", method="session/request_permission", params={
            "sessionId": "fixture", "options": [
                {"kind": "allow_once", "optionId": "allow"},
                {"kind": "reject_once", "optionId": "reject"},
            ],
        })
        assert json.loads(next(sys.stdin))["result"]["outcome"]["optionId"] == "reject"
        received.append(prompt)
        update("agent_message_chunk", content={"type": "text", "text": label + ": "})
        update("agent_message_chunk", content={"type": "text", "text": ", ".join(received)})
        response(message["id"], stopReason="end_turn")
    elif method == "session/cancel":
        assert active is not None
        assert message["params"]["sessionId"] == "fixture"
        response(active, stopReason="cancelled")
    else:
        raise AssertionError(method)
