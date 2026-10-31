#!/usr/bin/env python3
"""A scripted stand-in for the model server, for testing the harness without a GPU.

    python3 fake_server.py --port 8099 &
    python3 run.py run --no-server --port 8099 --workdir /tmp/walk-test
    cargo run --release -- run --no-server --port 8099 --workdir /tmp/walk-test-rs

It answers /v1/models and /v1/chat/completions (OpenAI shape) with a fixed
policy that reads the brief, so the Python and Rust harnesses get the same
replies and should write the same files. On the way it exercises every path
the walk has: a survey, tool calls before a save, a reply with no tool call
(a nudge), malformed tool-call JSON, a round that overflows the budget and is
retried, a skipped test file, and the switch from notes to tests.

It also checks the harness: every request must hold a single brief (system +
user + this round's turns), never a growing history. A violation is answered
with HTTP 500, which stops the run.
"""

import argparse
import json
import re
from http.server import BaseHTTPRequestHandler, HTTPServer

START = ["vba:CairnForms.QuoteToOrder", "query:qryCommissionDetail", "vba:CairnForms.NextNumber", "table:Quotes"]
RULES = ("QuoteToOrder", "NextNumber", "qryCommissionDetail", "qryOrderBalances")
NUDGE_UNIT = "table:Quotes"          # first reply has no tool call
BAD_JSON_UNIT = "table:OrderLines"   # first tool call has malformed arguments
OVERFLOW_UNIT = "table:Orders"       # first attempt reports a prompt far over budget
MAX_ROUND_MESSAGES = 12

seen_overflow = False


def call(name, args, raw=False):
    return {"id": f"call_{name}", "type": "function",
            "function": {"name": name, "arguments": args if raw else json.dumps(args)}}


def reply(tool_calls=None, content="", prompt_tokens=500):
    msg = {"role": "assistant", "content": content}
    if tool_calls:
        msg["tool_calls"] = tool_calls
    return {"choices": [{"message": msg, "finish_reason": "tool_calls" if tool_calls else "stop"}],
            "usage": {"prompt_tokens": prompt_tokens, "completion_tokens": 50}}


def policy(messages):
    global seen_overflow
    brief = messages[1]["content"]
    turns = sum(1 for m in messages if m["role"] == "assistant")
    if "This is the survey round" in brief:
        return reply([call("save_notes", {"text": "Overview: quotes, orders, commission.", "follow": START})])
    unit = re.search(r"^Unit: (\S+)", brief, re.M).group(1)
    tests = "Phase: tests" in brief
    if unit == OVERFLOW_UNIT and not tests and not seen_overflow:
        seen_overflow = True
        return reply([call("sql", {"query": "SELECT COUNT(*) FROM Orders"})], prompt_tokens=50_000)
    if turns == 0:
        if unit == NUDGE_UNIT and not tests:
            return reply(content="Let me think about this table.")
        if unit == BAD_JSON_UNIT and not tests:
            return reply([call("get_schema", '{"table": "OrderLines"', raw=True)])
        if unit.startswith("vba:"):
            name = unit.rsplit(".", 1)[-1]
            return reply([call("search_vba", {"pattern": f"\\b{name}\\b"})])
        return reply([call("get_schema" if unit.startswith("table:") else "get_saved_query",
                           {"table" if unit.startswith("table:") else "name": unit.split(":", 1)[1]})])
    if tests:
        if unit.endswith("NextNumber"):
            return reply([call("save_tests", {"code": "", "skip": "covered by QuoteToOrder"})])
        fn = re.sub(r"[^a-z0-9]+", "_", unit.lower())
        return reply([call("save_tests", {"code": f"#[test]\nfn {fn}_rule() {{\n    assert!(true);\n}}"})])
    new = re.search(r"^Not yet seen \(candidates to follow\): (.+)$", brief, re.M)
    follow = new.group(1).split(", ")[:2] if new else []
    return reply([call("save_notes", {"text": f"Notes for {unit}.", "follow": follow,
                                      "has_rules": unit.endswith(RULES)})])


class Handler(BaseHTTPRequestHandler):
    def _send(self, code, body):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self._send(200, {"data": [{"id": "fake-model"}]})

    def do_POST(self):
        req = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        msgs = req["messages"]
        problems = []
        if [m["role"] for m in msgs[:2]] != ["system", "user"]:
            problems.append("a round must start with system + user")
        if sum(1 for m in msgs if m["role"] == "user" and "Phase:" in (m.get("content") or "")) != 1:
            problems.append("exactly one brief per request")
        if len(msgs) > MAX_ROUND_MESSAGES:
            problems.append(f"{len(msgs)} messages: history is growing")
        if problems:
            return self._send(500, {"error": "; ".join(problems)})
        self._send(200, policy(msgs))

    def log_message(self, *_):
        pass


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8099)
    HTTPServer(("127.0.0.1", ap.parse_args().port), Handler).serve_forever()
