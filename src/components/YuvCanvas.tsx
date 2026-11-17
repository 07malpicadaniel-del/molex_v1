import React, { useEffect, useRef } from "react";

interface YuvCanvasProps {
  wsUrl: string | null;
  hideLocalCursor: boolean;
  onMouseMoveNorm: (x: number, y: number) => void;
  onMouseButtonNorm: (
    x: number,
    y: number,
    button: "LEFT" | "RIGHT" | "MIDDLE",
    pressed: boolean
  ) => void;
  onWheelInput: (dx: number, dy: number) => void;
  onFrameMetrics?: (metrics: { width: number; height: number; fps: number }) => void;
  onFocusCanvas?: () => void;
}

const VERTEX_SHADER_SRC = `
  attribute vec2 a_pos;
  attribute vec2 a_uv;
  varying vec2 v_uv;
  void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_uv = a_uv;
  }
`;

// Conversión BT.601/709 YUV420p -> RGB directamente en GPU
const FRAGMENT_SHADER_SRC = `
  precision mediump float;
  varying vec2 v_uv;
  uniform sampler2D u_texY;
  uniform sampler2D u_texU;
  uniform sampler2D u_texV;
  void main() {
    float y = texture2D(u_texY, v_uv).r;
    float u = texture2D(u_texU, v_uv).r - 0.5;
    float v = texture2D(u_texV, v_uv).r - 0.5;
    float r = y + 1.402 * v;
    float g = y - 0.344136 * u - 0.714136 * v;
    float b = y + 1.772 * u;
    gl_FragColor = vec4(clamp(vec3(r, g, b), 0.0, 1.0), 1.0);
  }
`;

function compileShader(gl: WebGLRenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("No se pudo crear shader WebGL");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const info = gl.getShaderInfoLog(shader) || "Error compilando shader";
    gl.deleteShader(shader);
    throw new Error(info);
  }
  return shader;
}

function createTexture(gl: WebGLRenderingContext, unit: number): WebGLTexture {
  const tex = gl.createTexture();
  if (!tex) throw new Error("No se pudo crear textura WebGL");
  gl.activeTexture(gl.TEXTURE0 + unit);
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  return tex;
}

function browserButtonToName(button: number): "LEFT" | "RIGHT" | "MIDDLE" {
  if (button === 2) return "RIGHT";
  if (button === 1) return "MIDDLE";
  return "LEFT";
}

export const YuvCanvas: React.FC<YuvCanvasProps> = ({
  wsUrl,
  hideLocalCursor,
  onMouseMoveNorm,
  onMouseButtonNorm,
  onWheelInput,
  onFrameMetrics,
  onFocusCanvas,
}) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const videoSizeRef = useRef<{ width: number; height: number }>({ width: 1920, height: 1080 });
  const latestMoveRef = useRef<{ x: number; y: number } | null>(null);
  const moveRafRef = useRef<number | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !wsUrl) return;

    const gl = canvas.getContext("webgl", {
      alpha: false,
      antialias: false,
      depth: false,
      stencil: false,
      powerPreference: "high-performance",
      preserveDrawingBuffer: false,
    });
    if (!gl) return;

    const vs = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER_SRC);
    const fs = compileShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER_SRC);
    const program = gl.createProgram();
    if (!program) return;
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);
    gl.useProgram(program);

    const quadBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, quadBuffer);
    // Pos X, Pos Y, U, V
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([
        -1, -1, 0, 1,
         1, -1, 1, 1,
        -1,  1, 0, 0,
         1,  1, 1, 0,
      ]),
      gl.STATIC_DRAW
    );

    const aPos = gl.getAttribLocation(program, "a_pos");
    const aUv = gl.getAttribLocation(program, "a_uv");
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 16, 0);
    gl.enableVertexAttribArray(aUv);
    gl.vertexAttribPointer(aUv, 2, gl.FLOAT, false, 16, 8);

    const texY = createTexture(gl, 0);
    const texU = createTexture(gl, 1);
    const texV = createTexture(gl, 2);
    gl.uniform1i(gl.getUniformLocation(program, "u_texY"), 0);
    gl.uniform1i(gl.getUniformLocation(program, "u_texU"), 1);
    gl.uniform1i(gl.getUniformLocation(program, "u_texV"), 2);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);

    let pendingBuffer: ArrayBuffer | null = null;
    let allocatedW = 0;
    let allocatedH = 0;
    let disposed = false;
    let rafId = 0;
    let fpsCounter = 0;
    let fpsLastTs = performance.now();

    const renderFrame = () => {
      if (disposed) return;
      rafId = requestAnimationFrame(renderFrame);

      const buf = pendingBuffer;
      if (!buf || buf.byteLength < 24) return;
      pendingBuffer = null;

      const view = new DataView(buf);
      // Magic "MOLX" = 0x4D 0x4F 0x4C 0x58
      if (
        view.getUint8(0) !== 0x4d ||
        view.getUint8(1) !== 0x4f ||
        view.getUint8(2) !== 0x4c ||
        view.getUint8(3) !== 0x58
      ) {
        return;
      }

      const width = view.getUint32(4, true);
      const height = view.getUint32(8, true);
      if (width === 0 || height === 0) return;

      const chromaW = Math.ceil(width / 2);
      const chromaH = Math.ceil(height / 2);
      const yLen = width * height;
      const uLen = chromaW * chromaH;
      const vLen = chromaW * chromaH;
      if (buf.byteLength < 24 + yLen + uLen + vLen) return;

      const yBytes = new Uint8Array(buf, 24, yLen);
      const uBytes = new Uint8Array(buf, 24 + yLen, uLen);
      const vBytes = new Uint8Array(buf, 24 + yLen + uLen, vLen);

      if (allocatedW !== width || allocatedH !== height) {
        allocatedW = width;
        allocatedH = height;
        videoSizeRef.current = { width, height };
        canvas.width = width;
        canvas.height = height;
        gl.viewport(0, 0, width, height);

        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, texY);
        gl.texImage2D(
          gl.TEXTURE_2D,
          0,
          gl.LUMINANCE,
          width,
          height,
          0,
          gl.LUMINANCE,
          gl.UNSIGNED_BYTE,
          yBytes
        );

        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, texU);
        gl.texImage2D(
          gl.TEXTURE_2D,
          0,
          gl.LUMINANCE,
          chromaW,
          chromaH,
          0,
          gl.LUMINANCE,
          gl.UNSIGNED_BYTE,
          uBytes
        );

        gl.activeTexture(gl.TEXTURE2);
        gl.bindTexture(gl.TEXTURE_2D, texV);
        gl.texImage2D(
          gl.TEXTURE_2D,
          0,
          gl.LUMINANCE,
          chromaW,
          chromaH,
          0,
          gl.LUMINANCE,
          gl.UNSIGNED_BYTE,
          vBytes
        );
      } else {
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, texY);
        gl.texSubImage2D(
          gl.TEXTURE_2D,
          0,
          0,
          0,
          width,
          height,
          gl.LUMINANCE,
          gl.UNSIGNED_BYTE,
          yBytes
        );

        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, texU);
        gl.texSubImage2D(
          gl.TEXTURE_2D,
          0,
          0,
          0,
          chromaW,
          chromaH,
          gl.LUMINANCE,
          gl.UNSIGNED_BYTE,
          uBytes
        );

        gl.activeTexture(gl.TEXTURE2);
        gl.bindTexture(gl.TEXTURE_2D, texV);
        gl.texSubImage2D(
          gl.TEXTURE_2D,
          0,
          0,
          0,
          chromaW,
          chromaH,
          gl.LUMINANCE,
          gl.UNSIGNED_BYTE,
          vBytes
        );
      }

      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);

      fpsCounter += 1;
      const now = performance.now();
      if (now - fpsLastTs >= 500) {
        const fps = Math.round(((fpsCounter * 1000) / (now - fpsLastTs)) * 10) / 10;
        onFrameMetrics?.({ width, height, fps });
        fpsCounter = 0;
        fpsLastTs = now;
      }
    };

    rafId = requestAnimationFrame(renderFrame);

    let ws: WebSocket | null = null;
    let reconnectTimer: number | null = null;

    const connectWs = () => {
      if (disposed) return;
      ws = new WebSocket(wsUrl);
      ws.binaryType = "arraybuffer";
      ws.onmessage = (evt) => {
        if (evt.data instanceof ArrayBuffer) {
          pendingBuffer = evt.data;
        }
      };
      ws.onclose = () => {
        if (!disposed) {
          reconnectTimer = window.setTimeout(connectWs, 500);
        }
      };
      ws.onerror = () => {
        ws?.close();
      };
    };

    connectWs();

    return () => {
      disposed = true;
      cancelAnimationFrame(rafId);
      if (reconnectTimer !== null) clearTimeout(reconnectTimer);
      ws?.close();
      gl.deleteTexture(texY);
      gl.deleteTexture(texU);
      gl.deleteTexture(texV);
      gl.deleteBuffer(quadBuffer);
      gl.deleteProgram(program);
      gl.deleteShader(vs);
      gl.deleteShader(fs);
    };
  }, [wsUrl, onFrameMetrics]);

  // Mapeo matemático idéntico al de la app móvil para descartar zonas de letterbox/pillarbox
  const mapClientCoords = (clientX: number, clientY: number): [number, number] | null => {
    const container = containerRef.current;
    if (!container) return null;
    const rect = container.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return null;

    const { width: imgW, height: imgH } = videoSizeRef.current;
    const containerRatio = rect.width / rect.height;
    const imageRatio = imgW / imgH;

    let visibleW: number;
    let visibleH: number;
    let offsetX: number;
    let offsetY: number;

    if (imageRatio > containerRatio) {
      visibleW = rect.width;
      visibleH = rect.width / imageRatio;
      offsetX = 0;
      offsetY = (rect.height - visibleH) / 2;
    } else {
      visibleH = rect.height;
      visibleW = rect.height * imageRatio;
      offsetX = (rect.width - visibleW) / 2;
      offsetY = 0;
    }

    const localX = clientX - rect.left;
    const localY = clientY - rect.top;

    if (
      localX < offsetX ||
      localX > offsetX + visibleW ||
      localY < offsetY ||
      localY > offsetY + visibleH
    ) {
      return null;
    }

    const normX = Math.max(0, Math.min(1, (localX - offsetX) / visibleW));
    const normY = Math.max(0, Math.min(1, (localY - offsetY) / visibleH));
    return [normX, normY];
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    const mapped = mapClientCoords(e.clientX, e.clientY);
    if (!mapped) return;
    latestMoveRef.current = { x: mapped[0], y: mapped[1] };
    if (moveRafRef.current === null) {
      moveRafRef.current = requestAnimationFrame(() => {
        moveRafRef.current = null;
        if (latestMoveRef.current) {
          onMouseMoveNorm(latestMoveRef.current.x, latestMoveRef.current.y);
        }
      });
    }
  };

  const handleMouseDown = (e: React.MouseEvent<HTMLDivElement>) => {
    onFocusCanvas?.();
    const mapped = mapClientCoords(e.clientX, e.clientY);
    if (!mapped) return;
    onMouseButtonNorm(mapped[0], mapped[1], browserButtonToName(e.button), true);
  };

  const handleMouseUp = (e: React.MouseEvent<HTMLDivElement>) => {
    const mapped = mapClientCoords(e.clientX, e.clientY);
    if (!mapped) return;
    onMouseButtonNorm(mapped[0], mapped[1], browserButtonToName(e.button), false);
  };

  const handleWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    const dy = e.deltaY === 0 ? 0 : e.deltaY < 0 ? 1 : -1;
    const dx = e.deltaX === 0 ? 0 : e.deltaX < 0 ? -1 : 1;
    if (dx !== 0 || dy !== 0) {
      onWheelInput(dx, dy);
    }
  };

  return (
    <div
      ref={containerRef}
      onMouseMove={handleMouseMove}
      onMouseDown={handleMouseDown}
      onMouseUp={handleMouseUp}
      onWheel={handleWheel}
      onContextMenu={(e) => e.preventDefault()}
      className={`relative flex h-full w-full items-center justify-center bg-black select-none ${
        hideLocalCursor ? "cursor-none" : "cursor-crosshair"
      }`}
    >
      <canvas
        ref={canvasRef}
        className="max-h-full max-w-full object-contain pointer-events-none"
      />
    </div>
  );
};
