# Clean rebuild policy

v0.10 is a deliberate reset because the earlier builds were development prototypes and did not contain real user data.

The application therefore does **not** contain:

- PromptDock database migration
- `imagelore.db` migration
- old schema `ALTER TABLE` compatibility code
- `.promptdock.json` import
- old localStorage key fallbacks

The only supported database from v0.10 onward is `library.sqlite3`.

If prototype files exist on a Windows machine, run `CLEAN_LEGACY_DATA.bat`. The cleanup script removes `%LOCALAPPDATA%\PromptDock` and the obsolete `%LOCALAPPDATA%\ImageLore\imagelore.db` / old thumbnail folder. It does not remove `library.sqlite3`.
