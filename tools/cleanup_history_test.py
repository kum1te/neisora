"""Destructive cleanup must preserve the new release and its verification run."""
import importlib.util,json,unittest
from pathlib import Path
spec=importlib.util.spec_from_file_location('cleanup',Path(__file__).with_name('clean-release-history.py'))
cleanup=importlib.util.module_from_spec(spec);spec.loader.exec_module(cleanup)
class CleanupTests(unittest.TestCase):
 def setUp(self):
  self.deleted=[];self.release={'id':200,'tag_name':'v1.0.0','draft':False,'prerelease':False}
  cleanup.gh=self.gh
 def gh(self,*args):
  endpoint=args[-1]
  if 'DELETE' in args:self.deleted.append(endpoint);return ''
  if endpoint.endswith('/releases/tags/v1.0.0'):value=self.release
  elif '/git/matching-refs/tags/' in endpoint:
   value=[{'ref':'refs/tags/v0.6.3'},{'ref':'refs/tags/v0.6.3-extra'}] if endpoint.endswith('/v0.6.3') else []
  elif '/releases?' in endpoint:value=[[{'id':100,'tag_name':'v0.6.3'},self.release]]
  elif '/actions/runs?' in endpoint:value=[{'workflow_runs':[{'id':11,'status':'in_progress'},{'id':22,'status':'completed'}]}]
  elif '/actions/artifacts?' in endpoint:value=[{'artifacts':[{'id':44,'workflow_run':{'id':11}},{'id':55,'workflow_run':{'id':22}}]}]
  elif '/actions/caches?' in endpoint:value=[{'actions_caches':[{'id':66}]}]
  else:raise AssertionError(endpoint)
  return json.dumps(value)
 def test_dry_run_never_mutates(self):
  cleanup.clean(11,False);self.assertEqual(self.deleted,[])
 def test_execution_preserves_current_release_run_and_artifacts(self):
  cleanup.clean(11,True)
  self.assertEqual(set(self.deleted),{f'repos/{cleanup.REPO}/{p}' for p in ['releases/100','git/refs/tags/v0.6.3','actions/runs/22','actions/artifacts/55','actions/caches/66']})
 def test_unpublished_release_prevents_every_deletion(self):
  self.release['draft']=True
  with self.assertRaises(RuntimeError):cleanup.clean(11,True)
  self.assertEqual(self.deleted,[])
 def test_another_active_run_is_not_deleted(self):
  with self.assertRaises(RuntimeError):cleanup.old_items([{'workflow_runs':[{'id':22,'status':'in_progress'}]}],'workflow_runs',11)
if __name__=='__main__':unittest.main()
