import React, {ReactNode, useRef} from "react";
import useEvent from "react-use-event-hook";

export type CursorStateInfo = {
  cursor: string;
  cursorClassNameForBody: string;
  handleDragging: (cursorValue: number, dragAnchorValue: number, rect: DOMRect) => void;
};

type DraggingProps<T extends string> = {
  cursorStateInfos: Map<T, CursorStateInfo>;
  calcCursorPos: (e: MouseEvent | React.MouseEvent, rect: DOMRect) => number;
  determineCursorStates: (cursorValue: number) => T;
  calcDragAnchor: (e: MouseEvent | React.MouseEvent, cursorState: T, rect: DOMRect) => number;
  dragAnchorDefault?: number;
  children: ReactNode;
};

function Draggable<T extends string>(props: DraggingProps<T>) {
  const {
    cursorStateInfos,
    calcCursorPos,
    determineCursorStates,
    calcDragAnchor: calcAnchorValue,
    dragAnchorDefault,
    children,
  } = props;
  const dragAnchorRef = useRef<number>(dragAnchorDefault ?? -1);
  const cursorStateRef = useRef<T>();
  const divElem = useRef<HTMLDivElement>(null);

  const updateCursorState = (e: React.MouseEvent | MouseEvent) => {
    if (!divElem.current) return;
    const cursorValue = calcCursorPos(e, divElem.current.getBoundingClientRect());
    cursorStateRef.current = determineCursorStates(cursorValue);
  };

  const onDragging = useEvent((e: React.MouseEvent | MouseEvent) => {
    if (!divElem.current) return;
    e.preventDefault();

    const rect = divElem.current.getBoundingClientRect();
    const cursorValue = calcCursorPos(e, rect);
    cursorStateInfos.forEach((value, key) => {
      if (cursorStateRef.current === key) {
        value.handleDragging(cursorValue, dragAnchorRef.current, rect);
      }
    });
  });

  const onMouseUp = (e: MouseEvent) => {
    e.preventDefault();
    dragAnchorRef.current = dragAnchorDefault ?? -1;
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
    dragAnchorRef.current = calcAnchorValue(
      e,
      cursorStateRef.current as T,
      divElem.current.getBoundingClientRect(),
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

Draggable.defaultProps = {
  dragAnchorDefault: -1,
};

const genericMemo: <T>(component: T) => T = React.memo;

export default genericMemo(Draggable);
