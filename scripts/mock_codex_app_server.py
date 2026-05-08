#!/usr/bin/env python3
import json
import os
import sys


def send(payload):
    sys.stdout.write(json.dumps(payload, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def read_message():
    line = sys.stdin.readline()
    if not line:
        return None
    return json.loads(line)


def main():
    issue_id = os.environ.get("SYMPHONY_TEST_ISSUE_ID")
    tool_request_id = 10_000
    for message in iter(read_message, None):
        method = message.get("method")
        request_id = message.get("id")
        if method == "initialize":
            send(
                {
                    "id": request_id,
                    "result": {
                        "userAgent": "symphony-mock/0.1",
                        "codexHome": os.getcwd(),
                        "platformFamily": "unix",
                        "platformOs": "mock",
                    },
                }
            )
        elif method == "thread/start":
            send(
                {
                    "id": request_id,
                    "result": {
                        "thread": {
                            "id": "mock-thread",
                            "cwd": os.getcwd(),
                            "createdAt": 0,
                            "updatedAt": 0,
                            "cliVersion": "mock",
                            "ephemeral": True,
                            "modelProvider": "mock",
                            "preview": "",
                            "sessionId": "mock-session",
                            "source": {"type": "user"},
                            "status": {"type": "active", "activeFlags": []},
                            "turns": [],
                        },
                        "approvalPolicy": "never",
                        "approvalsReviewer": "user",
                        "cwd": os.getcwd(),
                        "model": "mock",
                        "modelProvider": "mock",
                        "sandbox": {"mode": "workspace-write"},
                    },
                }
            )
        elif method == "turn/start":
            turn_id = "mock-turn"
            send(
                {
                    "id": request_id,
                    "result": {
                        "turn": {
                            "id": turn_id,
                            "items": [],
                            "status": {"type": "running"},
                        }
                    },
                }
            )
            if issue_id:
                for action in [
                    {
                        "action": "comment",
                        "issue_id": issue_id,
                        "body": "Symphony mock app-server processed this issue.",
                    },
                    {
                        "action": "set_state",
                        "issue_id": issue_id,
                        "state": "Done",
                    },
                ]:
                    tool_request_id += 1
                    send(
                        {
                            "id": tool_request_id,
                            "method": "item/tool/call",
                            "params": {
                                "threadId": "mock-thread",
                                "turnId": turn_id,
                                "callId": f"call-{tool_request_id}",
                                "tool": "github_issue",
                                "arguments": action,
                            },
                        }
                    )
                    read_message()
            send(
                {
                    "method": "turn/completed",
                    "params": {
                        "turn": {
                            "id": turn_id,
                            "items": [],
                            "status": {"type": "completed"},
                        },
                        "usage": {
                            "input_tokens": 1,
                            "output_tokens": 1,
                        },
                    },
                }
            )


if __name__ == "__main__":
    main()
