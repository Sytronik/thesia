import React, {useRef, useState, useEffect} from "react";
import useEvent from "react-use-event-hook";

type DropzoneProps = {
  targetRef: React.RefObject<HTMLElement>;
  handleDrop: (e: DragEvent) => Promise<void>;
};

function useDropzone(props: DropzoneProps) {
  const {targetRef, handleDrop} = props;

  const dragCounterRef = useRef<number>(0);
  const [isDragActive, setIsDragActive] = useState<boolean>(false);

  const onDragOver = useEvent((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  });

  const onDragEnter = useEvent((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    dragCounterRef.current += 1;
    if (e.dataTransfer?.items && e.dataTransfer.items.length > 0) {
      setIsDragActive(true);
    }
    return false;
  });

  const onDragLeave = useEvent((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    dragCounterRef.current -= 1;
    if (dragCounterRef.current === 0) {
      setIsDragActive(false);
    }
    return false;
  });

  const onDrop = useEvent(async (e: DragEvent) => {
    await handleDrop(e);
    dragCounterRef.current = 0;
    setIsDragActive(false);
  });

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
