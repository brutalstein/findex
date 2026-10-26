# Intermediate MSVC bridge

115-query matrix-only run after switching the Windows package from GNU/LLVM
to MSVC/static CRT. The source-hit and lossless assertions all passed, with
the same output-token counts as the matrix-only GNU run. This established
build compatibility before the additional zero-sync encoding. Final results
are in `../20261027-final-comparison/`; this run is retained as an intermediate
check, not an independent replication.
