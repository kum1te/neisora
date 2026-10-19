"""Manual lyric selection races and native output UI in the isolated fixture suite."""
import json,time,subprocess
from webdriver_client import execute,invoke

def wait(script):
    end=time.monotonic()+12
    while time.monotonic()<end:
        if execute(script):return
        time.sleep(.05)
    raise AssertionError(script+'\n'+str(execute('return document.body.innerText.slice(-1600)')))
def click(selector):execute('document.querySelector('+json.dumps(selector)+').click()')
execute('''window.lyricBase=window.__VESPER_TEST_INVOKE__;window.lyricSaved={};window.lyricCalls=[];window.failLyricSave=false;window.outputFixture={preferred:'auto',selected:'auto',fallback:false,silent:false,devices:[{name:'auto',description:'Как в системе'},{name:'pulse/test',description:'Test headphones'}]};
window.lyricRecord={id:42,title:'Selected recording',artist:'Test artist',album:'Album',duration:200,synced:true,wordTimed:false};
window.lyricValue={record:lyricRecord,lyrics:{source:'LRCLIB',synced:true,plain:'Selected line',lines:[{time:0,text:'Selected line'},{time:12,text:'Second selected line'}]}};
window.__VESPER_TEST_INVOKE__=(cmd,args,...rest)=>{
 lyricCalls.push({cmd,args:JSON.parse(JSON.stringify(args??{}))});
 if(cmd==='get_lyric_choice')return Promise.resolve(lyricSaved[args.id]??null);
 if(cmd==='lyric_versions')return Promise.resolve([lyricRecord]);
 if(cmd==='preview_lyric_version')return Promise.resolve(lyricValue);
 if(cmd==='select_lyric_version'){if(failLyricSave)return Promise.reject('Cannot save fixture');const value=args.recordId===null?null:lyricValue;lyricSaved[args.id]=value;return Promise.resolve(value);}
 if(cmd==='get_word_lyrics')return Promise.resolve(null);
 if(cmd==='get_lyrics')return new Promise(r=>setTimeout(()=>r({source:'Automatic',synced:true,plain:'',lines:[{time:0,text:'Automatic line'}]}),1200));
 if(cmd==='audio_outputs')return Promise.resolve(outputFixture);
 if(cmd==='set_audio_output'){outputFixture={...outputFixture,preferred:args.name,selected:args.name,fallback:false};return Promise.resolve(outputFixture);}
 return lyricBase(cmd,args,...rest);
};document.querySelector('nav button').click();''')
wait("return !!document.querySelector('.hero .primary')")
click('.hero .primary');wait("return !document.querySelector('.main-play').disabled")
execute("localStorage.setItem('player:lyrics-visible','true')")
click('.expand-player');wait("return !!document.querySelector('[aria-label=\"Выбрать версию текста\"]')")
click('[aria-label="Выбрать версию текста"]');wait("return !!document.querySelector('.version-result')")
click('.version-result');wait("return document.querySelector('.version-preview pre')?.textContent==='Selected line'")
execute('failLyricSave=true');click('.lyric-versions footer .primary')
wait("return document.querySelector('.version-error')?.textContent.includes('Cannot save')")
assert execute("return !!document.querySelector('.lyric-versions') && !Object.values(lyricSaved).some(Boolean)")
execute('failLyricSave=false');click('.lyric-versions footer .primary')
wait("return !document.querySelector('.lyric-versions') && document.querySelector('.lyric-line')?.textContent==='Selected line'")
time.sleep(1.4)
assert execute("return document.querySelector('.lyric-line')?.textContent==='Selected line'"), 'Late automatic response replaced manual choice'
click('[aria-label="Текст позже на полсекунды"]')
click('[aria-label="Свернуть плеер"]');click('.expand-player')
wait("return document.querySelector('.lyric-line')?.textContent==='Selected line' && document.querySelector('.offset-reset')?.textContent.includes('0.5')")
click('[aria-label="Выбрать версию текста"]');wait("return !document.querySelector('.lyric-versions footer .secondary').disabled")
click('.lyric-versions footer .secondary')
wait("return !document.querySelector('.lyric-versions') && document.querySelector('.lyric-line')?.textContent==='Automatic line'")
assert execute("return document.querySelector('.offset-reset').textContent.includes('0.0')")
# Unmounting a chooser on Next must not leave a modal attached to another track.
click('[aria-label="Выбрать версию текста"]');wait("return !!document.querySelector('.lyric-versions')")
click('.expanded-controls [aria-label="Следующий трек"]');wait("return !document.querySelector('.lyric-versions')")
click('[aria-label="Свернуть плеер"]')
print('Lyrics: preview, failed persistence, late automatic result, reopen, per-version offset, reset and track change: PASS',flush=True)
click('nav button[aria-label="Настройки"]');wait("return !!document.querySelector('.audio-settings summary')")
click('.audio-settings summary');click('[data-output="pulse/test"]')
wait("return document.querySelector('.audio-settings summary').textContent.includes('Test headphones')")
assert execute("return !document.querySelector('.audio-settings details').open")
assert invoke('plugin:event|emit',{'event':'audio-output-state','payload':{'preferred':'pulse/test','selected':'auto','fallback':True,'silent':False,'devices':[{'name':'auto','description':'Как в системе'}]}})['ok']
wait("return document.querySelector('.output-note').textContent.includes('отключено')")
click('.audio-settings summary');click('[data-output="auto"]')
wait("return !document.querySelector('.output-note').textContent.includes('отключено')")
for name in ['../../etc/passwd','null','pulse/unknown','x\nrun']:
 assert not invoke('set_audio_output',{'name':name})['ok'],name
assert not invoke('get_lyric_choice',{'id':'../../etc/passwd'})['ok']
assert not invoke('preview_lyric_version',{'recordId':0})['ok']
assert not invoke('lyric_versions',{'title':'Track','artist':'Artist','query':'x'*201})['ok']
execute('window.__VESPER_TEST_INVOKE__=lyricBase;document.querySelector("nav button").click()')
wait("return !!document.querySelector('.hero .primary')")
print('Outputs: selection, unavailable device event, fallback reset; native command validation: PASS',flush=True)
