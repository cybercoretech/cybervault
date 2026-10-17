# HOLODRIVE local bridge v1

The optional `cybervault bridge` command is an attended integration point for
HOLODRIVE. Start it in a terminal on the host that runs HOLODRIVE. It prompts
for the master password locally, opens the encrypted vault, and listens on
`~/.local/state/cybervault/cybervault.sock` (override with
`cybervault --path <vault> bridge --socket <absolute-path>`). It exits after
five minutes idle or when stopped with Ctrl+C.

The protocol is one JSON request per Unix connection. The client writes one
JSON object, shuts down its write side, then reads one JSON reply until EOF.
The socket is mode `0600` inside a `0700` directory and is intended for a
HOLODRIVE server running under the same Unix account.

Status request:

```json
{"action":"status"}
```

```json
{"api_version":"v1","ok":true,"unlocked":true}
```

Append-only store request:

```json
{"action":"store","label":"Holodrive share: vacation","secret":"<share URL and passphrase>"}
```

The reply contains the version, success state, and label, never the submitted
secret. Labels must be unique; an existing label returns `label_exists` rather
than overwriting. Secret payloads are limited to 16 KiB and labels to 128
bytes. Protocol errors return stable codes and do not include request content.

This local protocol intentionally has no remote unlock, get, list, remove, or
overwrite operation. HOLODRIVE should offer it only to an authenticated
password session, never a device-token-only session. The Android user chooses
whether to save a newly created share; the master password stays in the
terminal and the secret remains encrypted in the vault at rest.
