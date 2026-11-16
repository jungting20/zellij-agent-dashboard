# Prompt editor route fix

- [x] Ground. Focused source read confirms plugin sends Editor without a destination, host opens and polls target.session_name. Lead observed seven editor panes in the remote agent session.
- [x] Sketch. Compare persisted optional session with host runtime environment routing. The latter loses request recovery destination.
- [x] Agree. Readonly editor_arch confirmed persisted optional route; rejected runtime environment route.
- [x] Implement. Add optional persisted editor destination, pass dashboard session and poll the recorded destination. Preserve absent session for old requests.
- [x] Scrap. skip: creation and recovery share the same persisted request route.
- [x] Verify. Core legacy serialization and duplicate payload tests, host cross-session creation and completion polling, empty session rejection. Lead owns original-surface verification and install.

Focused-fix exception is unavailable because the request route and JSON shape change. Architect/how simple grounded review delegated. Existing Lazygit request contract demonstrates optional-session compatibility. DB and domain versions can remain unchanged because absent session preserves old data and serialized payload; old hosts must be restored with an earlier DB backup when new routed editor requests exist.

Core 49 and host 72 tests passed with one manual timing test ignored. Final added empty-destination test passed separately. cargo fmt check passed. First host regression failed with actual dev versus expected dashboard, then passed after route correction. Independent editor_review gpt-6.1-terra review found no findings. Original-surface Zellij verification and build/install remain with lead.
