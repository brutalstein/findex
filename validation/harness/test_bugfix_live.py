"""Pilot must not contact a model client before both explicit gates pass."""
import os
import unittest
import tempfile
import subprocess
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import bugfix_live
from bugfix_fixtures import FISH_REPRO


class BugfixLiveTests(unittest.TestCase):
    def test_semantic_setup_requires_explicit_prepare_option(self):
        for extra,expected in [([],False),(['--with-semantic-evidence'],True)]:
            with patch.object(bugfix_live,'OUT'), patch('sys.argv',['bugfix_live.py','prepare',*extra]), patch.object(bugfix_live,'prepare') as prepare:
                bugfix_live.main()
                prepare.assert_called_once_with(semantic=expected)

    def test_capture_diagnostics_report_pending_mcp_without_source_or_arguments(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder)
            (root/'events.jsonl').write_text('{"type":"thread.started"}\n'+
                '{"type":"item.started","item":{"type":"mcp_tool_call","server":"findex","tool":"findex_context","arguments":{"task":"private-source"}}}\n'+
                '{"incomplete":',encoding='utf-8')
            report=bugfix_live.capture_diagnostics(root,300.0,True)
            self.assertEqual(report['last_phase'],'mcp_in_progress')
            self.assertEqual(report['pending_tools'],['findex:findex_context'])
            self.assertFalse(report['usage_complete'])
            self.assertEqual(report['malformed_lines'],1)
            self.assertNotIn('private-source',str(report))

    def test_capture_diagnostics_accept_native_result_and_track_call_ids(self):
        import json
        events=[{'type':'item.started','item':{'id':i,'type':'mcp_tool_call','server':'findex','tool':'findex_context'}} for i in ('a','b')]
        events.append({'type':'item.completed','item':{'id':'a','type':'mcp_tool_call','server':'findex','tool':'findex_context','status':'completed','result':{'content':[]}}})
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);(root/'events.jsonl').write_text('\n'.join(map(json.dumps,events)),encoding='utf-8')
            report=bugfix_live.capture_diagnostics(root,1.0,False)
            self.assertEqual(report['failed_mcp_calls'],0)
            self.assertEqual(report['pending_tools'],['findex:findex_context'])

    def test_timeout_retains_partial_events_without_publishing_command_paths(self):
        def interrupted(argv,**options):
            options['stdout'].write('{"type":"item.completed"}\n');options['stdout'].flush()
            options['stderr'].write('native diagnostic\n');options['stderr'].flush()
            raise subprocess.TimeoutExpired(argv,300)
        with tempfile.TemporaryDirectory() as folder, patch('subprocess.run',side_effect=interrupted):
            root=Path(folder)
            with self.assertRaisesRegex(RuntimeError,'partial events retained'):
                bugfix_live.capture_process(['codex','private-command-path'],{},root)
            self.assertIn('item.completed',(root/'events.jsonl').read_text())
            self.assertIn('native diagnostic',(root/'stderr.txt').read_text())
            self.assertNotIn('private-command-path',(root/'timeout.log').read_text())

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
