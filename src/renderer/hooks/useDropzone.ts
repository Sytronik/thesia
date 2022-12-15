import React, {useRef, useCallback, useState, useEffect} from "react";

type DropzoneProps = {
  targetRef: React.RefObject<HTMLElement>;
  handleDrop: (e: DragEvent) => void;
};

function useDropzone(props: DropzoneProps) {
  const {targetRef, handleDrop} = props;

  const dragCounterRef = useRef<number>(0);
  const [isDragActive, setIsDragActive] = useState<boolean>(false);

  const onDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const onDragEnter = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    dragCounterRef.current += 1;
    if (e.dataTransfer?.items && e.dataTransfer.items.length > 0) {
      setIsDragActive(true);
    }
    return false;
  }, []);

  const onDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    dragCounterRef.current -= 1;
    if (dragCounterRef.current === 0) {
      setIsDragActive(false);
    }
    return false;
  }, []);

  const onDrop = useCallback(
    (e: DragEvent) => {
      handleDrop(e);
      dragCounterRef.current = 0;
      setIsDragActive(false);
    },
    [handleDrop],
  );

  useEffect(() => {
    const target = targetRef.current;

    target?.addEventListener("dragenter", onDragEnter);
    target?.addEventListener("dragleave", onDragLeave);
    target?.addEventListener("dragover", onDragOver);
    target?.addEventListener("drop", onDrop);

    return () => {
      target?.removeEventListener("dragenter", onDragEnter);
      target?.removeEventListener("dragleave", onDragLeave);
      target?.removeEventListener("dragover", onDragOver);
      target?.removeEventListener("drop", onDrop);
    };
  }, [targetRef, onDragEnter, onDragLeave, onDragOver, onDrop]);

  return {
    isDropzoneActive: isDragActive,
  };
}

export default useDropzone;
