"""Exercise WebGL lifecycle, display resolution and the 2D fallback in a fixture."""
import time
from webdriver_client import execute, request, sid


def wait(script):
    for _ in range(100):
        if execute(script):
            return
        time.sleep(.05)
    raise AssertionError(script)


execute('''
window.savedCanvasContext=HTMLCanvasElement.prototype.getContext;
window.visualizerPump=setInterval(()=>nativeInvoke('plugin:event|emit',{event:'audio-level',payload:{rms:.7,peak:.9}}),40);
emitState({paused:false,buffering:false,active:true});
document.querySelector('.expand-player').click();
''')
try:
    wait("return !!document.querySelector('.audio-visualizer')?.dataset.renderer")
    result = request('/session/' + sid() + '/execute/async', {'args': [], 'script': '''
      const done=arguments[arguments.length-1], c=document.querySelector('.audio-visualizer');
      const gpu=c.dataset.renderer==='webgl', ctx=c.getContext(gpu?'webgl':'2d'), method=gpu?'clear':'clearRect';
      const original=ctx[method]; let draws=0,frames=0,start=0;
      ctx[method]=function(...args){draws++;return original.apply(this,args)};
      function sample(t){
        start ||= t;frames++;
        if(t-start<1500){requestAnimationFrame(sample);return;}
        ctx[method]=original;
        done({renderer:c.dataset.renderer,frames,draws,elapsed:t-start});
      }
      requestAnimationFrame(sample);
    '''})
    assert result['frames'] > 3 and result['draws'] >= result['frames'] * .9, result
    print('Visualizer draws on each available display frame:', result, flush=True)
    execute("document.querySelector('.audio-visualizer').style.width='80%'")
    wait("const c=document.querySelector('.audio-visualizer');return Math.abs(c.width-c.clientWidth*devicePixelRatio)<=1")
    execute("document.querySelector('.audio-visualizer').style.width=''")
    wait("const c=document.querySelector('.audio-visualizer');return Math.abs(c.width-c.clientWidth*devicePixelRatio)<=1")
    if result['renderer'] == 'webgl':
        supported = execute("const gl=document.querySelector('.audio-visualizer').getContext('webgl');window.lossExtension=gl.getExtension('WEBGL_lose_context');if(!lossExtension)return false;lossExtension.loseContext();return true")
        if supported:
            wait("return document.querySelector('.audio-visualizer').getContext('webgl').isContextLost()")
            time.sleep(.15)
            execute('lossExtension.restoreContext()')
            wait("return !document.querySelector('.audio-visualizer').getContext('webgl').isContextLost()")
            time.sleep(.15)
            assert execute("const gl=document.querySelector('.audio-visualizer').getContext('webgl');return !!gl.getParameter(gl.CURRENT_PROGRAM)"), 'Shaders did not recover'
            print('WebGL context loss/restoration: PASS', flush=True)
    execute('''
      document.querySelector('.expanded-header [aria-label=\"Свернуть плеер\"]').click();
      HTMLCanvasElement.prototype.getContext=function(type,...args){if(type==='webgl')return null;return savedCanvasContext.call(this,type,...args)};
    ''')
    time.sleep(.1)
    execute("document.querySelector('.expand-player').click()")
    wait("return document.querySelector('.audio-visualizer')?.dataset.renderer==='canvas2d'")
    assert execute("const c=document.querySelector('.audio-visualizer');return c.width===Math.round(c.clientWidth*devicePixelRatio)")
    print('2D fallback retains display resolution: PASS', flush=True)
    if result['renderer'] == 'webgl':
        execute("""
          document.querySelector('.expanded-header [aria-label=\"Свернуть плеер\"]').click();
          HTMLCanvasElement.prototype.getContext=savedCanvasContext;
          window.savedShaderParameter=WebGLRenderingContext.prototype.getShaderParameter;
          window.shaderFailureTriggered=false;
          window.expectedShaderError=e=>e.preventDefault();window.addEventListener('error',expectedShaderError);
          WebGLRenderingContext.prototype.getShaderParameter=function(shader,pname){
            if(pname===this.COMPILE_STATUS){shaderFailureTriggered=true;return false;}
            return savedShaderParameter.call(this,shader,pname);
          };
        """)
        time.sleep(.1)
        execute("document.querySelector('.expand-player').click()")
        wait('return shaderFailureTriggered')
        wait("return document.querySelector('.audio-visualizer')?.dataset.renderer==='canvas2d'")
        assert execute("const c=document.querySelector('.audio-visualizer');return c.width===Math.round(c.clientWidth*devicePixelRatio)")
        print('Failed GPU shader falls back without breaking the player: PASS',flush=True)

finally:
    execute("if(window.savedShaderParameter)WebGLRenderingContext.prototype.getShaderParameter=savedShaderParameter;if(window.expectedShaderError)window.removeEventListener('error',expectedShaderError);HTMLCanvasElement.prototype.getContext=savedCanvasContext;clearInterval(visualizerPump);document.querySelector('.expanded-header [aria-label=\"Свернуть плеер\"]')?.click()")
