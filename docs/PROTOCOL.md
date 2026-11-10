# clortho wire protocol, version 1

Everything that talks to the daemon (the shims, the TUI, native clients) uses
this protocol. The Rust definitions in `crates/clortho-core/src/proto.rs` are
authoritative.

## Transport

An `AF_UNIX` stream socket, on all three platforms (Windows 10 1803 and later
support them natively; .NET exposes them as `UnixDomainSocketEndPoint`).

Location, first match wins:

1. `$CLORTHO_SOCKET`
2. `$XDG_RUNTIME_DIR/clortho/clortho.sock`
3. `~/.clortho/clortho.sock` (Unix), `%LOCALAPPDATA%\clortho\clortho.sock` (Windows)

On Unix the daemon rejects peers whose effective uid differs from its own and
the socket is mode 0600. On Windows the profile directory's ACL is the gate.

Framing: one UTF-8 JSON object per line, `\n` terminated, both directions.
Unknown fields must be ignored. Absent optional fields are `null` or omitted.

## Handshake

Client sends `hello`; daemon answers `welcome` or `error`.

```json
{"type":"hello","version":1,"role":"ui","name":"clortho-mac","notifies":true}
{"type":"welcome","version":1,"daemon_pid":4437,"store":"confirm"}
```

`role` is `requester` or `ui`. A UI sets `notifies` when it raises its own
desktop notifications; while one such UI is attached the daemon does not run
its `notify_command`. `store` is the key-store policy: `off`, `confirm`, `auto`.

## UI clients

After `welcome` the daemon sends a `snapshot`, and another after every state
change. A snapshot is the complete state; clients keep no state of their own
beyond "highest request id already notified".

```json
{"type":"snapshot","pending":[RequestView],"recent":[HistoryEntry]}
```

`RequestView`:

| field | meaning |
|---|---|
| `id` | request number, increasing |
| `group` | requests with equal `group` are answered together |
| `prompt` | see below |
| `origin` | `pid`, `uid`, `host`, `tty`, `cwd`, and `chain`: the requesting process and its ancestors, nearest first, each `{pid, name, cmdline}` |
| `created_ms` | Unix time in milliseconds |
| `stored` | the key store holds a secret for this group |

`prompt`:

| field | meaning |
|---|---|
| `kind` | `pin`, `confirm` (yes/no) or `message` (acknowledge) |
| `source` | `gpg`, `ssh`, `sudo`, `git`, `pinentry`, `askpass` |
| `title`, `description`, `prompt` | text from the requester |
| `error` | set when the previous answer was rejected |
| `key` | stable identity of what is being unlocked, if known |
| `repeat` | a new secret is being chosen: ask twice |
| `echo` | the answer is not secret (a username) |
| `allow_store` | the secret may be saved to the key store |
| `ok_label`, `cancel_label`, `notok_label` | button text suggestions |
| `timeout_secs` | the daemon resolves it as timed out after this long |

`HistoryEntry` is `{request, outcome, by, resolved_ms}` with `outcome` one of
`answered`, `from_store`, `confirmed`, `denied`, `cancelled`, `timed_out`,
`withdrawn`. History never contains secrets.

A UI resolves a request with `answer`:

```json
{"type":"answer","id":7,"reply":{"action":"secret","secret":"...","remember":true}}
{"type":"answer","id":7,"reply":{"action":"use_stored"}}
{"type":"answer","id":8,"reply":{"action":"confirm"}}
{"type":"answer","id":8,"reply":{"action":"deny"}}
{"type":"answer","id":7,"reply":{"action":"cancel"}}
{"type":"answer","id":7,"reply":{"action":"forget"}}
```

- `secret` and `cancel` apply to every pending request in the same group.
- `use_stored` releases the key-store secret to the group. Under the `confirm`
  policy this is the only way a stored secret leaves the store, which makes
  the UI the place to put a presence check (Touch ID, Windows Hello, polkit).
- `forget` deletes the stored secret and leaves the request pending.
- `confirm` is valid for `confirm` and `message` kinds; `deny` for `confirm`.

Success produces a new `snapshot`. Failure produces
`{"type":"error","message":"..."}`; the request stays pending.

## Requesters

One outstanding request per connection; a connection may make several in
sequence. Closing the connection withdraws the outstanding request.

```json
{"type":"request","prompt":{"kind":"pin","source":"ssh","description":"Enter passphrase for key '/home/v/.ssh/id_ed25519': ","key":"/home/v/.ssh/id_ed25519","allow_store":true},"origin":{"pid":4242}}
{"type":"result","result":{"status":"secret","secret":"...","from_store":false}}
```

`status` is `secret`, `confirmed`, `denied`, `cancelled` or `timed_out`.
The daemon fills `origin.chain` from `origin.pid`.

When a request arrives with `prompt.error` set and a `key`, the daemon deletes
any stored secret for that group: the requester has just reported it wrong.
