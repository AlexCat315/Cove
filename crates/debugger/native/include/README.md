# Vendored LLDB SB API headers

Unmodified copies from [llvm-project](https://github.com/llvm/llvm-project); the tag and commit are in `VERSION`, the licence in `LICENSE.TXT` (Apache-2.0 WITH LLVM-exception).

They are in the repository because the shim in `../shim.cpp` needs them to compile and LLVM's Windows installer and Xcode ship liblldb without headers.

Do not edit anything here by hand. Refresh with:

```bash
cargo run -p cove-debugger --example vendor-lldb-headers -- llvmorg-<version>
```

The tool needs `git` and Python 3. It copies `lldb/include/lldb/API/*.h` and the `lldb/...` headers they include, then runs LLDB's `lldb/scripts/generate-sbapi-dwarf-enum.py` over `llvm/include/llvm/BinaryFormat/Dwarf.def` to produce `lldb/API/SBLanguages.h`, the one header LLDB generates at build time instead of keeping in its source tree.

## When to update

The SB API only adds symbols between releases, so these headers work with any liblldb at or above the functions the shim calls. Update the tag when the shim needs a newer SB function, not for each LLVM release. A liblldb older than a function the shim uses fails at link time on that host and the build falls back to the stub.
