import React, {useRef, useCallback} from "react";

type RefsObject<T> = {
  [key: string]: T;
};

type RegisterRefFnsObject<T> = {
  [key: string]: (ref: T) => void;
};

function useRefs<T>(): [
  React.MutableRefObject<RefsObject<T>>,
  (refName: string) => React.RefCallback<T>,
] {
  const refs = useRef<RefsObject<T>>({});
  const registerRefFns = useRef<RegisterRefFnsObject<T>>({});

  const register = useCallback((refName: string) => {
    if (!registerRefFns.current[refName])
      registerRefFns.current[refName] = (ref: T) => {
        refs.current[refName] = ref;
      };
    return registerRefFns.current[refName];
  }, []);

  return [refs, register];
}

export default useRefs;
