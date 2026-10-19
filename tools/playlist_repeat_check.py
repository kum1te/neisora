"""Real WebKit UI checks, isolated fixtures only: repeat, playlist writes and narrow windows."""
import json
import time
import subprocess
from pathlib import Path
from webdriver_client import execute, invoke

def wait(script, seconds=8):
    until=time.monotonic()+seconds
    while time.monotonic()<until:
        if execute(script): return
        time.sleep(.05)
    raise AssertionError(script + '\n' + str(execute('return {body:document.body.innerText.slice(-1800),calls:playlistCalls?.slice(-3)}')))

def click(selector):
    execute('document.querySelector('+json.dumps(selector)+').click()')

def bus(method,*args):
    return subprocess.check_output(['gdbus','call','--session','--dest','org.mpris.MediaPlayer2.neisora','--object-path','/org/mpris/MediaPlayer2','--method','org.freedesktop.DBus.Properties.'+method,*args],text=True,timeout=5)

execute('''
window.playlistBase=window.__VESPER_TEST_INVOKE__;window.playlistOldQueue=structuredClone(testQueue);window.playlistCalls=[];
window.playlistTracks=[0,1,2].map(n=>({...makeTrack(n),setVideoId:'entry-'+n}));playlistTracks[1].id=playlistTracks[0].id;
window.playlistTitle='Fixture playlist';window.playlistDescription='';window.playlistPrivacy='PRIVATE';window.playlistReject=false;
window.playlistFeed=()=>({title:playlistTitle,authenticated:true,continuation:null,collection:{browseId:'VLPLfixture',playlistId:'PLfixture',saved:true,editable:true,description:playlistDescription,privacy:playlistPrivacy},sections:[{title:'Треки',subtitle:'',items:structuredClone(playlistTracks)}]});
window.__VESPER_TEST_INVOKE__=async(cmd,args,...rest)=>{
 if(cmd==='status')return {authenticated:true,browser:[],mpv:true,extractor:true,volume:60};
 if(cmd==='track_like')return false;
 if(cmd==='search'){
  if(args.query==='slow')return new Promise(resolve=>{window.playlistSlowSearch=()=>resolve({sections:[{items:[makeTrack(200)]}]});});
  return {sections:[{items:[makeTrack(201),makeTrack(202),makeTrack(201),{...makeTrack(203),kind:'album'}]}]};
 }
 if(cmd==='library')return {title:'Медиатека',authenticated:true,continuation:null,sections:[{title:'Плейлисты',subtitle:'',items:[{...makeTrack(99),id:'PLfixture',title:playlistTitle,kind:'playlist',endpoint:{browseEndpoint:{browseId:'VLPLfixture'}}}]}]};
 if(cmd==='browse'&&(args.endpoint.browseEndpoint??args.endpoint).browseId?.startsWith('VLPL'))return playlistFeed();
 if(cmd==='playlist_change'){
  playlistCalls.push(structuredClone(args.change));
  if(playlistReject)throw 'Fixture: YouTube не подтвердил изменение';
  const c=args.change;
  if(c.action==='create'||c.action==='update'){playlistTitle=c.title;playlistDescription=c.description;playlistPrivacy=c.privacy;}
  if(c.action==='add'&&window.playlistCaptureAdds)playlistTracks.push({...makeTrack(Number(c.videoId.slice(4))),setVideoId:'added-'+playlistCalls.length});
  if(c.action==='remove')playlistTracks=playlistTracks.filter(i=>i.setVideoId!==c.setVideoId);
  if(c.action==='move'){const at=playlistTracks.findIndex(i=>i.setVideoId===c.setVideoId);const [item]=playlistTracks.splice(at,1);const to=c.before?playlistTracks.findIndex(i=>i.setVideoId===c.before):playlistTracks.length;playlistTracks.splice(to,0,item);}
  return c.action==='create'?'PLfixture':null;
 }
 return playlistBase(cmd,args,...rest);
};
testQueue={title:'Finite playlist',items:[0,1,2].map(makeTrack),selected:0,dynamic:false,context:{playlistId:'PLfixture'},continuation:null};
''')
invoke('plugin:event|emit',{'event':'auth-updated','payload':None})
wait("return !!document.querySelector('.hero .primary')")
click('.hero .primary')
wait("return document.querySelector('.now-title').textContent==='Track 0' && !document.querySelector('.main-play').disabled")
# Repeat controls + actual Linux bus integration.
for _ in range(3):
    if execute("return document.querySelector('.playback .repeat-button').getAttribute('aria-label')==='Повтор выключен'"): break
    click('.playback .repeat-button')
click('.playback .repeat-button')
wait("return document.querySelector('.playback .repeat-button').getAttribute('aria-label')==='Повтор очереди'")
click('.playback .repeat-button')
wait("return document.querySelector('.playback .repeat-button').getAttribute('aria-label')==='Повтор одного трека'")
before=execute("return testCalls.filter(c=>c.cmd==='play').length")
refresh=execute("return testCalls.filter(c=>c.cmd==='refresh_queue').length")
invoke('plugin:event|emit',{'event':'track-ended','payload':None})
wait(f"return testCalls.filter(c=>c.cmd==='play').length==={before+1} && !document.querySelector('.main-play').disabled")
assert execute("return document.querySelector('.now-title').textContent==='Track 0'")
assert execute(f"return testCalls.filter(c=>c.cmd==='refresh_queue').length==={refresh}")
click('.playback [aria-label="Следующий трек"]')
wait("return document.querySelector('.now-title').textContent==='Track 1' && !document.querySelector('.main-play').disabled")
bus('Set','org.mpris.MediaPlayer2.Player','LoopStatus',"<'Playlist'>")
wait("return document.querySelector('.playback .repeat-button').getAttribute('aria-label')==='Повтор очереди'")
time.sleep(.4)
assert 'Playlist' in bus('Get','org.mpris.MediaPlayer2.Player','LoopStatus')
click('.playback [aria-label="Следующий трек"]')
wait("return document.querySelector('.now-title').textContent==='Track 2' && !document.querySelector('.main-play').disabled")
invoke('plugin:event|emit',{'event':'track-ended','payload':None})
wait("return document.querySelector('.now-title').textContent==='Track 0' && !document.querySelector('.main-play').disabled")
click('.playback .repeat-button');click('.playback .repeat-button')
print('repeat: EOF replays one, manual Next advances, finite playlist wraps and MPRIS LoopStatus works: PASS',flush=True)

# A real form emits typed mutations; failures keep dialog open without falsely announcing success.
click('nav button[aria-label="Медиатека"]')
wait("return !!document.querySelector('.library-tabs')")
click('.library-tabs button')
wait("return !!document.querySelector('.new-playlist')")
click('.new-playlist')
wait("return !!document.querySelector('.playlist-dialog[open]')")
execute('''const input=document.querySelector('[aria-label="Название плейлиста"]');input.value='My test playlist';input.dispatchEvent(new Event('input',{bubbles:true}));playlistReject=true;''')
click('.playlist-dialog button[type=submit]')
wait("return document.querySelector('.playlist-error')?.textContent.includes('не подтвердил')")
assert execute("return !!document.querySelector('.playlist-dialog[open]')")
execute('playlistReject=false')
click('.playlist-dialog button[type=submit]')
wait("return !document.querySelector('.playlist-dialog') && document.querySelector('h1').textContent==='My test playlist'")
assert execute("return playlistCalls.at(-1).privacy==='PRIVATE'")
click('.track-menu summary')
execute("[...document.querySelectorAll('.track-menu-items button')].find(b=>b.textContent.includes('Добавить в плейлист')).click()")
wait("return document.querySelectorAll('.playlist-choices button').length===1")
click('.playlist-choices button')
wait("return !document.querySelector('.playlist-dialog') && playlistCalls.at(-1).action==='add'")
# Remove second occurrence of the same video, never the first.
execute("document.querySelectorAll('.track-menu summary')[1].click();[...document.querySelectorAll('.track-menu[open] button')].find(b=>b.textContent.includes('Убрать из плейлиста')).click()")
wait("return playlistTracks.length===2 && document.querySelectorAll('.track-menu').length===2")
assert execute("return playlistCalls.at(-1).setVideoId==='entry-1' && playlistTracks[0].setVideoId==='entry-0'")
execute("document.querySelector('.track-menu summary').click();[...document.querySelectorAll('.track-menu[open] button')].find(b=>b.textContent.includes('Переместить ниже')).click()")
wait("return playlistTracks[1].setVideoId==='entry-0' && !document.querySelector('.track-menu[open]')")
execute("[...document.querySelectorAll('.collection-actions button')].find(b=>b.textContent.includes('Изменить')).click()")
wait("return !!document.querySelector('.playlist-dialog[open]')")
execute('''const input=document.querySelector('[aria-label="Название плейлиста"]');input.value='Renamed playlist';input.dispatchEvent(new Event('input',{bubbles:true}));document.querySelector('input[name="playlist-privacy"][value="PUBLIC"]').click();''')
click('.playlist-dialog button[type=submit]')
wait("return !document.querySelector('.playlist-dialog') && document.querySelector('h1').textContent==='Renamed playlist'")
assert execute("return playlistPrivacy==='PUBLIC'")
click('.track-menu summary')
execute("[...document.querySelectorAll('.track-menu-items button')].find(b=>b.textContent.includes('Добавить в плейлист')).click()")
wait("return !!document.querySelector('.playlist-choices')")
click('.playlist-dialog .secondary')
wait("return !!document.querySelector('[aria-label=\"Видимость плейлиста\"]')")
assert execute("return document.querySelector('input[name=\"playlist-privacy\"]:checked').value==='PRIVATE'"), 'New playlist must not inherit PUBLIC privacy from the currently viewed playlist'
click('.playlist-dialog [aria-label="Закрыть"]')
click('[aria-label="Удалить плейлист"]')
assert execute("return playlistCalls.at(-1).action==='update'")
wait("return !!document.querySelector('.playlist-dialog[open]')")
click('.playlist-dialog .danger')
wait("return !document.querySelector('.playlist-dialog') && playlistCalls.at(-1).action==='delete' && !!document.querySelector('.new-playlist')")
print('playlists: create PRIVATE by default, add, remove exact duplicate, move, rename, delete confirmation and failed-write UI: PASS',flush=True)

# Empty owned playlists expose an obvious add action. Search remains in the dialog,
# ignores a late response and permits several additions before a single refresh.
execute("playlistTracks=[];playlistCaptureAdds=true")
click('.new-playlist')
wait("return !!document.querySelector('.playlist-dialog[open]')")
execute("const e=document.querySelector('[aria-label=\"Название плейлиста\"]');e.value='Empty playlist';e.dispatchEvent(new Event('input',{bubbles:true}))")
click('.playlist-dialog button[type=submit]')
wait("return !document.querySelector('.playlist-dialog') && !!document.querySelector('.playlist-add-tracks') && !document.querySelector('.track-menu')")
click('.playlist-add-tracks')
wait("return document.activeElement?.getAttribute('aria-label')==='Найти треки для плейлиста'")
def search_tracks(query):
    execute("const e=document.querySelector('[aria-label=\"Найти треки для плейлиста\"]');e.value="+json.dumps(query)+';e.dispatchEvent(new Event("input",{bubbles:true}));document.querySelector(".playlist-search").requestSubmit()')
search_tracks('slow')
wait("return typeof playlistSlowSearch==='function'")
search_tracks('fresh')
wait("return document.querySelectorAll('.playlist-result').length===2")
execute('playlistSlowSearch()')
time.sleep(.15)
assert execute("return document.querySelector('.playlist-result strong').textContent==='Track 201'")
execute('playlistReject=true')
click('.playlist-result button')
wait("return !!document.querySelector('.playlist-error')")
assert execute("return playlistTracks.length===0 && !document.querySelector('.playlist-result button').disabled")
execute('playlistReject=false')
click('.playlist-result button')
wait("return playlistTracks.length===1 && document.querySelector('.playlist-result button').disabled")
assert execute("return !!document.querySelector('.playlist-dialog[open]') && playlistCalls.at(-1).playlistId==='PLfixture' && playlistCalls.at(-1).videoId==='test0000201'")
execute("document.querySelectorAll('.playlist-result button')[1].click()")
wait("return playlistTracks.length===2 && document.querySelectorAll('.playlist-result button:disabled').length===2")
click('.playlist-dialog-actions .primary')
wait("return !document.querySelector('.playlist-dialog') && document.querySelectorAll('.track-menu').length===2")
print('playlist discovery: empty playlist Add tracks, stale search isolation, error retry, duplicate suppression, multiple adds and refresh: PASS',flush=True)

# Direct X11 resize works under Xvfb without a window manager. WebDriver's
# SetWindowRect otherwise waits for WM acknowledgement that never arrives.
window_id=subprocess.check_output(['xdotool','search','--onlyvisible','--class','neisora'],text=True,timeout=5).splitlines()[0]
def resize(width,height):
    subprocess.run(['xdotool','windowsize',window_id,str(width),str(height)],check=True,timeout=5)
    wait(f'return innerWidth==={width} && innerHeight==={height}')
# WebKit geometry, not just matching CSS strings. Every core control must remain clickable.
for width,height in [(1360,860),(1024,700),(800,600),(640,600),(480,600),(420,440)]:
    resize(width,height)
    time.sleep(.2)
    result=execute('''
const selectors=['.like-button','.playback .repeat-button','.playback .main-play','.playback [aria-label="Следующий трек"]','.player-options .mute-button','.player-options .expand-player','.seek input','.search input'];
return {width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth,controls:selectors.map(selector=>{const e=document.querySelector(selector),r=e?.getBoundingClientRect(),style=e&&getComputedStyle(e);const hit=r&&document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return {selector,rect:r&&{x:r.x,y:r.y,w:r.width,h:r.height},visible:!!r&&r.width>4&&r.height>4&&r.x>=0&&r.y>=0&&r.right<=innerWidth+1&&r.bottom<=innerHeight+1&&style.visibility==='visible'&&style.display!=='none'&&!!hit&&(e===hit||e.contains(hit))};})};''')
    assert not result['overflow'],result
    assert all(c['visible'] for c in result['controls']),result
    Path('artifacts').mkdir(exist_ok=True)
    subprocess.run(['import','-window',window_id,f'artifacts/responsive-{width}.png'],check=True,timeout=10)
    print(f'responsive {width}x{height}: core controls PASS',flush=True)
    click('.expand-player')
    wait("return !!document.querySelector('.expanded-player')")
    assert execute('''const s=document.querySelector('[aria-label="Свернуть плеер"]'),r=s.getBoundingClientRect();return r.right<=innerWidth&&r.bottom<=innerHeight&&r.x>=0&&r.y>=0''')
    assert execute("return [...document.querySelectorAll('.expanded-controls button')].every(e=>{const r=e.getBoundingClientRect();return r.x>=0&&r.right<=innerWidth+1&&r.y>=0&&r.bottom<=innerHeight+1})"), execute("return [...document.querySelectorAll('.expanded-controls button')].map(e=>({label:e.getAttribute('aria-label'),rect:e.getBoundingClientRect()}))")
    if width == 1360:
        click('[aria-label="Добавить текущий трек в плейлист"]')
        wait("return !!document.querySelector('.playlist-dialog[open]')")
        click('.playlist-dialog [aria-label="Закрыть"]')
    click('[aria-label="Свернуть плеер"]')
    wait("return !document.querySelector('.expanded-player')")
    if width == 420:
        click('.playlist-add-tracks')
        wait("return !!document.querySelector('.playlist-search')")
        assert execute('''const d=document.querySelector('.playlist-dialog'),r=d.getBoundingClientRect();return r.x>=0&&r.right<=innerWidth&&r.y>=0&&r.bottom<=innerHeight&&d.scrollWidth<=d.clientWidth''')
        click('.playlist-dialog [aria-label="Закрыть"]')
# Settings and local-library controls can scroll vertically, but must fit horizontally.
click('nav button[aria-label="Настройки"]')
wait("return !!document.querySelector('.settings-page')")
time.sleep(.25)
assert execute("return [...document.querySelectorAll('.settings-page button,.settings-page input')].every(e=>{const r=e.getBoundingClientRect();return !r.width || (r.x>=60&&r.right<=innerWidth+1)})"),execute("return document.querySelector('.settings-page').getBoundingClientRect()")
click('nav button[aria-label="На устройстве"]')
wait("return !!document.querySelector('.local-filters')")
assert execute("return [...document.querySelectorAll('.local-filters input,.local-filters select')].every(e=>{const r=e.getBoundingClientRect();return r.x>=60&&r.right<=innerWidth+1})")
print('responsive: core controls unobstructed, settings and local filters fit at 420 px: PASS',flush=True)
resize(1360,860)
execute("testQueue=playlistOldQueue;window.__VESPER_TEST_INVOKE__=playlistBase;document.querySelector('nav button').click();document.querySelector('.toast button')?.click()")
wait("return !!document.querySelector('.hero .primary')")
