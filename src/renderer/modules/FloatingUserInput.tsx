import React, {forwardRef, useEffect, useImperativeHandle, useRef} from "react";

type FloatingUserInputProps = {
  value: string;
  onEndEditing: (v: string | null) => void;
  hidden: boolean;
  className?: string;
  focusOnShow?: boolean;
  style?: React.CSSProperties;
};

const FloatingUserInput = forwardRef(
  ({focusOnShow = true, ...props}: FloatingUserInputProps, ref) => {
    const {value, onEndEditing: endEditingCallback, hidden, className, style} = props;
    const changedRef = useRef<boolean>(false);
    const inputElem = useRef<HTMLInputElement>(null);

    useEffect(() => {
      if (!focusOnShow || hidden) return;
      if (inputElem.current !== null && inputElem.current !== document.activeElement) {
        changedRef.current = false;
        inputElem.current.value = value;
        inputElem.current?.focus();
        inputElem.current?.select();
      }
    }, [focusOnShow, value, hidden, inputElem]);

    const imperativeInstanceRef = useRef<FloatingUserInputElement>({
      setValue: (v) => {
        if (inputElem.current) inputElem.current.value = v;
      },
      isEditing: () => inputElem.current === document.activeElement,
    });
    useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

    return (
      <input
        type="text"
        ref={inputElem}
        hidden={hidden}
        className={className}
        style={style}
        defaultValue={value}
        tabIndex={-1}
        onMouseDown={(e) => {
          e.stopPropagation();
        }}
        onChange={() => {
          changedRef.current = true;
        }}
        onBlur={(e) => {
          endEditingCallback(changedRef.current ? (e.target as HTMLInputElement).value : null);
        }}
        onKeyDown={(e) => {
          const target = e.target as HTMLInputElement;
          switch (e.key) {
            case "Enter":
              target.blur();
              break;
            case "Escape":
              changedRef.current = false;
              target.blur();
              break;
            case "Tab":
              e.preventDefault();
              target.select();
              break;
            default:
              break;
          }
        }}
      />
    );
  },
);

FloatingUserInput.displayName = "FloatingUserInput";

export default React.memo(FloatingUserInput);
