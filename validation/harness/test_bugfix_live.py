"""Pilot must not contact a model client before both explicit gates pass."""
import os
import unittest
import tempfile
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import bugfix_live
from bugfix_fixtures import FISH_REPRO


class BugfixLiveTests(unittest.TestCase):
    def test_campaign_cannot_write_outside_result_directory(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(bugfix_live,'OUT'), patch('sys.argv',['bugfix_live.py','prepare','--out',folder]), patch('subprocess.run') as run:
            with self.assertRaises(SystemExit):bugfix_live.main()
            run.assert_not_called()
            self.assertFalse(list(Path(folder).iterdir()))

    def test_fish_oracle_accepts_equivalent_option_quoting(self):
        for quote in ["'",'"','']:
            output='\n'.join(f'complete -c rg -l {quote}{name}{quote}' for name in ['no-ignore','no-hidden','no-ignore-dot'])
            with patch('subprocess.run',return_value=SimpleNamespace(stdout=output)):
                exec(FISH_REPRO,{'__file__':__file__})

    def test_fish_oracle_rejects_missing_long_flag_and_inherited_short_alias(self):
        for prefix,suffix in [('', ''),('-l ',' -s x')]:
            output='\n'.join(f'complete -c rg {prefix}{name}{suffix}' for name in ['no-ignore','no-hidden','no-ignore-dot'])
            with patch('subprocess.run',return_value=SimpleNamespace(stdout=output)):
                with self.assertRaises(AssertionError):exec(FISH_REPRO,{'__file__':__file__})

    def test_child_does_not_inherit_parent_app_permission_context(self):
        env=bugfix_live.bugfix_env({'CODEX_HOME':'official-home','CODEX_THREAD_ID':'parent',
            'CODEX_APP_TOOLS_PIPE_PATH':'parent-pipe','CODEX_PERMISSION_PROFILE':':read-only',
            'OPENAI_API_KEY':'forbidden','PATH':'native-path'})
        self.assertEqual(env,{'CODEX_HOME':'official-home','PATH':'native-path'})

    def test_reported_read_only_environment_is_not_a_valid_bugfix_trial(self):
        events=[{'type':'item.completed','item':{'type':'mcp_tool_call','server':'findex',
                 'status':'completed','result':{'isError':False}}},
                {'type':'item.completed','item':{'type':'agent_message',
                 'text':'No files changed: this session permits read-only access.'}},
                {'type':'turn.completed','usage':{'input_tokens':100,'output_tokens':10}}]
        result=bugfix_live.execution_record(0,events,'findex')
        self.assertFalse(result['valid'])
        self.assertEqual(result['infrastructure_failure'],'reported_read_only')

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
