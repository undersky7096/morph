import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import type Konva from "konva";
import { Arrow, Ellipse, Image as KImage, Layer, Line, Rect, Stage, Text, Transformer } from "react-konva";

import type { AnnotateTool, DrawStyle, Label, Shape } from "./model";
import { t } from "../../i18n";

interface Props {
  /** Image rendue par le moteur WebGL, redessinée quand `version` change. */
  base: HTMLCanvasElement;
  version: number;
  width: number;
  height: number;
  scale: number;
  shapes: Shape[];
  tool: AnnotateTool;
  style: DrawStyle;
  selected: string | null;
  onSelect: (id: string | null) => void;
  /** Annotation ajoutée ou modifiée. */
  onChange: (shape: Shape) => void;
}

export interface CanvasHandle {
  /** Image finale en pleine résolution (sans le cadre de sélection). */
  exportPng: () => Promise<Blob>;
}

const newId = () => Math.random().toString(36).slice(2, 10);

const DRAW_TOOLS: AnnotateTool[] = ["brush", "highlight", "eraser", "arrow", "line", "rect", "ellipse"];

/** Image retouchée et annotations par-dessus (Konva), en coordonnées de l'image. */
const Canvas = forwardRef<CanvasHandle, Props>(function Canvas(props, ref) {
  const { base, version, width, height, scale, shapes, tool, style, selected, onSelect, onChange } = props;
  const stageRef = useRef<Konva.Stage>(null);
  const imageRef = useRef<Konva.Image>(null);
  const transformerRef = useRef<Konva.Transformer>(null);
  const [draft, setDraft] = useState<Shape | null>(null);
  const origin = useRef<{ x: number; y: number } | null>(null);
  const side = Math.max(width, height);

  useEffect(() => {
    imageRef.current?.getLayer()?.batchDraw();
  }, [version]);

  // Cadre de redimensionnement/rotation sur l'annotation sélectionnée.
  useEffect(() => {
    const tr = transformerRef.current;
    if (!tr) return;
    const node = selected && tool === "select" ? stageRef.current?.findOne(`#${selected}`) : null;
    tr.nodes(node ? [node] : []);
    tr.getLayer()?.batchDraw();
  }, [selected, tool, shapes]);

  useImperativeHandle(ref, () => ({
    exportPng: () => {
      const stage = stageRef.current!;
      const tr = transformerRef.current!;
      const nodes = tr.nodes();
      tr.nodes([]);
      const prev = { w: stage.width(), h: stage.height(), s: stage.scaleX() };
      stage.size({ width, height });
      stage.scale({ x: 1, y: 1 });
      const canvas = stage.toCanvas({ pixelRatio: 1 });
      stage.size({ width: prev.w, height: prev.h });
      stage.scale({ x: prev.s, y: prev.s });
      tr.nodes(nodes);
      return new Promise((resolve, reject) =>
        canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error("export impossible"))), "image/png"),
      );
    },
  }));

  const pointer = () => stageRef.current!.getRelativePointerPosition()!;

  function onDown(e: Konva.KonvaEventObject<PointerEvent>) {
    if (tool === "select") {
      if (e.target === stageRef.current || e.target === imageRef.current) onSelect(null);
      return;
    }
    const { x, y } = pointer();
    const common = { id: newId(), x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, color: style.color };
    if (tool === "text") {
      const label: Label = {
        ...common,
        type: "text",
        x,
        y,
        text: t("editor.text"),
        font: style.font,
        size: Math.round(style.size * side),
        bold: style.bold,
        outline: style.outline,
        shadow: style.shadow,
      };
      onChange(label);
      onSelect(label.id);
      return;
    }
    if (!DRAW_TOOLS.includes(tool)) return;
    const strokeWidth = Math.max(1, style.width * side * (tool === "highlight" ? 4 : tool === "eraser" ? 3 : 1));
    origin.current = { x, y };
    if (tool === "brush" || tool === "highlight" || tool === "eraser") {
      setDraft({ ...common, type: tool, points: [x, y, x, y], width: strokeWidth });
    } else if (tool === "arrow" || tool === "line") {
      setDraft({ ...common, type: tool, points: [x, y, x, y], width: strokeWidth });
    } else {
      setDraft({ ...common, type: tool, x, y, w: 0, h: 0, width: strokeWidth, fill: style.fill });
    }
  }

  function onMove() {
    if (!draft || !origin.current) return;
    const { x, y } = pointer();
    const o = origin.current;
    if (draft.type === "brush" || draft.type === "highlight" || draft.type === "eraser") {
      setDraft({ ...draft, points: [...draft.points, x, y] });
    } else if (draft.type === "arrow" || draft.type === "line") {
      setDraft({ ...draft, points: [o.x, o.y, x, y] });
    } else if (draft.type === "rect") {
      setDraft({ ...draft, x: Math.min(o.x, x), y: Math.min(o.y, y), w: Math.abs(x - o.x), h: Math.abs(y - o.y) });
    } else if (draft.type === "ellipse") {
      // Ellipse positionnée par son centre, comme dans Konva.
      setDraft({ ...draft, x: (o.x + x) / 2, y: (o.y + y) / 2, w: Math.abs(x - o.x), h: Math.abs(y - o.y) });
    }
  }

  function onUp() {
    if (!draft) return;
    const tiny =
      ((draft.type === "rect" || draft.type === "ellipse") && draft.w < 3 && draft.h < 3) ||
      ((draft.type === "arrow" || draft.type === "line") &&
        Math.hypot(draft.points[2] - draft.points[0], draft.points[3] - draft.points[1]) < 3);
    if (!tiny) onChange(draft);
    setDraft(null);
    origin.current = null;
  }

  /** Position, rotation et échelle reprises du nœud après un déplacement ou une transformation. */
  function commitNode(shape: Shape, node: Konva.Node) {
    onChange({ ...shape, x: node.x(), y: node.y(), rotation: node.rotation(), scaleX: node.scaleX(), scaleY: node.scaleY() });
  }

  function render(shape: Shape, live: boolean) {
    const selectable = tool === "select" && live && shape.type !== "eraser";
    const common = {
      key: shape.id,
      id: shape.id,
      rotation: shape.rotation,
      scaleX: shape.scaleX,
      scaleY: shape.scaleY,
      draggable: selectable,
      listening: selectable,
      onPointerDown: () => selectable && onSelect(shape.id),
      onDragEnd: (e: Konva.KonvaEventObject<DragEvent>) => commitNode(shape, e.target),
      onTransformEnd: (e: Konva.KonvaEventObject<Event>) => commitNode(shape, e.target),
    };
    switch (shape.type) {
      case "brush":
      case "highlight":
      case "eraser":
        return (
          <Line
            {...common}
            x={shape.x}
            y={shape.y}
            points={shape.points}
            stroke={shape.color}
            strokeWidth={shape.width}
            lineCap={shape.type === "highlight" ? "butt" : "round"}
            lineJoin="round"
            tension={0.4}
            opacity={shape.type === "highlight" ? 0.4 : 1}
            globalCompositeOperation={shape.type === "eraser" ? "destination-out" : "source-over"}
            hitStrokeWidth={Math.max(shape.width, 12)}
          />
        );
      case "line":
        return <Line {...common} x={shape.x} y={shape.y} points={shape.points} stroke={shape.color} strokeWidth={shape.width} lineCap="round" hitStrokeWidth={Math.max(shape.width, 12)} />;
      case "arrow":
        return (
          <Arrow
            {...common}
            x={shape.x}
            y={shape.y}
            points={shape.points}
            stroke={shape.color}
            fill={shape.color}
            strokeWidth={shape.width}
            pointerLength={shape.width * 4}
            pointerWidth={shape.width * 4}
            lineCap="round"
            lineJoin="round"
            hitStrokeWidth={Math.max(shape.width, 12)}
          />
        );
      case "rect":
        return (
          <Rect
            {...common}
            x={shape.x}
            y={shape.y}
            width={shape.w}
            height={shape.h}
            stroke={shape.fill ? undefined : shape.color}
            fill={shape.fill ? shape.color : undefined}
            strokeWidth={shape.width}
            cornerRadius={shape.width}
          />
        );
      case "ellipse":
        return (
          <Ellipse
            {...common}
            x={shape.x}
            y={shape.y}
            radiusX={shape.w / 2}
            radiusY={shape.h / 2}
            stroke={shape.fill ? undefined : shape.color}
            fill={shape.fill ? shape.color : undefined}
            strokeWidth={shape.width}
          />
        );
      case "text":
        return (
          <Text
            {...common}
            x={shape.x}
            y={shape.y}
            text={shape.text}
            fontFamily={shape.font}
            fontSize={shape.size}
            fontStyle={shape.bold ? "bold" : "normal"}
            fill={shape.color}
            stroke={shape.outline ? (isLight(shape.color) ? "#000" : "#fff") : undefined}
            strokeWidth={shape.outline ? shape.size * 0.08 : 0}
            fillAfterStrokeEnabled
            shadowEnabled={shape.shadow}
            shadowColor="#000"
            shadowBlur={shape.size * 0.2}
            shadowOffset={{ x: shape.size * 0.04, y: shape.size * 0.04 }}
            shadowOpacity={0.6}
          />
        );
    }
  }

  return (
    <Stage
      ref={stageRef}
      width={Math.round(width * scale)}
      height={Math.round(height * scale)}
      scaleX={scale}
      scaleY={scale}
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={onUp}
      onPointerLeave={onUp}
      style={{ cursor: tool === "select" ? "default" : tool === "text" ? "text" : "crosshair" }}
    >
      <Layer>
        <KImage ref={imageRef} image={base} width={width} height={height} />
      </Layer>
      <Layer>
        {shapes.map((s) => render(s, true))}
        {draft && render(draft, false)}
        <Transformer
          ref={transformerRef}
          rotateEnabled
          keepRatio={false}
          anchorSize={9}
          borderStroke="#4d8bff"
          anchorStroke="#4d8bff"
          ignoreStroke
        />
      </Layer>
    </Stage>
  );
});

export default Canvas;

/** Couleur claire : contour noir ; sinon blanc. */
function isLight(hex: string) {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return 0.299 * r + 0.587 * g + 0.114 * b > 150;
}
