export function difference<T>(arr1: T[], arr2: T[]): T[] {
  return arr1.filter((x) => !arr2.includes(x));
}

export function chunk<T>(array: T[], size = 1) {
  const length = array == null ? 0 : array.length;

  if (!length || size < 1) {
    return [];
  }

  let index = 0;
  let resIndex = 0;

  const result = new Array(Math.ceil(length / size));
  while (index < length) {
    result[resIndex] = array.slice(index, (index += size));
    resIndex += 1;
  }
  return result;
}

export function intersection<T>(arr1: T[], arr2: T[]): T[] {
  return arr1.filter((x) => arr2.includes(x));
}

export function isNil<T>(val: T): boolean {
  return val === null;
}

export function last<T>(arr: T[]): T {
  return arr[arr.length - 1];
}

export function areDOMRectsEqual(rect1: DOMRect, rect2: DOMRect, eps = 1e-6): boolean {
  const properties: (keyof DOMRect)[] = [
    "top",
    "right",
    "bottom",
    "left",
    "width",
    "height",
    "x",
    "y",
  ];

  return properties.every(
    (prop) => Math.abs((rect1[prop] as number) - (rect2[prop] as number)) < eps,
  );
}
