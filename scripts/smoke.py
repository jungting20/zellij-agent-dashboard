#!/usr/bin/env python3
"""Run Zellij integration checks in owned temporary sessions only."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import shutil
import signal
import subprocess
import struct
import termios
import time
import uuid

ROOT = Path(__file__).resolve().parent.parent
ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")


class Client:
    def __init__(self, command, directory):
        self.pid, self.fd = pty.fork()
        self.closed = False
        self.text = ""
        self.capture = ""
        if self.pid != 0:
            fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 120, 0, 0))
        if self.pid == 0:
            environment = os.environ.copy()
            for key in ("ZELLIJ", "ZELLIJ_SESSION_NAME", "ZELLIJ_PANE_ID", "CLAUDECODE"):
                environment.pop(key, None)
            environment["TERM"] = "xterm-256color"
            os.chdir(directory)
            os.execvpe(command[0], command, environment)

    def pump(self):
        if self.closed:
            return
        for _ in range(16):
            if not select.select([self.fd], [], [], 0)[0]:
                break
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                break
            if not data:
                break
            decoded = data.decode(errors="replace")
            self.text += decoded
            self.capture += decoded
            self.capture = self.capture[-2_000_000:]
        self.text = self.text[-200000:]
        plain = ANSI.sub("", self.text).lower()
        if "permission" in plain and ("(y/n)" in plain or "(y)" in plain or "[y]" in plain):
            os.write(self.fd, b"y")
            self.text = ""

    def stop(self):
        if self.closed:
            return
        self.closed = True
        try:
            os.kill(self.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            deadline = time.monotonic() + 1
            while time.monotonic() < deadline:
                if os.waitpid(self.pid, os.WNOHANG)[0]:
                    break
                time.sleep(0.05)
            else:
                os.kill(self.pid, signal.SIGKILL)
                # Some macOS Zellij clients remain in kernel teardown after a
                # signal. Do not let temporary-test cleanup wait indefinitely.
                os.waitpid(self.pid, os.WNOHANG)
        except (ChildProcessError, ProcessLookupError):
            pass
        finally:
            os.close(self.fd)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--real-claude", action="store_true", help="also start Claude without submitting a model request")
    args = parser.parse_args()
    zellij = shutil.which("zellij")
    assert zellij, "zellij is required"
    host = ROOT / "dist/dashboard-host"
    assert host.exists(), "run scripts/build.sh first"
    run_id = uuid.uuid4().hex[:10]
    directory = ROOT / ".local" / f"smoke-{run_id}"
    directory.mkdir(parents=True)
    state = directory / "state"
    wasm = directory / "dashboard.wasm"
    shutil.copy2(ROOT / "dist/agent-dashboard.wasm", wasm)
    config = directory / "config"
    (config / "layouts").mkdir(parents=True)
    (config / "layouts/test.kdl").write_text("layout {\n    pane\n}\n")
    (config / "config.kdl").write_text(f'''default_layout "test"
show_startup_tips false
show_release_notes false
load_plugins {{
    "file:{wasm}" {{
        mode "collector"
        host_path "{host}"
        state_dir "{state}"
    }}
}}
''')
    sessions = [f"zad-smoke-{run_id}", f"zad smoke {run_id}"]
    clients = []
    base_config = f"host_path={host},state_dir={state}"

    def call(session, *command, timeout=8):
        for client in clients:
            client.pump()
        result = subprocess.run([zellij, "--session", session, *command], stdin=subprocess.DEVNULL,
                                capture_output=True, text=True, timeout=timeout)
        if result.returncode:
            raise RuntimeError(f"Zellij {command[0]} failed: {result.stderr.strip()}")
        return result.stdout

    def host_call(*command, input=None):
        return json.loads(subprocess.check_output([str(host), "--state-dir", str(state), *command],
                                                 input=input, text=True))

    def ping(session, mode="collector", plugin=wasm):
        output = call(session, "pipe", "--plugin", f"file:{plugin}", "--plugin-configuration",
                      f"mode={mode},{base_config}", "--name", "agent-dashboard-ping", "--", "ping", timeout=3)
        return [json.loads(line) for line in output.splitlines() if line.strip().startswith("{")]

    def wait_for(description, check, seconds=25):
        deadline = time.monotonic() + seconds
        last_error = ""
        while time.monotonic() < deadline:
            for client in clients:
                client.pump()
            try:
                result = check()
                if result:
                    print(f"PASS {description}", flush=True)
                    return result
            except (RuntimeError, subprocess.TimeoutExpired, FileNotFoundError) as error:
                last_error = str(error)
            time.sleep(0.1)
        (directory / "terminal.log").write_text("\n".join(c.capture for c in clients))
        raise AssertionError(f"{description}: {last_error}; artifacts: {directory}")

    try:
        for session in sessions:
            clients.append(Client([zellij, "--config-dir", str(config), "--session", session], directory))
            wait_for(f"collector loaded in {session}", lambda: any(p["permissions"] for p in ping(session)))
        first = sessions[0]
        pane = call(first, "plugin", "--configuration", f"mode=dashboard,{base_config}", "--", f"file:{wasm}").strip()
        wait_for("dashboard loaded", lambda: any(p["permissions"] and p["revision"] is not None for p in ping(first, "dashboard")))
        call(first, "action", "close-pane", "--pane-id", pane)
        before = max(p["polls"] for p in ping(first))
        wait_for("collector survives dashboard pane close", lambda: max(p["polls"] for p in ping(first)) > before)
        clients.append(Client([zellij, "attach", first], directory))
        wait_for("collector with two attached clients", lambda: any(p["permissions"] for p in ping(first)))

        # A fixture executable named codex tests discovery without inference.
        fake = directory / "codex"
        source = directory / "codex.rs"
        source.write_text("fn main() { std::thread::sleep(std::time::Duration::from_secs(300)); }\n")
        rustup = os.environ.get("ZAD_RUSTUP", str(Path.home() / ".cargo/bin/rustup"))
        subprocess.run([rustup, "run", "1.88.0", "rustc", str(source), "-o", str(fake)], check=True)
        for session in sessions:
            call(session, "action", "new-pane", "--name", "discovery-test", "--", str(fake), "300")
        def discovered():
            return [a for a in host_call("snapshot")["agents"] if a["identity"]["session_name"] in sessions and a["tool"] == "codex"]
        rows = wait_for("discovery across two sessions including spaces", lambda: discovered() if len(discovered()) == 2 else None)
        # Exercise keyboard search and the actual cross-session focus API.
        target = next(a for a in rows if a["identity"]["session_name"] == sessions[1])
        def attached(session):
            return [line.split() for line in call(session, "action", "list-clients").splitlines()
                    if re.match(r"^\d+\s+", line)]
        target_clients = len(attached(sessions[1]))
        focus_ui = call(first, "plugin", "--configuration", f"mode=dashboard,{base_config}", "--", f"file:{wasm}").strip()
        wait_for("focus dashboard loaded", lambda: any(p["revision"] is not None for p in ping(first, "dashboard")))
        for client in (clients[0], clients[2]):
            os.write(client.fd, f"/{sessions[1]}\r".encode())
        wait_for("keyboard search selects requested agent", lambda: any(p["selected_id"] == target["identity"]["agent_id"]
                 and p["query"] == sessions[1] for p in ping(first, "dashboard")))
        for client in (clients[0], clients[2]):
            os.write(client.fd, b"\r")
        wait_for("Enter focuses agent in other session", lambda: len(attached(sessions[1])) > target_clients
                 and any(row[1] == f'terminal_{target["identity"]["pane_id"]}' for row in attached(sessions[1])))
        clients.append(Client([zellij, "attach", first], directory))
        wait_for("collector available after focus", lambda: any(p["permissions"] for p in ping(first)))
        call(first, "action", "close-pane", "--pane-id", focus_ui)
        row = next(a for a in rows if a["identity"]["session_name"] == first)
        event = {"schema_version":1, "event_id":"smoke-event", "identity":row["identity"], "tool":"codex",
                 "kind":"turn_started", "sequence":1, "observed_at_ms":int(time.time()*1000),
                 "summary":"한글, 여러 줄\n테스트", "cwd":str(directory)}
        first_result = host_call("ingest", input=json.dumps(event))
        second_result = host_call("ingest", input=json.dumps(event))
        assert first_result["applied"] and not second_result["applied"]
        print("PASS duplicate event persisted once", flush=True)
        resolved = host_call("resolve", row["identity"]["agent_id"])
        assert resolved["identity"] == row["identity"]
        call(resolved["identity"]["session_name"], "action", "close-pane", "--pane-id", f'terminal_{resolved["identity"]["pane_id"]}')
        result = subprocess.run([str(host), "--state-dir", str(state), "resolve", row["identity"]["agent_id"]], capture_output=True)
        assert result.returncode == 1
        print("PASS stale pane identity rejected", flush=True)

        if args.real_claude:
            claude = shutil.which("claude")
            assert claude, "claude is required for --real-claude"
            settings = directory / "claude-settings.json"
            settings.write_text(json.dumps(host_call("hook-config")))
            # Keep onboarding/trust records inside this test, outside user settings.
            claude_config = directory / "claude-config"
            claude_config.mkdir(mode=0o700)
            (claude_config / ".claude.json").write_text(json.dumps({"hasCompletedOnboarding": True}))
            claude_pane = call(first, "action", "new-pane", "--name", "real-claude-hook", "--cwd", str(ROOT), "--",
                 "/usr/bin/env", f"CLAUDE_CONFIG_DIR={claude_config}", claude,
                 "--setting-sources", "", "--settings", str(settings)).strip()
            def reported():
                screen = call(first, "action", "dump-screen", "--pane-id", claude_pane)
                if "Yes, I trust this folder" in screen:
                    keys = ["27", "91", "66", "13"] if re.search(r"❯\s*(?:\d\.\s*)?No", screen) else ["13"]
                    call(first, "action", "write", "--pane-id", claude_pane, *keys)
                return any(a["tool"] == "claude" and a["last_report_ms"] is not None
                           for a in host_call("snapshot")["agents"] if a["identity"]["session_name"] == first)
            wait_for("real Claude SessionStart hook", reported, seconds=40)

        for client in clients:
            client.stop()
        before = max(p["polls"] for p in ping(first))
        wait_for("collector survives all clients detaching", lambda: max(p["polls"] for p in ping(first)) > before)
        # Zellij 0.45.0 explicitly rejects plugin reload with no attached client.
        clients.append(Client([zellij, "attach", first], directory))
        wait_for("collector available after reattach", lambda: any(p["revision"] is not None for p in ping(first)))
        revision = host_call("snapshot")["revision"]
        call(first, "action", "start-or-reload-plugin", f"file:{wasm}", "--configuration", f"mode=collector,{base_config}")
        wait_for("collector reload recovers stored state", lambda: any(p["revision"] is not None and p["revision"] >= revision for p in ping(first)))
        environment = os.environ.copy()
        environment["ZAD_STATE_DIR"] = str(state)
        subprocess.run([str(ROOT / "scripts/dashboard.sh"), first], env=environment,
                       stdin=subprocess.DEVNULL, capture_output=True, text=True, check=True, timeout=8)
        wait_for("launcher opens floating dashboard", lambda: any(p["revision"] is not None
                 for p in ping(first, "dashboard", ROOT / "dist/agent-dashboard.wasm")))
        (directory / "result.json").write_text(json.dumps({"passed":True, "sessions":sessions, "real_claude":args.real_claude}, indent=2))
        print(f"Artifacts: {directory}", flush=True)
    finally:
        for session in sessions:
            subprocess.run([zellij, "kill-session", session], stdin=subprocess.DEVNULL, capture_output=True, timeout=5)
        for client in clients:
            client.stop()


if __name__ == "__main__":
    main()
