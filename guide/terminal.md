## terminal

- new < path > - creates a new game example at path
- load < path > - loads an app file
- pack < path > - packs a directory into an app file
- unpack < file > - unpacks an app file into a zip archive
- ls - show directory contents relative to home
- show - open current app folder or file
- config - create config or show if already exists
- exit - exit to boot menu
- reload - reloads game, or press Super + R
- atlas - dump texture atlas to png
- dev - toggle dev mode this session
- clear or cls - clear the console
- bg < color > - set console background color
- find < name > - loaded assets search
- stats - show stats
- notif < message > - make a notification (debug)
- bundles - list active bundles
- help - display these console commands
- version - show engine and codex versions
- pwd - print the current app's directory
- copy < text > - copy text to the clipboard; with no text, copies whatever the previous piped command logged

Commands chain with `&&` (run each regardless of the last) and pipe with `|`
(the left command's logged output is appended as trailing arguments to the
right command) — e.g. `pwd | copy` copies the current app's directory to the
clipboard. Piping works with any command, not just `copy`: whatever a
command logs to the console is what the next stage receives.
