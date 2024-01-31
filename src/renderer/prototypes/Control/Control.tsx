import React, {ReactNode, useMemo, useRef, useState} from "react";
import useEvent from "react-use-event-hook";
import {debounce, throttle} from "throttle-debounce";
import {
  SpecSetting,
  GuardClippingMode,
  NormalizeTarget,
  FreqScale,
  NormalizeOnType,
  NormalizeOnTypeValues,
} from "renderer/api/backend-wrapper";
import FloatRangeInput from "renderer/modules/FloatRangeInput";
import styles from "./Control.module.scss";
import {
  COMMON_NORMALIZE_DB_DETENTS,
  DB_RANGE_DETENTS,
  DB_RANGE_MIN_MAX,
  MIN_COMMON_NORMALIZE_dB,
  T_OVERLAP_VALUES,
} from "../constants";

type ControlProps = {
  specSetting: SpecSetting;
  setSpecSetting: (specSetting: SpecSetting) => Promise<void>;
  dBRange: number;
  setdBRange: (dBRange: number) => void;
  commonGuardClipping: GuardClippingMode;
  setCommonGuardClipping: (commonGuardClipping: GuardClippingMode) => Promise<void>;
  commonNormalize: NormalizeTarget;
  setCommonNormalize: (commonNormalize: NormalizeTarget) => Promise<void>;
};

function Control(props: ControlProps) {
  const {
    specSetting,
    setSpecSetting,
    dBRange,
    setdBRange,
    commonGuardClipping,
    setCommonGuardClipping,
    commonNormalize,
    setCommonNormalize,
  } = props;

  const [commonNormalizePeakdB, setCommonNormalizePeakdB] = useState<number>(0.0);
  const [commonNormalizedB, setCommonNormalizedB] = useState<number>(-18.0);
  const [isCommonNormalizeOn, setIsCommonNormalizeOn] = useState<boolean>(false);

  const commonNormalizedBInputElem = useRef<FloatRangeInputElement>(null);

  const toggleFreqScale = useEvent((freqScale: FreqScale) =>
    freqScale === FreqScale.Linear ? FreqScale.Mel : FreqScale.Linear,
  );

  const onFreqScaleBtnClick = async () => {
    await setSpecSetting({
      ...specSetting,
      freqScale: toggleFreqScale(specSetting.freqScale),
    });
  };

  const throttledSetdBRange = useMemo(() => throttle(1000 / 70, setdBRange), [setdBRange]);

  const onWinMillisecChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const winMillisec = Number.parseFloat(e.target.value);
    if (e.target.value !== "" && winMillisec > 0) {
      setSpecSetting({
        ...specSetting,
        winMillisec,
      });
    }
  };

  const onTOverlapChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const tOverlap = Number.parseFloat(e.target.value);
    if (e.target.value !== "" && tOverlap > 0) {
      setSpecSetting({
        ...specSetting,
        tOverlap,
      });
    }
  };

  const onCommonNormalizeTypeChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const type = e.target.selectedOptions[0].value;
    switch (type) {
      case "Off":
        setCommonNormalize({type: "Off"});
        setIsCommonNormalizeOn(false);
        break;
      case "PeakdB":
        setCommonNormalize({type: "PeakdB", target: commonNormalizePeakdB});
        setIsCommonNormalizeOn(true);
        if (commonNormalizedBInputElem.current)
          commonNormalizedBInputElem.current.setValue(commonNormalizePeakdB);
        break;
      default:
        setCommonNormalize({type: type as NormalizeOnType, target: commonNormalizedB});
        setIsCommonNormalizeOn(true);
        if (commonNormalizedBInputElem.current)
          commonNormalizedBInputElem.current.setValue(commonNormalizedB);
        break;
    }
  };

  const debouncedChangeCommonNormalizedB = debounce(250, (dB: number) => {
    if (isCommonNormalizeOn) setCommonNormalize({type: commonNormalize.type, target: dB});
  });

  const onCommonNormalizedBInputChange = useEvent((value: number) => {
    if (!isCommonNormalizeOn) return;
    if (commonNormalize.type === "PeakdB") setCommonNormalizePeakdB(value);
    else setCommonNormalizedB(value);
    debouncedChangeCommonNormalizedB(value);
  });

  const onCommonGuardClippingModeChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    setCommonGuardClipping(e.target.selectedOptions[0].value as GuardClippingMode);
  };

  return (
    <div className={`flex-item-fixed ${styles.Control}`}>
      <div className={styles.sectionContainer}>
        <div className={styles.itemContainer}>
          <label htmlFor="dBRange">Dynamic Range</label>
          <FloatRangeInput
            id="dBRange"
            unit="dB"
            min={DB_RANGE_MIN_MAX[0]}
            max={DB_RANGE_MIN_MAX[1]}
            step={1}
            precision={0}
            detents={DB_RANGE_DETENTS}
            initialValue={dBRange}
            doubleClickValue={DB_RANGE_MIN_MAX[1]}
            onChangeValue={throttledSetdBRange}
          />
        </div>
      </div>
      <div className={styles.sectionContainer}>
        <div className={styles.itemContainer}>
          <label htmlFor="winMillisec">Window Size</label>
          <input
            type="text"
            inputMode="decimal"
            id="winMillisec"
            className={styles.winMillisecInput}
            defaultValue={specSetting.winMillisec.toFixed(1)}
            onChange={onWinMillisecChange}
          />
          ms
        </div>
        <div className={styles.itemContainer}>
          <label htmlFor="tOverlap">Time Overlap</label>
          <select
            name="tOverlap"
            id="tOverlap"
            defaultValue={specSetting.tOverlap}
            onChange={onTOverlapChange}
          >
            {T_OVERLAP_VALUES.map((v) => (
              <option key={`t-overlap-${v}x`} value={`${v}`}>{`${v}x`}</option>
            ))}
          </select>
        </div>
      </div>
      <div className={styles.sectionContainer}>
        <div className={styles.itemContainer}>
          <label htmlFor="freqScale">Frequency Scale</label>
          <input
            type="checkbox"
            role="switch"
            className={styles.changeFreqScaleBtn}
            onClick={onFreqScaleBtnClick}
            id="freqScale"
          />
          <div className={styles.freqScaleSwitchBox}>
            <label className={styles.freqScaleToggle} htmlFor="freqScale"></label>
            <label
              className={`${styles.freqScaleLabelBox} ${styles.freqScaleLinear}`}
              htmlFor="freqScale"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12">
                <path
                  id="logo"
                  d="M11.625,0H.375A.376.376,0,0,0,0,.375v11.25A.376.376,0,0,0,.375,12h11.25A.376.376,0,0,0,12,11.625V.375A.376.376,0,0,0,11.625,0Zm-.187.563L.563,11.438V.563Z"
                />
              </svg>
              <span>Linear</span>
            </label>
            <label
              className={`${styles.freqScaleLabelBox} ${styles.freqScaleMel}`}
              htmlFor="freqScale"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12">
                <path
                  id="logo"
                  d="M11.525.1H.275A.376.376,0,0,0-.1.475v11.25a.376.376,0,0,0,.375.375h11.25a.376.376,0,0,0,.375-.375V.475A.376.376,0,0,0,11.525.1ZM.463,10.75V.662H11.338V3.438C5.263,4.694,1.644,7.281.463,10.75Z"
                  transform="translate(0.1 -0.1)"
                />
              </svg>
              <span>Mel</span>
            </label>
          </div>
        </div>
      </div>
      <div className={styles.sectionContainer}>
        <div className={styles.itemContainer}>
          <label htmlFor="commonNormalize">Common Normalization</label>
          <select
            name="commonNormalize"
            id="commonNormalize"
            onChange={onCommonNormalizeTypeChange}
            defaultValue={commonNormalize.type}
          >
            <option value="Off">Off</option>
            {NormalizeOnTypeValues.map((type) => (
              <option key={type} value={type}>
                {type.replace("dB", "")}
              </option>
            ))}
          </select>
          <FloatRangeInput
            ref={commonNormalizedBInputElem}
            id="commonNormalizedBInput"
            unit="dB"
            min={MIN_COMMON_NORMALIZE_dB}
            max={0}
            step={0.01}
            precision={2}
            detents={COMMON_NORMALIZE_DB_DETENTS}
            initialValue={MIN_COMMON_NORMALIZE_dB}
            disabled={!isCommonNormalizeOn}
            onChangeValue={onCommonNormalizedBInputChange}
          />
        </div>
        <div className={styles.itemContainer}>
          <label htmlFor="commonGuardClipping">Common Clipping Guard</label>
          <select
            name="commonGuardClipping"
            id="commonGuardClipping"
            onChange={onCommonGuardClippingModeChange}
            defaultValue={commonGuardClipping}
          >
            <option value={GuardClippingMode.ReduceGlobalLevel}>Reducing Global Level</option>
            <option value={GuardClippingMode.Limiter}>Applying Limiter</option>
            <option value={GuardClippingMode.Clip}>Off (Hard Clipping)</option>
          </select>
        </div>
      </div>
    </div>
  );
}

export default Control;
