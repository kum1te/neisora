"""Read-only live checks for the reported examples in a fresh production profile."""
import os,sys,time
from webdriver_client import create, execute, invoke, request, sid

assert os.environ.get('NEISORA_SMOKE_CONFIG','').startswith('/tmp/neisora-')
if '--create' in sys.argv:
    create()
    for _ in range(100):
        if execute('return !!document.querySelector(".search input")'):break
        time.sleep(.1)
    else:raise AssertionError('App did not initialize')
status=invoke('status')
assert status['ok'] and not status['value']['authenticated'], 'Requires an isolated unauthenticated profile'
for title,artist,duration,video,video_id in [
    ('Sellit!','Niko B',93,False,'2P7Cm1GDd6I'),
    ('ДИНАСТИЯ','madk1d и VILLIAN',165,True,'J7NFL-eOxiQ'),
]:
    metadata={'title':title,'artist':artist,'duration':duration,'musicVideo':video}
    recording=invoke('get_recording',metadata)
    assert recording['ok'] and recording['value'] and recording['value']['image'], recording
    if video: assert recording['value']['duration']==150, recording
    # Loading is tested through the WebView too, including the production CSP.
    loaded=request('/session/'+sid()+'/execute/async',{'args':[recording['value']['image']],'script':'''
      const url=arguments[0],done=arguments[arguments.length-1],img=new Image();
      img.referrerPolicy='no-referrer';
      const timer=setTimeout(()=>done(false),6000);
      img.onload=()=>{clearTimeout(timer);done(img.naturalWidth>=600 && img.naturalHeight>=600)};
      img.onerror=()=>{clearTimeout(timer);done(false)};img.src=url;
    '''})
    assert loaded, 'Artwork could not load at full size in the WebView'
    lyrics=invoke('get_lyrics',{'id':video_id,**metadata,'refresh':True})
    assert lyrics['ok'], lyrics
    value=lyrics['value']
    assert value['plain'] or value['lines']
    if video: assert value['synced'], 'The official clip should have timed lyrics'
    print(f'{title}: artwork loads; lyrics source={value["source"]}, synced={value["synced"]}, lines={len(value["lines"])}',flush=True)
