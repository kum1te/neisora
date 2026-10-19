"""One-time, maintainer-authorized 1.0.0 cleanup. Never deletes the current run.

Run only in the publish job after both platforms and release creation succeed.
Default is a dry run. Application data and local backup bundles are untouched.
"""
import argparse,json,os,subprocess

REPO='kum1te/neisora'
OLD_TAGS=('v0.5.0','v0.6.0','v0.6.1','v0.6.2','v0.6.3')
def gh(*args):
    return subprocess.check_output(['gh',*args],text=True)
def pages(endpoint):
    return json.loads(gh('api','--paginate','--slurp',endpoint+'?per_page=100'))
def old_items(data,key,current):
    result=[]
    for page in data:
        for item in page[key]:
            if key=='workflow_runs':
                if item['id']==current:continue
                if item['status']!='completed':
                    raise RuntimeError('Another workflow is active; refusing to delete it')
            elif key=='artifacts' and item.get('workflow_run',{}).get('id')==current:continue
            result.append(int(item['id']))
    return result

def clean(current,execute):
    # Confirm the new release exists before deleting any prior result.
    release=json.loads(gh('api',f'repos/{REPO}/releases/tags/v1.0.0'))
    if release.get('tag_name')!='v1.0.0' or release.get('draft') or release.get('prerelease'):
        raise RuntimeError('1.0.0 must be a published release before cleanup')
    for tag in OLD_TAGS:
        # Missing pre-1.0 releases/tags are normal; do not make broader deletions.
        releases=pages(f'repos/{REPO}/releases')
        existing=next((r for page in releases for r in page if r['tag_name']==tag),None)
        if existing:
            print('Old release:',tag,flush=True)
            if execute:gh('api','--method','DELETE',f'repos/{REPO}/releases/{int(existing["id"])}')
        refs=json.loads(gh('api',f'repos/{REPO}/git/matching-refs/tags/{tag}'))
        if any(r['ref']=='refs/tags/'+tag for r in refs):
            print('Old tag:',tag,flush=True)
            if execute:gh('api','--method','DELETE',f'repos/{REPO}/git/refs/tags/{tag}')
    for key,endpoint in [('workflow_runs','runs'),('artifacts','artifacts'),('actions_caches','caches')]:
        ids=old_items(pages(f'repos/{REPO}/actions/{endpoint}'),key,current)
        print(f'Old {key}: {len(ids)}; keeping current run {current}',flush=True)
        if execute:
            for identifier in ids:gh('api','--method','DELETE',f'repos/{REPO}/actions/{endpoint}/{identifier}')

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--execute',action='store_true');args=parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS')!='true' or os.environ.get('GITHUB_REPOSITORY')!=REPO:
        raise SystemExit('Only the authorized repository publish job may run this cleanup')
    if os.environ.get('RELEASE_TAG')!='v1.0.0':raise SystemExit('Cleanup is restricted to 1.0.0')
    clean(int(os.environ['GITHUB_RUN_ID']),args.execute)
