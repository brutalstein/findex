# Interrupted harness setup

No timed responses completed. McpProcess does not implement a context manager; the attempted ExitStack.enter_context failed. Corrected with existing explicit close callbacks in the v2 run. This is not passing performance evidence.
