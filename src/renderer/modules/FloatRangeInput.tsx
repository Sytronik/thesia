import React, {useRef, useEffect, forwardRef, useImperativeHandle} from "react";
import useEvent from "react-use-event-hook";
import {DEFAULT_RANGE_COLOR} from "renderer/prototypes/constants/colors";
import styles from "./FloatRangeInput.module.scss";

type FloatRangeInputProps = {
  id: string;
  className?: string;
  unit: string;
  min: number;
  max: number;
  step: number;
  precision: number;
  detents: number[];
  disabled?: boolean;
  initialValue: number;
  disabledValue?: number;
  doubleClickValue?: number | null;
  onChangeValue?: (value: number) => void;
};

const FloatRangeInput = forwardRef(
  (
    {
      className = "",
      disabled = false,
      disabledValue = NaN,
      doubleClickValue = null,
      onChangeValue = () => {},
      ...props
    }: FloatRangeInputProps,
    ref,
  ) => {
    const {id, unit, min, max, step, precision, initialValue, detents} = props;
    const rangeElem = useRef<HTMLInputElement>(null);
    const textElem = useRef<HTMLInputElement>(null);
    const prevValueRef = useRef<number>(initialValue);

    const getRangeBackground = (value?: number) => {
      const v = value ?? Number(textElem.current?.value ?? initialValue);
      const rangeRatio = Math.min(Math.max((v - min) / (max - min), 0), 1);
      return disabled
        ? ""
        : `linear-gradient(to right, ${DEFAULT_RANGE_COLOR.LEFT} ${rangeRatio * 100}%, ${
            DEFAULT_RANGE_COLOR.RIGHT
          } ${rangeRatio * 100}%)`;
    };

    const getTextElemSize = () =>
      textElem.current?.value.length || initialValue.toFixed(precision).length;

    const updateStyle = useEvent(() => {
      if (rangeElem.current) {
        rangeElem.current.style.background = getRangeBackground();
      }
      if (textElem.current) {
        textElem.current.size = getTextElemSize();
      }
    });

    const setValue = useEvent((value: number) => {
      if (value >= min && value <= max) {
        if (rangeElem.current) rangeElem.current.value = value.toFixed(precision);
        if (textElem.current) textElem.current.value = value.toFixed(precision);
      }
      updateStyle();
    });

    const onRangeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = Number.parseFloat(e.target.value);
      if (textElem.current) textElem.current.value = value.toFixed(precision);
      onChangeValue(value);
      updateStyle();
    };

    const onRangeClick = (e: React.MouseEvent<HTMLInputElement>) => {
      if (e.button !== 0) return;
      if (e.detail === 2 || (e.detail === 1 && e.altKey)) {
        if (doubleClickValue !== null) {
          if (rangeElem.current) rangeElem.current.value = doubleClickValue.toFixed(precision);
          if (textElem.current) textElem.current.value = doubleClickValue.toFixed(precision);
          onChangeValue(doubleClickValue);
          updateStyle();
        }
      }
    };

    const onTextFocus = () => {
      let value = Number.parseFloat(textElem.current?.value ?? "");
      if (Number.isNaN(value)) {
        value = Number.parseFloat(rangeElem.current?.value ?? "");
        if (Number.isNaN(value)) value = initialValue;
      }
      prevValueRef.current = value;
    };

    const onTextBlur = (e: React.FocusEvent<HTMLInputElement>) => {
      let value = Number.parseFloat(e.target.value);
      if (Number.isNaN(value)) {
        value = Number.parseFloat(rangeElem.current?.value ?? "");
        if (Number.isNaN(value)) value = prevValueRef.current;
      }
      const clamppedValue = Math.min(Math.max(value, min), max);
      if (clamppedValue !== prevValueRef.current) {
        if (rangeElem.current) rangeElem.current.value = clamppedValue.toFixed(precision);
        onChangeValue(clamppedValue);
        updateStyle();
      }

      if (rangeElem.current?.value ?? e.target.value !== "" ?? "") {
        e.target.value = rangeElem.current?.value ?? "";
      }
    };

    const onTextKeyDown = (e: React.KeyboardEvent) => {
      switch (e.key) {
        case "Enter":
          (e.target as HTMLInputElement).blur();
          break;
        case "Escape":
          (e.target as HTMLInputElement).value = prevValueRef.current.toFixed(precision);
          (e.target as HTMLInputElement).blur();
          break;
        default:
          break;
      }
    };

    useEffect(() => {
      if (disabled) {
        const value = Number.isNaN(disabledValue) ? initialValue : disabledValue;
        if (rangeElem.current) rangeElem.current.value = value.toFixed(precision);
        if (textElem.current) textElem.current.value = value.toFixed(precision);
      }
    }, [disabled, initialValue, disabledValue, precision]);

    const imperativeInstanceRef = useRef<FloatRangeInputElement>({setValue});
    useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

    return (
      <div className={`${styles.FloatRangeInput} ${className}`}>
        <input
          ref={rangeElem}
          id={id}
          style={{
            background: getRangeBackground(initialValue),
          }}
          type="range"
          min={min}
          max={max}
          step={step}
          defaultValue={initialValue}
          disabled={disabled}
          onChange={onRangeChange}
          onClick={onRangeClick}
          list={`${id}Detents`}
        />
        <datalist id={`${id}Detents`}>
          {detents.map((detent) => (
            <option
              key={`${id}Detents.value${detent}`}
              aria-label={`${id} detent value ${detent}`}
              value={detent}
            />
          ))}
        </datalist>
        <input
          ref={textElem}
          id={`${id}Text`}
          type="text"
          inputMode="decimal"
          size={getTextElemSize()}
          defaultValue={initialValue.toFixed(precision)}
          disabled={disabled}
          onFocus={onTextFocus}
          onBlur={onTextBlur}
          onKeyDown={onTextKeyDown}
        />
        <label htmlFor={`${id}Text`}>{unit}</label>
      </div>
    );
  },
);

FloatRangeInput.displayName = "FloatRangeInput";

export default React.memo(FloatRangeInput);
