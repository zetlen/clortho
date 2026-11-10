# clortho

*Are you the Keymaster?*

clortho is a pinentry replacement for machines where several things ask for
secrets at once: coding agents in a dozen terminals, background `git` signing,
`ssh` in a tmux pane you are not looking at. Every passphrase, PIN and yes/no
prompt goes into one queue held by a small daemon. You answer from whichever
client is in front of you, and you can see which process asked.

```
 clortho   3 pending   store: confirm   daemon pid 4437
┌ Pending ──────────────────────────────────────────────────────────────────┐
│▶  36s sudo     [sudo] password for vinz:  sudo ← make ← zsh               │
│    2s ssh      Enter passphrase for key '~/.ssh/id_rsa':  ssh ← git ← zsh │
│    2s gpg      key "Vinz Clortho <vinz@gozer.test>"  gpg ← git  ● stored  │
└───────────────────────────────────────────────────────────────────────────┘
```

Status: alpha. The Rust core (daemon, shims, TUI) is tested on Linux against
GnuPG 2.4. The macOS and Windows code paths have not yet been compiled or run;
the CI workflow builds all three. The native GUI clients in `clients/` do not
exist yet.

## Pieces

| Binary | Role |
|---|---|
| `clortho daemon` | The broker. Started on demand by the shims; no service setup needed. |
| `pinentry-clortho` | Speaks the pinentry Assuan protocol. For gpg-agent and anything else that runs a pinentry (rbw, age plugins, `pass`). |
| `clortho-askpass` | Askpass convention: `SSH_ASKPASS`, `SUDO_ASKPASS`, `GIT_ASKPASS`. |
| `clortho` | The TUI. `clortho --once` exits when the queue is empty, for popups. |
| `clortho status\|answer\|approve\|deny\|cancel\|forget` | Scriptable equivalents. |

## Install

```sh
cargo install --path crates/clortho
clortho setup        # prints the lines to add for gpg, ssh, sudo and git
```

## Behaviour

**Grouping.** Requests for the same key (gpg keygrip, ssh key path, git URL)
form a group; one answer resolves all of them. gpg-agent already serializes
its own pinentry calls and caches the result, so grouping matters mostly for
the askpass paths, which have no such lock.

**Provenance.** gpg-agent tells the pinentry which client pid asked; askpass
callers are the shim's parent. The daemon walks the process tree from there.

**Key stores.** macOS Keychain, Windows Credential Manager, or the freedesktop
Secret Service (GNOME Keyring, KWallet), through the `keyring` crate. Policy,
in `config.toml`:

- `confirm` (default): a remembered secret is released only when a UI approves
  that request (`u` in the TUI). Nothing unlocks silently.
- `auto`: remembered secrets answer matching requests with no interaction.
- `off`: the store is never read or written.

A secret is offered for storage only when the requester permits it (gpg's
`allow-external-password-cache`, ssh key passphrases, git passwords). sudo and
ssh login passwords are never stored. If the requester reports a stored secret
as wrong, it is deleted.

**Failure.** If the daemon cannot be reached or started, `pinentry-clortho`
execs a stock pinentry from `PATH` (or `fallback_pinentry`) so signing does not
break.

**Notifications.** On each new request the daemon runs `notify_command`
(default: `notify-send` on Linux, `osascript` on macOS) unless a client that
does its own notifying is attached.

## Configuration

`clortho paths` prints the locations. All keys are optional.

```toml
store = "confirm"            # off | confirm | auto
store_backend = "os"         # os | memory
history = 50
default_timeout_secs = 0     # 0: wait as long as the requester does
fallback_pinentry = "/usr/bin/pinentry-curses"
notify_command = ["notify-send", "{title}", "{body}"]   # [] disables
```

## Security notes

- The socket is `AF_UNIX`, mode 0600, and on Unix the daemon checks the peer's
  uid. Any process running as you can already read your terminal and your
  agent sockets; clortho does not change that boundary.
- Any same-user process can attach as a UI and answer or see pending prompts
  (not secrets, which flow only daemon → requester). Under `auto`, any
  same-user process can obtain a stored secret by asking. Use `confirm` if
  that matters.
- Secrets are zeroed after use where the code controls the buffer; JSON
  encoding makes short-lived copies that are not.
- History records prompts and outcomes, never secrets, and is in memory only.

## Protocol

`docs/PROTOCOL.md`. JSON lines over the socket; a UI client is roughly a
hundred lines in any language.

## History

Versions 1.x (2015) were a Node library with the same idea at a smaller scale:
prompt for a credential, keep it in the OS keychain. That code is in the git
history before the `rust-rewrite` branch.

## License

ISC
