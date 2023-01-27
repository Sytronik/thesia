import React, {useRef, useLayoutEffect, useState, useCallback} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import ColorBarCanvas from "renderer/prototypes/MainViewer/ColorBarCanvas";
import styles from "./ColorMap.scss";
import {COLORBAR_CANVAS_WIDTH, DB_CANVAS_WIDTH, DB_MARKER_POS, MIN_HEIGHT} from "../constants";

type ColorMapProps = {
  height: number;
  setHeight: (height: number) => void;
  dbAxisCanvasElem: React.RefObject<AxisCanvasHandleElement>;
};

function ColorMap(props: ColorMapProps) {
  const {height, setHeight, dbAxisCanvasElem} = props;

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
      <ColorBarCanvas ref={drawedColorBarElem} width={COLORBAR_CANVAS_WIDTH} height={height} />
      <AxisCanvas
        ref={dbAxisCanvasElem}
        width={DB_CANVAS_WIDTH}
        height={height}
        markerPos={DB_MARKER_POS}
        direction="V"
        className="dbAxis"
      />
    </div>
  );
}

export default ColorMap;
