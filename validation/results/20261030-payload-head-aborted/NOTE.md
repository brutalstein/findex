# Aborted attempt

The first HEAD run stopped on `requests:HTTPProxyAuth:words` because the guard compared the baseline binary's 17-digit `bm25_score` with the current three-decimal value. The guard now compares scores at three decimals (`same_hits`). Partial outputs are kept; the completed run is `20261030-payload-head`.
