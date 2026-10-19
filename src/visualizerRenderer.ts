/** Small antialiased stroke renderer. WebGL draws the ribbons; 2D is a fallback. */
type Strokes = Pick<CanvasRenderingContext2D, 'strokeStyle' | 'lineWidth' | 'lineCap' | 'globalAlpha' | 'setTransform' | 'clearRect' | 'beginPath' | 'moveTo' | 'lineTo' | 'closePath' | 'stroke'> & { dispose(): void };

export function visualizerRenderer(canvas: HTMLCanvasElement, force2d = false): Strokes | null {
  const gl = force2d ? null : canvas.getContext('webgl', { alpha: true, antialias: false, depth: false, stencil: false });
  if (!gl) {
    const ctx = canvas.getContext('2d');
    canvas.dataset.renderer = 'canvas2d';
    return ctx ? Object.assign(ctx, { dispose() {} }) : null;
  }
  const shaders: WebGLShader[] = [];
  const shader = (type: number, source: string) => {
    const value = gl.createShader(type)!;
    shaders.push(value); gl.shaderSource(value, source); gl.compileShader(value);
    if (!gl.getShaderParameter(value, gl.COMPILE_STATUS)) throw new Error('Visualizer shader compilation failed');
    return value;
  };
  const program = gl.createProgram()!;
  const buffer = gl.createBuffer()!;
  try {
    gl.attachShader(program, shader(gl.VERTEX_SHADER, `
      attribute vec3 vertex;
      uniform vec2 resolution;
      varying float edge;
      void main() {
        vec2 p = vertex.xy / resolution * 2.0 - 1.0;
        gl_Position = vec4(p.x, -p.y, 0.0, 1.0);
        edge = vertex.z;
      }`));
    gl.attachShader(program, shader(gl.FRAGMENT_SHADER, `
      precision mediump float;
      uniform vec4 color;
      uniform float halfWidth;
      uniform float feather;
      varying float edge;
      void main() {
        float coverage = 1.0 - smoothstep(halfWidth - feather, halfWidth + feather, abs(edge));
        gl_FragColor = vec4(color.rgb, color.a * coverage);
      }`));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error('Visualizer shader linking failed');
  } catch (error) {
    gl.deleteBuffer(buffer); gl.deleteProgram(program); shaders.forEach(s => gl.deleteShader(s));
    throw error;
  }
  gl.useProgram(program); gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  const attribute = gl.getAttribLocation(program, 'vertex');
  gl.enableVertexAttribArray(attribute); gl.vertexAttribPointer(attribute, 3, gl.FLOAT, false, 0, 0);
  gl.enable(gl.BLEND); gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  gl.clearColor(0, 0, 0, 0);
  const resolution = gl.getUniformLocation(program, 'resolution');
  const color = gl.getUniformLocation(program, 'color');
  const halfWidth = gl.getUniformLocation(program, 'halfWidth');
  const feather = gl.getUniformLocation(program, 'feather');
  const points = new Float32Array(2048), vertices = new Float32Array(6144);
  gl.bufferData(gl.ARRAY_BUFFER, vertices.byteLength, gl.DYNAMIC_DRAW);
  const swatch = document.createElement('canvas'); swatch.width = swatch.height = 1;
  const parser = swatch.getContext('2d');
  if (!parser) {
    gl.deleteBuffer(buffer); gl.deleteProgram(program); shaders.forEach(s => gl.deleteShader(s));
    throw new Error('Visualizer color context unavailable');
  }
  let count = 0, closed = false, ratio = 1, cachedColor: unknown, rgb = [1, 1, 1];
  const context: Strokes = {
    strokeStyle: '#ffffff', lineWidth: 1, lineCap: 'round', globalAlpha: 1,
    setTransform(a: number | DOMMatrix2DInit = 1) {
      ratio = typeof a === 'number' ? a : (a.a ?? 1);
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.uniform2f(resolution, canvas.width / ratio, canvas.height / ratio);
      gl.uniform1f(feather, .75 / ratio);
    },
    clearRect() { gl.clear(gl.COLOR_BUFFER_BIT); },
    beginPath() { count = 0; closed = false; },
    moveTo(x, y) { points[count * 2] = x; points[count * 2 + 1] = y; count++; },
    lineTo(x, y) { context.moveTo(x, y); },
    closePath() { closed = true; },
    stroke() {
      if (gl.isContextLost() || count < 2) return;
      if (cachedColor !== context.strokeStyle) {
        cachedColor = context.strokeStyle;
        parser.clearRect(0, 0, 1, 1); parser.fillStyle = context.strokeStyle; parser.fillRect(0, 0, 1, 1);
        const pixel = parser.getImageData(0, 0, 1, 1).data;
        rgb = [pixel[0] / 255, pixel[1] / 255, pixel[2] / 255];
      }
      const half = context.lineWidth / 2, extent = half + .75 / ratio;
      for (let n = 0; n < count; n++) {
        const prev = n ? n - 1 : (closed ? count - 2 : 0);
        const next = n + 1 < count ? n + 1 : (closed ? 1 : count - 1);
        const dx = points[next * 2] - points[prev * 2], dy = points[next * 2 + 1] - points[prev * 2 + 1];
        const length = Math.hypot(dx, dy) || 1;
        const nx = -dy / length, ny = dx / length;
        for (let side = 0; side < 2; side++) {
          const edge = side ? extent : -extent, i = n * 6 + side * 3;
          vertices[i] = points[n * 2] + nx * edge;
          vertices[i + 1] = points[n * 2 + 1] + ny * edge;
          vertices[i + 2] = edge;
        }
      }
      gl.uniform4f(color, rgb[0], rgb[1], rgb[2], context.globalAlpha);
      gl.uniform1f(halfWidth, half);
      gl.bufferSubData(gl.ARRAY_BUFFER, 0, vertices.subarray(0, count * 6));
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, count * 2);
    },
    dispose() { gl.deleteBuffer(buffer); gl.deleteProgram(program); shaders.forEach(s => gl.deleteShader(s)); },
  };
  canvas.dataset.renderer = 'webgl';
  return context;
}
