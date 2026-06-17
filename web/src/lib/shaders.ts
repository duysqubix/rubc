export type ShaderEffect = "off" | "lcd-grid" | "scanline" | "crt";

const VERTEX_SHADER = `
  attribute vec2 a_position;
  attribute vec2 a_texCoord;
  varying vec2 v_texCoord;
  void main() {
    gl_Position = vec4(a_position, 0.0, 1.0);
    v_texCoord = a_texCoord;
  }
`;

const FRAGMENT_SHADER_PREFIX = `
  precision mediump float;
  varying vec2 v_texCoord;
  uniform sampler2D u_image;
  uniform vec2 u_resolution;
  uniform vec2 u_srcResolution;
`;

const SHADERS: Record<ShaderEffect, string> = {
  "off": `
    void main() {
      gl_FragColor = texture2D(u_image, v_texCoord);
    }
  `,
  "lcd-grid": `
    void main() {
      vec4 color = texture2D(u_image, v_texCoord);
      // Position within the current source (GB) pixel cell, 0..1.
      vec2 g = fract(v_texCoord * u_srcResolution);
      // Darken the outer gutter of each cell to draw the LCD grid lines.
      float line = step(0.82, g.x) + step(0.82, g.y);
      color.rgb *= (line > 0.0 ? 0.55 : 1.0);
      gl_FragColor = color;
    }
  `,
  "scanline": `
    void main() {
      vec4 color = texture2D(u_image, v_texCoord);
      // Darken every third OUTPUT row (scales with the upscaled canvas).
      float s = mod(floor(gl_FragCoord.y), 3.0);
      color.rgb *= (s < 1.0 ? 0.6 : 1.0);
      gl_FragColor = color;
    }
  `,
  "crt": `
    void main() {
      vec2 uv = v_texCoord;
      vec4 color = texture2D(u_image, uv);
      // Scanlines on output rows.
      float s = mod(floor(gl_FragCoord.y), 3.0);
      color.rgb *= (s < 1.0 ? 0.6 : 1.0);
      // Vignette toward the edges.
      vec2 cc = uv * 2.0 - 1.0;
      float vig = 1.0 - dot(cc, cc) * 0.25;
      color.rgb *= vig;
      // Slight CRT warmth.
      color.r *= 1.05;
      color.b *= 0.95;
      gl_FragColor = color;
    }
  `
};

export class GLRenderer {
  canvas: HTMLCanvasElement;
  gl: WebGLRenderingContext | WebGL2RenderingContext | null;
  programs: Record<string, WebGLProgram> = {};
  currentEffect: ShaderEffect = "off";
  texture: WebGLTexture | null = null;
  positionBuffer: WebGLBuffer | null = null;
  texCoordBuffer: WebGLBuffer | null = null;

  constructor() {
    this.canvas = document.createElement("canvas");
    // Upscale the offscreen canvas (5x) so the per-source-pixel grid/scanline
    // math has sub-pixel output space to render into; the source texture stays
    // 160x144 (NEAREST). A native-res offscreen makes every effect a no-op.
    this.canvas.width = 160 * 5;
    this.canvas.height = 144 * 5;
    this.gl = (this.canvas.getContext("webgl2") || this.canvas.getContext("webgl")) as WebGLRenderingContext | null;
    
    if (!this.gl) {
      console.warn("WebGL not supported, falling back to 2D canvas");
      return;
    }

    this.initGL();
  }

  private compileShader(type: number, source: string): WebGLShader | null {
    if (!this.gl) return null;
    const shader = this.gl.createShader(type);
    if (!shader) return null;
    this.gl.shaderSource(shader, source);
    this.gl.compileShader(shader);
    if (!this.gl.getShaderParameter(shader, this.gl.COMPILE_STATUS)) {
      console.error("Shader compile error:", this.gl.getShaderInfoLog(shader));
      this.gl.deleteShader(shader);
      return null;
    }
    return shader;
  }

  private createProgram(fragmentSource: string): WebGLProgram | null {
    if (!this.gl) return null;
    const vs = this.compileShader(this.gl.VERTEX_SHADER, VERTEX_SHADER);
    const fs = this.compileShader(this.gl.FRAGMENT_SHADER, FRAGMENT_SHADER_PREFIX + fragmentSource);
    if (!vs || !fs) return null;

    const program = this.gl.createProgram();
    if (!program) return null;
    this.gl.attachShader(program, vs);
    this.gl.attachShader(program, fs);
    this.gl.linkProgram(program);

    if (!this.gl.getProgramParameter(program, this.gl.LINK_STATUS)) {
      console.error("Program link error:", this.gl.getProgramInfoLog(program));
      this.gl.deleteProgram(program);
      return null;
    }
    return program;
  }

  private initGL() {
    const gl = this.gl;
    if (!gl) return;

    for (const [effect, source] of Object.entries(SHADERS)) {
      const prog = this.createProgram(source);
      if (prog) this.programs[effect] = prog;
    }

    this.positionBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.positionBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([
      -1.0, -1.0,
       1.0, -1.0,
      -1.0,  1.0,
      -1.0,  1.0,
       1.0, -1.0,
       1.0,  1.0,
    ]), gl.STATIC_DRAW);

    this.texCoordBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.texCoordBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([
      0.0, 1.0,
      1.0, 1.0,
      0.0, 0.0,
      0.0, 0.0,
      1.0, 1.0,
      1.0, 0.0,
    ]), gl.STATIC_DRAW);

    this.texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  }

  setEffect(effect: ShaderEffect) {
    this.currentEffect = effect;
  }

  draw(rgbaData: Uint8ClampedArray): HTMLCanvasElement | null {
    const gl = this.gl;
    if (!gl) return null;

    const program = this.programs[this.currentEffect] || this.programs["off"];
    if (!program) return null;

    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(0, 0, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);

    gl.useProgram(program);

    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 160, 144, 0, gl.RGBA, gl.UNSIGNED_BYTE, rgbaData);

    const posLoc = gl.getAttribLocation(program, "a_position");
    gl.enableVertexAttribArray(posLoc);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.positionBuffer);
    gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);

    const texLoc = gl.getAttribLocation(program, "a_texCoord");
    gl.enableVertexAttribArray(texLoc);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.texCoordBuffer);
    gl.vertexAttribPointer(texLoc, 2, gl.FLOAT, false, 0, 0);

    const resLoc = gl.getUniformLocation(program, "u_resolution");
    if (resLoc) gl.uniform2f(resLoc, this.canvas.width, this.canvas.height);

    const srcResLoc = gl.getUniformLocation(program, "u_srcResolution");
    if (srcResLoc) gl.uniform2f(srcResLoc, 160.0, 144.0);

    gl.drawArrays(gl.TRIANGLES, 0, 6);

    return this.canvas;
  }

  dispose() {
    const gl = this.gl;
    if (!gl) return;
    if (this.texture) gl.deleteTexture(this.texture);
    if (this.positionBuffer) gl.deleteBuffer(this.positionBuffer);
    if (this.texCoordBuffer) gl.deleteBuffer(this.texCoordBuffer);
    for (const prog of Object.values(this.programs)) {
      gl.deleteProgram(prog);
    }
    this.programs = {};
    this.gl = null;
  }
}
