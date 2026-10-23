#!/usr/bin/env python3
"""Verify notification lifecycle in an owned temporary Zellij session."""
import concurrent.futures
import fcntl
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import termios
import time

sys.dont_write_bytecode = True
from smoke import ANSI, Client, ROOT


def main():
    directory = Path(tempfile.mkdtemp(prefix="notification-", dir=ROOT / ".local"))
    for name in ("dashboard-host", "agent-dashboard.wasm", "agent-notification.wasm"):
        shutil.copy2(ROOT / "dist" / name, directory / name)
    host = directory / "dashboard-host"
    wasm = directory / "agent-dashboard.wasm"
    notice = directory / "agent-notification.wasm"
    state = directory / "state"
    config = directory / "config"
    (config / "layouts").mkdir(parents=True)
    (config / "layouts/test.kdl").write_text("layout {\n pane\n}\n")
    settings = f"host_path={host},state_dir={state}"
    (config / "config.kdl").write_text('''default_layout "test"
show_startup_tips false
show_release_notes false
''')
    # A real local process with a recognized executable name, without model calls.
    source = directory / "fixture.rs"
    source.write_text('fn main() { loop { std::thread::sleep(std::time::Duration::from_secs(60)); } }')
    subprocess.run([str(Path.home() / ".cargo/bin/rustup"), "run", "1.88.0", "rustc", str(source),
                    "-o", str(directory / "codex")], check=True)
    session = directory.name
    # Keep discovery inside this owned session; other live user sessions must not
    # influence the waiting count or make lifecycle assertions nondeterministic.
    (directory / "bin").mkdir()
    ps = directory / "bin/ps"
    ps.write_text(f'''#!/usr/bin/python3
import os, subprocess, sys
text = subprocess.check_output(["/bin/ps", *sys.argv[1:]], text=True)
for line in text.splitlines():
    if {session!r} in line or line.split()[0] == str(os.getppid()):
        print(line)
''')
    ps.chmod(0o755)
    os.environ["PATH"] = str(directory / "bin") + os.pathsep + os.environ["PATH"]
    environment = os.environ.copy()
    for key in ("ZELLIJ", "ZELLIJ_SESSION_NAME", "ZELLIJ_PANE_ID"):
        environment.pop(key, None)
    clients = [Client(["zellij", "--config-dir", str(config), "--session", session], directory)]
    checks = []

    def pump():
        for client in clients:
            client.pump()
            # Long plugin paths can clip the word "permission" in the dialog.
            plain = ANSI.sub("", client.text).lower()
            if "allow? (y/n)" in plain:
                os.write(client.fd, b"y")
                client.text = ""

    def call(*args):
        pump()
        result = subprocess.run(["zellij", "--session", session, *args], env=environment,
                                capture_output=True, text=True, timeout=8)
        if result.returncode:
            raise RuntimeError(result.stderr)
        return result.stdout

    def panes():
        return json.loads(call("action", "list-panes", "--all", "--json"))

    def notices():
        return [p for p in panes() if p.get("plugin_url") == f"file:{notice}"]

    def snapshot():
        return json.loads(subprocess.check_output([str(host), "--state-dir", str(state), "snapshot"], text=True))

    def ping():
        output = call("pipe", "--plugin", f"file:{wasm}", "--plugin-configuration", f"mode=collector,{settings}",
                      "--name", "agent-dashboard-ping", "--", "ping")
        return [json.loads(line) for line in output.splitlines() if line.startswith("{")]

    def wait(label, check):
        end = time.monotonic() + 30
        while time.monotonic() < end:
            pump()
            try:
                value = check()
                if value:
                    checks.append(label)
                    print("PASS", label, flush=True)
                    return value
            except (subprocess.SubprocessError, RuntimeError, ValueError, FileNotFoundError):
                pass
            time.sleep(.1)
        raise AssertionError(label)

    def event(agent, kind, sequence):
        payload = dict(schema_version=1, identity=agent["identity"], tool="codex", event_id=f"event-{sequence}",
                       sequence=sequence, kind=kind, observed_at_ms=int(time.time() * 1000),
                       cwd=str(directory), summary="", detail="")
        result = subprocess.run([str(host), "--state-dir", str(state), "ingest"],
                                input=json.dumps(payload), text=True, capture_output=True, check=True)
        assert json.loads(result.stdout)["applied"]

    def sync():
        subprocess.run([str(host), "--state-dir", str(state), "sync-notification", session],
                       capture_output=True, check=True, timeout=15)

    try:
        wait("temporary session ready", panes)
        call("plugin", "--configuration", f"mode=collector,{settings}", "--", f"file:{wasm}")
        wait("collector permissions granted", lambda: any(p["permissions"] and p["revision"] is not None for p in ping()))
        call("action", "new-pane", "--", str(directory / "codex"))
        agent = wait("fixture discovered", lambda: next((a for a in snapshot()["agents"] if a["identity"]["session_name"] == session), None))
        wait("pane metadata present", lambda: snapshot()["agents"][0]["pane"]["presence"] == "present")
        focused = {p["id"] for p in panes() if not p["is_plugin"] and p["is_focused"]}
        event(agent, "permission_required", 1)
        wait("waiting creates notification", lambda: len(notices()) == 1)
        sync()
        (directory / "notice-pane.json").write_text(json.dumps(notices(), ensure_ascii=False, indent=2))
        wait("notification renders waiting count", lambda: any("응답 대기 1개" in ANSI.sub("", c.capture) for c in clients))
        notice_pane = notices()[0]
        assert notice_pane["is_floating"] and not notice_pane["is_selectable"], notice_pane
        assert notice_pane["pane_x"] + notice_pane["pane_columns"] == 120, notice_pane
        assert notice_pane["pane_y"] == 0 and notice_pane["pane_rows"] == 4, notice_pane
        assert {p["id"] for p in panes() if not p["is_plugin"] and p["is_focused"]} == focused
        print("PASS top-right 40x4 geometry and terminal focus preserved", flush=True)
        checks.append("top-right geometry and focus")
        clients.append(Client(["zellij", "--config-dir", str(config), "attach", session], directory))
        wait("second client attached", lambda: len(call("action", "list-clients").splitlines()) >= 3)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            list(pool.map(lambda _: sync(), range(8)))
        assert len(notices()) == 1
        checks.append("concurrent helpers and two clients do not duplicate")
        print("PASS", checks[-1], flush=True)
        call("action", "start-or-reload-plugin", f"file:{wasm}", "--configuration", f"mode=collector,{settings}")
        wait("collector reload reuses notification", lambda: (
            any(p["permissions"] and p["revision"] is not None for p in ping())
            and len(notices()) == 1 and notices()[0]["id"] == notice_pane["id"]))
        call("action", "new-tab", "--name", "second", "--no-focus")
        wait("new tab gets one notification", lambda: len(notices()) == 2)
        # A resize must keep the toast attached to the right edge.
        for client in clients:
            fcntl.ioctl(client.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 100, 0, 0))
        wait("resize keeps right edge", lambda: all(p["pane_x"] + p["pane_columns"] == 100 for p in notices()))
        event(agent, "turn_finished", 2)
        wait("resolved waiting removes all notifications", lambda: not notices())
        event(agent, "permission_required", 3)
        wait("waiting recreates notifications", lambda: len(notices()) == 2)
        call("action", "close-pane", "--pane-id", f'terminal_{agent["identity"]["pane_id"]}')
        wait("ended execution removes notifications", lambda: not notices())
    finally:
        (directory / "terminal.log").write_text("\n".join(c.capture for c in clients))
        (directory / "result.json").write_text(json.dumps(checks, ensure_ascii=False, indent=2))
        subprocess.run(["zellij", "kill-session", session], env=environment, capture_output=True, timeout=8)
        for client in clients:
            client.stop()
        print("Artifacts:", directory, flush=True)


if __name__ == "__main__":
    main()
