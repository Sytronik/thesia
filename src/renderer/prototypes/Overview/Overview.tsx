import React, {
  useRef,
  useEffect,
  useState,
  useMemo,
  forwardRef,
  useImperativeHandle,
  useContext,
} from "react";
import useEvent from "react-use-event-hook";
import {DevicePixelRatioContext} from "renderer/contexts";
import styles from "./Overview.scss";
import NativeAPI from "../../api";
import {OVERVIEW_LENS_STYLE} from "../constants";

const {OUT_LENS_FILL_STYLE, LENS_STROKE_STYLE, OUT_TRACK_FILL_STYLE, LINE_WIDTH, RESIZE_CURSOR} =
  OVERVIEW_LENS_STYLE;

const THICKNESS = 3;

type OverviewProps = {
  selectedTrackId: number | null;
  maxTrackSec: number;
  moveLens: (sec: number, anchorRatio: number) => void;
  resizeLensLeft: (sec: number) => void;
  resizeLensRight: (sec: number) => void;
};

enum OverviewMouseState {
  Left,
  Right,
  InLens,
  OutLens,
}

type ArgsGetOverview = [trackId: number, width: number, height: number];
type ArgsLens = [durationSec: number, startSec: number, lensDurationSec: number];

function calcX(e: React.MouseEvent | MouseEvent) {
  const elem = e.target as Element;
  const x = e.clientX - elem.getBoundingClientRect().left;
  return x;
}

function calcRatioX(e: React.MouseEvent) {
  const elem = e.target as Element;
  const x = e.clientX - elem.getBoundingClientRect().left;
  return x / elem.clientWidth;
}

const Overview = forwardRef((props: OverviewProps, ref) => {
  const {selectedTrackId, maxTrackSec, moveLens, resizeLensLeft, resizeLensRight} = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const [resizeObserver, setResizeObserver] = useState(new ResizeObserver(() => {}));
  const durationSec = useMemo(
    () => (selectedTrackId !== null ? NativeAPI.getLength(selectedTrackId) : 0),
    [selectedTrackId],
  );

  const backgroundElem = useRef<HTMLCanvasElement>(null);
  const lensElem = useRef<HTMLCanvasElement>(null);
  const lensCtxRef = useRef<CanvasRenderingContext2D | null>(null);
  const prevArgsRef = useRef<ArgsGetOverview | null>(null);
  const prevArgsLensRef = useRef<ArgsLens | null>(null);

  const dragAnchorRatioRef = useRef<number>(0.5);
  const mouseStateRef = useRef<OverviewMouseState>(OverviewMouseState.OutLens);

  const calcPxPerSec = useEvent(() => {
    const width = lensElem.current?.clientWidth ?? 0;
    return width / maxTrackSec;
  });

  const drawLens = (trackDurationSec: number, startSec: number, lensDurationSec: number) => {
    const ctx = lensCtxRef.current;
    if (!lensElem.current || !ctx) return;
    const {clientWidth: width, clientHeight: height} = lensElem.current;
    const pxPerSec = calcPxPerSec();
    const lensEndSec = (startSec + lensDurationSec) * pxPerSec;
    const endSec = trackDurationSec * pxPerSec;
    ctx.clearRect(0, 0, width, height);
    if (trackDurationSec < maxTrackSec) {
      ctx.save();
      ctx.fillStyle = OUT_TRACK_FILL_STYLE;
      ctx.fillRect(endSec, 0, width, height);
      ctx.restore();
    }
    if (startSec > 0) ctx.fillRect(0, 0, startSec * pxPerSec, height);
    ctx.strokeRect(
      startSec * pxPerSec + LINE_WIDTH / 2,
      LINE_WIDTH / 2,
      lensDurationSec * pxPerSec - LINE_WIDTH,
      height - LINE_WIDTH,
    );
    if (width > lensEndSec) ctx.fillRect(lensEndSec, 0, width - lensEndSec, height);
    prevArgsLensRef.current = [trackDurationSec, startSec, lensDurationSec];
  };

  const draw = useEvent(async (startSec: number, lensDurationSec: number, forced = false) => {
    if (!backgroundElem.current || !lensElem.current) return;

    const backgroundCtx = backgroundElem.current.getContext("bitmaprenderer");

    if (selectedTrackId === null) {
      if (forced || prevArgsRef.current !== null) {
        backgroundCtx?.transferFromImageBitmap(null);
        lensCtxRef.current?.clearRect(
          0,
          0,
          lensElem.current.clientWidth,
          lensElem.current.clientHeight,
        );
      }
      prevArgsRef.current = null;
      prevArgsLensRef.current = null;
      return;
    }

    const argsLens: ArgsLens = [durationSec, startSec, lensDurationSec];
    if (
      forced ||
      prevArgsLensRef.current === null ||
      prevArgsLensRef.current.some((v, i) => argsLens[i] !== v)
    ) {
      drawLens(durationSec, startSec, lensDurationSec);
    }

    if (!backgroundCtx) return;
    const {width, height} = backgroundElem.current;
    const args: ArgsGetOverview = [selectedTrackId, width, height];
    if (
      forced ||
      prevArgsRef.current === null ||
      prevArgsRef.current.some((v, i) => args[i] !== v)
    ) {
      const buf = await NativeAPI.getOverview(selectedTrackId, width, height, devicePixelRatio);
      let imbmp = null;
      if (buf.length === width * height * 4) {
        const imdata = new ImageData(new Uint8ClampedArray(buf), width, height);
        imbmp = await createImageBitmap(imdata);
      }
      backgroundCtx.transferFromImageBitmap(imbmp);
      prevArgsRef.current = args;
    }
  });

  useEffect(() => {
    setResizeObserver(
      new ResizeObserver((entries) => {
        const backgroundElemTarget = entries[0].target as HTMLCanvasElement;
        backgroundElemTarget.width = backgroundElemTarget.clientWidth * devicePixelRatio;
        backgroundElemTarget.height = backgroundElemTarget.clientHeight * devicePixelRatio;
        if (!lensElem.current) return;
        lensElem.current.width = backgroundElemTarget.width;
        lensElem.current.height = backgroundElemTarget.height;
        const lensCtx = lensElem.current.getContext("2d", {desynchronized: true});
        lensCtxRef.current = lensCtx;
        if (!lensCtx) return;
        lensCtx.scale(devicePixelRatio, devicePixelRatio);
        lensCtx.lineWidth = LINE_WIDTH;
        lensCtx.fillStyle = OUT_LENS_FILL_STYLE;
        lensCtx.strokeStyle = LENS_STROKE_STYLE;
        if (prevArgsLensRef.current)
          draw(prevArgsLensRef.current[1], prevArgsLensRef.current[2], true);
      }),
    );
  }, [draw, devicePixelRatio]);

  useEffect(() => {
    if (backgroundElem.current) {
      resizeObserver.observe(backgroundElem.current);
    }

    return () => {
      resizeObserver.disconnect();
    };
  }, [resizeObserver]);

  const imperativeInstanceRef = useRef<OverviewHandleElement>({draw});
  useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

  const updateMouseState = (e: React.MouseEvent | MouseEvent) => {
    mouseStateRef.current = OverviewMouseState.OutLens;
    if (!prevArgsLensRef.current) return;

    const [trackDurationSec, startSec, lensDuratoinSec] = prevArgsLensRef.current;
    const pxPerSec = calcPxPerSec();
    const lensStartX = Math.round(startSec * pxPerSec);
    const lensEndX = Math.round((startSec + lensDuratoinSec) * pxPerSec);
    const x = calcX(e);
    if (lensStartX - THICKNESS <= x && x <= lensStartX + THICKNESS) {
      mouseStateRef.current = OverviewMouseState.Left;
    } else if (lensStartX + THICKNESS < x && x < lensEndX - THICKNESS) {
      mouseStateRef.current = OverviewMouseState.InLens;
    } else if (lensEndX - THICKNESS <= x && x <= lensEndX + THICKNESS) {
      mouseStateRef.current = OverviewMouseState.Right;
    }
  };

  const onDragging = useEvent((e: React.MouseEvent | MouseEvent) => {
    e.preventDefault();
    if (!backgroundElem.current) return;

    const x = e.clientX - backgroundElem.current.getBoundingClientRect().left;
    const ratioX = x / backgroundElem.current.clientWidth;
    const sec = ratioX * maxTrackSec;
    switch (mouseStateRef.current) {
      case OverviewMouseState.Left:
        resizeLensLeft(sec);
        break;
      case OverviewMouseState.Right:
        resizeLensRight(sec);
        break;
      default:
        moveLens(sec, dragAnchorRatioRef.current);
        break;
    }
  });

  const onMouseUp = (e: MouseEvent) => {
    e.preventDefault();
    dragAnchorRatioRef.current = 0.5;
    const bodyElem = document.querySelector("body");
    if (bodyElem !== null) {
      bodyElem.className = bodyElem.className.replace(" textCursor", "");
      bodyElem.className = bodyElem.className.replace(" colResizeCursor", "");
    }
    updateMouseState(e);
    document.removeEventListener("mousemove", onDragging);
  };

  const onMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    if (selectedTrackId === null) return;
    updateMouseState(e);
    if (prevArgsLensRef.current !== null && mouseStateRef.current === OverviewMouseState.InLens) {
      const ratioX = calcRatioX(e);
      const secOfX = ratioX * maxTrackSec;
      const [trackDurationSec, startSec, lensDurationSec] = prevArgsLensRef.current;
      dragAnchorRatioRef.current = (secOfX - startSec) / lensDurationSec;
    } else {
      dragAnchorRatioRef.current = 0.5;
    }
    if (mouseStateRef.current === OverviewMouseState.OutLens) onDragging(e);

    const bodyElem = document.querySelector("body");
    if (bodyElem) {
      if (
        mouseStateRef.current === OverviewMouseState.Left ||
        mouseStateRef.current === OverviewMouseState.Right
      ) {
        bodyElem.className += " colResizeCursor";
      } else {
        bodyElem.className += " textCursor";
      }
    }
    document.addEventListener("mousemove", onDragging);
    document.addEventListener("mouseup", onMouseUp, {once: true});
  };

  const onMouseMove = (e: React.MouseEvent) => {
    e.preventDefault();
    if (e.buttons === 1 || !lensElem.current || selectedTrackId === null) return;
    updateMouseState(e);
    if (
      mouseStateRef.current === OverviewMouseState.Left ||
      mouseStateRef.current === OverviewMouseState.Right
    ) {
      lensElem.current.style.cursor = RESIZE_CURSOR;
    } else {
      lensElem.current.style.cursor = "";
    }
  };

  return (
    <div className={styles.Overview} role="navigation">
      <canvas ref={backgroundElem} />
      <canvas ref={lensElem} onMouseDown={onMouseDown} onMouseMove={onMouseMove} />
    </div>
  );
});
Overview.displayName = "Overview";

export default React.memo(Overview);
