# Initial development checks, not final package evidence

These checks initially used the standalone GNU/LLVM Rust selected by this
machine's PATH. Its binary depends on libunwind.dll, and fails the subsequently
added Windows-system-only PATH check. An attempted MSRV command also inherited
the wrong compiler/runtime environment. Those attempts do not establish portable
release or MSRV success. Their original logs/results are preserved.

The final authoritative MSVC/static-CRT run is `../20261027-release-candidate/`,
with explicit compiler selection, stable/MSRV tests, final native plugin checks
and final executable identities. The intermediate MSVC run is separately labelled.
