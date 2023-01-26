export function difference<T>(arr1: T[], arr2: T[]): T[] {
  return arr1.filter((x) => !arr2.includes(x));
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
