import React, {
  useRef,
  useEffect,
  useMemo,
  forwardRef,
  useImperativeHandle,
  useContext,
  useCallback,
} from "react";
import useEvent from "react-use-event-hook";
import {DevicePixelRatioContext} from "renderer/contexts";
import styles from "./Overview.module.scss";
import BackendAPI from "../../api";
import {OVERVIEW_LENS_STYLE} from "../constants/tracks";
import Draggable, {CursorStateInfo} from "../../modules/Draggable";

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

type ArgsGetOverview = [trackId: number, width: number, height: number];
type ArgsLens = [durationSec: number, startSec: number, lensDurationSec: number];

type OverviewCursorState = "left" | "right" | "inlens" | "outlens";

const Overview = forwardRef((props: OverviewProps, ref) => {
  const {selectedTrackId, maxTrackSec, moveLens, resizeLensLeft, resizeLensRight} = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const durationSec = useMemo(
    () => (selectedTrackId !== null ? BackendAPI.getLengthSec(selectedTrackId) : 0),
    [selectedTrackId],
  );

  const backgroundElem = useRef<HTMLCanvasElement>(null);
  const lensElem = useRef<HTMLCanvasElement>(null);

  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const lensCtxRef = useRef<CanvasRenderingContext2D | null>(null);
  const prevArgsRef = useRef<ArgsGetOverview | null>(null);
  const prevArgsLensRef = useRef<ArgsLens | null>(null);

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
    ctx.beginPath();
    ctx.roundRect(
      startSec * pxPerSec + LINE_WIDTH / 2,
      LINE_WIDTH / 2,
      lensDurationSec * pxPerSec - LINE_WIDTH,
      height - LINE_WIDTH,
      2,
    );
    ctx.stroke();
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
      prevArgsLensRef.current.some((v, i) => Math.abs(argsLens[i] - v) > 1e-3)
    ) {
      drawLens(durationSec, startSec, lensDurationSec);
    }

    if (!backgroundCtx) return;
    const rect = backgroundElem.current.getBoundingClientRect();
    const width = rect.width * devicePixelRatio;
    const height = rect.height * devicePixelRatio;
    const args: ArgsGetOverview = [selectedTrackId, width, height];
    if (
      forced ||
      prevArgsRef.current === null ||
      prevArgsRef.current.some((v, i) => Math.abs(args[i] - v) > 1e-3)
    ) {
      let imbmp = null;
      if (width >= 1) {
        const buf = await BackendAPI.getOverview(selectedTrackId, width, height, devicePixelRatio);
        if (buf.length === width * height * 4) {
          const imdata = new ImageData(new Uint8ClampedArray(buf), width, height);
          imbmp = await createImageBitmap(imdata);
        }
      }
      backgroundCtx.transferFromImageBitmap(imbmp);
      prevArgsRef.current = args;
    }
  });

  useEffect(() => {
    resizeObserverRef.current?.disconnect();
    resizeObserverRef.current = new ResizeObserver(() => {
      if (!lensElem.current) return;
      lensElem.current.width = lensElem.current.clientWidth * devicePixelRatio;
      lensElem.current.height = lensElem.current.clientHeight * devicePixelRatio;
      const lensCtx = lensElem.current.getContext("2d", {desynchronized: true});
      lensCtxRef.current = lensCtx;
      if (!lensCtx) return;
      lensCtx.scale(devicePixelRatio, devicePixelRatio);
      lensCtx.lineWidth = LINE_WIDTH;
      lensCtx.fillStyle = OUT_LENS_FILL_STYLE;
      lensCtx.strokeStyle = LENS_STROKE_STYLE;
      if (prevArgsLensRef.current)
        draw(prevArgsLensRef.current[1], prevArgsLensRef.current[2], true);
    });
    if (lensElem.current) resizeObserverRef.current.observe(lensElem.current);
  }, [draw, devicePixelRatio]);

  useEffect(() => {
    if (!prevArgsLensRef.current) return;
    draw(prevArgsLensRef.current[1], prevArgsLensRef.current[2], true);
  }, [maxTrackSec, draw]);

  const imperativeInstanceRef = useRef<OverviewHandleElement>({draw});
  useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

  const calcSecFromX = useEvent((cursorX: number, rect: DOMRect) => {
    const ratioX = cursorX / rect.width;
    return ratioX * maxTrackSec;
  });

  const getInfoForResize = useCallback(
    (resizeLensFunc: (sec: number) => void) => ({
      cursor: RESIZE_CURSOR,
      cursorClassNameForBody: "colResizeCursor",
      handleDragging: (
        _: OverviewCursorState,
        cursorX: number,
        anchorValue: number,
        rect: DOMRect,
      ) => {
        resizeLensFunc(calcSecFromX(cursorX, rect));
      },
    }),
    [calcSecFromX],
  );

  const infoForInOutLens: CursorStateInfo<OverviewCursorState, number> = useMemo(
    () => ({
      cursor: "text",
      cursorClassNameForBody: "textCursor",
      handleDragging: (
        _: OverviewCursorState,
        cursorX: number,
        anchorValue: number,
        rect: DOMRect,
      ) => {
        moveLens(calcSecFromX(cursorX, rect), anchorValue);
      },
    }),
    [moveLens, calcSecFromX],
  );

  const cursorStateInfos: Map<
    OverviewCursorState,
    CursorStateInfo<OverviewCursorState, number>
  > = useMemo(
    () =>
      new Map([
        ["left", getInfoForResize(resizeLensLeft)],
        ["right", getInfoForResize(resizeLensRight)],
        ["inlens", infoForInOutLens],
        ["outlens", infoForInOutLens],
      ]),
    [resizeLensLeft, resizeLensRight, getInfoForResize, infoForInOutLens],
  );

  const determineCursorStates = useEvent((cursorX: number) => {
    if (!prevArgsLensRef.current) return "outlens";

    const [_trackDurationSec, startSec, lensDuratoinSec] = prevArgsLensRef.current;
    const pxPerSec = calcPxPerSec();
    const lensStartX = Math.round(startSec * pxPerSec);
    const lensEndX = Math.round((startSec + lensDuratoinSec) * pxPerSec);
    if (lensStartX - THICKNESS <= cursorX && cursorX <= lensStartX + THICKNESS) {
      return "left";
    }
    if (lensStartX + THICKNESS < cursorX && cursorX < lensEndX - THICKNESS) {
      return "inlens";
    }
    if (lensEndX - THICKNESS <= cursorX && cursorX <= lensEndX + THICKNESS) {
      return "right";
    }
    return "outlens";
  });

  const calcDragAnchor = useEvent(
    (cursorState: OverviewCursorState, cursorPos: number, rect: DOMRect) => {
      if (prevArgsLensRef.current !== null && cursorState === "inlens") {
        const sec = calcSecFromX(cursorPos, rect);
        const [_trackDurationSec, startSec, lensDurationSec] = prevArgsLensRef.current;
        return (sec - startSec) / lensDurationSec;
      }
      return 0.5;
    },
  );

  return (
    <div className={styles.Overview} role="navigation">
      <canvas className={styles.OverviewBackground} ref={backgroundElem} />
      <Draggable
        cursorStateInfos={cursorStateInfos}
        calcCursorPos="x"
        determineCursorStates={determineCursorStates}
        calcDragAnchor={calcDragAnchor}
        dragAnchorDefault={0.5}
      >
        <canvas
          className={styles.OverviewLens}
          ref={lensElem}
          style={{display: selectedTrackId !== null ? "block" : "none"}}
        />
      </Draggable>
    </div>
  );
});
Overview.displayName = "Overview";

export default React.memo(Overview);
