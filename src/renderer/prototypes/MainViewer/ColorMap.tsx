import React, {useRef, useLayoutEffect, useState, useCallback} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import ColorBarCanvas from "renderer/prototypes/MainViewer/ColorBarCanvas";
import styles from "./ColorMap.scss";
import {
  COLORBAR_CANVAS_WIDTH,
  DB_CANVAS_WIDTH,
  DB_MARKER_POS,
  MIN_HEIGHT,
  VERTICAL_AXIS_PADDING,
} from "../constants";

type ColorMapProps = {
  height: number;
  setHeight: (height: number) => void;
  colorBarHeight: number;
  dbAxisCanvasElem: React.RefObject<AxisCanvasHandleElement>;
};

function ColorMap(props: ColorMapProps) {
  const {height, setHeight, colorBarHeight, dbAxisCanvasElem} = props;

  const colorMapElem = useRef<HTMLDivElement>(null);
  const drawedColorBarElem = useCallback((ref: ColorBarCanvasHandleElement) => {
    if (ref) {
      ref.draw();
    }
  }, []);

  const [resizeObserver, setResizeObserver] = useState(
    new ResizeObserver((entries) => {
      const {target} = entries[0];
      // Need throttle?
      setHeight(Math.max(target.clientHeight - (16 + 2 + 24), MIN_HEIGHT));
    }),
  );

  useLayoutEffect(() => {
    if (colorMapElem.current) {
      resizeObserver.observe(colorMapElem.current);
    }

    return () => {
      resizeObserver.disconnect();
    };
  }, [colorMapElem, resizeObserver]);

  return (
    <div className={styles.colorBar} ref={colorMapElem}>
      <div className={styles.colorMapHeader}>dB</div>
      <div style={{display: "flex", justifyContent: "center", alignItems: "center"}}>
        <ColorBarCanvas
          ref={drawedColorBarElem}
          width={COLORBAR_CANVAS_WIDTH}
          height={colorBarHeight}
        />
        <AxisCanvas
          ref={dbAxisCanvasElem}
          width={DB_CANVAS_WIDTH}
          height={height}
          axisPadding={VERTICAL_AXIS_PADDING}
          markerPos={DB_MARKER_POS}
          direction="V"
          className="dbAxis"
        />
      </div>
    </div>
  );
}

export default ColorMap;
