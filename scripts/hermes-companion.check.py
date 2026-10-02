"""Offline protocol checks; no Hermes credentials or provider requests."""
import importlib.util
import json
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location('companion',Path(__file__).with_name('hermes-companion.py'))
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class Protocol(unittest.TestCase):
    def test_private_config(self):
        for url in ('http://example.test','https://user:secret@example.test','https://example.test/another','https://example.test/?token=secret'):
            with self.assertRaises(ValueError):module.validate_config({'url':url,'token':'a'*64})
        self.assertEqual(module.validate_config({'url':'https://example.test/','token':'a'*64})['url'],'https://example.test')

    def test_requested_model_reasoning_and_schema(self):
        body={'model':'gpt-6-luna','reasoning_effort':'xhigh','messages':[{'role':'system','content':'Return proposals only.'},{'role':'user','content':'Add milk.'}],'response_format':{'type':'json_schema','json_schema':{'name':'proposal','strict':True,'schema':{'type':'object'}}}}
        converted=module.chat_body(body)
        self.assertEqual(converted['model'],'gpt-6-luna')
        self.assertEqual(converted['reasoning'],{'effort':'xhigh'})
        self.assertEqual(converted['text']['format']['name'],'proposal')
        self.assertFalse(converted['store'])
        self.assertTrue(converted['stream'])
        self.assertNotIn('tools',converted)
        self.assertEqual(converted['instructions'],'Return proposals only.')

    def test_only_completed_text_is_accepted(self):
        delta='data: '+json.dumps({'type':'response.output_text.delta','delta':'Connected'})
        completed='data: '+json.dumps({'type':'response.completed','response':{'status':'completed'}})
        self.assertEqual(module.stream_text([delta,completed])['choices'][0]['message']['content'],'Connected')
        for lines in ([delta],['data: {"type":"response.failed"}'],['data: {"type":"response.incomplete"}']):
            with self.assertRaises(RuntimeError):module.stream_text(lines)

    def test_clothing_images_preserve_inline_data_and_detail(self):
        body={'model':'gpt-6-luna','messages':[{'role':'system','content':'Read clothing tags.'},{'role':'user','content':[{'type':'text','text':'Read this tag.'},{'type':'image_url','image_url':{'url':'data:image/png;base64,AAAA','detail':'high'}}]}]}
        parts=module.chat_body(body)['input'][0]['content']
        self.assertEqual(parts,[{'type':'input_text','text':'Read this tag.'},{'type':'input_image','image_url':'data:image/png;base64,AAAA','detail':'high'}])
        body['messages'][1]['content'][1]['image_url']['url']='https://example.test/private.png'
        with self.assertRaises(ValueError):module.chat_body(body)

    def test_oversized_output_is_rejected(self):
        with self.assertRaises(ValueError):module.stream_text(['data: '+json.dumps({'type':'response.output_text.delta','delta':'x'*(1024*1024+1)})])


class PersonalCredentials(unittest.TestCase):
    def runtime(self, stores, expired=True):
        import contextlib
        import tempfile
        import types
        from unittest.mock import Mock, patch
        stack=contextlib.ExitStack()
        self.addCleanup(stack.close)
        root=Path(stack.enter_context(tempfile.TemporaryDirectory()))
        home=root/'profile';home.mkdir()
        constants=types.ModuleType('hermes_constants')
        constants.get_hermes_home=lambda:home
        constants.get_default_hermes_root=lambda:root
        auth=types.ModuleType('hermes_cli.auth')
        auth._load_auth_store=Mock(side_effect=stores)
        auth._save_auth_store=Mock()
        held=[]
        @contextlib.contextmanager
        def lock():
            held.append(True)
            try:yield
            finally:held.pop()
        auth._auth_store_lock=lock
        codex=types.ModuleType('hermes_cli.auth_codex')
        codex._codex_access_token_is_expiring=Mock(side_effect=expired if isinstance(expired,list) else None,return_value=expired)
        def renew(access,refresh):
            self.assertTrue(held)
            self.assertEqual((access,refresh),('owned-access','owned-refresh'))
            return {'access_token':'renewed-access','refresh_token':'renewed-refresh'}
        codex.refresh_codex_oauth_pure=Mock(side_effect=renew)
        stack.enter_context(patch.dict('sys.modules',{'hermes_constants':constants,'hermes_cli.auth':auth,'hermes_cli.auth_codex':codex}))
        return constants,auth,codex

    def store(self,**changes):
        entry={'label':'personal','source':'manual:device_code','access_token':'owned-access','refresh_token':'owned-refresh'}
        entry.update(changes)
        return {'credential_pool':{'openai-codex':[entry]},'unrelated':{'preserve':True}}

    def test_refresh_owns_one_grant_under_lock(self):
        first=self.store();locked=self.store()
        _,auth,codex=self.runtime([first,locked])
        self.assertEqual(module.personal.personal_credential(True)[0],'renewed-access')
        codex.refresh_codex_oauth_pure.assert_called_once()
        auth._save_auth_store.assert_called_once_with(locked)
        self.assertTrue(locked['unrelated']['preserve'])
        self.assertEqual(len(locked['credential_pool']['openai-codex']),1)

    def test_fresh_credential_never_loads_general_pool(self):
        _,auth,codex=self.runtime([self.store()],False)
        self.assertEqual(module.personal.personal_credential(True)[0],'owned-access')
        auth._save_auth_store.assert_not_called()
        codex.refresh_codex_oauth_pure.assert_not_called()

    def test_ambiguous_credentials_fail_without_refresh(self):
        store=self.store();store['credential_pool']['openai-codex'].append({'source':'device_code'})
        _,auth,codex=self.runtime([store])
        with self.assertRaises(RuntimeError):module.personal.personal_credential(True)
        auth._save_auth_store.assert_not_called()
        codex.refresh_codex_oauth_pure.assert_not_called()

    def test_root_profile_cannot_be_used(self):
        constants,auth,codex=self.runtime([])
        constants.get_hermes_home=constants.get_default_hermes_root
        with self.assertRaises(RuntimeError):module.personal.personal_credential(True)
        auth._load_auth_store.assert_not_called()
        codex.refresh_codex_oauth_pure.assert_not_called()

    def test_refresh_rechecks_concurrent_rotation(self):
        _,auth,codex=self.runtime([self.store(),self.store(access_token='peer-access')],[True,False])
        self.assertEqual(module.personal.personal_credential(True)[0],'peer-access')
        codex.refresh_codex_oauth_pure.assert_not_called()
        auth._save_auth_store.assert_not_called()

    def test_external_route_rejected_before_refresh(self):
        _,auth,codex=self.runtime([self.store(base_url='https://example.test/v1')])
        with self.assertRaises(RuntimeError):module.personal.personal_credential(True)
        codex.refresh_codex_oauth_pure.assert_not_called()

    def test_expired_credential_requires_explicit_refresh(self):
        _,auth,codex=self.runtime([self.store()])
        with self.assertRaises(RuntimeError):module.personal.personal_credential(False)
        codex.refresh_codex_oauth_pure.assert_not_called()

if __name__=='__main__':unittest.main()
