# Project tasks and branch updates

The SDK owns the `guava` command, task scheduling and generated executables.
Projects declare tasks in `guava.workspace.json` and implement public Swift
library functions. They do not need launcher scripts or a second CLI.

```json
{
  "packages": [".", "App"],
  "dependencyUpdates": "branches",
  "beforeBuild": ["assets"],
  "tasks": {
    "assets": {
      "package": "Tools/Tasks",
      "product": "ProjectAssets",
      "entry": "Assets.run"
    }
  }
}
```

The package exports a library product named `ProjectAssets`. Its entry point is
`public static func run(_ arguments: [String]) throws` on `public enum Assets`.
`module` defaults to the product name and can be supplied when they differ.
`arguments` supplies default arguments, `dependsOn` orders prerequisite tasks,
and `platforms` optionally limits a task to `mac`, `win` or `linux` hosts.
Package paths must stay inside the workspace. Cycles and invalid declarations
are rejected before execution.

```sh
guava task
guava task assets -- --help
guava task assets /path/to/project -- --output "a directory"
guava build App
```

Task arguments after `--` remain literal, including `@file` arguments. The SDK
builds only the selected task product, caches generated hosts in
`.guava/build/tasks`, and sets `GUAVA_ROOT` and `GUAVA_SDK_ROOT` for the library.
Projects keep their implementation code and data; generated hosts belong to the SDK.

`beforeBuild` runs for normal build/run operations. `dependencyUpdates` defaults
to `locked`; `branches` checks branch pins in the listed packages before local
build/run. Unchanged branches avoid resolution. Changed branches are fetched
and refreshed with SwiftPM; version-range dependencies are not advanced by this
policy. CI keeps committed lockfiles. Offline remote checks retain the resolved
version. No project-specific dependency updater or manual manifest edits are needed.
