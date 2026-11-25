# Keep portable settings and writes in the portable workspace

Resolve portable mode from the executable directory or its parent when the documented archive layout places binaries in platform subdirectories. If the workspace is read-only, keep it readable and report that saving is unavailable. Falling back to the host profile would scatter configuration and could store private run data on someone else's machine.
