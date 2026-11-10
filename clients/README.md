# Native clients

Not written yet. Each is a thin UI over `docs/PROTOCOL.md`: connect as a `ui`
with `notifies: true`, render the latest snapshot, raise a notification when a
request id above the last-seen one appears, send `answer`.

| Platform | Toolkit | Presence check before `use_stored` |
|---|---|---|
| macOS | Swift, AppKit menu-bar item (`NSStatusItem`), `NSSecureTextField` | `LAContext` (Touch ID / Watch) |
| Windows | C#/.NET, tray icon, toast notifications | `UserConsentVerifier` (Windows Hello) |
| Linux | GTK4/libadwaita, `GNotification`, StatusNotifierItem | polkit, or none |

The daemon owns key-store access on every platform, so the clients need no
keychain entitlements of their own.
