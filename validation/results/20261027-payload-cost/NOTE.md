# Incomplete adapter-validation attempt

This attempt stopped at `fmt:KeyMatcher:words`: the codebase tree adapter did
not preserve a quoted C++ qualified name containing spaces. The native JSON/tree
hit-equivalence assertion failed. A focused regression reproduced the parser
bug, then passed after standard-library `shlex.split` replaced the split logic.

This partial attempt is not a quality result. The completed run is
`../20261027-payload-cost-v2/`, with the same frozen query set and binaries.
No Findex behavior was changed between attempts. Original partial records and
raw responses are preserved.
