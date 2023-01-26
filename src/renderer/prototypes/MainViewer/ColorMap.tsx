import React, {useRef, useLayoutEffect, useState} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import styles from "./ColorMap.scss";
import {DB_CANVAS_WIDTH, DB_MARKER_POS, MIN_HEIGHT} from "../constants";

type ColorBarProps = {
  height: number;
  setHeight: (height: number) => void;
  dbAxisCanvasElem: React.RefObject<AxisCanvasHandleElement>;
};

function ColorMap(props: ColorBarProps) {
  const {height, setHeight, dbAxisCanvasElem} = props;

  const colorMapElem = useRef<HTMLDivElement>(null);

  const [resizeObserver, setResizeObserver] = useState(
    new ResizeObserver((entries) => {
      const {target} = entries[0];
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
