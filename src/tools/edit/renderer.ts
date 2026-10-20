// Rendu des réglages image en WebGL : rapide en pleine résolution, identique sous
// Windows (WebView2) et Linux (WebKitGTK), contrairement aux filtres CSS du canvas 2D.

import type { Adjust, Geometry } from "./model";

const VERTEX = `
attribute vec2 a_pos;
void main() { gl_Position = vec4(a_pos, 0.0, 1.0); }
`;

/** Flou gaussien séparable (une direction par passe), 13 échantillons. */
const BLUR = `
precision highp float;
uniform sampler2D u_tex;
uniform vec2 u_size;
uniform vec2 u_dir;
void main() {
  vec2 uv = gl_FragCoord.xy / u_size;
  vec4 sum = vec4(0.0);
  float total = 0.0;
  for (int i = -6; i <= 6; i++) {
    float w = exp(-float(i * i) / 18.0);
    sum += texture2D(u_tex, uv + u_dir * float(i) / u_size) * w;
    total += w;
  }
  gl_FragColor = sum / total;
}
`;

/** Géométrie (pixel de sortie → pixel source), netteté, couleurs, vignettage, grain. */
const FINAL = `
precision highp float;
uniform sampler2D u_tex;
uniform vec2 u_src;
uniform vec2 u_out;
uniform float u_quarter;
uniform vec2 u_flip;
uniform float u_angle;
uniform float u_cover;
uniform float u_exposure, u_brightness, u_contrast, u_saturation, u_temperature;
uniform float u_sharpness, u_vignette, u_grain, u_sepia, u_fade, u_invert;

vec2 rot(vec2 v, float a) {
  float c = cos(a), s = sin(a);
  return vec2(v.x * c - v.y * s, v.x * s + v.y * c);
}

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}

void main() {
  // Origine en haut à gauche, comme l'image.
  vec2 p = vec2(gl_FragCoord.x, u_out.y - gl_FragCoord.y) / u_out;
  vec2 q = (p - 0.5) * u_out;
  q = rot(q / u_cover, -u_angle);
  // Quarts de tour inverses (anti-horaires).
  for (int i = 0; i < 3; i++) {
    if (float(i) < u_quarter) q = vec2(q.y, -q.x);
  }
  q *= u_flip;
  vec2 uv = clamp(q / u_src + 0.5, 0.0, 1.0);

  vec4 c = texture2D(u_tex, uv);
  vec3 col = c.rgb;
  if (u_sharpness > 0.0) {
    vec2 t = 1.0 / u_src;
    vec3 around = texture2D(u_tex, uv + vec2(t.x, 0.0)).rgb + texture2D(u_tex, uv - vec2(t.x, 0.0)).rgb
                + texture2D(u_tex, uv + vec2(0.0, t.y)).rgb + texture2D(u_tex, uv - vec2(0.0, t.y)).rgb;
    col += u_sharpness * (4.0 * col - around);
  }
  col *= exp2(u_exposure);
  col += u_brightness;
  col = (col - 0.5) * u_contrast + 0.5;
  float l = dot(col, vec3(0.2126, 0.7152, 0.0722));
  col = mix(vec3(l), col, u_saturation);
  col += vec3(u_temperature, u_temperature * 0.2, -u_temperature);
  vec3 sepia = vec3(dot(col, vec3(0.393, 0.769, 0.189)), dot(col, vec3(0.349, 0.686, 0.168)), dot(col, vec3(0.272, 0.534, 0.131)));
  col = mix(col, sepia, u_sepia);
  col = u_fade + col * (1.0 - u_fade);
  col = mix(col, 1.0 - col, u_invert);
  float d = length(p - 0.5) * 1.4142;
  col *= 1.0 - u_vignette * smoothstep(0.35, 1.0, d);
  col += (hash(gl_FragCoord.xy) - 0.5) * u_grain;
  gl_FragColor = vec4(clamp(col, 0.0, 1.0), c.a);
}
`;

export interface RenderParams {
  adjust: Adjust;
  sepia: number;
  fade: number;
  invert: boolean;
  geometry: Geometry;
}

interface Target {
  fb: WebGLFramebuffer;
  tex: WebGLTexture;
}

export class Renderer {
  readonly canvas = document.createElement("canvas");
  /** Taille de l'image source (éventuellement réduite à la taille de texture maximale). */
  width = 0;
  height = 0;
  /** L'image a dû être réduite pour tenir dans une texture. */
  downscaled = false;
  private gl: WebGLRenderingContext;
  private blur: WebGLProgram;
  private final: WebGLProgram;
  private source: WebGLTexture | null = null;
  private staging = document.createElement("canvas");
  private targets: Target[] = [];

  constructor() {
    const gl = this.canvas.getContext("webgl", { premultipliedAlpha: false, preserveDrawingBuffer: true });
    if (!gl) throw new Error("WebGL indisponible : impossible d'afficher l'éditeur.");
    this.gl = gl;
    this.blur = program(gl, BLUR);
    this.final = program(gl, FINAL);
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW);
  }

  /** Charge l'image (orientation EXIF appliquée par le navigateur au dessin). */
  setImage(img: CanvasImageSource & { width: number; height: number }, maxSide = Infinity) {
    const gl = this.gl;
    // Une vidéo a sa taille dans videoWidth/videoHeight (width/height = attributs de l'élément).
    const [iw, ih] = img instanceof HTMLVideoElement ? [img.videoWidth, img.videoHeight] : [img.width, img.height];
    const max = Math.min(gl.getParameter(gl.MAX_TEXTURE_SIZE) as number, maxSide);
    const ratio = Math.min(1, max / Math.max(iw, ih));
    this.downscaled = ratio < 1 && maxSide === Infinity;
    this.width = Math.max(1, Math.round(iw * ratio));
    this.height = Math.max(1, Math.round(ih * ratio));

    const staging = this.staging;
    staging.width = this.width;
    staging.height = this.height;
    staging.getContext("2d")!.drawImage(img, 0, 0, this.width, this.height);

    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
    this.source = texture(gl, this.width, this.height, staging);
    for (const t of this.targets) {
      gl.deleteFramebuffer(t.fb);
      gl.deleteTexture(t.tex);
    }
    this.targets = [0, 1].map(() => {
      const tex = texture(gl, this.width, this.height, null);
      const fb = gl.createFramebuffer()!;
      gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
      return { fb, tex };
    });
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  }

  /** Nouvelle image à la même taille (image suivante d'une vidéo) : seule la texture source change. */
  updateImage(img: CanvasImageSource) {
    const gl = this.gl;
    if (!this.source) return;
    this.staging.getContext("2d")!.drawImage(img, 0, 0, this.width, this.height);
    gl.bindTexture(gl.TEXTURE_2D, this.source);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, gl.RGBA, gl.UNSIGNED_BYTE, this.staging);
  }

  /** Libère le contexte WebGL (le navigateur en limite le nombre). */
  dispose() {
    this.gl.getExtension("WEBGL_lose_context")?.loseContext();
  }

  /** Taille de sortie : les quarts de tour impairs échangent largeur et hauteur. */
  outputSize(geometry: Geometry) {
    return geometry.quarter % 2 ? { w: this.height, h: this.width } : { w: this.width, h: this.height };
  }

  render({ adjust, sepia, fade, invert, geometry }: RenderParams) {
    const gl = this.gl;
    if (!this.source) return;
    let input = this.source;

    // Flou : deux passes (horizontale puis verticale) dans des textures intermédiaires.
    const radius = (adjust.blur / 100) * Math.max(this.width, this.height) * 0.012;
    if (radius > 0.05) {
      gl.useProgram(this.blur);
      gl.viewport(0, 0, this.width, this.height);
      [
        [radius / 3, 0],
        [0, radius / 3],
      ].forEach(([dx, dy], i) => {
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.targets[i].fb);
        gl.bindTexture(gl.TEXTURE_2D, input);
        uniforms(gl, this.blur, { u_size: [this.width, this.height], u_dir: [dx, dy] });
        draw(gl, this.blur);
        input = this.targets[i].tex;
      });
    }

    const { w, h } = this.outputSize(geometry);
    this.canvas.width = w;
    this.canvas.height = h;
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, w, h);
    gl.useProgram(this.final);
    gl.bindTexture(gl.TEXTURE_2D, input);

    const angle = (geometry.straighten * Math.PI) / 180;
    const c = Math.abs(Math.cos(angle));
    const s = Math.abs(Math.sin(angle));
    const cover = Math.max((w * c + h * s) / w, (w * s + h * c) / h);
    uniforms(gl, this.final, {
      u_src: [this.width, this.height],
      u_out: [w, h],
      u_quarter: geometry.quarter,
      u_flip: [geometry.flipH ? -1 : 1, geometry.flipV ? -1 : 1],
      u_angle: angle,
      u_cover: cover,
      u_exposure: (adjust.exposure / 100) * 1.5,
      u_brightness: (adjust.brightness / 100) * 0.3,
      u_contrast: 1 + adjust.contrast / 100,
      u_saturation: 1 + adjust.saturation / 100,
      u_temperature: (adjust.temperature / 100) * 0.12,
      u_sharpness: (adjust.sharpness / 100) * 1.5,
      u_vignette: adjust.vignette / 100,
      u_grain: (adjust.grain / 100) * 0.25,
      u_sepia: sepia,
      u_fade: fade,
      u_invert: invert ? 1 : 0,
    });
    draw(gl, this.final);
  }
}

function program(gl: WebGLRenderingContext, fragment: string): WebGLProgram {
  const compile = (type: number, src: string) => {
    const shader = gl.createShader(type)!;
    gl.shaderSource(shader, src);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader) ?? "shader");
    return shader;
  };
  const p = gl.createProgram()!;
  gl.attachShader(p, compile(gl.VERTEX_SHADER, VERTEX));
  gl.attachShader(p, compile(gl.FRAGMENT_SHADER, fragment));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) ?? "program");
  return p;
}

function texture(gl: WebGLRenderingContext, w: number, h: number, src: TexImageSource | null): WebGLTexture {
  const tex = gl.createTexture()!;
  gl.bindTexture(gl.TEXTURE_2D, tex);
  // Textures de taille quelconque (non puissance de 2) : pas de mipmaps, bords bloqués.
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  if (src) gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, src);
  else gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
  return tex;
}

function uniforms(gl: WebGLRenderingContext, p: WebGLProgram, values: Record<string, number | number[]>) {
  for (const [name, value] of Object.entries(values)) {
    const loc = gl.getUniformLocation(p, name);
    if (typeof value === "number") gl.uniform1f(loc, value);
    else gl.uniform2f(loc, value[0], value[1]);
  }
}

function draw(gl: WebGLRenderingContext, p: WebGLProgram) {
  const loc = gl.getAttribLocation(p, "a_pos");
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  gl.drawArrays(gl.TRIANGLES, 0, 6);
}
