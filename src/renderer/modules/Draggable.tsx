import React, {ReactNode, useRef} from "react";
import useEvent from "react-use-event-hook";

export type CursorStateInfo<T extends string, U> = {
  cursor: string;
  cursorClassNameForBody: string;
  handleDragging: (cursorState: T, cursorPos: number, dragAnchorValue: U, rect: DOMRect) => void;
};

type DraggingProps<T extends string, U> = {
  cursorStateInfos: Map<T, CursorStateInfo<T, U>>;
  calcCursorPos: "x" | "y" | ((e: MouseEvent | React.MouseEvent, rect: DOMRect) => number);
  determineCursorStates: (cursorPos: number, rect: DOMRect) => T;
  calcDragAnchor: (cursorState: T, cursorPos: number, rect: DOMRect) => U;
  dragAnchorDefault: U;
  children: ReactNode;
};

const calcCursorX = (e: MouseEvent | React.MouseEvent, rect: DOMRect) => {
  return e.clientX - rect.left;
};

const calcCursorY = (e: MouseEvent | React.MouseEvent, rect: DOMRect) => {
  return e.clientY - rect.top;
};

function Draggable<T extends string, U>(props: DraggingProps<T, U>) {
  const {
    cursorStateInfos,
    calcCursorPos,
    determineCursorStates,
    calcDragAnchor,
    dragAnchorDefault,
    children,
  } = props;
  const dragAnchorRef = useRef<U>(dragAnchorDefault);
  const cursorStateRef = useRef<T>();
  const divElem = useRef<HTMLDivElement>(null);

  let calcCursorPosFunc: (e: MouseEvent | React.MouseEvent, rect: DOMRect) => number;
  if (calcCursorPos === "x") calcCursorPosFunc = calcCursorX;
  else if (calcCursorPos === "y") calcCursorPosFunc = calcCursorY;
  else calcCursorPosFunc = calcCursorPos;

  const updateCursorState = (e: React.MouseEvent | MouseEvent) => {
    if (!divElem.current) return;
    const rect = divElem.current.getBoundingClientRect();
    const cursorPos = calcCursorPosFunc(e, rect);
    cursorStateRef.current = determineCursorStates(cursorPos, rect);
  };

  const onDragging = useEvent((e: React.MouseEvent | MouseEvent) => {
    if (!divElem.current) return;
    e.preventDefault();

    const rect = divElem.current.getBoundingClientRect();
    const cursorPos = calcCursorPosFunc(e, rect);
    cursorStateInfos.forEach((value, key) => {
      if (cursorStateRef.current === key) {
        value.handleDragging(cursorStateRef.current, cursorPos, dragAnchorRef.current, rect);
      }
    });
  });

  const onMouseUp = (e: MouseEvent) => {
    e.preventDefault();
    dragAnchorRef.current = dragAnchorDefault;
    const bodyElem = document.querySelector("body");
    if (bodyElem !== null) {
      bodyElem.classList.remove(
        ...Array.from(cursorStateInfos.values()).map((v) => v.cursorClassNameForBody),
      );
    }
    updateCursorState(e);
    document.removeEventListener("mousemove", onDragging);
  };

  const onMouseDown = (e: React.MouseEvent) => {
    if (!divElem.current) return;
    e.preventDefault();
    updateCursorState(e);
    const rect = divElem.current.getBoundingClientRect();
    dragAnchorRef.current = calcDragAnchor(
      cursorStateRef.current as T,
      calcCursorPosFunc(e, rect),
      rect,
    );
    onDragging(e);

    const bodyElem = document.querySelector("body");
    if (bodyElem) {
      cursorStateInfos.forEach((value, key) => {
        if (cursorStateRef.current === key) {
          bodyElem.classList.add(value.cursorClassNameForBody);
        }
      });
    }
    document.addEventListener("mousemove", onDragging);
    document.addEventListener("mouseup", onMouseUp, {once: true});
  };

  const onMouseMove = (e: React.MouseEvent) => {
    e.preventDefault();
    if (e.buttons === 1) return;
    updateCursorState(e);

    cursorStateInfos.forEach((value, key) => {
      if (cursorStateRef.current === key && divElem.current) {
        divElem.current.style.cursor = value.cursor;
      }
    });
  };

  return (
    <div role="presentation" ref={divElem} onMouseDown={onMouseDown} onMouseMove={onMouseMove}>
      {children}
    </div>
  );
}

const genericMemo: <T>(component: T) => T = React.memo;

export default genericMemo(Draggable);
