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
import sqlite3
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
    parser.add_argument("--portable-paths", action="store_true", help="check aliases and ~/ host/state paths")
    args = parser.parse_args()
    zellij = shutil.which("zellij")
    assert zellij, "zellij is required"
    host = ROOT / "dist/dashboard-host"
    assert host.exists(), "run scripts/build.sh first"
    run_id = uuid.uuid4().hex[:10]
    directory = ROOT / ".local" / f"smoke-{run_id}"
    directory.mkdir(parents=True)
    state = directory / "state"
    state.mkdir(mode=0o700)
    # Seed the previous format before collectors start. Concurrent initialization
    # must import once while preserving the original bytes for rollback.
    legacy_bytes = json.dumps({"schema_version": 2, "revision": 17,
                               "last_scan_ms": 0, "agents": {}, "activities": []}).encode()
    (state / "store.json").write_bytes(legacy_bytes)
    wasm = directory / "dashboard.wasm"
    shutil.copy2(ROOT / "dist/agent-dashboard.wasm", wasm)
    host_path = str(host)
    state_path = str(state)
    plugin_url = f"file:{wasm}"
    alias_block = ""
    if args.portable_paths:
        host_path = "~/" + str(host.relative_to(Path.home()))
        state_path = "~/" + str(state.relative_to(Path.home()))
        plugin_url = "agent-dashboard"
        alias_block = f'plugins {{\n    agent-dashboard location="file:~/{wasm.relative_to(Path.home())}"\n}}\n'
    config = directory / "config"
    (config / "layouts").mkdir(parents=True)
    (config / "layouts/test.kdl").write_text("layout {\n    pane\n}\n")
    (config / "config.kdl").write_text(f'''default_layout "test"
show_startup_tips false
show_release_notes false
{alias_block}keybinds {{
    shared_except "locked" {{
        bind "Alt u" {{
            MessagePlugin "{plugin_url}" {{
                mode "collector"
                host_path "{host_path}"
                state_dir "{state_path}"
                name "agent-next"
                payload "pinned-only"
            }}
        }}
        bind "Alt i" {{
            MessagePlugin "{plugin_url}" {{
                mode "collector"
                host_path "{host_path}"
                state_dir "{state_path}"
                name "agent-next"
                payload "idle-and-pinned"
            }}
        }}
    }}
}}
load_plugins {{
    "{plugin_url}" {{
        mode "collector"
        host_path "{host_path}"
        state_dir "{state_path}"
    }}
}}
''')
    sessions = [f"zad-smoke-{run_id}", f"zad smoke {run_id}"]
    clients = []
    background_fixtures = []
    base_config = f"host_path={host_path},state_dir={state_path}"

    def call(session, *command, timeout=8):
        for client in clients:
            client.pump()
        environment = os.environ.copy()
        for key in ("ZELLIJ", "ZELLIJ_SESSION_NAME", "ZELLIJ_PANE_ID"):
            environment.pop(key, None)
        result = subprocess.run([zellij, "--session", session, *command], env=environment, stdin=subprocess.DEVNULL,
                                capture_output=True, text=True, timeout=timeout)
        if result.returncode and "already focused" not in result.stderr:
            raise RuntimeError(f"Zellij {command[0]} failed: {result.stderr.strip()}")
        return result.stdout

    def host_call(*command, input=None):
        return json.loads(subprocess.check_output([str(host), "--state-dir", str(state), *command],
                                                 input=input, text=True))

    def ping(session, mode="collector", plugin=wasm):
        # The CLI launcher resolves paths before configuring its dashboard.
        configuration = (f"host_path={host},state_dir={state}"
                         if plugin == ROOT / "dist/agent-dashboard.wasm" else base_config)
        output = call(session, "pipe", "--plugin", f"file:{plugin}", "--plugin-configuration",
                      f"mode={mode},{configuration}", "--name", "agent-dashboard-ping", "--", "ping", timeout=3)
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
        (directory / "navigation-debug.json").write_text(json.dumps(ping(sessions[0]), indent=2))
        (directory / "terminal.log").write_text("\n".join(c.capture for c in clients))
        if description.startswith("Enter focuses"):
            diagnostics = {}
            for session in sessions:
                diagnostics[session] = {"clients": call(session, "action", "list-clients"),
                                        "dashboard": ping(session, "dashboard") if session == sessions[0] else []}
            (directory / "focus-debug.json").write_text(json.dumps(diagnostics, indent=2))
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
        source.write_text(r'''use std::io::{BufRead, Write};
fn main() {
    println!("fixture ready\n› ");
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open("input.log").unwrap();
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        writeln!(log, "{line}").unwrap();
        log.flush().unwrap();
        let screen = match line.trim() {
            ":working" => Some("• Thinking (3s • esc to interrupt)\n› "),
            ":waiting" => Some("› task\nAllow command?"),
            ":idle" => Some("› "),
            ":overlay" => Some("› \n↑/↓ to scroll · PgUp/PgDn to scroll · Home/End to jump · q to quit · esc to edit prev"),
            ":unknown" => Some("ordinary output"),
            _ => None,
        };
        if let Some(screen) = screen {
            print!("\x1b[2J\x1b[H{screen}\n");
            std::io::stdout().flush().unwrap();
        }
    }
}
''')
        rustup = os.environ.get("ZAD_RUSTUP", str(Path.home() / ".cargo/bin/rustup"))
        subprocess.run([rustup, "run", "1.88.0", "rustc", str(source), "-o", str(fake)], check=True)
        for session in sessions:
            call(session, "action", "new-pane", "--name", "discovery-test", "--cwd", str(directory), "--", str(fake), "300")
        def discovered():
            return [a for a in host_call("snapshot")["agents"] if a["identity"]["session_name"] in sessions and a["tool"] == "codex"]
        rows = wait_for("discovery across two sessions including spaces", lambda: discovered() if len(discovered()) == 2 else None)
        screen_row = next(a for a in rows if a["identity"]["session_name"] == first)
        def current_screen_row():
            return next(a for a in discovered() if a["identity"] == screen_row["identity"])
        def show_fixture(command):
            pane_id = f'terminal_{screen_row["identity"]["pane_id"]}'
            # Clear any buffered keys from permission-dialog automation.
            call(first, "action", "write", "--pane-id", pane_id, "21")
            call(first, "action", "write-chars", "--pane-id", pane_id, command)
            call(first, "action", "write", "--pane-id", pane_id, "13")
        wait_for("hook-free fixture selects screen adapter", lambda: current_screen_row()["status_source"] == "screen")
        for command, status in [(":working", "working"), (":waiting", "waiting"), (":idle", "idle")]:
            show_fixture(command)
            wait_for(f"screen adapter detects {status}", lambda: current_screen_row()["status"] == status)
        show_fixture(":working")
        wait_for("screen working before overlay", lambda: current_screen_row()["status"] == "working")
        show_fixture(":overlay")
        before_screen = current_screen_row()["last_screen_report_ms"]
        wait_for("transcript overlay preserves working", lambda: current_screen_row()["last_screen_report_ms"] > before_screen
                 and current_screen_row()["status"] == "working")
        show_fixture(":unknown")
        wait_for("unmatched idle requires fresh confirmations", lambda: current_screen_row()["status"] == "idle")
        # Processes can outlive their pane while retaining its environment.
        orphan_environment = os.environ.copy()
        orphan_environment["ZELLIJ_SESSION_NAME"] = first
        orphan_environment["ZELLIJ_PANE_ID"] = "999999"
        for arguments in [[], ["app-server"]]:
            background_fixtures.append(subprocess.Popen([str(fake), *arguments], env=orphan_environment,
                                        cwd=directory, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                                        stderr=subprocess.DEVNULL))
        orphan, app_server = background_fixtures
        def missing_pane_record():
            host_call("scan")
            return next((a for a in host_call("snapshot")["agents"]
                         if a["identity"]["pid"] == orphan.pid and a["pane"]["presence"] == "missing"), None)
        missing = wait_for("live orphan process has confirmed missing pane", missing_pane_record)
        assert missing["liveness"] == "live"
        assert all(a["identity"]["pid"] != app_server.pid for a in host_call("snapshot")["agents"])
        rejected = subprocess.run([str(host), "--state-dir", str(state), "resolve", missing["identity"]["agent_id"]],
                                  capture_output=True, text=True)
        assert rejected.returncode == 1 and "pane no longer exists" in rejected.stderr
        print("PASS app-server excluded and missing pane focus rejected", flush=True)

        # Drive host adapters with a local fixture, never a model request.
        action_environment = os.environ.copy()
        # A caller's pane ID belongs to its own session, not this test session.
        for key in ("ZELLIJ", "ZELLIJ_SESSION_NAME", "ZELLIJ_PANE_ID"):
            action_environment.pop(key, None)
        action_environment["PATH"] = str(directory) + os.pathsep + action_environment.get("PATH", "")
        def host_action(request):
            return json.loads(subprocess.check_output([str(host), "--state-dir", str(state), "action", json.dumps(request)],
                                                     env=action_environment, text=True))
        command_dir = directory / "명령 검증"
        command_dir.mkdir()
        epoch = next(a["identity"]["session_epoch"] for a in rows if a["identity"]["session_name"] == first)
        launch_request = {"request_id": "adapter-launch", "action": {"kind": "launch", "session": first,
                          "epoch": epoch, "cwd": str(command_dir), "tool": "codex"}}
        launched = host_action(launch_request)
        assert launched["state"] == "succeeded", launched
        assert host_action(launch_request)["pane_id"] == launched["pane_id"]
        def launched_agent():
            host_call("scan")
            return next((a for a in host_call("snapshot")["agents"] if a["identity"]["session_name"] == first
                         and a["identity"]["pane_id"] == launched["pane_id"] and a["liveness"] == "live"), None)
        agent = wait_for("host adapter creates and discovers pane once", launched_agent)
        wait_for("host adapter reads pane screen", lambda: "fixture ready" in host_call("preview", agent["identity"]["agent_id"])["text"])
        input_request = {"request_id": "adapter-input", "action": {"kind": "input", "target": agent["identity"], "text": "한글 첫 줄\nsecond line"}}
        assert host_action(input_request)["state"] == "succeeded"
        assert host_action(input_request)["state"] == "succeeded"
        input_log = command_dir / "input.log"
        wait_for("host adapter sends multiline input", lambda: "second line" in input_log.read_text())
        assert input_log.read_text().count("한글 첫 줄") == 1
        print("PASS duplicate input does not repeat delivery", flush=True)
        # Tab selects working rows across panels, without leaving the dashboard.
        show_fixture(":working")
        wait_for("screen working for Tab navigation", lambda: current_screen_row()["status"] == "working")
        fixture_pane = f'terminal_{agent["identity"]["pane_id"]}'
        call(first, "action", "write-chars", "--pane-id", fixture_pane, ":working")
        call(first, "action", "write", "--pane-id", fixture_pane, "13")
        host_call("pin", agent["identity"]["agent_id"], "true")
        wait_for("second working fixture", lambda: next(a for a in host_call("snapshot")["agents"]
                 if a["identity"] == agent["identity"])["status"] == "working")
        tab_ui = call(first, "plugin", "--configuration", f"mode=dashboard,{base_config}", "--", f"file:{wasm}").strip()
        wait_for("Tab dashboard loaded", lambda: any(p["permissions"] for p in ping(first, "dashboard")))
        os.write(clients[0].fd, f"/{first}\r".encode())
        wait_for("Tab query scoped to temporary session", lambda: any(p["query"] == first for p in ping(first, "dashboard")))
        tab_client = next(p["client_id"] for p in ping(first, "dashboard") if p["query"] == first)
        os.write(clients[0].fd, b"h")
        wait_for("Tab anchor in pinned panel", lambda: any(p["query"] == first and p["selected_id"] == agent["identity"]["agent_id"]
                 for p in ping(first, "dashboard")))
        os.write(clients[0].fd, b"\t")
        wait_for("Tab selects working agent across panels", lambda: any(p["query"] == first and not p["pinned_panel"]
                 and p["selected_id"] == screen_row["identity"]["agent_id"] for p in ping(first, "dashboard")))
        os.write(clients[0].fd, b"\t")
        wait_for("Tab wraps to pinned working agent", lambda: any(p["query"] == first and p["pinned_panel"]
                 and p["selected_id"] == agent["identity"]["agent_id"] for p in ping(first, "dashboard")))
        call(first, "action", "close-pane", "--pane-id", tab_ui)
        # Only these two owned agents are pinned in this isolated state.
        host_call("pin", screen_row["identity"]["agent_id"], "true")
        call(first, "action", "focus-pane-id", fixture_pane)
        def client_panes(session):
            return {int(line.split()[0]): line.split()[1] for line in call(session, "action", "list-clients").splitlines()
                    if re.match(r"^\d+\s+", line)}
        before_navigation = client_panes(first)
        os.write(clients[0].fd, b"\x1bu")
        wait_for("global agent-next works without dashboard", lambda: client_panes(first).get(tab_client) == f'terminal_{screen_row["identity"]["pane_id"]}')
        assert all(pane == before_navigation[cid] for cid, pane in client_panes(first).items() if cid != tab_client)
        print("PASS global navigation leaves other client focus unchanged", flush=True)
        os.write(clients[0].fd, b"\x1bu")
        wait_for("global agent-next wraps using actual focus", lambda: client_panes(first).get(tab_client) == fixture_pane)
        before_messages = max(p["next_messages"] for p in ping(first))
        os.write(clients[0].fd, b"\x1bu\x1bu")
        wait_for("rapid global keys each advance once", lambda: any(p["client_id"] == tab_client
                 and p["next_messages"] >= before_messages + 2 and not p["next_pending"] and not p.get("next_check_pending", False)
                 and p.get("next_queued", 0) == 0 for p in ping(first))
                 and client_panes(first).get(tab_client) == fixture_pane)
        call(first, "action", "write-chars", "--pane-id", fixture_pane, ":idle")
        call(first, "action", "write", "--pane-id", fixture_pane, "13")
        wait_for("idle fixture for global filter", lambda: next(a for a in host_call("snapshot")["agents"]
                 if a["identity"] == agent["identity"])["status"] == "idle")
        call(first, "action", "focus-pane-id", f'terminal_{screen_row["identity"]["pane_id"]}')
        os.write(clients[0].fd, b"\x1bi")
        wait_for("global idle-and-pinned skips working agent", lambda: client_panes(first).get(tab_client) == fixture_pane)
        other_agent = next(a for a in rows if a["identity"]["session_name"] == sessions[1])
        host_call("pin", other_agent["identity"]["agent_id"], "true")
        host_call("pin", agent["identity"]["agent_id"], "false")
        first_hop = host_call("next", json.dumps({"filter": "pinned-only", "session": first,
                                                  "pane_id": agent["identity"]["pane_id"]}))
        second_hop = host_call("next", json.dumps({"filter": "pinned-only", "session": first_hop["identity"]["session_name"],
                                                   "pane_id": first_hop["identity"]["pane_id"]}))
        assert first_hop["identity"]["session_name"] == sessions[1]
        assert second_hop["identity"]["session_name"] == first
        before_messages = max(p["next_messages"] for p in ping(first))
        os.write(clients[0].fd, b"\x1bu\x1bu")
        wait_for("rapid keys finish cross-session cycle before switching", lambda: any(p["client_id"] == tab_client
                 and p["next_messages"] >= before_messages + 2 and not p["next_pending"]
                 and not p.get("next_check_pending", False) and p.get("next_queued", 0) == 0 for p in ping(first))
                 and client_panes(first).get(tab_client) == f'terminal_{second_hop["identity"]["pane_id"]}')
        host_call("pin", other_agent["identity"]["agent_id"], "false")
        host_call("pin", screen_row["identity"]["agent_id"], "false")
        host_call("pin", agent["identity"]["agent_id"], "false")
        for next_filter in ("unpinned-only", "idle-and-unpinned"):
            next_row = host_call("next", json.dumps({"filter": next_filter, "session": first,
                                                     "pane_id": agent["identity"]["pane_id"]}))
            assert next_row and not next_row["pinned"]
            if next_filter == "idle-and-unpinned":
                assert next_row["status"] == "idle"
        print("PASS host next unpinned and idle-unpinned filters", flush=True)
        before_messages = max(p["next_messages"] for p in ping(first))
        before_navigation = client_panes(first)
        os.write(clients[0].fd, b"\x1bu")
        wait_for("empty pinned filter completes", lambda: any(p["client_id"] == tab_client
                 and p["next_messages"] > before_messages and not p["next_pending"] and not p.get("next_check_pending", False)
                 and p.get("next_queued", 0) == 0 for p in ping(first)))
        assert client_panes(first) == before_navigation
        print("PASS no matching global target preserves focus", flush=True)
        close_request = {"request_id": "adapter-close", "action": {"kind": "close", "target": agent["identity"]}}
        assert host_action(close_request)["state"] == "succeeded"
        assert host_action(close_request)["state"] == "succeeded"
        assert not any(p["id"] == launched["pane_id"] and not p["is_plugin"]
                       for p in json.loads(call(first, "action", "list-panes", "--all", "--json")))
        print("PASS host adapter closes pane once", flush=True)

        # Exercise keyboard search and the actual cross-session focus API.
        target = next(a for a in rows if a["identity"]["session_name"] == sessions[1])
        alias_request = json.dumps({"request_id": "sqlite-alias-once", "action": {
            "kind": "alias", "target": target["identity"], "alias": "SQLite 보존 확인"}})
        first_alias = host_call("action", alias_request)
        assert first_alias["state"] == "succeeded"
        assert host_call("action", alias_request) == first_alias
        print("PASS duplicate action result and alias persisted", flush=True)
        def attached(session):
            return [line.split() for line in call(session, "action", "list-clients").splitlines()
                    if re.match(r"^\d+\s+", line)]
        source_clients = len(attached(first))
        focus_ui = call(first, "plugin", "--floating", "--configuration", f"mode=dashboard,{base_config}", "--", f"file:{wasm}").strip()
        wait_for("focus dashboard loaded", lambda: any(p["revision"] is not None for p in ping(first, "dashboard")))
        # Drive one attached client; each client has its own dashboard view.
        os.write(clients[0].fd, f"/{sessions[1]}\r".encode())
        wait_for("keyboard search selects requested agent", lambda: any(p["selected_id"] == target["identity"]["agent_id"]
                 and p["query"] == sessions[1] for p in ping(first, "dashboard")))
        # gg must open the selected remote agent's repository over this dashboard.
        def lazygit_panes(session):
            return [p for p in json.loads(call(session, "action", "list-panes", "--all", "--json"))
                    if not p["is_plugin"] and p["title"] == "lazygit"]
        target_pane = next(p for p in json.loads(call(sessions[1], "action", "list-panes", "--all", "--json"))
                           if not p["is_plugin"] and p["id"] == target["identity"]["pane_id"])
        os.write(clients[0].fd, b"gg")
        git_pane = wait_for("gg opens lazygit in dashboard session", lambda: next(iter(lazygit_panes(first)), None))
        assert git_pane["is_floating"], git_pane
        assert not lazygit_panes(sessions[1])
        git_result = next(r for r in host_call("requests") if r["request"]["action"]["kind"] == "lazygit")
        assert git_result["state"] == "succeeded", git_result
        assert git_result["request"]["action"]["session"] == first
        assert git_result["request"]["action"]["target"] == target["identity"]
        assert Path(git_pane["pane_cwd"]).resolve() == Path(target_pane["pane_cwd"]).resolve(), git_pane
        assert host_call("action", json.dumps(git_result["request"]))["pane_id"] == git_result["pane_id"]
        assert len(lazygit_panes(first)) == 1
        call(first, "action", "write-chars", "--pane-id", f'terminal_{git_pane["id"]}', "q")
        wait_for("quitting lazygit closes floating pane", lambda: not lazygit_panes(first))
        wait_for("lazygit request completes in dashboard", lambda: any(p["menu"] is None for p in ping(first, "dashboard")))
        host_call("pin", target["identity"]["agent_id"], "true")
        wait_for("selected agent moves to pinned rows", lambda: any(
            a["pinned"] and a["identity"]["agent_id"] == target["identity"]["agent_id"]
            for a in host_call("snapshot")["agents"]))
        def panel_matches(pinned, selected):
            return any(p["pinned_panel"] == pinned and p["selected_id"] == selected
                       and p["query"] == sessions[1] for p in ping(first, "dashboard"))
        os.write(clients[0].fd, b"hh")
        wait_for("h selects pinned panel and repeated h stays there",
                 lambda: panel_matches(True, target["identity"]["agent_id"]))
        os.write(clients[0].fd, b"ll")
        wait_for("l selects empty unpinned panel and repeated l stays there",
                 lambda: panel_matches(False, None))
        os.write(clients[0].fd, b"h")
        wait_for("h restores pinned selection",
                 lambda: panel_matches(True, target["identity"]["agent_id"]))
        host_call("pin", target["identity"]["agent_id"], "false")
        wait_for("selected agent returns to unpinned rows", lambda: any(
            not a["pinned"] and a["identity"]["agent_id"] == target["identity"]["agent_id"]
            for a in host_call("snapshot")["agents"]))
        os.write(clients[0].fd, b"l")
        wait_for("l selects unpinned agent",
                 lambda: panel_matches(False, target["identity"]["agent_id"]))
        os.write(clients[0].fd, b"\r")
        wait_for("Enter focuses agent in other session", lambda: len(attached(first)) < source_clients
                 and any(row[1] == f'terminal_{target["identity"]["pane_id"]}' for row in attached(sessions[1])))
        clients.append(Client([zellij, "attach", first], directory))
        wait_for("collector available after focus", lambda: any(p["permissions"] for p in ping(first)))
        call(first, "action", "close-pane", "--pane-id", focus_ui)
        # Pin after keyboard focus checks: pinning moves the row to the other panel.
        host_call("pin", target["identity"]["agent_id"], "true")
        row = next(a for a in rows if a["identity"]["session_name"] == first)
        event = {"schema_version":1, "event_id":"smoke-event", "identity":row["identity"], "tool":"codex",
                 "kind":"turn_started", "sequence":1, "observed_at_ms":int(time.time()*1000),
                 "summary":"한글, 여러 줄\n테스트", "cwd":str(directory)}
        first_result = host_call("ingest", input=json.dumps(event))
        second_result = host_call("ingest", input=json.dumps(event))
        assert first_result["applied"] and not second_result["applied"]
        print("PASS duplicate event persisted once", flush=True)
        assert current_screen_row()["status_source"] == "hook"
        show_fixture(":idle")
        before_scan = host_call("snapshot")["last_scan_ms"]
        wait_for("hook ownership rejects later idle screen", lambda: host_call("snapshot")["last_scan_ms"] > before_scan
                 and current_screen_row()["status_source"] == "hook" and current_screen_row()["status"] == "working")
        resolved = host_call("resolve", row["identity"]["agent_id"])
        assert resolved["identity"] == row["identity"]
        call(resolved["identity"]["session_name"], "action", "close-pane", "--pane-id", f'terminal_{resolved["identity"]["pane_id"]}')
        result = subprocess.run([str(host), "--state-dir", str(state), "resolve", row["identity"]["agent_id"]], capture_output=True)
        assert result.returncode == 1
        print("PASS stale pane identity rejected", flush=True)

        for client in clients:
            client.stop()
        before = max(p["polls"] for p in ping(first))
        wait_for("collector survives all clients detaching", lambda: max(p["polls"] for p in ping(first)) > before)
        # Zellij 0.45.0 explicitly rejects plugin reload with no attached client.
        clients.append(Client([zellij, "attach", first], directory))
        # A background collector can answer before the new client registers.
        # Reload needs a real attached client, not merely a responsive collector.
        wait_for("client registered before reload", lambda: attached(first))
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
        # Discovery includes other live user sessions. Restrict the UI search to
        # this test session, whose agents are now ended or have no pane.
        os.write(clients[-1].fd, f"/{first}\r".encode())
        wait_for("ended and missing panes excluded from selection",
                 lambda: any(p["revision"] is not None and p["query"] == first and p["selected_id"] is None
                             for p in ping(first, "dashboard", ROOT / "dist/agent-dashboard.wasm")))
        # Keep optional tool onboarding after the collector lifecycle checks.
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
                return any(a["tool"] == "claude" and a["status_source"] == "hook"
                           for a in host_call("snapshot")["agents"] if a["identity"]["session_name"] == first)
            wait_for("real Claude SessionStart hook", reported, seconds=40)

        saved_target = next(a for a in host_call("snapshot")["agents"]
                            if a["identity"] == target["identity"])
        assert saved_target["pinned"] and saved_target["alias"] == "SQLite 보존 확인"
        assert host_call("result", "sqlite-alias-once") == first_alias
        assert (state / "store.json").read_bytes() == legacy_bytes
        with sqlite3.connect(state / "store.sqlite3") as database:
            assert database.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
            assert database.execute("PRAGMA user_version").fetchone()[0] == 2
            assert database.execute("PRAGMA journal_mode").fetchone()[0] == "wal"
            metadata = json.loads(database.execute("SELECT body FROM metadata WHERE id='store'").fetchone()[0])
            assert metadata["revision"] >= 17
            assert database.execute("SELECT count(*) FROM agents").fetchone()[0] >= 2
        print("PASS SQLite persistence, integrity and preserved JSON import", flush=True)
        source_clients = len(attached(first))
        os.write(clients[-1].fd, b"\x1bu")
        wait_for("global agent-next crosses sessions after detach and reload", lambda: len(attached(first)) < source_clients
                 and any(row[1] == f'terminal_{target["identity"]["pane_id"]}' for row in attached(sessions[1])))
        (directory / "result.json").write_text(json.dumps({"passed":True, "sessions":sessions, "real_claude":args.real_claude, "screen_adapter":True, "sqlite_repository":True, "legacy_import":True, "pane_presence":True, "agent_next":True, "working_tab":True}, indent=2))
        print(f"Artifacts: {directory}", flush=True)
    finally:
        for fixture in background_fixtures:
            if fixture.poll() is None:
                fixture.terminate()
                fixture.wait(timeout=5)
            if fixture.stdin:
                fixture.stdin.close()
        for session in sessions:
            subprocess.run([zellij, "kill-session", session], stdin=subprocess.DEVNULL, capture_output=True, timeout=5)
        for client in clients:
            client.stop()


if __name__ == "__main__":
    main()
