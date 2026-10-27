"""Pilot must not contact a model client before both explicit gates pass."""
import os
import unittest
from unittest.mock import patch

import bugfix_live


class BugfixLiveTests(unittest.TestCase):
    def test_missing_live_flag_never_starts_client(self):
        with patch('sys.argv',['bugfix_live.py','run']), patch('subprocess.run') as run:
            with self.assertRaises(SystemExit): bugfix_live.main()
            run.assert_not_called()

    def test_missing_renewed_authorization_never_starts_client(self):
        with patch('sys.argv',['bugfix_live.py','run','--enable-live-model-tests']), patch.dict(os.environ,{bugfix_live.APPROVAL_ENV:''}), patch('subprocess.run') as run:
            with self.assertRaises(SystemExit): bugfix_live.main()
            run.assert_not_called()

    def test_ci_never_starts_client(self):
        with patch.dict(os.environ,{'CI':'true'}), patch('subprocess.run') as run:
            with self.assertRaises(SystemExit): bugfix_live.run_live()
            run.assert_not_called()
