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

if __name__=='__main__':unittest.main()
