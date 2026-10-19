"""Library/like/history/MPRIS regressions in an isolated fixture profile/session bus."""
import subprocess, time
from webdriver_client import execute, invoke

def wait(script, seconds=8):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        if execute(script):return
        time.sleep(.05)
    print(execute('return {heading:document.querySelector("h1")?.textContent,requests:featuresRequests.slice(-8),collectionButtons:document.querySelectorAll(".collection-actions button").length}'),flush=True)
    raise AssertionError(script)

def bus(interface,method,*args):
    return subprocess.check_output(['gdbus','call','--session','--dest','org.mpris.MediaPlayer2.neisora','--object-path','/org/mpris/MediaPlayer2','--method',interface+'.'+method,*args],text=True,timeout=5)

execute('''
window.featuresBase=window.__VESPER_TEST_INVOKE__;window.featuresLikes=new Map();window.featuresLikeCalls=[];window.featuresSaved=false;window.featuresBrowseCompleted=0;window.featuresRequests=[];
window.__VESPER_TEST_INVOKE__=async(cmd,args,...rest)=>{
 featuresRequests.push({cmd,args});
 if(cmd==='status')return {authenticated:true,browser:[],mpv:true,extractor:true,volume:60};
 if(cmd==='track_like')return featuresLikes.get(args.id)??false;
 if(cmd==='like'){featuresLikeCalls.push(args);await new Promise(r=>setTimeout(r,80));featuresLikes.set(args.id,args.liked);return null;}
 if(cmd==='library')return {...structuredClone(testFeed),title:'Медиатека: '+args.category};
 if(cmd==='save_collection'){featuresSaved=args.saved;return null;}
 if(cmd==='browse'&&(args.endpoint.browseEndpoint??args.endpoint).browseId==='VLfixture')return {...structuredClone(testFeed),title:'Fixture playlist',collection:{browseId:'VLfixture',playlistId:'fixture',saved:featuresSaved}};
 if(cmd==='browse'){const value=await featuresBase(cmd,args,...rest);featuresBrowseCompleted++;return value;}
 return featuresBase(cmd,args,...rest);
};
''')
invoke('plugin:event|emit',{'event':'auth-updated','payload':None})
wait("return !!document.querySelector('.hero .primary')")
execute("document.querySelector('.hero .primary').click()")
wait("return document.querySelector('.now-title').textContent==='Track 0' && !document.querySelector('.main-play').disabled")
execute("document.querySelector('.like-button').click();document.querySelector('.like-button').click()")
wait("return document.querySelector('.like-button').classList.contains('liked')")
assert execute('return featuresLikeCalls.length===1'), 'Duplicate in-flight like mutation'
execute("document.querySelector('.hero .primary').click()")
wait("return document.querySelector('.now-title').textContent==='Track 0' && !document.querySelector('.main-play').disabled")
assert execute("return document.querySelector('.like-button').classList.contains('liked')")
print('likes survive replay and serialize mutations: PASS',flush=True)
execute("[...document.querySelectorAll('.sidebar nav button')].find(b=>b.textContent==='Медиатека').click()")
wait("return document.querySelectorAll('.library-tabs button').length===5")
execute("document.querySelectorAll('.library-tabs button')[1].click()")
wait("return document.querySelector('h1').textContent==='Медиатека: albums'")
execute("document.querySelectorAll('.library-tabs button')[0].click()")
wait("return document.querySelector('h1').textContent==='Медиатека: playlists'")
execute("document.querySelector('.history button[aria-label=\"Назад\"]').click()")
wait("return document.querySelector('h1').textContent==='Медиатека: albums' && document.querySelectorAll('.library-tabs button')[1].classList.contains('active')")
execute("window.featuresBrowseBefore=featuresBrowseCompleted;testFeed.sections[0].items[0].endpoint={browseEndpoint:{browseId:'VLfixture'}};document.querySelector('nav button').click()")
wait("return featuresBrowseCompleted>featuresBrowseBefore && !!document.querySelector('.hero .secondary')")
execute("document.querySelector('.hero .secondary').click()")
wait("return !!document.querySelector('.collection-actions button')")
execute("document.querySelector('.collection-actions button').click()")
wait("return document.querySelector('.collection-actions button').textContent.includes('В медиатеке')")
execute("document.querySelector('.collection-actions button').click()")
wait("return !featuresSaved")
print('library categories and save/remove collection: PASS',flush=True)
execute("document.querySelector('.toast button')?.click();testFeed.sections[0].items[0].endpoint={watchPlaylistEndpoint:{playlistId:'RDtest'}};document.querySelector('nav button').click()")
wait("return !!document.querySelector('.hero .primary')")
assert invoke('clear_history')['ok']
execute("document.querySelector('.hero .primary').click()")
wait("return !document.querySelector('.main-play').disabled")
for n in range(25):
    execute(f'emitState({{position:{n/4}}})');time.sleep(.25)
entries=invoke('listening_history');assert entries['ok'] and len(entries['value'])==1, entries
execute("[...document.querySelectorAll('.sidebar nav button')].find(b=>b.textContent==='История').click()")
wait("return document.querySelectorAll('.history-track').length===1")
execute("const n=document.querySelector('input[aria-label=\"Поиск в истории\"]');n.value='no such song';n.dispatchEvent(new Event('input',{bubbles:true}))")
wait("return document.querySelectorAll('.history-track').length===0")
execute("const n=document.querySelector('input[aria-label=\"Поиск в истории\"]');n.value='';n.dispatchEvent(new Event('input',{bubbles:true}))")
wait("return document.querySelectorAll('.history-track').length===1")
print('history records audible playback once and searches locally: PASS',flush=True)
time.sleep(.5)
metadata=bus('org.freedesktop.DBus.Properties','Get','org.mpris.MediaPlayer2.Player','Metadata')
assert 'Track 0' in metadata,metadata
before=execute("return testCalls.filter(c=>c.cmd==='control'&&c.args.action==='toggle').length")
assert invoke('plugin:window|minimize',{'label':'main'})['ok']
bus('org.mpris.MediaPlayer2.Player','PlayPause')
wait(f"return testCalls.filter(c=>c.cmd==='control'&&c.args.action==='toggle').length==={before+1}")
bus('org.freedesktop.DBus.Properties','Set','org.mpris.MediaPlayer2.Player','Volume','<0.42>')
wait("return testCalls.some(c=>c.cmd==='control'&&c.args.action==='volume_commit'&&c.args.value===42)")
bus('org.mpris.MediaPlayer2.Player','Seek','1000000')
wait("return testCalls.some(c=>c.cmd==='control'&&c.args.action==='seek'&&c.args.value>6)")
before=execute("return testCalls.filter(c=>c.cmd==='control'&&c.args.action==='seek').length")
bus('org.mpris.MediaPlayer2.Player','SetPosition','/org/mpris/MediaPlayer2/track/stale','2000000')
time.sleep(.1)
assert execute(f"return testCalls.filter(c=>c.cmd==='control'&&c.args.action==='seek').length==={before}")
before=execute("return testCalls.filter(c=>c.cmd==='refresh_queue').length")
bus('org.mpris.MediaPlayer2.Player','Next')
wait("return document.querySelector('.now-title').textContent==='Track 1'")
wait(f"return testCalls.filter(c=>c.cmd==='refresh_queue').length==={before+1}")
bus('org.mpris.MediaPlayer2','Raise')
print('real MPRIS bus without window focus: metadata, play/pause, volume, seek, stale seek and radio Next: PASS',flush=True)
execute("document.querySelector('.history-clear button').click();document.querySelector('.history-clear button').click()")
wait("return !document.querySelector('.history-track') && document.querySelector('h2')?.textContent==='Здесь будет твоя музыка'")
assert invoke('listening_history')['value']==[]
execute("document.querySelector('nav button').click();document.querySelector('.toast button')?.click();window.__VESPER_TEST_INVOKE__=featuresBase")
wait("return !!document.querySelector('.hero .primary')")
print('history can be cleared without affecting playback: PASS',flush=True)
