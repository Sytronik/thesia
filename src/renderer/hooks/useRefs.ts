import React, {useRef, useCallback} from "react";

type ReactRefsObject<T> = {
  [key: string]: T;
};

function useRefs<T>(): [
  React.MutableRefObject<ReactRefsObject<T>>,
  (refName: string) => React.RefCallback<T>,
] {
  const refs = useRef<ReactRefsObject<T>>({});

  const register = useCallback(
    (refName: string) => (ref: T) => {
      refs.current[refName] = ref;
    },
    [],
  );

  return [refs, register];
}

export default useRefs;
